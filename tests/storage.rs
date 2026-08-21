//! 저장 공간과 저장장치 수집·판정.
//!
//! 마운트 표와 장치 구성은 픽스처로, 디렉터리 측정은 임시 디렉터리로, 드라이브 자기 진단은
//! 실제 도구 출력을 저장한 텍스트로 검증한다. 실제 시스템 상태에 의존하지 않는다.

mod common;

use syschk::analyze::Verdict;
use syschk::analyze::storage::{
    self, FullCause, deleted_finding, diagnose_full, drive_findings, filesystem_findings,
    inode_findings, log_finding, space_findings,
};
use syschk::collect::ProbeCtx;
use syschk::collect::blockdev;
use syschk::collect::deleted;
use syschk::collect::dirsize;
use syschk::collect::fsinfo;
use syschk::collect::logspace;
use syschk::collect::mounts::{self, Mount, MountUsage, Usage};
use syschk::collect::smart::{self, DriveHealth, DriveKind};
use syschk::collect::storage_errors::{ErrorHits, StorageErrors};

fn ctx() -> ProbeCtx {
    ProbeCtx::with_root(common::fixture_root())
}

fn read_fixture(name: &str) -> String {
    std::fs::read_to_string(common::fixture_root().join("output").join(name))
        .unwrap_or_else(|e| panic!("fixture {name}: {e}"))
}

// ── 마운트 표 ──────────────────────────────────────────────────────

#[test]
fn mountinfo_is_parsed_including_escaped_paths() {
    let all = mounts::mounts(&ctx());
    let root = all.iter().find(|m| m.target == "/").expect("root mount");
    assert_eq!(root.source, "/dev/sda1");
    assert_eq!(root.fstype, "ext4");
    assert_eq!((root.major, root.minor), (8, 1));
    assert!(root.options.contains(&"rw".to_string()));
    assert!(!root.is_read_only());

    let data = all
        .iter()
        .find(|m| m.target == "/mnt/data")
        .expect("data mount");
    assert!(data.is_read_only(), "ro in the options means read-only");

    // mountinfo 는 공백을 8진 이스케이프로 적는다.
    assert!(
        all.iter().any(|m| m.target == "/export/home dir"),
        "escaped spaces in mount points must be decoded"
    );
}

#[test]
fn pseudo_filesystems_and_snaps_are_left_out() {
    let all = mounts::mounts(&ctx());
    let user: Vec<&str> = all
        .iter()
        .filter(|m| m.is_user_filesystem())
        .map(|m| m.target.as_str())
        .collect();

    assert!(user.contains(&"/"));
    assert!(user.contains(&"/mnt/data"));
    // 커널 가상 파일시스템은 사용자가 신경 쓸 대상이 아니다.
    assert!(!user.contains(&"/proc"));
    assert!(!user.contains(&"/sys"));
    // snap 이미지는 항상 100% 라서 목록을 오염시킨다.
    assert!(!user.iter().any(|t| t.starts_with("/snap/")));
}

#[test]
fn usage_separates_available_from_free() {
    // 예약 블록은 free 에는 들어가고 available 에는 들어가지 않는다.
    let usage = Usage {
        total_bytes: 100_000,
        free_bytes: 10_000,
        available_bytes: 5_000,
        inodes_total: 1_000,
        inodes_free: 100,
    };
    assert_eq!(usage.used_bytes(), 90_000);
    assert_eq!(usage.reserved_bytes(), 5_000);
    // df 와 같은 정의: 예약분은 사용 중으로 센다.
    assert!((usage.used_pct() - 94.7).abs() < 0.2);
    assert_eq!(usage.inodes_used(), 900);
    assert_eq!(usage.inodes_used_pct().map(|p| p as u32), Some(90));

    // inode 개념이 없는 파일시스템은 비율이 없다.
    let no_inodes = Usage {
        inodes_total: 0,
        ..usage
    };
    assert_eq!(no_inodes.inodes_used_pct(), None);
}

#[test]
fn real_filesystem_usage_can_be_measured() {
    // 실제 루트는 항상 측정 가능해야 한다.
    let usage = mounts::usage_of(std::path::Path::new("/")).expect("statvfs on /");
    assert!(usage.total_bytes > 0);
    assert!(usage.used_pct() >= 0.0 && usage.used_pct() <= 100.0);
}

// ── 장치 구성 ──────────────────────────────────────────────────────

#[test]
fn block_devices_come_with_partitions_and_mounts() {
    let ctx = ctx();
    let mount_list = mounts::mounts(&ctx);
    let devices = blockdev::devices(&ctx, &mount_list);

    let names: Vec<&str> = devices.iter().map(|d| d.name.as_str()).collect();
    assert!(names.contains(&"sda"));
    assert!(names.contains(&"nvme0n1"));
    assert!(!names.contains(&"loop0"), "loop devices are noise");

    let sda = devices.iter().find(|d| d.name == "sda").unwrap();
    assert!(sda.rotational, "sda is a spinning disk in the fixture");
    assert_eq!(sda.kind(), "HDD");
    assert_eq!(sda.size_bytes, 3_907_029_168 * 512);
    assert!(sda.model.contains("ST22000NT001"));
    assert_eq!(sda.partitions.len(), 1);
    assert_eq!(sda.partitions[0].mount_point.as_deref(), Some("/"));
    assert_eq!(sda.partitions[0].fstype.as_deref(), Some("ext4"));

    let nvme = devices.iter().find(|d| d.name == "nvme0n1").unwrap();
    assert!(!nvme.rotational);
    assert_eq!(nvme.kind(), "SSD");
    let part = &nvme.partitions[0];
    assert_eq!(part.mount_point.as_deref(), Some("/mnt/data"));
    assert!(part.read_only, "the fixture mounts it ro");
}

#[test]
fn raid_state_is_read_and_degradation_detected() {
    let arrays = blockdev::raid_arrays(&ctx());
    assert_eq!(arrays.len(), 2);

    let md0 = arrays.iter().find(|a| a.name == "md0").unwrap();
    assert_eq!(md0.level, "raid1");
    assert_eq!(md0.state, "[U_]");
    assert!(md0.degraded(), "a missing member shows as _");
    assert!(md0.progress.is_some(), "recovery progress should be kept");
    assert!(md0.members.iter().any(|m| m.starts_with("sdb1")));

    let md1 = arrays.iter().find(|a| a.name == "md1").unwrap();
    assert!(!md1.degraded());
}

// ── 파일시스템 상태 ────────────────────────────────────────────────

#[test]
fn filesystem_health_notices_read_only_and_recorded_errors() {
    let ctx = ctx();
    let usage = mounts::usage(&ctx);
    let health = fsinfo::health(&ctx, &usage);

    let root = health.iter().find(|f| f.target == "/").expect("root");
    assert!(!root.read_only);
    assert!(root.remounts_read_only_on_error());
    // ext4 가 스스로 기록한 오류 횟수를 읽는다.
    assert_eq!(root.ext4_errors, Some(3));
    assert!(root.ext4_last_error.is_some());

    let data = health
        .iter()
        .find(|f| f.target == "/mnt/data")
        .expect("data");
    assert!(data.read_only);
}

// ── 디렉터리 용량 ──────────────────────────────────────────────────

#[test]
fn directory_scan_measures_like_du() {
    let dir = tempdir("dirscan");
    std::fs::create_dir_all(dir.join("big")).unwrap();
    std::fs::create_dir_all(dir.join("small/nested")).unwrap();
    write_bytes(&dir.join("big/a.bin"), 200 * 1024);
    write_bytes(&dir.join("big/b.bin"), 100 * 1024);
    write_bytes(&dir.join("small/nested/c.bin"), 8 * 1024);
    write_bytes(&dir.join("loose.bin"), 4 * 1024);

    let scan = dirsize::scan(&dir, std::time::Duration::from_secs(5));
    assert!(!scan.truncated);
    assert_eq!(scan.files_counted, 4);

    // 큰 것부터 나온다.
    assert_eq!(scan.entries[0].name, "big");
    assert!(scan.entries[0].is_dir);
    assert!(scan.entries[0].bytes >= 300 * 1024);
    assert!(scan.total_bytes >= 312 * 1024);

    // 하위 디렉터리 항목 수도 센다.
    let small = scan.entries.iter().find(|e| e.name == "small").unwrap();
    assert!(small.items >= 2, "nested contents are counted");

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn hard_links_are_counted_once() {
    let dir = tempdir("hardlink");
    write_bytes(&dir.join("original.bin"), 64 * 1024);
    std::fs::hard_link(dir.join("original.bin"), dir.join("same.bin")).unwrap();

    let scan = dirsize::scan(&dir, std::time::Duration::from_secs(5));
    // du 와 같이 같은 inode 는 한 번만 센다.
    assert!(
        scan.total_bytes < 100 * 1024,
        "a hard link must not double the total, got {}",
        scan.total_bytes
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_zero_budget_reports_truncation_instead_of_lying() {
    let dir = tempdir("budget");
    for i in 0..40 {
        std::fs::create_dir_all(dir.join(format!("d{i}"))).unwrap();
        write_bytes(&dir.join(format!("d{i}/f.bin")), 4 * 1024);
    }
    let scan = dirsize::scan(&dir, std::time::Duration::from_nanos(1));
    assert!(
        scan.truncated,
        "a scan that ran out of time must say so rather than report a wrong total"
    );
    std::fs::remove_dir_all(&dir).ok();
}

// ── 삭제 후 점유 ───────────────────────────────────────────────────

#[test]
fn deleted_but_open_files_are_found() {
    let found = deleted::scan(&ctx());
    assert_eq!(
        found.files.len(),
        2,
        "two fixture fds point at deleted files"
    );
    assert!(
        found.files.iter().any(|f| f.path == "/var/log/huge.log"),
        "the ' (deleted)' suffix must be stripped from the path"
    );
    // /dev/null 처럼 공간과 무관한 항목은 세지 않는다.
    assert!(found.files.iter().all(|f| !f.path.starts_with("/dev/")));

    let by_process = found.by_process();
    assert_eq!(by_process.len(), 2);
    assert!(
        by_process
            .iter()
            .any(|(pid, name, _, _)| *pid == 100 && name == "worker")
    );
}

// ── 드라이브 자기 진단 ─────────────────────────────────────────────

#[test]
fn healthy_sata_drive_is_reported_as_healthy() {
    let text = read_fixture("smartctl-healthy.txt");
    assert_eq!(smart::parse_sata_health(&text), Some(true));

    let attrs = smart::parse_sata_attributes(&text);
    let labels: Vec<&str> = attrs.iter().map(|a| a.label).collect();
    assert!(labels.contains(&"reallocated sectors"));
    assert!(labels.contains(&"pending sectors"));
    assert!(labels.contains(&"power-on hours"));
    // 관심 없는 속성은 버린다.
    assert!(!labels.contains(&"Raw_Read_Error_Rate"));

    assert!(
        attrs.iter().all(|a| !a.concern),
        "a clean drive should raise nothing"
    );
    // 시간과 온도는 값이 커도 고장 신호가 아니다.
    let hours = attrs.iter().find(|a| a.label == "power-on hours").unwrap();
    assert_eq!(hours.value, "10036");
    assert!(!hours.concern);
}

#[test]
fn failing_sata_drive_raises_the_predictive_values() {
    let text = read_fixture("smartctl-failing.txt");
    assert_eq!(smart::parse_sata_health(&text), Some(false));

    let attrs = smart::parse_sata_attributes(&text);
    let realloc = attrs
        .iter()
        .find(|a| a.label == "reallocated sectors")
        .unwrap();
    assert_eq!(realloc.value, "1832");
    assert!(realloc.concern);
    assert!(
        !realloc.explain.is_empty(),
        "the user needs to know what it means"
    );

    let cable = attrs
        .iter()
        .find(|a| a.label == "cable errors (CRC)")
        .unwrap();
    assert!(cable.concern);
}

#[test]
fn nvme_log_is_parsed_with_wear_and_spares() {
    let good = smart::parse_nvme_log(&read_fixture("nvme-smart-log.txt"));
    assert_eq!(good.healthy, Some(true));
    let labels: Vec<&str> = good.attributes.iter().map(|a| a.label).collect();
    assert!(labels.contains(&"life used"));
    assert!(labels.contains(&"spare blocks"));
    assert!(labels.contains(&"media errors"));
    assert!(good.attributes.iter().all(|a| !a.concern));

    let worn = smart::parse_nvme_log(&read_fixture("nvme-worn.txt"));
    assert_eq!(worn.healthy, Some(false), "critical_warning is non-zero");
    let concerns: Vec<&str> = worn
        .attributes
        .iter()
        .filter(|a| a.concern)
        .map(|a| a.label)
        .collect();
    assert!(concerns.contains(&"critical warning"));
    assert!(concerns.contains(&"life used"));
    assert!(concerns.contains(&"spare blocks"));
    assert!(concerns.contains(&"media errors"));
    // 온도는 값만 보여주고 판정하지 않는다(임계값은 모델마다 다르다).
    let temp = worn
        .attributes
        .iter()
        .find(|a| a.label == "temperature")
        .unwrap();
    assert!(!temp.concern);
}

#[test]
fn a_drive_we_cannot_read_is_not_called_healthy() {
    let health = DriveHealth {
        device: "/dev/sda".into(),
        block: "sda".into(),
        kind: DriveKind::Sata,
        model: "test".into(),
        healthy: None,
        attributes: Vec::new(),
        availability: syschk::collect::Availability::NeedsPrivilege {
            hint: "needs root".into(),
        },
        raw: Vec::new(),
    };
    let findings = drive_findings(&[health]);
    assert_eq!(findings[0].verdict, Verdict::Unknown);
    assert!(findings[0].evidence[0].contains("needs privileges"));
}

// ── 로그 점유 ──────────────────────────────────────────────────────

#[test]
fn journal_usage_line_is_parsed() {
    assert_eq!(
        logspace::parse_journal_usage(
            "Archived and active journals take up 3.9G in the file system."
        ),
        Some((3.9 * 1024.0 * 1024.0 * 1024.0) as u64)
    );
    assert_eq!(
        logspace::parse_journal_usage("Journals take up 512.0M in the file system."),
        Some(512 * 1024 * 1024)
    );
    assert_eq!(logspace::parse_journal_usage("nothing here"), None);
}

// ── 판정 규칙 ──────────────────────────────────────────────────────

fn mount_with(target: &str, total: u64, available: u64, inodes: (u64, u64)) -> MountUsage {
    MountUsage {
        mount: Mount {
            source: "/dev/sda1".into(),
            target: target.into(),
            fstype: "ext4".into(),
            options: vec!["rw".into()],
            major: 8,
            minor: 1,
        },
        usage: Some(Usage {
            total_bytes: total,
            free_bytes: available,
            available_bytes: available,
            inodes_total: inodes.0,
            inodes_free: inodes.1,
        }),
    }
}

const GIB: u64 = 1024 * 1024 * 1024;
const MIB: u64 = 1024 * 1024;

#[test]
fn space_verdicts_follow_the_thresholds() {
    let roomy = mount_with("/", 100 * GIB, 60 * GIB, (1000, 900));
    assert_eq!(space_findings(&[roomy])[0].verdict, Verdict::Ok);

    let filling = mount_with("/", 100 * GIB, 8 * GIB, (1000, 900));
    assert_eq!(space_findings(&[filling])[0].verdict, Verdict::Warn);

    let nearly_gone = mount_with("/", 100 * GIB, 2 * GIB, (1000, 900));
    assert_eq!(space_findings(&[nearly_gone])[0].verdict, Verdict::Critical);

    // 사용률이 임계 아래라도 남은 절대량이 1GiB 미만이면 경고한다.
    let almost_no_room = mount_with("/boot", 8_900 * MIB, 900 * MIB, (1000, 900));
    let finding = &space_findings(&[almost_no_room])[0];
    assert_eq!(finding.verdict, Verdict::Warn);
    assert!(finding.evidence[0].contains("available"));
}

#[test]
fn inode_exhaustion_is_critical_even_with_space_left() {
    let mount = mount_with("/", 100 * GIB, 80 * GIB, (1_000_000, 20_000));
    let findings = inode_findings(&[mount]);
    assert_eq!(findings[0].verdict, Verdict::Critical);
    assert!(
        findings[0]
            .evidence
            .iter()
            .any(|e| e.contains("of space is still available")),
        "the verdict must show that space is not the problem"
    );
}

#[test]
fn filesystems_without_fixed_inodes_are_not_flagged() {
    let mut mount = mount_with("/data", 100 * GIB, 50 * GIB, (0, 0));
    mount.mount.fstype = "btrfs".into();
    let findings = inode_findings(&[mount]);
    assert_eq!(findings[0].verdict, Verdict::Ok);
    assert!(
        findings[0]
            .headline
            .contains("does not have a fixed inode count")
    );
}

#[test]
fn deleted_files_holding_space_are_a_warning() {
    let files = syschk::collect::deleted::DeletedFiles {
        processes_scanned: 10,
        files: vec![syschk::collect::deleted::DeletedFile {
            pid: 42,
            process: "journald".into(),
            path: "/var/log/big.log".into(),
            bytes: Some(4 * GIB),
        }],
        ..Default::default()
    };
    let finding = deleted_finding(&files);
    assert_eq!(finding.verdict, Verdict::Warn);
    assert!(finding.headline.contains("4.0G"));
    assert!(!finding.learn.is_empty());

    // 몇 KB 짜리는 문제가 아니다.
    let small = syschk::collect::deleted::DeletedFiles {
        processes_scanned: 10,
        files: vec![syschk::collect::deleted::DeletedFile {
            pid: 43,
            process: "app".into(),
            path: "/tmp/x".into(),
            bytes: Some(4096),
        }],
        ..Default::default()
    };
    assert_eq!(deleted_finding(&small).verdict, Verdict::Ok);
}

#[test]
fn diagnose_full_picks_inodes_over_everything_else() {
    // 용량도 빠듯하지만 inode 가 먼저 바닥났다면 그것이 원인이다.
    let mount = mount_with("/", 100 * GIB, 5 * GIB, (1_000_000, 30_000));
    let diagnosis = diagnose_full(&mount, None, None, None);
    assert_eq!(diagnosis.cause, FullCause::InodesExhausted);
    assert!(!diagnosis.findings.is_empty());
    assert!(!diagnosis.cause.what_to_do().is_empty());
}

#[test]
fn diagnose_full_blames_held_files_when_they_explain_the_usage() {
    let mount = mount_with("/", 100 * GIB, 2 * GIB, (1_000_000, 900_000));
    let held = syschk::collect::deleted::DeletedFiles {
        processes_scanned: 5,
        files: vec![syschk::collect::deleted::DeletedFile {
            pid: 7,
            process: "writer".into(),
            path: "/var/tmp/old".into(),
            bytes: Some(40 * GIB),
        }],
        ..Default::default()
    };
    let diagnosis = diagnose_full(&mount, Some(&held), None, None);
    assert_eq!(diagnosis.cause, FullCause::DeletedButHeld);
    assert!(
        diagnosis.cause.what_to_do().contains("Restarting"),
        "the user should be told the space comes back on its own"
    );
}

#[test]
fn diagnose_full_blames_logs_when_they_dominate() {
    let mount = mount_with("/var", 100 * GIB, 3 * GIB, (1_000_000, 900_000));
    let logs = logspace::LogFootprint {
        journal_bytes: Some(60 * GIB),
        journal_report: Some("journals take up 60.0G".into()),
        var_log: None,
        availability: syschk::collect::Availability::Ok,
    };
    let diagnosis = diagnose_full(&mount, None, Some(&logs), None);
    assert_eq!(diagnosis.cause, FullCause::Logs);
}

#[test]
fn diagnose_full_falls_back_to_large_directories() {
    let mount = mount_with("/", 100 * GIB, 4 * GIB, (1_000_000, 900_000));
    let mut scan = dirsize::DirScan {
        path: "/".into(),
        total_bytes: 90 * GIB,
        ..Default::default()
    };
    scan.entries.push(dirsize::Entry {
        name: "var".into(),
        path: "/var".into(),
        bytes: 80 * GIB,
        is_dir: true,
        items: 1000,
    });
    let diagnosis = diagnose_full(&mount, None, None, Some(&scan));
    assert_eq!(diagnosis.cause, FullCause::LargeDirectories);
    assert!(
        diagnosis.findings[0]
            .evidence
            .iter()
            .any(|e| e.contains("/var")),
        "the biggest directory should be named"
    );
}

#[test]
fn a_filesystem_with_room_needs_no_explanation() {
    let mount = mount_with("/", 100 * GIB, 70 * GIB, (1_000_000, 900_000));
    let diagnosis = diagnose_full(&mount, None, None, None);
    assert_eq!(diagnosis.cause, FullCause::NotFull);
    assert!(diagnosis.findings.is_empty());
}

#[test]
fn only_root_having_space_is_recognised() {
    // free 는 남아 있지만 available 이 0 이면 일반 사용자는 쓸 수 없다.
    let mount = MountUsage {
        mount: Mount {
            source: "/dev/sda1".into(),
            target: "/".into(),
            fstype: "ext4".into(),
            options: vec!["rw".into()],
            major: 8,
            minor: 1,
        },
        usage: Some(Usage {
            total_bytes: 100 * GIB,
            free_bytes: 5 * GIB,
            available_bytes: 0,
            inodes_total: 1_000_000,
            inodes_free: 900_000,
        }),
    };
    let diagnosis = diagnose_full(&mount, None, None, None);
    assert_eq!(diagnosis.cause, FullCause::ReservedForRoot);
}

#[test]
fn an_empty_kernel_log_is_a_useful_finding() {
    let clean = StorageErrors {
        hits: Vec::new(),
        availability: syschk::collect::Availability::Ok,
        scope: "this boot".into(),
    };
    let findings = filesystem_findings(&[], Some(&clean), &[]);
    let kernel = findings.iter().find(|f| f.axis == "kernel").unwrap();
    assert_eq!(kernel.verdict, Verdict::Ok);
    assert!(
        kernel.learn.contains("rules out"),
        "absence of errors should be presented as evidence"
    );

    let with_errors = StorageErrors {
        hits: vec![ErrorHits {
            pattern: "I/O error",
            meaning: "test",
            count: 4,
            samples: vec!["kernel: blk_update_request: I/O error".into()],
        }],
        availability: syschk::collect::Availability::Ok,
        scope: "this boot".into(),
    };
    let findings = filesystem_findings(&[], Some(&with_errors), &[]);
    let kernel = findings.iter().find(|f| f.axis == "kernel").unwrap();
    assert_eq!(kernel.verdict, Verdict::Warn);
    assert!(kernel.headline.contains('4'));
}

#[test]
fn a_read_only_filesystem_is_critical() {
    let ctx = ctx();
    let usage = mounts::usage(&ctx);
    let fs = fsinfo::health(&ctx, &usage);
    let findings = filesystem_findings(&fs, None, &[]);
    assert!(
        findings
            .iter()
            .any(|f| f.verdict == Verdict::Critical && f.headline.contains("read-only")),
        "a read-only mount must be flagged"
    );
    // ext4 가 기록한 오류도 별도 판정으로 나온다.
    assert!(
        findings
            .iter()
            .any(|f| f.headline.contains("recorded filesystem errors")),
        "recorded ext4 errors must be reported"
    );
}

#[test]
fn a_degraded_array_is_critical() {
    let arrays = blockdev::raid_arrays(&ctx());
    let findings = filesystem_findings(&[], None, &arrays);
    let md0 = findings
        .iter()
        .find(|f| f.headline.starts_with("md0"))
        .unwrap();
    assert_eq!(md0.verdict, Verdict::Critical);
    assert!(md0.headline.contains("degraded"));

    let md1 = findings
        .iter()
        .find(|f| f.headline.starts_with("md1"))
        .unwrap();
    assert_eq!(md1.verdict, Verdict::Ok);
}

#[test]
fn every_storage_finding_carries_evidence_and_an_explanation() {
    let mount = mount_with("/", 100 * GIB, 2 * GIB, (1_000_000, 50_000));
    let logs = logspace::LogFootprint {
        journal_bytes: Some(10 * GIB),
        journal_report: Some("journals take up 10.0G".into()),
        var_log: None,
        availability: syschk::collect::Availability::Ok,
    };
    let mut all = space_findings(std::slice::from_ref(&mount));
    all.extend(inode_findings(std::slice::from_ref(&mount)));
    all.push(log_finding(&logs, Some(&mount)));
    all.extend(storage::drive_findings(&[]));

    for finding in all {
        assert!(
            !finding.evidence.is_empty(),
            "{} has a verdict with no numbers",
            finding.headline
        );
        assert!(
            !finding.learn.is_empty(),
            "{} does not explain what its numbers mean",
            finding.headline
        );
        assert!(
            finding.headline.is_ascii(),
            "user-facing text must be English"
        );
    }
}

// ── 헬퍼 ───────────────────────────────────────────────────────────

fn tempdir(name: &str) -> std::path::PathBuf {
    let base = std::env::temp_dir().join(format!("syschk-test-{name}-{}", std::process::id()));
    std::fs::remove_dir_all(&base).ok();
    std::fs::create_dir_all(&base).expect("temp dir");
    base
}

fn write_bytes(path: &std::path::Path, len: usize) {
    use std::io::Write;
    let mut file = std::fs::File::create(path).expect("create test file");
    file.write_all(&vec![b'x'; len]).expect("write test file");
}

/// 실제 드라이브 조회. 권한이 없는 것이 정상이므로, 결과를 꾸며내지 않는지만 본다.
#[test]
fn real_drive_query_never_invents_a_verdict() {
    let ctx = ctx();
    let mount_list = mounts::mounts(&ctx);
    let devices = blockdev::devices(&ctx, &mount_list);
    for (device, block, kind, model) in smart::drives(&devices) {
        let health = match kind {
            DriveKind::Sata => smart::read_sata(&device, &block, &model),
            DriveKind::Nvme => smart::read_nvme(&device, &block, &model),
        };
        // 자료를 얻지 못했다면 건강하다고 말해서는 안 된다.
        if !health.availability.is_ok() {
            assert!(
                health.healthy.is_none() && health.attributes.is_empty(),
                "{device} reported values despite {:?}",
                health.availability
            );
            let findings = drive_findings(&[health]);
            assert_eq!(findings[0].verdict, Verdict::Unknown);
        }
    }
}

/// 컨트롤러 경로를 올바르게 만드는지. `nvme0n1` 의 SMART 는 `/dev/nvme0` 에 물어본다.
#[test]
fn nvme_health_is_queried_on_the_controller() {
    let ctx = ctx();
    let mount_list = mounts::mounts(&ctx);
    let devices = blockdev::devices(&ctx, &mount_list);
    let list = smart::drives(&devices);

    let nvme = list
        .iter()
        .find(|(_, block, _, _)| block == "nvme0n1")
        .expect("nvme drive from fixtures");
    assert_eq!(nvme.0, "/dev/nvme0");
    assert_eq!(nvme.2, DriveKind::Nvme);

    let sata = list
        .iter()
        .find(|(_, block, _, _)| block == "sda")
        .expect("sata drive from fixtures");
    assert_eq!(sata.0, "/dev/sda");
    assert_eq!(sata.2, DriveKind::Sata);
}
