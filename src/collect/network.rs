//! 네트워크 인터페이스 처리량과 오류.
//!
//! `/proc/net/dev` 의 누적값 차이로 초당 처리량을 만든다. 오류·드롭은 누적 자체가
//! 의미가 있으므로 값과 증가율을 함께 다룬다.

use super::{Availability, Field, Probe, ProbeCtx, ProbeData, ProbeResult};

/// 인터페이스 하나의 누적 통계.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NetStat {
    pub name: String,
    pub rx_bytes: u64,
    pub rx_packets: u64,
    pub rx_errors: u64,
    pub rx_dropped: u64,
    pub tx_bytes: u64,
    pub tx_packets: u64,
    pub tx_errors: u64,
    pub tx_dropped: u64,
}

#[derive(Clone, Debug, Default)]
pub struct NetSnapshot {
    pub interfaces: Vec<NetStat>,
}

impl NetSnapshot {
    pub fn read(ctx: &ProbeCtx) -> Option<Self> {
        let text = ctx.read("/proc/net/dev")?;
        let mut interfaces = Vec::new();
        for line in text.lines().skip(2) {
            let Some((name, rest)) = line.split_once(':') else {
                continue;
            };
            let name = name.trim().to_string();
            if name == "lo" {
                continue;
            }
            let f: Vec<u64> = rest
                .split_whitespace()
                .map(|v| v.parse().unwrap_or(0))
                .collect();
            if f.len() < 16 {
                continue;
            }
            interfaces.push(NetStat {
                name,
                rx_bytes: f[0],
                rx_packets: f[1],
                rx_errors: f[2],
                rx_dropped: f[3],
                tx_bytes: f[8],
                tx_packets: f[9],
                tx_errors: f[10],
                tx_dropped: f[11],
            });
        }
        (!interfaces.is_empty()).then_some(NetSnapshot { interfaces })
    }
}

/// 인터페이스 하나의 초당 지표.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NetRate {
    pub name: String,
    pub rx_bytes: f32,
    pub tx_bytes: f32,
    pub rx_packets: f32,
    pub tx_packets: f32,
    /// 인터페이스 오류 누적(rx_errors + tx_errors). 링크 품질 문제의 신호다.
    pub errors_total: u64,
    pub errors_per_sec: f32,
    /// 드롭 누적(rx_dropped + tx_dropped).
    ///
    /// 드롭은 오류와 다르다. 받을 사람이 없는 패킷(멀티캐스트 등)도 드롭으로 집계되므로
    /// 정상 시스템에서도 꾸준히 늘어난다. 따라서 오류와 분리해서 다룬다.
    pub drops_total: u64,
    pub drops_per_sec: f32,
}

pub fn rates(prev: &NetSnapshot, now: &NetSnapshot, secs: f32) -> Vec<NetRate> {
    if secs <= 0.0 {
        return Vec::new();
    }
    let mut out = Vec::new();
    for cur in &now.interfaces {
        let Some(old) = prev.interfaces.iter().find(|i| i.name == cur.name) else {
            continue;
        };
        let errors_now = cur.rx_errors + cur.tx_errors;
        let errors_before = old.rx_errors + old.tx_errors;
        let drops_now = cur.rx_dropped + cur.tx_dropped;
        let drops_before = old.rx_dropped + old.tx_dropped;
        out.push(NetRate {
            name: cur.name.clone(),
            rx_bytes: cur.rx_bytes.saturating_sub(old.rx_bytes) as f32 / secs,
            tx_bytes: cur.tx_bytes.saturating_sub(old.tx_bytes) as f32 / secs,
            rx_packets: cur.rx_packets.saturating_sub(old.rx_packets) as f32 / secs,
            tx_packets: cur.tx_packets.saturating_sub(old.tx_packets) as f32 / secs,
            errors_total: errors_now,
            errors_per_sec: errors_now.saturating_sub(errors_before) as f32 / secs,
            drops_total: drops_now,
            drops_per_sec: drops_now.saturating_sub(drops_before) as f32 / secs,
        });
    }
    out.sort_by(|a, b| {
        (b.rx_bytes + b.tx_bytes)
            .partial_cmp(&(a.rx_bytes + a.tx_bytes))
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    out
}

pub struct NetworkProbe;

impl Probe for NetworkProbe {
    fn id(&self) -> &'static str {
        "network.io"
    }

    fn describe(&self) -> &'static str {
        "Bytes and packets per second per interface, with error and drop counters"
    }

    fn commands(&self) -> Vec<&'static str> {
        vec!["cat /proc/net/dev"]
    }

    fn run(&self, ctx: &ProbeCtx) -> ProbeResult {
        let Some(snap) = NetSnapshot::read(ctx) else {
            return ProbeResult::unavailable(
                "network.io",
                Availability::ParseFailed {
                    reason: "/proc/net/dev is not readable".into(),
                },
            );
        };
        ProbeResult::ok(
            "network.io",
            ProbeData::Fields(
                snap.interfaces
                    .iter()
                    .map(|i| Field::new(i.name.clone(), format!("{} rx bytes", i.rx_bytes)))
                    .collect(),
            ),
        )
    }
}
