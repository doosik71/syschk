//! 수집기 계약. 픽스처를 루트로 삼아 실제 시스템과 무관하게 검증한다.

mod common;

use syschk::collect::{Availability, ProbeCtx, probes};

#[test]
fn probes_parse_fixture_system() {
    let ctx = ProbeCtx::with_root(common::fixture_root());
    for probe in probes() {
        let result = probe.run(&ctx);
        assert!(
            result.availability.is_ok(),
            "probe {} failed on fixtures: {}",
            probe.id(),
            result.availability.message()
        );
        match probe.id() {
            "system.identity" => {
                assert_eq!(result.data.field("hostname"), Some("testbox"));
                assert_eq!(result.data.field("os"), Some("Ubuntu 24.04.4 LTS"));
                assert_eq!(result.data.field("kernel"), Some("6.8.0-138-generic"));
            }
            "system.uptime" => {
                // 12345 초 = 3h 25m
                assert_eq!(result.data.field("uptime"), Some("3h 25m"));
            }
            "system.load" => {
                assert_eq!(result.data.field("load1"), Some("0.52"));
                assert_eq!(result.data.field("cores"), Some("4"));
            }
            // M1 수집기는 픽스처에서 값이 읽히는지만 확인한다(세부 검증은 live_metrics.rs).
            "cpu.usage" => {
                assert_eq!(result.data.field("cores"), Some("2"));
                assert_eq!(result.data.field("blocked"), Some("1"));
            }
            "memory.usage" => {
                assert_eq!(result.data.field("total_kb"), Some("16384000"));
            }
            "disk.io" | "network.io" => {}
            "process.list" => {
                assert_eq!(result.data.field("processes"), Some("2"));
                assert_eq!(result.data.field("blocked"), Some("1"));
            }
            other => panic!("unexpected probe id: {other}"),
        }
    }
}

/// 읽을 수 없는 항목은 앱을 죽이지 않고 사유를 남긴다.
#[test]
fn missing_sources_degrade_gracefully() {
    let ctx = ProbeCtx::with_root("/nonexistent-root-for-tests");
    for probe in probes() {
        let result = probe.run(&ctx);
        match probe.id() {
            // identity 는 값이 없어도 "unknown" 으로 채워 계속 진행한다.
            "system.identity" => assert!(result.availability.is_ok()),
            // 프로세스 목록은 디렉터리를 열 수 없다는 사실을 알린다.
            "process.list" | "disk.io" | "network.io" => assert!(
                matches!(result.availability, Availability::ParseFailed { .. }),
                "probe {} should report why it could not read",
                probe.id()
            ),
            _ => assert!(
                matches!(result.availability, Availability::ParseFailed { .. }),
                "probe {} should report why it could not read, got {:?}",
                probe.id(),
                result.availability
            ),
        }
    }
}

#[test]
fn probe_ids_are_unique_and_described() {
    let mut ids = std::collections::HashSet::new();
    for probe in probes() {
        assert!(ids.insert(probe.id()), "duplicate probe id {}", probe.id());
        assert!(!probe.describe().is_empty());
        assert!(
            probe.describe().is_ascii(),
            "probe description must be English"
        );
    }
}
