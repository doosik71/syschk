//! 로그가 차지하는 공간.
//!
//! `/var` 가 차는 가장 흔한 이유가 로그다. 저널 점유량은 systemd 가 직접 알려주고,
//! 파일 로그는 디렉터리를 재어 본다.

use super::dirsize::{self, DirScan};
use super::{Availability, Field, Probe, ProbeCtx, ProbeData, ProbeResult};
use crate::util::exec::ReadOnlyCommand;
use std::time::Duration;

/// 로그 점유 현황.
#[derive(Clone, Debug, Default)]
pub struct LogFootprint {
    /// systemd 저널이 쓰는 바이트. 얻지 못하면 `None`.
    pub journal_bytes: Option<u64>,
    /// 저널이 스스로 보고한 문장(그대로 근거로 보여준다).
    pub journal_report: Option<String>,
    /// `/var/log` 아래 항목.
    pub var_log: Option<DirScan>,
    pub availability: Availability,
}

impl LogFootprint {
    pub fn total_bytes(&self) -> u64 {
        // 저널은 보통 /var/log/journal 아래에 있으므로 이중 계산을 피한다.
        self.var_log
            .as_ref()
            .map(|s| s.total_bytes)
            .unwrap_or_else(|| self.journal_bytes.unwrap_or(0))
    }
}

/// `journalctl --disk-usage` 출력에서 바이트를 뽑는다.
///
/// 예: `Archived and active journals take up 3.9G in the file system.`
pub fn parse_journal_usage(text: &str) -> Option<u64> {
    let token = text.split_whitespace().find(|t| {
        let bytes = t.as_bytes();
        bytes.len() >= 2 && bytes[0].is_ascii_digit() && t.chars().any(|c| "KMGT".contains(c))
    })?;
    let digits: String = token
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    let value: f64 = digits.parse().ok()?;
    let unit = token[digits.len()..].chars().next()?;
    let multiplier = match unit {
        'K' => 1024_f64,
        'M' => 1024_f64 * 1024.0,
        'G' => 1024_f64 * 1024.0 * 1024.0,
        'T' => 1024_f64 * 1024.0 * 1024.0 * 1024.0,
        _ => 1.0,
    };
    Some((value * multiplier) as u64)
}

/// 로그 점유량을 조사한다. `/var/log` 를 재는 데 시간이 걸리므로 예산을 받는다.
pub fn footprint(ctx: &ProbeCtx, budget: Duration) -> LogFootprint {
    let mut result = LogFootprint::default();

    // 저널은 systemd 에게 물어보는 것이 가장 정확하다.
    if crate::util::exec::find_in_path("journalctl").is_some()
        && let Ok(cmd) = ReadOnlyCommand::new("journalctl", &["--disk-usage"])
    {
        let out = cmd.run();
        if out.succeeded() {
            result.journal_bytes = parse_journal_usage(&out.stdout);
            result.journal_report = Some(out.stdout.trim().to_string());
        } else {
            result.availability = Availability::NeedsPrivilege {
                hint: "the journal did not answer; some journals are only readable by root".into(),
            };
        }
    } else {
        result.availability = Availability::NotInstalled { tool: "systemd" };
    }

    let log_dir = ctx.path("/var/log");
    if log_dir.exists() {
        result.var_log = Some(dirsize::scan(&log_dir, budget));
    }
    result
}

pub struct LogSpaceProbe;

impl Probe for LogSpaceProbe {
    fn id(&self) -> &'static str {
        "storage.logs"
    }

    fn describe(&self) -> &'static str {
        "How much space logs are using, journal and plain files"
    }

    fn commands(&self) -> Vec<&'static str> {
        vec!["journalctl --disk-usage"]
    }

    fn required_tools(&self) -> &'static [&'static str] {
        &["systemd"]
    }

    fn run(&self, ctx: &ProbeCtx) -> ProbeResult {
        let found = footprint(ctx, Duration::from_millis(500));
        let mut result = ProbeResult::ok(
            "storage.logs",
            ProbeData::Fields(vec![
                Field::new(
                    "journal_bytes",
                    found
                        .journal_bytes
                        .map(|b| b.to_string())
                        .unwrap_or_else(|| "unknown".into()),
                ),
                Field::new("var_log_bytes", found.total_bytes().to_string()),
            ]),
        );
        result.availability = found.availability;
        result
    }
}
