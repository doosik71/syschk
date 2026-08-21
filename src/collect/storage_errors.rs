//! 저장장치 관련 커널 오류.
//!
//! "디스크가 이상하다"는 의심은 커널 로그에서 확인하는 것이 가장 빠르다.
//! 여기서는 저장장치 패턴만 좁혀 본다. 로그 탐색 화면 전체는 M3 에서 다룬다.

use super::{Availability, Field, Probe, ProbeCtx, ProbeData, ProbeResult};
use crate::util::exec::ReadOnlyCommand;

/// 찾을 패턴과 그 뜻.
const PATTERNS: &[(&str, &str)] = &[
    (
        "I/O error",
        "The kernel could not complete a read or write. The most direct sign of a failing drive or link.",
    ),
    (
        "medium error",
        "The drive reported that the data on the surface could not be read.",
    ),
    (
        "critical medium error",
        "An unrecoverable read failure reported by the drive itself.",
    ),
    (
        "failed command",
        "A command the drive did not complete. Often accompanies link resets.",
    ),
    (
        "link is slow to respond",
        "The link between board and drive is unstable - frequently a cable or power issue.",
    ),
    (
        "hard resetting link",
        "The kernel reset the connection to the drive after it stopped answering.",
    ),
    (
        "EXT4-fs error",
        "The filesystem found something inconsistent. Worth checking even if the drive is healthy.",
    ),
    (
        "Remounting filesystem read-only",
        "The kernel gave up on writing and made the filesystem read-only to protect it.",
    ),
    (
        "nvme.*(reset|timeout|abort)",
        "An NVMe drive stopped answering and had to be reset.",
    ),
];

/// 한 패턴에 대한 결과.
#[derive(Clone, Debug)]
pub struct ErrorHits {
    pub pattern: &'static str,
    pub meaning: &'static str,
    pub count: usize,
    /// 가장 최근 몇 줄. 원문 그대로 보여준다.
    pub samples: Vec<String>,
}

/// 커널 로그에서 저장장치 오류를 찾은 결과.
#[derive(Clone, Debug, Default)]
pub struct StorageErrors {
    pub hits: Vec<ErrorHits>,
    pub availability: Availability,
    /// 조회한 범위 설명.
    pub scope: String,
}

impl StorageErrors {
    pub fn total(&self) -> usize {
        self.hits.iter().map(|h| h.count).sum()
    }

    pub fn clean(&self) -> bool {
        self.availability.is_ok() && self.total() == 0
    }
}

/// 이번 부팅의 커널 로그에서 저장장치 오류를 찾는다.
pub fn scan(boots_back: u32) -> StorageErrors {
    let mut result = StorageErrors {
        scope: if boots_back == 0 {
            "this boot".to_string()
        } else {
            format!("{boots_back} boot(s) ago")
        },
        ..Default::default()
    };

    if crate::util::exec::find_in_path("journalctl").is_none() {
        result.availability = Availability::NotInstalled { tool: "systemd" };
        return result;
    }

    let boot = format!("-{boots_back}");
    for (pattern, meaning) in PATTERNS {
        let args = vec!["-k", "-b", boot.as_str(), "--grep", pattern, "--no-pager"];
        let Ok(cmd) = ReadOnlyCommand::new("journalctl", &args) else {
            continue;
        };
        let out = cmd.run();
        if out.spawn_error.is_some() {
            result.availability = Availability::ParseFailed {
                reason: out.spawn_error.clone().unwrap_or_default(),
            };
            return result;
        }
        // `--grep` 이 아무것도 못 찾으면 0 이 아닌 코드로 끝난다. 오류가 아니다.
        let lines: Vec<String> = out
            .stdout
            .lines()
            .filter(|l| !l.starts_with("-- ") && !l.trim().is_empty())
            .map(str::to_string)
            .collect();
        if lines.is_empty() {
            continue;
        }
        let count = lines.len();
        let samples = lines.into_iter().rev().take(3).collect();
        result.hits.push(ErrorHits {
            pattern,
            meaning,
            count,
            samples,
        });
    }
    result
}

pub struct StorageErrorProbe;

impl Probe for StorageErrorProbe {
    fn id(&self) -> &'static str {
        "storage.errors"
    }

    fn describe(&self) -> &'static str {
        "Storage-related kernel errors: I/O failures, link resets, filesystem errors"
    }

    fn commands(&self) -> Vec<&'static str> {
        vec!["journalctl -k -b -0 --grep \"I/O error\" --no-pager"]
    }

    fn required_tools(&self) -> &'static [&'static str] {
        &["systemd"]
    }

    fn run(&self, _ctx: &ProbeCtx) -> ProbeResult {
        let found = scan(0);
        let mut result = ProbeResult::ok(
            "storage.errors",
            ProbeData::Fields(
                found
                    .hits
                    .iter()
                    .map(|h| Field::new(h.pattern.to_string(), h.count.to_string()))
                    .collect(),
            ),
        );
        result.availability = found.availability;
        result
    }
}
