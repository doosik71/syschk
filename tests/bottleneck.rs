//! 병목 판정 규칙.
//!
//! 지표 조합 → 기대 판정을 표로 검증한다. 실제 부하를 만들지 않고도 규칙이
//! 의도대로 동작하는지 확인할 수 있어야 한다.

use syschk::analyze::{Axis, Metrics, Verdict, assess};
use syschk::collect::blockio::DiskRate;
use syschk::collect::cpu::CpuUsage;
use syschk::collect::memory::{Memory, SwapRate};
use syschk::collect::network::NetRate;
use syschk::collect::pressure::{Pressure, PressureSet};

/// 여유로운 시스템의 기준값.
fn idle_cpu() -> CpuUsage {
    CpuUsage {
        user: 3.0,
        system: 1.0,
        iowait: 0.0,
        steal: 0.0,
        irq: 0.0,
        idle: 96.0,
        busy: 4.0,
        per_core: vec![4.0, 4.0],
    }
}

fn healthy_memory() -> Memory {
    Memory {
        total: 16_000_000,
        free: 8_000_000,
        available: 12_000_000, // 75% 여유
        buffers: 100_000,
        cached: 3_000_000,
        swap_total: 2_000_000,
        swap_free: 2_000_000,
        dirty: 0,
        shmem: 0,
    }
}

fn quiet_disk() -> Vec<DiskRate> {
    vec![DiskRate {
        name: "sda".into(),
        read_bytes: 1000.0,
        write_bytes: 1000.0,
        read_iops: 2.0,
        write_iops: 2.0,
        await_ms: 0.5,
        util_pct: 2.0,
        in_flight: 0,
    }]
}

fn quiet_net() -> Vec<NetRate> {
    vec![NetRate {
        name: "eth0".into(),
        rx_bytes: 1000.0,
        tx_bytes: 500.0,
        rx_packets: 5.0,
        tx_packets: 3.0,
        errors_total: 0,
        errors_per_sec: 0.0,
        drops_total: 0,
        drops_per_sec: 0.0,
    }]
}

fn psi(cpu: f32, memory: f32, io: f32) -> PressureSet {
    let p = |v: f32| {
        Some(Pressure {
            some_avg10: v,
            some_avg60: v,
            full_avg10: 0.0,
        })
    };
    PressureSet {
        cpu: p(cpu),
        memory: p(memory),
        io: p(io),
    }
}

struct Case {
    cpu: CpuUsage,
    load1: f32,
    memory: Memory,
    swap: SwapRate,
    pressure: PressureSet,
    disks: Vec<DiskRate>,
    nets: Vec<NetRate>,
    blocked: u32,
    samples: u32,
}

impl Case {
    fn healthy() -> Self {
        Self {
            cpu: idle_cpu(),
            load1: 0.4,
            memory: healthy_memory(),
            swap: SwapRate::default(),
            pressure: psi(0.0, 0.0, 0.0),
            disks: quiet_disk(),
            nets: quiet_net(),
            blocked: 0,
            samples: 10,
        }
    }

    fn axis_and_verdict(&self) -> (Axis, Verdict) {
        let m = Metrics {
            cpu: &self.cpu,
            load1: self.load1,
            cores: 2,
            pressure: &self.pressure,
            memory: &self.memory,
            swap: self.swap,
            disks: &self.disks,
            nets: &self.nets,
            blocked: self.blocked,
            samples: self.samples,
        };
        let a = assess(&m);
        (a.axis, a.worst())
    }

    fn verdict_for(&self, axis: &str) -> Verdict {
        let m = Metrics {
            cpu: &self.cpu,
            load1: self.load1,
            cores: 2,
            pressure: &self.pressure,
            memory: &self.memory,
            swap: self.swap,
            disks: &self.disks,
            nets: &self.nets,
            blocked: self.blocked,
            samples: self.samples,
        };
        assess(&m)
            .finding(axis)
            .map(|f| f.verdict)
            .expect("every axis is judged")
    }
}

#[test]
fn a_healthy_system_names_no_bottleneck() {
    let (axis, verdict) = Case::healthy().axis_and_verdict();
    assert_eq!(axis, Axis::None);
    assert_eq!(verdict, Verdict::Ok);
}

#[test]
fn one_sample_is_not_enough_to_judge() {
    let mut case = Case::healthy();
    case.samples = 1;
    let (axis, verdict) = case.axis_and_verdict();
    assert_eq!(axis, Axis::None);
    assert_eq!(verdict, Verdict::Unknown, "rates need two samples");
}

#[test]
fn a_long_run_queue_points_at_cpu() {
    let mut case = Case::healthy();
    case.load1 = 3.0; // 코어 2개에 1.5배
    case.cpu.busy = 95.0;
    case.pressure = psi(30.0, 0.0, 0.0);
    let (axis, verdict) = case.axis_and_verdict();
    assert_eq!(axis, Axis::Cpu);
    assert_eq!(verdict, Verdict::Warn);
}

#[test]
fn an_overloaded_queue_is_critical() {
    let mut case = Case::healthy();
    case.load1 = 8.0; // 코어당 4.0
    case.pressure = psi(70.0, 0.0, 0.0);
    let (axis, verdict) = case.axis_and_verdict();
    assert_eq!(axis, Axis::Cpu);
    assert_eq!(verdict, Verdict::Critical);
}

#[test]
fn stolen_time_is_called_out_on_virtual_machines() {
    let mut case = Case::healthy();
    case.cpu.steal = 20.0;
    assert_eq!(case.verdict_for("cpu"), Verdict::Warn);
}

#[test]
fn almost_no_memory_left_is_critical() {
    let mut case = Case::healthy();
    case.memory.available = 400_000; // 2.5%
    let (axis, verdict) = case.axis_and_verdict();
    assert_eq!(axis, Axis::Memory);
    assert_eq!(verdict, Verdict::Critical);
}

#[test]
fn steady_swapping_out_is_a_warning() {
    let mut case = Case::healthy();
    case.memory.swap_free = 1_000_000; // 스왑이 실제로 쓰이고 있다
    case.swap = SwapRate {
        pages_in: 10.0,
        pages_out: 500.0,
        major_faults: 20.0,
    };
    assert_eq!(case.verdict_for("memory"), Verdict::Warn);
}

#[test]
fn a_full_cache_alone_is_not_a_memory_problem() {
    let mut case = Case::healthy();
    // 캐시가 메모리를 거의 다 채웠지만 available 은 넉넉하다 → 문제가 아니다.
    case.memory.free = 200_000;
    case.memory.cached = 11_000_000;
    assert_eq!(case.verdict_for("memory"), Verdict::Ok);
}

#[test]
fn a_saturated_drive_points_at_disk() {
    let mut case = Case::healthy();
    case.disks = vec![DiskRate {
        name: "sda".into(),
        read_bytes: 50_000_000.0,
        write_bytes: 10_000_000.0,
        read_iops: 500.0,
        write_iops: 100.0,
        await_ms: 45.0,
        util_pct: 99.0,
        in_flight: 12,
    }];
    case.cpu.iowait = 30.0;
    case.pressure = psi(0.0, 0.0, 35.0);
    let (axis, verdict) = case.axis_and_verdict();
    assert_eq!(axis, Axis::Disk);
    assert_eq!(verdict, Verdict::Warn);
}

#[test]
fn a_busy_but_responsive_drive_is_fine() {
    let mut case = Case::healthy();
    // 바쁘지만 대기시간이 짧다 → 건강한 상태다.
    case.disks = vec![DiskRate {
        name: "nvme0n1".into(),
        read_bytes: 900_000_000.0,
        write_bytes: 100_000_000.0,
        read_iops: 20_000.0,
        write_iops: 2_000.0,
        await_ms: 0.3,
        util_pct: 95.0,
        in_flight: 4,
    }];
    assert_eq!(case.verdict_for("disk"), Verdict::Ok);
}

#[test]
fn processes_stuck_on_storage_are_a_warning() {
    let mut case = Case::healthy();
    case.blocked = 3;
    assert_eq!(case.verdict_for("disk"), Verdict::Warn);
}

#[test]
fn link_errors_warn_but_drops_alone_do_not() {
    let mut case = Case::healthy();
    case.nets[0].drops_total = 500_000;
    case.nets[0].drops_per_sec = 25.0;
    assert_eq!(
        case.verdict_for("network"),
        Verdict::Ok,
        "drops include packets nobody was listening for"
    );

    case.nets[0].errors_total = 12;
    case.nets[0].errors_per_sec = 4.0;
    assert_eq!(case.verdict_for("network"), Verdict::Warn);
}

#[test]
fn the_worst_axis_wins_and_evidence_is_always_present() {
    let mut case = Case::healthy();
    case.load1 = 3.0; // cpu 경고
    case.memory.available = 300_000; // 메모리 심각
    let (axis, verdict) = case.axis_and_verdict();
    assert_eq!(axis, Axis::Memory, "critical beats warning");
    assert_eq!(verdict, Verdict::Critical);

    let m = Metrics {
        cpu: &case.cpu,
        load1: case.load1,
        cores: 2,
        pressure: &case.pressure,
        memory: &case.memory,
        swap: case.swap,
        disks: &case.disks,
        nets: &case.nets,
        blocked: case.blocked,
        samples: case.samples,
    };
    for finding in assess(&m).findings {
        assert!(
            !finding.evidence.is_empty(),
            "{} has a verdict but no numbers behind it",
            finding.axis
        );
        assert!(
            !finding.learn.is_empty(),
            "{} has no explanation of what its numbers mean",
            finding.axis
        );
        assert!(finding.headline.is_ascii(), "verdict text must be English");
    }
}
