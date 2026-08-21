//! 저장장치 I/O.
//!
//! `/proc/diskstats` 의 누적값을 두 표본으로 나눠 처리량·대기시간·사용률을 만든다.
//! `iostat -x` 가 계산하는 값과 같은 정의를 쓴다.

use super::{Availability, Field, Probe, ProbeCtx, ProbeData, ProbeResult};

const SECTOR_BYTES: u64 = 512;

/// 장치 하나의 누적 통계.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DiskStat {
    pub name: String,
    pub reads: u64,
    pub sectors_read: u64,
    pub ms_reading: u64,
    pub writes: u64,
    pub sectors_written: u64,
    pub ms_writing: u64,
    pub ios_in_flight: u64,
    /// 장치가 I/O 를 처리하고 있던 시간(ms). 사용률 계산에 쓴다.
    pub ms_doing_io: u64,
}

/// 한 시점의 전체 장치 통계.
#[derive(Clone, Debug, Default)]
pub struct DiskSnapshot {
    pub devices: Vec<DiskStat>,
}

impl DiskSnapshot {
    pub fn read(ctx: &ProbeCtx) -> Option<Self> {
        let text = ctx.read("/proc/diskstats")?;
        let mut devices = Vec::new();
        for line in text.lines() {
            let f: Vec<&str> = line.split_whitespace().collect();
            if f.len() < 14 {
                continue;
            }
            let name = f[2].to_string();
            // 파티션·가상 장치를 걸러 전체 디스크만 남긴다.
            if !is_whole_device(ctx, &name) {
                continue;
            }
            let num = |i: usize| f[i].parse::<u64>().unwrap_or(0);
            devices.push(DiskStat {
                name,
                reads: num(3),
                sectors_read: num(5),
                ms_reading: num(6),
                writes: num(7),
                sectors_written: num(9),
                ms_writing: num(10),
                ios_in_flight: num(11),
                ms_doing_io: num(12),
            });
        }
        (!devices.is_empty()).then_some(DiskSnapshot { devices })
    }
}

/// `/sys/block/<name>` 이 있으면 전체 디스크다. 없으면 파티션이다.
fn is_whole_device(ctx: &ProbeCtx, name: &str) -> bool {
    if name.starts_with("loop") || name.starts_with("ram") || name.starts_with("zram") {
        return false;
    }
    if ctx.exists(&format!("/sys/block/{name}")) {
        return true;
    }
    // `/sys` 가 없는 환경(시험 픽스처)에서는 이름 규칙으로 판단한다.
    if !ctx.exists("/sys/block") {
        let is_partition = name.starts_with("nvme") && name.contains('p')
            || (name.starts_with("sd") && name.chars().last().is_some_and(|c| c.is_ascii_digit()));
        return !is_partition;
    }
    false
}

/// 장치 하나의 초당 지표.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DiskRate {
    pub name: String,
    pub read_bytes: f32,
    pub write_bytes: f32,
    pub read_iops: f32,
    pub write_iops: f32,
    /// 요청 하나가 완료되기까지 걸린 평균 시간(ms).
    pub await_ms: f32,
    /// 장치가 바쁜 시간의 비율(%).
    pub util_pct: f32,
    pub in_flight: u64,
}

pub fn rates(prev: &DiskSnapshot, now: &DiskSnapshot, secs: f32) -> Vec<DiskRate> {
    if secs <= 0.0 {
        return Vec::new();
    }
    let mut out = Vec::new();
    for cur in &now.devices {
        let Some(old) = prev.devices.iter().find(|d| d.name == cur.name) else {
            continue;
        };
        let d_reads = cur.reads.saturating_sub(old.reads);
        let d_writes = cur.writes.saturating_sub(old.writes);
        let d_ms = cur.ms_reading.saturating_sub(old.ms_reading)
            + cur.ms_writing.saturating_sub(old.ms_writing);
        let ios = d_reads + d_writes;
        out.push(DiskRate {
            name: cur.name.clone(),
            read_bytes: cur.sectors_read.saturating_sub(old.sectors_read) as f32
                * SECTOR_BYTES as f32
                / secs,
            write_bytes: cur.sectors_written.saturating_sub(old.sectors_written) as f32
                * SECTOR_BYTES as f32
                / secs,
            read_iops: d_reads as f32 / secs,
            write_iops: d_writes as f32 / secs,
            await_ms: if ios == 0 {
                0.0
            } else {
                d_ms as f32 / ios as f32
            },
            util_pct: (cur.ms_doing_io.saturating_sub(old.ms_doing_io) as f32 / (secs * 1000.0)
                * 100.0)
                .clamp(0.0, 100.0),
            in_flight: cur.ios_in_flight,
        });
    }
    out.sort_by(|a, b| {
        (b.read_bytes + b.write_bytes)
            .partial_cmp(&(a.read_bytes + a.write_bytes))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    out
}

pub struct DiskIoProbe;

impl Probe for DiskIoProbe {
    fn id(&self) -> &'static str {
        "disk.io"
    }

    fn describe(&self) -> &'static str {
        "Throughput, IOPS, average wait and busy time per drive"
    }

    fn commands(&self) -> Vec<&'static str> {
        vec!["cat /proc/diskstats", "cat /proc/pressure/io"]
    }

    fn run(&self, ctx: &ProbeCtx) -> ProbeResult {
        let Some(snap) = DiskSnapshot::read(ctx) else {
            return ProbeResult::unavailable(
                "disk.io",
                Availability::ParseFailed {
                    reason: "/proc/diskstats is not readable".into(),
                },
            );
        };
        ProbeResult::ok(
            "disk.io",
            ProbeData::Fields(
                snap.devices
                    .iter()
                    .map(|d| Field::new(d.name.clone(), format!("{} reads", d.reads)))
                    .collect(),
            ),
        )
    }
}
