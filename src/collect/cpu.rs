//! CPU 사용률과 부하.
//!
//! `/proc/stat` 은 부팅 이후 누적값이므로, 두 표본의 차이로 비율을 계산한다.
//! `top` 이나 `mpstat` 이 하는 일과 같다.

use super::{Availability, Field, Probe, ProbeCtx, ProbeData, ProbeResult};

/// `/proc/stat` 의 CPU 시간 누적값(단위: jiffies).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CpuTimes {
    pub user: u64,
    pub nice: u64,
    pub system: u64,
    pub idle: u64,
    pub iowait: u64,
    pub irq: u64,
    pub softirq: u64,
    pub steal: u64,
}

impl CpuTimes {
    fn parse(line: &str) -> Option<Self> {
        let mut it = line.split_whitespace();
        it.next()?; // "cpu" 또는 "cpuN"
        let mut v = [0u64; 8];
        for slot in v.iter_mut() {
            *slot = it.next().and_then(|t| t.parse().ok()).unwrap_or(0);
        }
        Some(Self {
            user: v[0],
            nice: v[1],
            system: v[2],
            idle: v[3],
            iowait: v[4],
            irq: v[5],
            softirq: v[6],
            steal: v[7],
        })
    }

    fn total(&self) -> u64 {
        self.user
            + self.nice
            + self.system
            + self.idle
            + self.iowait
            + self.irq
            + self.softirq
            + self.steal
    }
}

/// 한 시점의 CPU 상태.
#[derive(Clone, Debug, Default)]
pub struct CpuSnapshot {
    pub total: CpuTimes,
    pub per_core: Vec<CpuTimes>,
    /// 실행 가능 상태로 대기 중인 프로세스 수.
    pub procs_running: u32,
    /// I/O 를 기다리며 멈춰 있는 프로세스 수(D 상태).
    pub procs_blocked: u32,
    pub context_switches: u64,
}

impl CpuSnapshot {
    pub fn read(ctx: &ProbeCtx) -> Option<Self> {
        let text = ctx.read("/proc/stat")?;
        let mut snap = CpuSnapshot::default();
        for line in text.lines() {
            if let Some(rest) = line.strip_prefix("cpu") {
                if rest.starts_with(' ') {
                    snap.total = CpuTimes::parse(line)?;
                } else if rest.chars().next().is_some_and(|c| c.is_ascii_digit())
                    && let Some(t) = CpuTimes::parse(line)
                {
                    snap.per_core.push(t);
                }
            } else if let Some(v) = line.strip_prefix("procs_running ") {
                snap.procs_running = v.trim().parse().unwrap_or(0);
            } else if let Some(v) = line.strip_prefix("procs_blocked ") {
                snap.procs_blocked = v.trim().parse().unwrap_or(0);
            } else if let Some(v) = line.strip_prefix("ctxt ") {
                snap.context_switches = v.trim().parse().unwrap_or(0);
            }
        }
        (!snap.per_core.is_empty() || snap.total.total() > 0).then_some(snap)
    }

    pub fn cores(&self) -> usize {
        self.per_core.len().max(1)
    }
}

/// 두 표본 사이의 CPU 사용률(%).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CpuUsage {
    pub user: f32,
    pub system: f32,
    pub iowait: f32,
    pub steal: f32,
    pub irq: f32,
    pub idle: f32,
    /// 실제로 일하고 있던 비율(idle 과 iowait 제외).
    pub busy: f32,
    /// 코어별 사용 비율(idle 제외).
    pub per_core: Vec<f32>,
}

fn ratio(delta: u64, total: u64) -> f32 {
    if total == 0 {
        0.0
    } else {
        (delta as f32 / total as f32) * 100.0
    }
}

fn usage_between(prev: CpuTimes, now: CpuTimes) -> (f32, f32, f32, f32, f32, f32) {
    let total = now.total().saturating_sub(prev.total());
    (
        ratio(
            now.user.saturating_sub(prev.user) + now.nice.saturating_sub(prev.nice),
            total,
        ),
        ratio(now.system.saturating_sub(prev.system), total),
        ratio(now.iowait.saturating_sub(prev.iowait), total),
        ratio(now.steal.saturating_sub(prev.steal), total),
        ratio(
            now.irq.saturating_sub(prev.irq) + now.softirq.saturating_sub(prev.softirq),
            total,
        ),
        ratio(now.idle.saturating_sub(prev.idle), total),
    )
}

/// 두 표본에서 사용률을 계산한다.
pub fn usage(prev: &CpuSnapshot, now: &CpuSnapshot) -> CpuUsage {
    let (user, system, iowait, steal, irq, idle) = usage_between(prev.total, now.total);
    let per_core = now
        .per_core
        .iter()
        .zip(prev.per_core.iter())
        .map(|(n, p)| {
            let (u, s, _, st, i, id) = usage_between(*p, *n);
            let _ = id;
            (u + s + st + i).clamp(0.0, 100.0)
        })
        .collect();
    CpuUsage {
        user,
        system,
        iowait,
        steal,
        irq,
        idle,
        busy: (user + system + irq + steal).clamp(0.0, 100.0),
        per_core,
    }
}

/// 부하 평균.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Load {
    pub one: f32,
    pub five: f32,
    pub fifteen: f32,
}

impl Load {
    pub fn read(ctx: &ProbeCtx) -> Option<Self> {
        let text = ctx.read("/proc/loadavg")?;
        let mut it = text.split_whitespace();
        Some(Self {
            one: it.next()?.parse().ok()?,
            five: it.next()?.parse().ok()?,
            fifteen: it.next()?.parse().ok()?,
        })
    }

    /// 논리 코어 수로 정규화한 값. 1.0 이면 코어를 꽉 채운 상태다.
    pub fn per_core(&self, cores: usize) -> f32 {
        if cores == 0 {
            self.one
        } else {
            self.one / cores as f32
        }
    }
}

/// 근거 표시용 수집기. 실제 표본 추출은 [`crate::app::sampler`] 가 담당한다.
pub struct CpuProbe;

impl Probe for CpuProbe {
    fn id(&self) -> &'static str {
        "cpu.usage"
    }

    fn describe(&self) -> &'static str {
        "CPU time split, per-core spread, run queue and load average"
    }

    fn commands(&self) -> Vec<&'static str> {
        vec![
            "cat /proc/stat",
            "cat /proc/loadavg",
            "cat /proc/pressure/cpu",
        ]
    }

    fn run(&self, ctx: &ProbeCtx) -> ProbeResult {
        let Some(snap) = CpuSnapshot::read(ctx) else {
            return ProbeResult::unavailable(
                "cpu.usage",
                Availability::ParseFailed {
                    reason: "/proc/stat is not readable".into(),
                },
            );
        };
        let load = Load::read(ctx).unwrap_or_default();
        ProbeResult::ok(
            "cpu.usage",
            ProbeData::Fields(vec![
                Field::new("cores", snap.cores().to_string()),
                Field::new("running", snap.procs_running.to_string()),
                Field::new("blocked", snap.procs_blocked.to_string()),
                Field::new("load1", format!("{:.2}", load.one)),
            ]),
        )
    }
}
