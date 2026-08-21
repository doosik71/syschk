//! 표본 추출.
//!
//! CPU 사용률·처리량 같은 값은 누적 카운터의 **차이**로만 얻을 수 있다. 이 모듈이
//! 이전 표본을 들고 있으면서 1초 간격으로 비율을 계산한다.
//!
//! 실시간 화면을 보고 있을 때만 표본을 뜬다. 다른 화면에서는 아무 것도 읽지 않으므로
//! 유휴 부하가 거의 없다(NFR-1).

use crate::analyze::{Assessment, Metrics, assess};
use crate::collect::blockio::{DiskRate, DiskSnapshot};
use crate::collect::cpu::{CpuSnapshot, CpuUsage, Load};
use crate::collect::memory::{Memory, SwapRate, VmStat};
use crate::collect::network::{NetRate, NetSnapshot};
use crate::collect::pressure::PressureSet;
use crate::collect::process::{ProcCache, ProcRow, ProcSnapshot};
use crate::collect::{ProbeCtx, blockio, memory, network, process};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// 추세 표시용 고정 길이 이력.
#[derive(Clone, Debug, Default)]
pub struct Trend {
    values: Vec<f32>,
}

impl Trend {
    const CAP: usize = 120;

    fn push(&mut self, v: f32) {
        if self.values.len() == Self::CAP {
            self.values.remove(0);
        }
        self.values.push(v);
    }

    pub fn values(&self) -> &[f32] {
        &self.values
    }

    pub fn peak(&self) -> f32 {
        self.values.iter().copied().fold(0.0, f32::max)
    }
}

/// 실시간 지표 표본 추출기.
pub struct Sampler {
    ctx: ProbeCtx,
    users: HashMap<u32, String>,
    prev_cpu: Option<CpuSnapshot>,
    prev_disk: Option<DiskSnapshot>,
    prev_net: Option<NetSnapshot>,
    prev_proc: Option<ProcSnapshot>,
    proc_cache: ProcCache,
    prev_vm: Option<VmStat>,
    last_tick: Option<Instant>,

    pub interval: Duration,
    pub samples: u32,
    /// 마지막 두 표본 사이의 실제 간격.
    pub elapsed: Duration,
    /// 표본 하나를 뜨는 데 든 시간. 관찰에도 비용이 든다는 사실을 숨기지 않는다.
    pub cost: Duration,

    pub cpu: CpuUsage,
    pub load: Load,
    pub cores: usize,
    pub running: u32,
    pub blocked: u32,
    pub memory: Memory,
    pub swap: SwapRate,
    pub pressure: PressureSet,
    pub disks: Vec<DiskRate>,
    pub nets: Vec<NetRate>,
    pub procs: Vec<ProcRow>,
    /// 다른 사용자 프로세스의 I/O 는 권한 없이 읽을 수 없다.
    pub io_visible: bool,

    pub cpu_trend: Trend,
    pub mem_trend: Trend,
    pub disk_trend: Trend,
    pub net_trend: Trend,
}

impl Sampler {
    pub fn new(ctx: ProbeCtx) -> Self {
        let users = process::user_names(&ctx);
        Self {
            ctx,
            users,
            prev_cpu: None,
            prev_disk: None,
            prev_net: None,
            prev_proc: None,
            proc_cache: ProcCache::default(),
            prev_vm: None,
            last_tick: None,
            interval: Duration::from_millis(1500),
            samples: 0,
            elapsed: Duration::ZERO,
            cost: Duration::ZERO,
            cpu: CpuUsage::default(),
            load: Load::default(),
            cores: 0,
            running: 0,
            blocked: 0,
            memory: Memory::default(),
            swap: SwapRate::default(),
            pressure: PressureSet::default(),
            disks: Vec::new(),
            nets: Vec::new(),
            procs: Vec::new(),
            io_visible: false,
            cpu_trend: Trend::default(),
            mem_trend: Trend::default(),
            disk_trend: Trend::default(),
            net_trend: Trend::default(),
        }
    }

    /// 다음 표본을 뜰 시각이 되었는가.
    pub fn due(&self, now: Instant) -> bool {
        match self.last_tick {
            None => true,
            Some(last) => now.duration_since(last) >= self.interval,
        }
    }

    /// 표본을 뜨고 비율을 갱신한다.
    pub fn tick(&mut self) {
        let now = Instant::now();
        let raw_secs = self
            .last_tick
            .map(|last| now.duration_since(last).as_secs_f32())
            .unwrap_or(0.0);
        // 간격이 너무 짧으면 비율이 과장된다. 최소 50ms 는 지나야 계산한다.
        let secs = if raw_secs >= 0.05 { raw_secs } else { 0.0 };
        self.elapsed = Duration::from_secs_f32(secs.max(0.0));

        // 점(point) 값은 표본 하나로 바로 쓸 수 있다.
        self.pressure = PressureSet::read(&self.ctx);
        if let Some(m) = Memory::read(&self.ctx) {
            self.memory = m;
        }
        if let Some(l) = Load::read(&self.ctx) {
            self.load = l;
        }

        // 비율(rate) 값은 이전 표본이 필요하다.
        let cpu_now = CpuSnapshot::read(&self.ctx);
        if let Some(cur) = &cpu_now {
            self.cores = cur.cores();
            self.running = cur.procs_running;
            self.blocked = cur.procs_blocked;
            if let Some(prev) = &self.prev_cpu
                && secs > 0.0
            {
                self.cpu = crate::collect::cpu::usage(prev, cur);
            }
        }
        self.prev_cpu = cpu_now;

        let vm_now = VmStat::read(&self.ctx);
        if let (Some(prev), Some(cur)) = (&self.prev_vm, &vm_now)
            && secs > 0.0
        {
            self.swap = memory::swap_rate(prev, cur, secs);
        }
        self.prev_vm = vm_now;

        let disk_now = DiskSnapshot::read(&self.ctx);
        if let (Some(prev), Some(cur)) = (&self.prev_disk, &disk_now) {
            self.disks = blockio::rates(prev, cur, secs);
        }
        self.prev_disk = disk_now;

        let net_now = NetSnapshot::read(&self.ctx);
        if let (Some(prev), Some(cur)) = (&self.prev_net, &net_now) {
            self.nets = network::rates(prev, cur, secs);
        }
        self.prev_net = net_now;

        let proc_now = ProcSnapshot::read_cached(&self.ctx, &mut self.proc_cache);
        if let Some(cur) = &proc_now {
            self.io_visible = cur.io_readable;
            if let Some(prev) = &self.prev_proc {
                self.procs = process::rows(prev, cur, secs, self.memory.total, &self.users);
            }
        }
        self.prev_proc = proc_now;

        if self.samples > 0 {
            self.cpu_trend.push(self.cpu.busy);
            self.mem_trend.push(self.memory.used_pct());
            self.disk_trend
                .push(self.disks.iter().map(|d| d.util_pct).fold(0.0, f32::max));
            self.net_trend.push(
                self.nets
                    .iter()
                    .map(|n| n.rx_bytes + n.tx_bytes)
                    .fold(0.0, f32::max),
            );
        }

        self.samples = self.samples.saturating_add(1);
        self.cost = now.elapsed();
        self.last_tick = Some(now);
    }

    /// 아직 비율을 계산할 표본이 모이지 않았는가.
    pub fn warming_up(&self) -> bool {
        self.samples < 2
    }

    pub fn metrics(&self) -> Metrics<'_> {
        Metrics {
            cpu: &self.cpu,
            load1: self.load.one,
            cores: self.cores,
            pressure: &self.pressure,
            memory: &self.memory,
            swap: self.swap,
            disks: &self.disks,
            nets: &self.nets,
            blocked: self.blocked,
            samples: self.samples,
        }
    }

    pub fn assessment(&self) -> Assessment {
        assess(&self.metrics())
    }

    /// 디스크 처리량 합계(초당 바이트).
    pub fn disk_throughput(&self) -> f32 {
        self.disks
            .iter()
            .map(|d| d.read_bytes + d.write_bytes)
            .sum()
    }

    pub fn net_throughput(&self) -> f32 {
        self.nets.iter().map(|n| n.rx_bytes + n.tx_bytes).sum()
    }

    /// I/O 를 기다리며 멈춘 프로세스.
    pub fn blocked_procs(&self) -> Vec<&ProcRow> {
        self.procs.iter().filter(|p| p.is_blocked()).collect()
    }
}
