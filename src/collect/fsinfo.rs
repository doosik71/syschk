//! 파일시스템 상태.
//!
//! "용량은 남았는데 쓸 수 없다"와 "파일시스템이 이상하다"를 구분하는 데 필요한 값을 모은다.
//! 외부 도구 없이 `statvfs` 와 `/sys/fs/ext4/*` 만 쓴다.

use super::mounts::{Mount, MountUsage};
use super::{Availability, Field, Probe, ProbeCtx, ProbeData, ProbeResult};

/// 파일시스템 하나의 상태.
#[derive(Clone, Debug)]
pub struct FsHealth {
    pub target: String,
    pub source: String,
    pub fstype: String,
    /// 읽기 전용으로 마운트되어 있는가.
    pub read_only: bool,
    /// 마운트 옵션 중 눈여겨볼 것.
    pub notable_options: Vec<String>,
    /// root 에게만 남은 예약 공간.
    pub reserved_bytes: u64,
    /// ext4 가 기록한 오류 횟수. 지원하지 않으면 `None`.
    pub ext4_errors: Option<u64>,
    /// 마지막 오류 시각(커널이 기록한 문자열).
    pub ext4_last_error: Option<String>,
}

impl FsHealth {
    /// 커널이 오류를 만나면 읽기 전용으로 전환하는 설정인가.
    pub fn remounts_read_only_on_error(&self) -> bool {
        self.notable_options
            .iter()
            .any(|o| o == "errors=remount-ro")
    }
}

/// 마운트 옵션 중 사용자에게 의미가 있는 것만 남긴다.
fn notable(mount: &Mount) -> Vec<String> {
    const INTERESTING: &[&str] = &[
        "ro",
        "noatime",
        "relatime",
        "nodiratime",
        "sync",
        "nobarrier",
        "discard",
        "errors=remount-ro",
        "errors=continue",
        "errors=panic",
        "data=writeback",
        "nofail",
    ];
    mount
        .options
        .iter()
        .filter(|o| INTERESTING.contains(&o.as_str()))
        .cloned()
        .collect()
}

/// ext4 가 `/sys/fs/ext4/<device>/` 에 남기는 오류 기록.
///
/// 커널 로그를 뒤지지 않고도 "이 파일시스템에서 오류가 있었다"를 알 수 있는 유일한 경로다.
fn ext4_errors(ctx: &ProbeCtx, source: &str) -> (Option<u64>, Option<String>) {
    let device = source.rsplit('/').next().unwrap_or(source);
    let base = format!("/sys/fs/ext4/{device}");
    let count = ctx
        .read(&format!("{base}/errors_count"))
        .and_then(|v| v.trim().parse::<u64>().ok());
    let last = ctx
        .read(&format!("{base}/last_error_time"))
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty() && v != "0");
    (count, last)
}

/// 파일시스템별 상태를 모은다.
pub fn health(ctx: &ProbeCtx, usage: &[MountUsage]) -> Vec<FsHealth> {
    usage
        .iter()
        .map(|mu| {
            let (ext4_errors, ext4_last_error) = if mu.mount.fstype.starts_with("ext") {
                ext4_errors(ctx, &mu.mount.source)
            } else {
                (None, None)
            };
            FsHealth {
                target: mu.mount.target.clone(),
                source: mu.mount.source.clone(),
                fstype: mu.mount.fstype.clone(),
                read_only: mu.mount.is_read_only(),
                notable_options: notable(&mu.mount),
                reserved_bytes: mu.usage.map(|u| u.reserved_bytes()).unwrap_or(0),
                ext4_errors,
                ext4_last_error,
            }
        })
        .collect()
}

pub struct FsHealthProbe;

impl Probe for FsHealthProbe {
    fn id(&self) -> &'static str {
        "storage.fs-health"
    }

    fn describe(&self) -> &'static str {
        "Mount options, read-only state, reserved space and recorded filesystem errors"
    }

    fn commands(&self) -> Vec<&'static str> {
        vec![
            "cat /proc/self/mountinfo",
            "cat /sys/fs/ext4/DEVICE/errors_count",
        ]
    }

    fn run(&self, ctx: &ProbeCtx) -> ProbeResult {
        let usage = super::mounts::usage(ctx);
        if usage.is_empty() {
            return ProbeResult::unavailable(
                "storage.fs-health",
                Availability::ParseFailed {
                    reason: "no mounted filesystems could be read".into(),
                },
            );
        }
        let list = health(ctx, &usage);
        ProbeResult::ok(
            "storage.fs-health",
            ProbeData::Fields(
                list.iter()
                    .map(|f| {
                        Field::new(
                            f.target.clone(),
                            if f.read_only {
                                "read-only".to_string()
                            } else {
                                "read-write".to_string()
                            },
                        )
                    })
                    .collect(),
            ),
        )
    }
}
