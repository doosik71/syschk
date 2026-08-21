//! 실시간 지표 계산. 두 시점 픽스처로 비율 계산을 검증한다.
//!
//! 실제 시스템 상태와 무관하게 결정적으로 확인할 수 있어야 한다. 시간 간격도
//! 인자로 넘기므로 시험이 타이밍에 흔들리지 않는다.

mod common;

use syschk::collect::ProbeCtx;
use syschk::collect::blockio::{self, DiskSnapshot};
use syschk::collect::cpu::{self, CpuSnapshot, Load};
use syschk::collect::memory::{self, Memory, VmStat};
use syschk::collect::network::{self, NetSnapshot};
use syschk::collect::pressure::PressureSet;
use syschk::collect::process::{self, ProcSnapshot};

fn ctx(sample: &str) -> ProbeCtx {
    ProbeCtx::with_root(common::fixture_root().join(sample))
}

fn close(actual: f32, expected: f32, tolerance: f32, what: &str) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "{what}: expected ~{expected}, got {actual}"
    );
}

#[test]
fn cpu_usage_is_the_difference_between_two_samples() {
    let before = CpuSnapshot::read(&ctx("t0")).expect("t0 /proc/stat");
    let after = CpuSnapshot::read(&ctx("t1")).expect("t1 /proc/stat");

    assert_eq!(before.cores(), 2);
    assert_eq!(before.procs_blocked, 2);
    assert_eq!(after.procs_running, 4);

    // 델타: user 100, system 50, idle 300, iowait 50 → 합계 500 jiffies
    let usage = cpu::usage(&before, &after);
    close(usage.user, 20.0, 0.01, "user");
    close(usage.system, 10.0, 0.01, "system");
    close(usage.iowait, 10.0, 0.01, "iowait");
    close(usage.idle, 60.0, 0.01, "idle");
    close(usage.busy, 30.0, 0.01, "busy");
    assert_eq!(usage.per_core.len(), 2);
    close(usage.per_core[0], 30.0, 0.01, "core 0");
}

#[test]
fn load_is_reported_against_core_count() {
    let load = Load::read(&ctx("t0")).expect("loadavg");
    close(load.one, 1.5, 0.001, "load1");
    close(load.per_core(2), 0.75, 0.001, "load per core");
}

#[test]
fn memory_separates_cache_from_real_usage() {
    let m = Memory::read(&ctx("t0")).expect("meminfo");
    assert_eq!(m.total, 16_384_000);
    // used 는 available 기준으로 센다: free 가 아니라 available.
    assert_eq!(m.used(), 8_192_000);
    close(m.used_pct(), 50.0, 0.01, "used%");
    close(m.available_pct(), 50.0, 0.01, "available%");
    assert_eq!(m.swap_used(), 0);
}

#[test]
fn swap_activity_is_a_rate_not_a_total() {
    let before = VmStat::read(&ctx("t0")).expect("vmstat t0");
    let after = VmStat::read(&ctx("t1")).expect("vmstat t1");
    let rate = memory::swap_rate(&before, &after, 1.0);
    close(rate.pages_out, 100.0, 0.01, "pages out per second");
    close(rate.pages_in, 0.0, 0.01, "pages in per second");
    close(rate.major_faults, 50.0, 0.01, "major faults per second");
}

#[test]
fn disk_rates_use_whole_devices_only() {
    let before = DiskSnapshot::read(&ctx("t0")).expect("diskstats t0");
    let after = DiskSnapshot::read(&ctx("t1")).expect("diskstats t1");

    // 파티션(sda1)과 loop 장치는 걸러진다.
    let names: Vec<&str> = after.devices.iter().map(|d| d.name.as_str()).collect();
    assert_eq!(names, vec!["sda"], "only whole devices should be counted");

    let rates = blockio::rates(&before, &after, 1.0);
    let sda = &rates[0];
    // 섹터 2000개 = 1,024,000 바이트
    close(sda.read_bytes, 1_024_000.0, 1.0, "read bytes per second");
    close(sda.write_bytes, 1_024_000.0, 1.0, "write bytes per second");
    close(sda.read_iops, 100.0, 0.01, "read iops");
    close(sda.write_iops, 50.0, 0.01, "write iops");
    // (200ms + 100ms) / 150 요청 = 2ms
    close(sda.await_ms, 2.0, 0.01, "average wait");
    // 1초 중 500ms 동안 바빴다
    close(sda.util_pct, 50.0, 0.01, "utilisation");
}

#[test]
fn network_separates_errors_from_drops() {
    let before = NetSnapshot::read(&ctx("t0")).expect("net/dev t0");
    let after = NetSnapshot::read(&ctx("t1")).expect("net/dev t1");

    // 루프백은 제외한다.
    assert!(after.interfaces.iter().all(|i| i.name != "lo"));

    let rates = network::rates(&before, &after, 1.0);
    let eth = &rates[0];
    assert_eq!(eth.name, "eth0");
    close(eth.rx_bytes, 2000.0, 0.01, "rx bytes per second");
    close(eth.tx_bytes, 1000.0, 0.01, "tx bytes per second");
    // 오류와 드롭은 다른 뜻이므로 따로 센다.
    assert_eq!(eth.errors_total, 2);
    close(eth.errors_per_sec, 2.0, 0.01, "errors per second");
    assert_eq!(eth.drops_total, 8);
    close(eth.drops_per_sec, 3.0, 0.01, "drops per second");
}

#[test]
fn process_rows_carry_cpu_memory_and_io() {
    let before = ProcSnapshot::read(&ctx("t0")).expect("proc t0");
    let after = ProcSnapshot::read(&ctx("t1")).expect("proc t1");
    let users = process::user_names(&ctx("t1"));
    let rows = process::rows(&before, &after, 1.0, 16_384_000, &users);

    let worker = rows.iter().find(|r| r.pid == 100).expect("pid 100");
    // 델타 50 jiffies = 0.5초의 CPU 시간 → 1초 동안 50%
    close(worker.cpu_pct, 50.0, 0.01, "process cpu");
    assert_eq!(worker.rss_kb, 4800); // 1200 페이지 * 4KiB
    assert!(worker.cmd.contains("worker --flag"));
    close(
        worker.read_bps.expect("io readable"),
        4000.0,
        1.0,
        "read bytes per second",
    );
    close(
        worker.write_bps.expect("io readable"),
        0.0,
        1.0,
        "write bytes per second",
    );
    assert_eq!(worker.threads, 4);

    let stuck = rows.iter().find(|r| r.pid == 200).expect("pid 200");
    assert!(stuck.is_blocked(), "state D means waiting on the kernel");
    assert_eq!(stuck.wchan.as_deref(), Some("wait_on_page_bit"));
    // 권한이 없어 읽을 수 없는 값은 0 으로 꾸미지 않고 없음으로 남긴다.
    assert_eq!(stuck.read_bps, None);
    close(stuck.cpu_pct, 0.0, 0.01, "idle process cpu");
}

#[test]
fn pressure_is_optional_but_parsed_when_present() {
    let psi = PressureSet::read(&ctx("t0"));
    assert!(psi.available());
    close(
        psi.cpu.expect("cpu psi").some_avg10,
        5.0,
        0.01,
        "cpu pressure",
    );
    close(
        psi.io.expect("io psi").some_avg10,
        25.0,
        0.01,
        "io pressure",
    );
    close(
        psi.io.expect("io psi").full_avg10,
        12.0,
        0.01,
        "io full pressure",
    );

    // 커널이 제공하지 않는 경우에도 앱은 계속 동작해야 한다.
    let none = PressureSet::read(&ProbeCtx::with_root("/nonexistent-root-for-tests"));
    assert!(!none.available());
}

/// 표본 하나를 뜨는 데 드는 시간. 실제 시스템에서 측정하며, 기본 실행에서는 건너뛴다.
///
/// `cargo test --test live_metrics -- --ignored --nocapture` 로 확인한다.
/// 진단 도구가 진단 대상에 부담을 주지 않아야 하므로(NFR-1) 이 값을 눈으로 확인한다.
#[test]
#[ignore = "measures the live system; run explicitly"]
fn sampling_cost_is_small() {
    use std::time::Instant;
    let mut sampler = syschk::app::sampler::Sampler::new(ProbeCtx::default());
    sampler.tick(); // 캐시 예열
    let rounds = 20;
    let started = Instant::now();
    for _ in 0..rounds {
        sampler.tick();
    }
    let per_tick = started.elapsed() / rounds;
    println!(
        "one sample took {:?} across {} processes",
        per_tick,
        sampler.procs.len()
    );
    assert!(
        per_tick.as_millis() < 100,
        "sampling should stay well under the one second interval"
    );
}
