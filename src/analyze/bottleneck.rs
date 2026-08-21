//! 병목 축 특정.
//!
//! "왜 느린가"에 답하려면 CPU·메모리·디스크·네트워크 중 어디가 일을 지연시키고 있는지를
//! 골라야 한다. 사용률만으로는 축 사이 비교가 어렵기 때문에, 커널의 압박 지표(PSI)를
//! 1순위 근거로 쓰고 사용률·대기시간을 보조 근거로 쓴다.

use super::rules::{self, Finding, Verdict};
use crate::collect::blockio::DiskRate;
use crate::collect::cpu::CpuUsage;
use crate::collect::memory::{Memory, SwapRate};
use crate::collect::network::NetRate;
use crate::collect::pressure::PressureSet;
use crate::util::fmt::{bytes_per_sec, kib, pct};

/// 병목 축.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    Cpu,
    Memory,
    Disk,
    Network,
    /// 어느 축도 포화되지 않았다.
    None,
}

impl Axis {
    pub fn label(self) -> &'static str {
        match self {
            Axis::Cpu => "cpu",
            Axis::Memory => "memory",
            Axis::Disk => "disk",
            Axis::Network => "network",
            Axis::None => "none",
        }
    }
}

/// 판정에 필요한 수치 묶음.
pub struct Metrics<'a> {
    pub cpu: &'a CpuUsage,
    pub load1: f32,
    pub cores: usize,
    pub pressure: &'a PressureSet,
    pub memory: &'a Memory,
    pub swap: SwapRate,
    pub disks: &'a [DiskRate],
    pub nets: &'a [NetRate],
    /// I/O 를 기다리며 멈춘 프로세스 수.
    pub blocked: u32,
    /// 지금까지 모은 표본 수. 2 미만이면 비율을 계산할 수 없다.
    pub samples: u32,
}

/// 축별 판정과 최종 결론.
#[derive(Clone, Debug)]
pub struct Assessment {
    pub axis: Axis,
    pub headline: String,
    pub findings: Vec<Finding>,
}

impl Assessment {
    pub fn finding(&self, axis: &str) -> Option<&Finding> {
        self.findings.iter().find(|f| f.axis == axis)
    }

    pub fn worst(&self) -> Verdict {
        self.findings
            .iter()
            .map(|f| f.verdict)
            .max()
            .unwrap_or(Verdict::Unknown)
    }
}

/// 네 축을 판정하고 병목을 고른다.
pub fn assess(m: &Metrics<'_>) -> Assessment {
    let findings = vec![cpu(m), memory(m), disk(m), network(m)];

    // 가장 심각한 축을 고르고, 동급이면 압박 지표가 큰 축을 고른다.
    let worst = findings
        .iter()
        .map(|f| f.verdict)
        .max()
        .unwrap_or(Verdict::Unknown);
    let axis = if worst <= Verdict::Ok {
        Axis::None
    } else {
        let mut best = (Axis::None, -1.0_f32);
        for f in &findings {
            if f.verdict != worst {
                continue;
            }
            let (axis, score) = match f.axis {
                "cpu" => (Axis::Cpu, psi(m, |p| p.cpu).max(m.cpu.busy)),
                "memory" => (
                    Axis::Memory,
                    psi(m, |p| p.memory).max(100.0 - m.memory.available_pct()),
                ),
                "disk" => (
                    Axis::Disk,
                    psi(m, |p| p.io).max(m.disks.first().map_or(0.0, |d| d.util_pct)),
                ),
                _ => (Axis::Network, 0.0),
            };
            if score > best.1 {
                best = (axis, score);
            }
        }
        best.0
    };

    let headline = match (axis, worst) {
        (Axis::None, Verdict::Unknown) => {
            "Still measuring - rates need a second sample".to_string()
        }
        (Axis::None, _) => "Nothing is saturated right now".to_string(),
        (a, Verdict::Critical) => format!("{} is the bottleneck, and it is severe", cap(a.label())),
        (a, _) => format!("{} is the most likely bottleneck", cap(a.label())),
    };

    Assessment {
        axis,
        headline,
        findings,
    }
}

fn cap(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(first) => first.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// PSI 값을 꺼낸다. 커널이 제공하지 않으면 0 으로 두고 다른 근거를 쓴다.
fn psi(
    m: &Metrics<'_>,
    pick: fn(&PressureSet) -> Option<crate::collect::pressure::Pressure>,
) -> f32 {
    pick(m.pressure).map_or(0.0, |p| p.some_avg10)
}

fn measuring(axis: &'static str) -> Finding {
    Finding::new(axis, Verdict::Unknown, "Measuring - one more sample needed")
        .evidence("rates are the difference between two samples, taken one second apart")
}

fn cpu(m: &Metrics<'_>) -> Finding {
    if m.samples < 2 {
        return measuring("cpu");
    }
    let per_core = if m.cores == 0 {
        m.load1
    } else {
        m.load1 / m.cores as f32
    };
    let p = psi(m, |s| s.cpu);

    let mut f = if m.cpu.steal > rules::CPU_STEAL_WARN {
        Finding::new(
            "cpu",
            Verdict::Warn,
            "The hypervisor is taking CPU time away from this machine",
        )
    } else if p > rules::PSI_CRITICAL || per_core > rules::LOAD_PER_CORE_CRITICAL {
        Finding::new(
            "cpu",
            Verdict::Critical,
            "Far more work is queued than the CPUs can run",
        )
    } else if p > rules::PSI_WARN
        || per_core > rules::LOAD_PER_CORE_WARN
        || m.cpu.busy > rules::CPU_BUSY_WARN
    {
        Finding::new("cpu", Verdict::Warn, "CPU is close to saturated")
    } else {
        Finding::new("cpu", Verdict::Ok, "CPU is not the bottleneck")
    };

    f = f
        .evidence(format!(
            "busy {} (user {}, system {}), idle {}",
            pct(m.cpu.busy),
            pct(m.cpu.user),
            pct(m.cpu.system),
            pct(m.cpu.idle)
        ))
        .evidence(format!(
            "load {:.2} across {} cores = {:.2} per core (warn above {:.1})",
            m.load1,
            m.cores,
            per_core,
            rules::LOAD_PER_CORE_WARN
        ));
    if m.cpu.steal > 0.1 {
        f = f.evidence(format!(
            "steal {} - time given to other guests",
            pct(m.cpu.steal)
        ));
    }
    f = match m.pressure.cpu {
        Some(cpu_psi) => f.evidence(format!(
            "pressure: work was delayed {} of the last 10s",
            pct(cpu_psi.some_avg10)
        )),
        None => f.evidence("pressure: this kernel does not report CPU pressure"),
    };

    f.learn(
        "Load counts how many processes want to run. Divided by the core count it says \
         whether the queue is longer than the machine can serve; 'busy' says how much of \
         the CPU was actually used.",
    )
    .next("live.cpu")
}

fn memory(m: &Metrics<'_>) -> Finding {
    if m.samples < 2 {
        return measuring("memory");
    }
    let avail = m.memory.available_pct();
    let p = psi(m, |s| s.memory);

    let mut f = if avail < rules::MEM_AVAILABLE_CRITICAL {
        Finding::new(
            "memory",
            Verdict::Critical,
            "Almost no memory is left for new work",
        )
    } else if m.swap.pages_out > rules::SWAP_OUT_WARN && m.memory.swap_used() > 0 {
        Finding::new(
            "memory",
            Verdict::Warn,
            "The system is pushing memory out to swap",
        )
    } else if p > rules::PSI_MEMORY_WARN {
        Finding::new(
            "memory",
            Verdict::Warn,
            "Reclaiming memory is stalling work",
        )
    } else if avail < rules::MEM_AVAILABLE_WARN {
        Finding::new("memory", Verdict::Warn, "Memory is getting tight")
    } else {
        Finding::new("memory", Verdict::Ok, "Memory is not the bottleneck")
    };

    f = f
        .evidence(format!(
            "{} of {} in use, {} still available ({})",
            kib(m.memory.used()),
            kib(m.memory.total),
            kib(m.memory.available),
            pct(avail)
        ))
        .evidence(format!(
            "{} held as cache - this is reusable, not lost",
            kib(m.memory.cached + m.memory.buffers)
        ));
    if m.memory.swap_total > 0 {
        f = f.evidence(format!(
            "swap {} of {} used, {:.0} pages/s in, {:.0} pages/s out",
            kib(m.memory.swap_used()),
            kib(m.memory.swap_total),
            m.swap.pages_in,
            m.swap.pages_out
        ));
    } else {
        f = f.evidence("no swap configured on this system");
    }
    if let Some(mem_psi) = m.pressure.memory {
        f = f.evidence(format!(
            "pressure: work was delayed {} of the last 10s",
            pct(mem_psi.some_avg10)
        ));
    }

    f.learn(
        "Cached memory is not lost memory: Linux keeps file cache to speed things up and \
         gives it back when a program needs it. Watch 'available', not 'free'. Swapping out \
         at a steady rate is the real warning sign.",
    )
    .next("live.memory")
}

fn disk(m: &Metrics<'_>) -> Finding {
    if m.samples < 2 {
        return measuring("disk");
    }
    let p = psi(m, |s| s.io);
    let busiest = m
        .disks
        .iter()
        .max_by(|a, b| {
            a.util_pct
                .partial_cmp(&b.util_pct)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .cloned();

    let saturated = busiest
        .as_ref()
        .map(|d| d.util_pct > rules::DISK_UTIL_WARN && d.await_ms > rules::DISK_AWAIT_WARN_MS)
        .unwrap_or(false);

    let mut f = if p > rules::PSI_CRITICAL {
        Finding::new(
            "disk",
            Verdict::Critical,
            "Storage is holding up most of the work",
        )
    } else if p > rules::PSI_WARN || saturated {
        Finding::new("disk", Verdict::Warn, "Storage is struggling to keep up")
    } else if m.cpu.iowait > rules::IOWAIT_WARN || m.blocked > 0 {
        Finding::new(
            "disk",
            Verdict::Warn,
            "Processes are spending time waiting on storage",
        )
    } else {
        Finding::new("disk", Verdict::Ok, "Storage is not the bottleneck")
    };

    f = f.evidence(format!(
        "cpu waited on storage {} of the time (iowait)",
        pct(m.cpu.iowait)
    ));
    match &busiest {
        Some(d) => {
            f = f
                .evidence(format!(
                    "busiest drive {}: {} busy, {:.1}ms average wait",
                    d.name,
                    pct(d.util_pct),
                    d.await_ms
                ))
                .evidence(format!(
                    "{} reading {}, writing {} ({:.0} + {:.0} ops/s)",
                    d.name,
                    bytes_per_sec(d.read_bytes),
                    bytes_per_sec(d.write_bytes),
                    d.read_iops,
                    d.write_iops
                ));
        }
        None => f = f.evidence("no block device activity could be measured"),
    }
    f = f.evidence(format!(
        "{} process(es) stuck in uninterruptible wait",
        m.blocked
    ));
    if let Some(io_psi) = m.pressure.io {
        f = f.evidence(format!(
            "pressure: work was delayed {} of the last 10s",
            pct(io_psi.some_avg10)
        ));
    }

    f.learn(
        "'Busy' is how much of the time the drive had work in flight; 'wait' is how long each \
         request took. A busy drive with a short wait is healthy - a long wait means requests \
         are queueing up.",
    )
    .next("live.disk-io")
}

fn network(m: &Metrics<'_>) -> Finding {
    if m.samples < 2 {
        return measuring("network");
    }
    let errors_now: f32 = m.nets.iter().map(|n| n.errors_per_sec).sum();
    let errors_total: u64 = m.nets.iter().map(|n| n.errors_total).sum();
    let drops_now: f32 = m.nets.iter().map(|n| n.drops_per_sec).sum();
    let drops_total: u64 = m.nets.iter().map(|n| n.drops_total).sum();

    // 오류는 링크 품질 문제를 뜻한다. 드롭만 늘어나는 것은 정상 시스템에서도 흔하다.
    let mut f = if errors_now > 0.0 {
        Finding::new(
            "network",
            Verdict::Warn,
            "Interfaces are reporting transmission errors right now",
        )
    } else if errors_total > 0 {
        Finding::new(
            "network",
            Verdict::Ok,
            "No new interface errors, though some happened earlier",
        )
    } else {
        Finding::new(
            "network",
            Verdict::Ok,
            "No interface errors are accumulating",
        )
    };

    match m.nets.first() {
        Some(n) => {
            f = f.evidence(format!(
                "busiest interface {}: in {}, out {}",
                n.name,
                bytes_per_sec(n.rx_bytes),
                bytes_per_sec(n.tx_bytes)
            ));
        }
        None => f = f.evidence("no interface activity could be measured"),
    }
    f = f
        .evidence(format!(
            "errors: {errors_total} since boot, {errors_now:.1} per second now"
        ))
        .evidence(format!(
            "drops: {drops_total} since boot, {drops_now:.1} per second now"
        ));

    f.learn(
        "Throughput alone cannot tell you whether a link is saturated - that needs the \
         negotiated link speed. Errors mean the link itself is failing and should stay at \
         zero. Drops are different: packets nobody was listening for are counted too, so a \
         steady trickle of drops is normal.",
    )
    .next("live.network")
}
