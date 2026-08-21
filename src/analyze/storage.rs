//! 저장 공간과 저장장치 판정.
//!
//! "디스크가 꽉 찼다"는 한 가지 상황이 아니다. 초보자가 스스로 구분하기 어려운 다섯 가지를
//! 이 모듈이 갈라 준다.
//!
//! 1. 큰 디렉터리가 실제로 공간을 쓰고 있다
//! 2. 지운 파일을 프로세스가 붙잡고 있어 공간이 돌아오지 않았다
//! 3. inode 가 고갈되어 용량과 무관하게 파일을 만들 수 없다
//! 4. 로그가 쌓였다
//! 5. 예약 블록 때문에 일반 사용자에게만 공간이 없다

use super::rules::{self, Finding, Verdict};
use crate::collect::blockdev::RaidArray;
use crate::collect::deleted::DeletedFiles;
use crate::collect::dirsize::DirScan;
use crate::collect::fsinfo::FsHealth;
use crate::collect::logspace::LogFootprint;
use crate::collect::mounts::MountUsage;
use crate::collect::smart::DriveHealth;
use crate::collect::storage_errors::StorageErrors;
use crate::util::fmt::{bytes, pct};

/// 공간 부족의 원인 후보.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FullCause {
    /// 아직 꽉 차지 않았다.
    NotFull,
    LargeDirectories,
    DeletedButHeld,
    InodesExhausted,
    Logs,
    ReservedForRoot,
    /// 원인을 좁히지 못했다(자료가 부족하다).
    Undetermined,
}

impl FullCause {
    pub fn label(self) -> &'static str {
        match self {
            FullCause::NotFull => "not full",
            FullCause::LargeDirectories => "large directories",
            FullCause::DeletedButHeld => "deleted files still held open",
            FullCause::InodesExhausted => "inodes exhausted",
            FullCause::Logs => "logs",
            FullCause::ReservedForRoot => "space reserved for root",
            FullCause::Undetermined => "undetermined",
        }
    }

    /// 사용자가 다음에 무엇을 하면 되는지. syschk 는 실행하지 않고 알려만 준다.
    pub fn what_to_do(self) -> &'static str {
        match self {
            FullCause::NotFull => "Nothing to do about space right now.",
            FullCause::LargeDirectories => {
                "Look at the largest directories and decide what can go. syschk does not delete anything."
            }
            FullCause::DeletedButHeld => {
                "Restarting the process that holds the files releases the space. A reboot also does."
            }
            FullCause::InodesExhausted => {
                "Millions of tiny files are the usual cause. Find the directory with the most entries and clear it out."
            }
            FullCause::Logs => {
                "Journal size is capped in /etc/systemd/journald.conf (SystemMaxUse). For plain log files, check logrotate."
            }
            FullCause::ReservedForRoot => {
                "The filesystem keeps a slice back for root. Free real space, or lower the reserve with tune2fs -m (this changes the filesystem)."
            }
            FullCause::Undetermined => {
                "Measure the largest directories, then check inodes and deleted-but-open files."
            }
        }
    }
}

/// 한 파일시스템에 대한 결론.
#[derive(Clone, Debug)]
pub struct SpaceDiagnosis {
    pub target: String,
    pub cause: FullCause,
    pub findings: Vec<Finding>,
}

/// 마운트별 공간·inode 판정.
pub fn space_findings(mounts: &[MountUsage]) -> Vec<Finding> {
    let mut out = Vec::new();
    for m in mounts {
        let Some(usage) = m.usage else {
            out.push(
                Finding::new("space", Verdict::Unknown, format!("{} could not be measured", m.mount.target))
                    .evidence("statvfs did not answer for this mount point")
                    .learn(
                        "A mount that cannot be measured is usually a network filesystem whose server is not answering.",
                    ),
            );
            continue;
        };
        let used = usage.used_pct();
        let verdict = if used >= rules::SPACE_CRITICAL_PCT {
            Verdict::Critical
        } else if used >= rules::SPACE_WARN_PCT
            // 이미 상당히 차 있으면서 남은 절대량이 적은 경우도 경고한다. 큰 디스크에서
            // 사용률만 보면 놓치는 상황이다.
            || (used >= rules::SPACE_TIGHT_PCT
                && usage.available_bytes < rules::SPACE_FREE_WARN_BYTES)
        {
            Verdict::Warn
        } else {
            Verdict::Ok
        };
        let headline = match verdict {
            Verdict::Critical => format!("{} is almost out of space", m.mount.target),
            Verdict::Warn => format!("{} is getting full", m.mount.target),
            _ => format!("{} has room", m.mount.target),
        };
        let mut finding = Finding::new("space", verdict, headline)
            .evidence(format!(
                "{} of {} used ({}), {} available",
                bytes(usage.used_bytes()),
                bytes(usage.total_bytes),
                pct(used),
                bytes(usage.available_bytes)
            ))
            .evidence(format!(
                "{} on {} ({})",
                m.mount.source, m.mount.target, m.mount.fstype
            ));
        if usage.reserved_bytes() > 0 {
            finding = finding.evidence(format!(
                "{} of the free space is reserved for root",
                bytes(usage.reserved_bytes())
            ));
        }
        out.push(finding.learn(
            "'Used' counts the space reserved for root as used, the same way df does. What a normal \
             user can still write is the 'available' figure.",
        ));
    }
    out
}

/// inode 판정. 용량과 별개로 파일 생성이 막히는 상황을 잡는다.
pub fn inode_findings(mounts: &[MountUsage]) -> Vec<Finding> {
    let mut out = Vec::new();
    for m in mounts {
        let Some(usage) = m.usage else { continue };
        let Some(used) = usage.inodes_used_pct() else {
            // btrfs 등은 inode 를 미리 정해 두지 않는다. 없는 것도 정보다.
            out.push(
                Finding::new(
                    "inodes",
                    Verdict::Ok,
                    format!("{} does not have a fixed inode count", m.mount.target),
                )
                .evidence(format!(
                    "{} allocates inodes as needed, so they cannot run out separately",
                    m.mount.fstype
                ))
                .learn(
                    "ext4 fixes the number of inodes when the filesystem is created; btrfs and XFS \
                     allocate them on demand.",
                ),
            );
            continue;
        };
        let verdict = if used >= rules::INODE_CRITICAL_PCT {
            Verdict::Critical
        } else if used >= rules::INODE_WARN_PCT {
            Verdict::Warn
        } else {
            Verdict::Ok
        };
        let headline = match verdict {
            Verdict::Critical => format!(
                "{} is out of inodes - new files will fail even with space left",
                m.mount.target
            ),
            Verdict::Warn => format!("{} is running low on inodes", m.mount.target),
            _ => format!("{} has inodes to spare", m.mount.target),
        };
        out.push(
            Finding::new("inodes", verdict, headline)
                .evidence(format!(
                    "{} of {} inodes used ({})",
                    usage.inodes_used(),
                    usage.inodes_total,
                    pct(used)
                ))
                .evidence(format!(
                    "{} of space is still available on this filesystem",
                    bytes(usage.available_bytes)
                ))
                .learn(
                    "An inode is the bookkeeping entry for one file. Millions of tiny files can use \
                     them all up while the disk still looks empty - the error is 'No space left on \
                     device' even though df shows free space.",
                ),
        );
    }
    out
}

/// 지운 파일이 붙잡고 있는 공간.
pub fn deleted_finding(deleted: &DeletedFiles) -> Finding {
    let held = deleted.held_bytes();
    let verdict = if held >= rules::DELETED_HELD_WARN_BYTES {
        Verdict::Warn
    } else {
        Verdict::Ok
    };
    let headline = if held >= rules::DELETED_HELD_WARN_BYTES {
        format!("{} is held by files that were already deleted", bytes(held))
    } else if deleted.files.is_empty() {
        "No deleted files are being held open".to_string()
    } else {
        format!(
            "{} held by deleted files - not enough to matter",
            bytes(held)
        )
    };

    let mut finding = Finding::new("deleted", verdict, headline).evidence(format!(
        "{} deleted file(s) still open across {} process(es)",
        deleted.files.len(),
        deleted.by_process().len()
    ));
    for (pid, process, size, count) in deleted.by_process().into_iter().take(3) {
        finding = finding.evidence(format!(
            "{process} (pid {pid}) holds {} across {count} file(s)",
            bytes(size)
        ));
    }
    if deleted.partial() {
        finding = finding.evidence(format!(
            "{} process(es) could not be inspected without privileges - the real total may be higher",
            deleted.processes_denied
        ));
    }
    finding.learn(
        "Deleting a file only removes its name. The space comes back when the last program that \
         had it open closes it - which is why df can stay full right after rm.",
    )
}

/// 로그가 차지하는 비중.
pub fn log_finding(logs: &LogFootprint, var_mount: Option<&MountUsage>) -> Finding {
    let total = logs.total_bytes();
    let share = var_mount
        .and_then(|m| m.usage)
        .map(|u| {
            if u.used_bytes() == 0 {
                0.0
            } else {
                total as f32 / u.used_bytes() as f32 * 100.0
            }
        })
        .unwrap_or(0.0);

    let verdict = if share >= rules::LOG_SHARE_WARN_PCT || total >= rules::LOG_SIZE_WARN_BYTES {
        Verdict::Warn
    } else {
        Verdict::Ok
    };
    let headline = if verdict == Verdict::Warn {
        format!("Logs are using {}", bytes(total))
    } else {
        format!("Logs are using {} - modest", bytes(total))
    };

    let mut finding = Finding::new("logs", verdict, headline);
    if let Some(report) = &logs.journal_report {
        finding = finding.evidence(report.clone());
    }
    if let Some(scan) = &logs.var_log {
        finding = finding.evidence(format!("/var/log holds {}", bytes(scan.total_bytes)));
        for entry in scan.entries.iter().take(3) {
            finding = finding.evidence(format!("  {} {}", bytes(entry.bytes), entry.name));
        }
        if scan.truncated {
            finding =
                finding.evidence("measurement stopped early - the total above is a lower bound");
        }
    }
    if share > 0.0 {
        finding = finding.evidence(format!(
            "that is {} of what is used on that filesystem",
            pct(share)
        ));
    }
    finding.learn(
        "The journal keeps growing until it hits its configured cap. If no cap is set it takes up to \
         10% of the filesystem, which on a large disk can be a lot.",
    )
}

/// 어떤 원인이 가장 그럴듯한지 좁힌다.
///
/// 단정하지 않는다. 각 후보의 근거를 함께 돌려주고, 확실한 것부터 순서대로 본다.
pub fn diagnose_full(
    mount: &MountUsage,
    deleted: Option<&DeletedFiles>,
    logs: Option<&LogFootprint>,
    dirs: Option<&DirScan>,
) -> SpaceDiagnosis {
    let target = mount.mount.target.clone();
    let mut findings = Vec::new();
    let Some(usage) = mount.usage else {
        return SpaceDiagnosis {
            target,
            cause: FullCause::Undetermined,
            findings,
        };
    };

    let tight = usage.used_pct() >= rules::SPACE_TIGHT_PCT
        && usage.available_bytes < rules::SPACE_FREE_WARN_BYTES;
    if usage.used_pct() < rules::SPACE_WARN_PCT && !tight {
        return SpaceDiagnosis {
            target,
            cause: FullCause::NotFull,
            findings,
        };
    }

    let mut cause = FullCause::Undetermined;

    // 1. inode 고갈은 용량과 무관하게 파일 생성을 막으므로 가장 먼저 확인한다.
    if let Some(inode_pct) = usage.inodes_used_pct()
        && inode_pct >= rules::INODE_WARN_PCT
    {
        cause = FullCause::InodesExhausted;
        findings.push(
            Finding::new(
                "cause",
                Verdict::Critical,
                "Inodes are the limit here, not bytes",
            )
            .evidence(format!(
                "{} of inodes used, while {} of space is still free",
                pct(inode_pct),
                bytes(usage.available_bytes)
            ))
            .learn("Writes fail with 'No space left on device' even though df shows free space."),
        );
    }

    // 2. 지운 파일이 붙잡고 있는 공간.
    if let Some(deleted) = deleted {
        let held = deleted.held_bytes();
        let share = if usage.used_bytes() == 0 {
            0.0
        } else {
            held as f32 / usage.used_bytes() as f32 * 100.0
        };
        if held >= rules::DELETED_HELD_WARN_BYTES || share >= 5.0 {
            if cause == FullCause::Undetermined {
                cause = FullCause::DeletedButHeld;
            }
            findings.push(
                Finding::new(
                    "cause",
                    Verdict::Warn,
                    "Deleted files are still holding space",
                )
                .evidence(format!(
                    "{} held open, about {} of what is in use",
                    bytes(held),
                    pct(share)
                ))
                .learn("This space returns by itself once those processes close or restart."),
            );
        }
    }

    // 3. 로그.
    if let Some(logs) = logs {
        let total = logs.total_bytes();
        let share = if usage.used_bytes() == 0 {
            0.0
        } else {
            total as f32 / usage.used_bytes() as f32 * 100.0
        };
        if share >= rules::LOG_SHARE_WARN_PCT {
            if cause == FullCause::Undetermined {
                cause = FullCause::Logs;
            }
            findings.push(
                Finding::new(
                    "cause",
                    Verdict::Warn,
                    "Logs are a large share of this filesystem",
                )
                .evidence(format!(
                    "{} in logs, {} of what is used",
                    bytes(total),
                    pct(share)
                ))
                .learn("Journal size is capped by SystemMaxUse in journald.conf."),
            );
        }
    }

    // 4. 큰 디렉터리.
    if let Some(scan) = dirs
        && !scan.entries.is_empty()
    {
        if cause == FullCause::Undetermined {
            cause = FullCause::LargeDirectories;
        }
        let mut finding =
            Finding::new("cause", Verdict::Warn, "Ordinary files are using the space");
        for entry in scan.entries.iter().take(3) {
            finding = finding.evidence(format!("{} {}", bytes(entry.bytes), entry.path.display()));
        }
        if scan.truncated {
            finding = finding.evidence("measurement stopped early - these are lower bounds");
        }
        findings.push(finding.learn(
            "Sizes here count blocks actually occupied and follow no mount points, the same way du -x does.",
        ));
    }

    // 5. 예약 블록. 일반 사용자에게만 공간이 없어 보이는 경우다.
    if usage.available_bytes == 0 && usage.reserved_bytes() > 0 {
        if cause == FullCause::Undetermined {
            cause = FullCause::ReservedForRoot;
        }
        findings.push(
            Finding::new(
                "cause",
                Verdict::Warn,
                "Only root has space left on this filesystem",
            )
            .evidence(format!(
                "{} free in total, but 0 available to normal users",
                bytes(usage.free_bytes)
            ))
            .learn(
                "ext filesystems keep 5% back by default so root can still log in and fix things.",
            ),
        );
    }

    SpaceDiagnosis {
        target,
        cause,
        findings,
    }
}

/// 드라이브 건전성 판정.
pub fn drive_findings(drives: &[DriveHealth]) -> Vec<Finding> {
    drives
        .iter()
        .map(|drive| {
            if !drive.availability.is_ok() {
                return Finding::new(
                    "drive",
                    Verdict::Unknown,
                    format!("{} could not be checked", drive.block),
                )
                .evidence(drive.availability.message())
                .learn(
                    "A drive's own diagnosis (SMART) is the only way to see wear and bad sectors. \
                     Kernel errors tell you when something already failed; SMART warns beforehand.",
                );
            }

            let concerning: Vec<&crate::collect::smart::HealthAttr> =
                drive.attributes.iter().filter(|a| a.concern).collect();
            let verdict = if drive.healthy == Some(false) {
                Verdict::Critical
            } else if !concerning.is_empty() {
                Verdict::Warn
            } else {
                Verdict::Ok
            };
            let headline = match verdict {
                Verdict::Critical => format!("{} reports itself as failing", drive.block),
                Verdict::Warn => format!("{} shows early warning signs", drive.block),
                _ => format!("{} looks healthy", drive.block),
            };

            let mut finding = Finding::new("drive", verdict, headline).evidence(format!(
                "{} ({})",
                drive.model.trim(),
                drive.device
            ));
            if let Some(healthy) = drive.healthy {
                finding = finding.evidence(format!(
                    "the drive's own overall assessment: {}",
                    if healthy { "passed" } else { "FAILED" }
                ));
            }
            for attr in &drive.attributes {
                finding = finding.evidence(format!(
                    "{}{}: {}",
                    if attr.concern { "! " } else { "" },
                    attr.label,
                    attr.value
                ));
            }
            finding.learn(
                "Reallocated and pending sectors are the values that predict failure. Cable (CRC) \
                 errors and command timeouts usually mean the connection, not the disk.",
            )
        })
        .collect()
}

/// 파일시스템 상태와 커널 오류 판정.
pub fn filesystem_findings(
    fs: &[FsHealth],
    errors: Option<&StorageErrors>,
    raid: &[RaidArray],
) -> Vec<Finding> {
    let mut out = Vec::new();

    for f in fs {
        if f.read_only {
            out.push(
                Finding::new(
                    "filesystem",
                    Verdict::Critical,
                    format!("{} is mounted read-only", f.target),
                )
                .evidence(format!("{} on {} ({})", f.source, f.target, f.fstype))
                .evidence(if f.remounts_read_only_on_error() {
                    "this filesystem is set to go read-only when the kernel hits an error"
                        .to_string()
                } else {
                    "mounted with the ro option".to_string()
                })
                .learn(
                    "A filesystem that turned read-only by itself has hit an error. Nothing can be \
                     written until it is checked - which is a repair step, not a diagnosis step.",
                ),
            );
        }
        if let Some(count) = f.ext4_errors
            && count > 0
        {
            out.push(
                Finding::new(
                    "filesystem",
                    Verdict::Warn,
                    format!("{} has recorded filesystem errors", f.target),
                )
                .evidence(format!("{count} error(s) counted by ext4 itself"))
                .evidence(
                    f.ext4_last_error
                        .clone()
                        .map(|t| format!("last error at {t}"))
                        .unwrap_or_else(|| "no timestamp recorded".into()),
                )
                .learn(
                    "ext4 keeps its own error counter, so this survives even after the kernel log \
                     has rotated away.",
                ),
            );
        }
    }

    if let Some(errors) = errors {
        if !errors.availability.is_ok() {
            out.push(
                Finding::new(
                    "kernel",
                    Verdict::Unknown,
                    "Kernel storage errors could not be checked",
                )
                .evidence(errors.availability.message())
                .learn("The kernel log is where a failing drive leaves its first trace."),
            );
        } else if errors.clean() {
            out.push(
                Finding::new(
                    "kernel",
                    Verdict::Ok,
                    "No storage errors in the kernel log",
                )
                .evidence(format!("looked at {} for I/O errors, link resets and filesystem errors", errors.scope))
                .learn(
                    "An empty result is a real finding: it rules out the drive as the cause of what \
                     you are chasing.",
                ),
            );
        } else {
            let mut finding = Finding::new(
                "kernel",
                Verdict::Warn,
                format!("{} storage error line(s) in the kernel log", errors.total()),
            );
            for hit in &errors.hits {
                finding = finding.evidence(format!("{} x  {}", hit.count, hit.pattern));
                if let Some(sample) = hit.samples.first() {
                    finding = finding.evidence(format!("    {sample}"));
                }
            }
            out.push(finding.learn(
                "Link resets and command timeouts point at cables or power more often than at the \
                 drive itself.",
            ));
        }
    }

    for array in raid {
        let verdict = if array.degraded() {
            Verdict::Critical
        } else {
            Verdict::Ok
        };
        let headline = if array.degraded() {
            format!("{} is degraded - a member is missing", array.name)
        } else {
            format!("{} is complete", array.name)
        };
        let mut finding = Finding::new("raid", verdict, headline)
            .evidence(format!("{} with {}", array.level, array.members.join(", ")))
            .evidence(format!("member state {}", array.state));
        if let Some(progress) = &array.progress {
            finding = finding.evidence(progress.clone());
        }
        out.push(finding.learn(
            "In the member state, U means up and _ means missing. A degraded array still works but \
             has no redundancy left.",
        ));
    }

    out
}
