//! 메모리와 스왑.
//!
//! `free` 가 보여주는 것과 같은 값을 `/proc/meminfo` 에서 직접 읽는다.
//! 초보자가 가장 많이 오해하는 지점이 "used"이므로, 캐시와 실사용을 구분해 제시한다.

use super::{Availability, Field, Probe, ProbeCtx, ProbeData, ProbeResult};

/// 메모리 현황(단위: KiB).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Memory {
    pub total: u64,
    pub free: u64,
    /// 커널이 계산한 "새 작업에 실제로 쓸 수 있는" 양. used 계산의 기준이다.
    pub available: u64,
    pub buffers: u64,
    pub cached: u64,
    pub swap_total: u64,
    pub swap_free: u64,
    pub dirty: u64,
    pub shmem: u64,
}

impl Memory {
    pub fn read(ctx: &ProbeCtx) -> Option<Self> {
        let text = ctx.read("/proc/meminfo")?;
        let mut m = Memory::default();
        for line in text.lines() {
            let Some((key, rest)) = line.split_once(':') else {
                continue;
            };
            let Some(value) = rest.split_whitespace().next().and_then(|v| v.parse().ok()) else {
                continue;
            };
            match key {
                "MemTotal" => m.total = value,
                "MemFree" => m.free = value,
                "MemAvailable" => m.available = value,
                "Buffers" => m.buffers = value,
                "Cached" => m.cached = value,
                "SwapTotal" => m.swap_total = value,
                "SwapFree" => m.swap_free = value,
                "Dirty" => m.dirty = value,
                "Shmem" => m.shmem = value,
                _ => {}
            }
        }
        (m.total > 0).then_some(m)
    }

    /// 캐시를 제외한 실사용량.
    pub fn used(&self) -> u64 {
        self.total.saturating_sub(self.available)
    }

    pub fn used_pct(&self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            self.used() as f32 / self.total as f32 * 100.0
        }
    }

    pub fn available_pct(&self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            self.available as f32 / self.total as f32 * 100.0
        }
    }

    pub fn swap_used(&self) -> u64 {
        self.swap_total.saturating_sub(self.swap_free)
    }

    pub fn swap_used_pct(&self) -> f32 {
        if self.swap_total == 0 {
            0.0
        } else {
            self.swap_used() as f32 / self.swap_total as f32 * 100.0
        }
    }
}

/// `/proc/vmstat` 의 누적 카운터 중 메모리 압박과 관련된 것.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VmStat {
    pub pswpin: u64,
    pub pswpout: u64,
    pub pgmajfault: u64,
    pub pgscan_direct: u64,
    pub oom_kill: u64,
}

impl VmStat {
    pub fn read(ctx: &ProbeCtx) -> Option<Self> {
        let text = ctx.read("/proc/vmstat")?;
        let mut v = VmStat::default();
        for line in text.lines() {
            let Some((key, value)) = line.split_once(' ') else {
                continue;
            };
            let Ok(n) = value.trim().parse::<u64>() else {
                continue;
            };
            match key {
                "pswpin" => v.pswpin = n,
                "pswpout" => v.pswpout = n,
                "pgmajfault" => v.pgmajfault = n,
                "pgscan_direct" => v.pgscan_direct = n,
                "oom_kill" => v.oom_kill = n,
                _ => {}
            }
        }
        Some(v)
    }
}

/// 초당 변화량.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SwapRate {
    pub pages_in: f32,
    pub pages_out: f32,
    pub major_faults: f32,
}

pub fn swap_rate(prev: &VmStat, now: &VmStat, secs: f32) -> SwapRate {
    let per = |a: u64, b: u64| {
        if secs <= 0.0 {
            0.0
        } else {
            b.saturating_sub(a) as f32 / secs
        }
    };
    SwapRate {
        pages_in: per(prev.pswpin, now.pswpin),
        pages_out: per(prev.pswpout, now.pswpout),
        major_faults: per(prev.pgmajfault, now.pgmajfault),
    }
}

pub struct MemoryProbe;

impl Probe for MemoryProbe {
    fn id(&self) -> &'static str {
        "memory.usage"
    }

    fn describe(&self) -> &'static str {
        "Memory in use versus cached, plus swap activity"
    }

    fn commands(&self) -> Vec<&'static str> {
        vec![
            "cat /proc/meminfo",
            "cat /proc/vmstat",
            "cat /proc/pressure/memory",
        ]
    }

    fn run(&self, ctx: &ProbeCtx) -> ProbeResult {
        let Some(m) = Memory::read(ctx) else {
            return ProbeResult::unavailable(
                "memory.usage",
                Availability::ParseFailed {
                    reason: "/proc/meminfo is not readable".into(),
                },
            );
        };
        ProbeResult::ok(
            "memory.usage",
            ProbeData::Fields(vec![
                Field::new("total_kb", m.total.to_string()),
                Field::new("used_pct", format!("{:.1}", m.used_pct())),
                Field::new("swap_used_kb", m.swap_used().to_string()),
            ]),
        )
    }
}
