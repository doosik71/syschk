//! 시스템 기본 정보 수집기.
//!
//! M0 에서 계약을 실제로 통과시키는 최소 수집기다. 외부 명령 없이 `/proc` 와 `/etc` 만
//! 읽으므로 도구 설치 여부와 무관하게 동작한다(설계 원칙: 설치 없이 되는 것은 되게 한다).

use super::{Availability, Field, Probe, ProbeCtx, ProbeData, ProbeResult};
use crate::util::fmt::duration_human;

/// 이 시스템이 무엇인가.
pub struct Identity;

impl Probe for Identity {
    fn id(&self) -> &'static str {
        "system.identity"
    }

    fn describe(&self) -> &'static str {
        "Which machine this is: hostname, Ubuntu release, kernel version"
    }

    fn commands(&self) -> Vec<&'static str> {
        vec!["hostnamectl status", "cat /etc/os-release", "uname -a"]
    }

    fn run(&self, ctx: &ProbeCtx) -> ProbeResult {
        let hostname = ctx
            .read("/proc/sys/kernel/hostname")
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|| "unknown".into());
        let kernel = ctx
            .read("/proc/sys/kernel/osrelease")
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|| "unknown".into());
        let os = ctx
            .read("/etc/os-release")
            .and_then(|s| pretty_name(&s))
            .unwrap_or_else(|| "unknown".into());

        ProbeResult::ok(
            "system.identity",
            ProbeData::Fields(vec![
                Field::new("hostname", hostname),
                Field::new("os", os),
                Field::new("kernel", kernel),
            ]),
        )
    }
}

fn pretty_name(os_release: &str) -> Option<String> {
    os_release
        .lines()
        .find_map(|l| l.strip_prefix("PRETTY_NAME="))
        .map(|v| v.trim_matches('"').to_string())
}

/// 언제부터 켜져 있나.
pub struct Uptime;

impl Probe for Uptime {
    fn id(&self) -> &'static str {
        "system.uptime"
    }

    fn describe(&self) -> &'static str {
        "How long the machine has been up"
    }

    fn commands(&self) -> Vec<&'static str> {
        vec!["uptime -p", "cat /proc/uptime"]
    }

    fn run(&self, ctx: &ProbeCtx) -> ProbeResult {
        let Some(raw) = ctx.read("/proc/uptime") else {
            return ProbeResult::unavailable(
                "system.uptime",
                Availability::ParseFailed {
                    reason: "/proc/uptime is not readable".into(),
                },
            );
        };
        let Some(secs) = raw
            .split_whitespace()
            .next()
            .and_then(|s| s.parse::<f64>().ok())
        else {
            return ProbeResult::unavailable(
                "system.uptime",
                Availability::ParseFailed {
                    reason: "unexpected /proc/uptime format".into(),
                },
            );
        };
        ProbeResult::ok(
            "system.uptime",
            ProbeData::Fields(vec![
                Field::new("uptime", duration_human(secs as u64)),
                Field::new("seconds", format!("{}", secs as u64)),
            ]),
        )
    }
}

/// 부하 평균과 논리 코어 수.
pub struct LoadAverage;

impl Probe for LoadAverage {
    fn id(&self) -> &'static str {
        "system.load"
    }

    fn describe(&self) -> &'static str {
        "Load average against the number of logical cores"
    }

    fn commands(&self) -> Vec<&'static str> {
        vec!["cat /proc/loadavg", "uptime"]
    }

    fn run(&self, ctx: &ProbeCtx) -> ProbeResult {
        let Some(raw) = ctx.read("/proc/loadavg") else {
            return ProbeResult::unavailable(
                "system.load",
                Availability::ParseFailed {
                    reason: "/proc/loadavg is not readable".into(),
                },
            );
        };
        let parts: Vec<&str> = raw.split_whitespace().collect();
        if parts.len() < 3 {
            return ProbeResult::unavailable(
                "system.load",
                Availability::ParseFailed {
                    reason: "unexpected /proc/loadavg format".into(),
                },
            );
        }
        let cores = count_cores(ctx);
        ProbeResult::ok(
            "system.load",
            ProbeData::Fields(vec![
                Field::new("load1", parts[0]),
                Field::new("load5", parts[1]),
                Field::new("load15", parts[2]),
                Field::new("cores", cores.map(|c| c.to_string()).unwrap_or_default()),
            ]),
        )
    }
}

/// `/proc/cpuinfo` 의 프로세서 항목 수. 읽을 수 없으면 런타임에 물어본다.
fn count_cores(ctx: &ProbeCtx) -> Option<usize> {
    if let Some(info) = ctx.read("/proc/cpuinfo") {
        let n = info.lines().filter(|l| l.starts_with("processor")).count();
        if n > 0 {
            return Some(n);
        }
    }
    std::thread::available_parallelism().ok().map(|n| n.get())
}
