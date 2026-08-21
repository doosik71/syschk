//! 드라이브 자기 진단(SMART) 요약.
//!
//! 이 화면의 목적은 속성 표를 그대로 옮기는 것이 아니라, **고장을 예고하는 몇 개의 값**만
//! 골라 뜻과 함께 보여주는 것이다. 초보자에게 200줄짜리 `smartctl -a` 출력은 도움이 되지 않는다.
//!
//! SATA/SAS 는 `smartmontools`, NVMe 는 `nvme-cli` 가 필요하고 둘 다 보통 root 권한을 요구한다.
//! 없거나 권한이 부족하면 그 사실과 해결 방법을 알린다.

use super::blockdev::BlockDevice;
use super::{Availability, ProbeCtx};
use crate::util::exec::{CommandOutput, ReadOnlyCommand};

/// 드라이브 종류. 조회 방법이 다르다.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DriveKind {
    Sata,
    Nvme,
}

/// 하나의 관심 지표.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HealthAttr {
    pub label: &'static str,
    pub value: String,
    /// 이 값이 우려스러운가.
    pub concern: bool,
    /// 이 값이 무엇인지 한 줄 설명.
    pub explain: &'static str,
}

/// 드라이브 하나의 건전성 요약.
#[derive(Clone, Debug)]
pub struct DriveHealth {
    /// 조회 대상 장치 경로(`/dev/sda`, `/dev/nvme0`).
    pub device: String,
    /// 이 장치가 담고 있는 블록 장치 이름(`sda`, `nvme0n1`).
    pub block: String,
    pub kind: DriveKind,
    pub model: String,
    /// 드라이브 자체의 종합 판단. 얻지 못하면 `None`.
    pub healthy: Option<bool>,
    pub attributes: Vec<HealthAttr>,
    pub availability: Availability,
    pub raw: Vec<CommandOutput>,
}

impl DriveHealth {
    /// 우려스러운 값이 하나라도 있는가.
    pub fn has_concern(&self) -> bool {
        self.healthy == Some(false) || self.attributes.iter().any(|a| a.concern)
    }
}

/// 조회할 드라이브 목록. 파티션이 아니라 드라이브 단위다.
pub fn drives(devices: &[BlockDevice]) -> Vec<(String, String, DriveKind, String)> {
    devices
        .iter()
        .filter_map(|d| {
            if let Some(rest) = d.name.strip_prefix("nvme") {
                // `nvme0n1` 은 컨트롤러 0 의 namespace 1 이다. SMART 는 컨트롤러에 물어본다.
                let index: String = rest.chars().take_while(char::is_ascii_digit).collect();
                if index.is_empty() {
                    return None;
                }
                Some((
                    format!("/dev/nvme{index}"),
                    d.name.clone(),
                    DriveKind::Nvme,
                    d.model.clone(),
                ))
            } else if d.name.starts_with("sd") || d.name.starts_with("hd") {
                Some((
                    format!("/dev/{}", d.name),
                    d.name.clone(),
                    DriveKind::Sata,
                    d.model.clone(),
                ))
            } else {
                // dm-*, md* 등 가상 장치는 자기 진단 대상이 아니다.
                None
            }
        })
        .collect()
}

/// 실행 결과에서 권한 문제인지 판단한다.
fn privilege_problem(out: &CommandOutput) -> bool {
    let text = format!("{} {}", out.stdout, out.stderr).to_lowercase();
    text.contains("permission denied")
        || text.contains("requires root")
        || text.contains("operation not permitted")
        || text.contains("must be run as root")
}

/// SATA/SAS 드라이브를 조회한다.
pub fn read_sata(device: &str, block: &str, model: &str) -> DriveHealth {
    let mut health = DriveHealth {
        device: device.to_string(),
        block: block.to_string(),
        kind: DriveKind::Sata,
        model: model.to_string(),
        healthy: None,
        attributes: Vec::new(),
        availability: Availability::Ok,
        raw: Vec::new(),
    };

    if crate::util::exec::find_in_path("smartctl").is_none() {
        health.availability = Availability::NotInstalled {
            tool: "smartmontools",
        };
        return health;
    }
    let Ok(cmd) = ReadOnlyCommand::new("smartctl", &["-H", "-A", device]) else {
        health.availability = Availability::Unsupported {
            reason: "command refused by the read-only policy".into(),
        };
        return health;
    };

    let out = cmd.run();
    if privilege_problem(&out) {
        health.availability = Availability::NeedsPrivilege {
            hint: "SMART data needs root - run: sudo syschk".into(),
        };
        health.raw.push(out);
        return health;
    }
    if out.spawn_error.is_some() {
        health.availability = Availability::ParseFailed {
            reason: out.spawn_error.clone().unwrap_or_default(),
        };
        health.raw.push(out);
        return health;
    }

    health.healthy = parse_sata_health(&out.stdout);
    health.attributes = parse_sata_attributes(&out.stdout);
    if health.healthy.is_none() && health.attributes.is_empty() {
        health.availability = Availability::ParseFailed {
            reason: "smartctl produced no readable health data for this drive".into(),
        };
    }
    health.raw.push(out);
    health
}

/// `SMART overall-health self-assessment test result: PASSED`
pub fn parse_sata_health(text: &str) -> Option<bool> {
    let line = text
        .lines()
        .find(|l| l.contains("overall-health") || l.contains("SMART Health Status"))?;
    let upper = line.to_uppercase();
    if upper.contains("PASSED") || upper.contains("OK") {
        Some(true)
    } else if upper.contains("FAILED") {
        Some(false)
    } else {
        None
    }
}

/// 고장을 예고하는 속성만 고른다. `(SMART id, 표시 이름, 설명)`.
const SATA_ATTRS: &[(u32, &str, &str)] = &[
    (
        5,
        "reallocated sectors",
        "Sectors that went bad and were swapped for spares. Any growth here means the surface is failing.",
    ),
    (
        187,
        "uncorrectable errors",
        "Reads the drive could not fix by itself. Non-zero means data was at risk.",
    ),
    (
        188,
        "command timeouts",
        "Commands the drive failed to answer in time. Often a cable or power problem rather than the disk.",
    ),
    (
        197,
        "pending sectors",
        "Sectors waiting to be reallocated. They are suspect but not yet replaced.",
    ),
    (
        198,
        "offline uncorrectable",
        "Sectors found unreadable during the drive's own background scan.",
    ),
    (
        199,
        "cable errors (CRC)",
        "Corrupted transfers between the drive and the board. Usually the cable, not the drive.",
    ),
    (
        9,
        "power-on hours",
        "How long the drive has been running. Context, not a fault.",
    ),
    (
        194,
        "temperature",
        "Drive temperature. Sustained heat shortens a drive's life.",
    ),
];

/// 속성 표를 파싱한다. 우려 대상은 값이 0 이 아닐 때다(시간·온도는 예외).
pub fn parse_sata_attributes(text: &str) -> Vec<HealthAttr> {
    let mut out = Vec::new();
    for line in text.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 10 {
            continue;
        }
        let Ok(id) = fields[0].parse::<u32>() else {
            continue;
        };
        let Some((_, label, explain)) = SATA_ATTRS.iter().find(|(a, _, _)| *a == id) else {
            continue;
        };
        // RAW_VALUE 는 마지막 열이며 `26 (Min/Max 20/35)` 처럼 꼬리가 붙을 수 있다.
        let raw = fields[9..].join(" ");
        let numeric = raw
            .split_whitespace()
            .next()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0);
        let concern = match id {
            9 | 194 => false, // 시간과 온도는 그 자체로 고장 신호가 아니다
            _ => numeric > 0,
        };
        out.push(HealthAttr {
            label,
            value: raw,
            concern,
            explain,
        });
    }
    out
}

/// NVMe 드라이브를 조회한다.
pub fn read_nvme(device: &str, block: &str, model: &str) -> DriveHealth {
    let mut health = DriveHealth {
        device: device.to_string(),
        block: block.to_string(),
        kind: DriveKind::Nvme,
        model: model.to_string(),
        healthy: None,
        attributes: Vec::new(),
        availability: Availability::Ok,
        raw: Vec::new(),
    };

    if crate::util::exec::find_in_path("nvme").is_none() {
        health.availability = Availability::NotInstalled { tool: "nvme-cli" };
        return health;
    }
    let Ok(cmd) = ReadOnlyCommand::new("nvme", &["smart-log", device]) else {
        health.availability = Availability::Unsupported {
            reason: "command refused by the read-only policy".into(),
        };
        return health;
    };

    let out = cmd.run();
    if privilege_problem(&out) {
        health.availability = Availability::NeedsPrivilege {
            hint: "NVMe health data needs root - run: sudo syschk".into(),
        };
        health.raw.push(out);
        return health;
    }
    if out.spawn_error.is_some() {
        health.availability = Availability::ParseFailed {
            reason: out.spawn_error.clone().unwrap_or_default(),
        };
        health.raw.push(out);
        return health;
    }

    let parsed = parse_nvme_log(&out.stdout);
    health.healthy = parsed.healthy;
    health.attributes = parsed.attributes;
    if health.attributes.is_empty() {
        health.availability = Availability::ParseFailed {
            reason: "nvme smart-log produced no readable fields".into(),
        };
    }
    health.raw.push(out);
    health
}

/// NVMe 로그 파싱 결과.
pub struct NvmeLog {
    pub healthy: Option<bool>,
    pub attributes: Vec<HealthAttr>,
}

/// `nvme smart-log` 출력에서 관심 필드를 뽑는다.
pub fn parse_nvme_log(text: &str) -> NvmeLog {
    let field = |key: &str| -> Option<String> {
        text.lines()
            .find(|l| l.split(':').next().is_some_and(|k| k.trim() == key))
            .and_then(|l| l.split_once(':'))
            .map(|(_, v)| v.trim().to_string())
    };
    let number = |key: &str| -> Option<u64> {
        field(key).and_then(|v| {
            v.split_whitespace()
                .next()
                .map(|n| n.replace([',', '%'], ""))
                .and_then(|n| n.parse::<u64>().ok())
        })
    };

    let mut attributes = Vec::new();
    let critical = number("critical_warning");
    if let Some(value) = critical {
        attributes.push(HealthAttr {
            label: "critical warning",
            value: value.to_string(),
            concern: value != 0,
            explain: "A bitmask the drive raises for spare capacity, temperature, reliability or read-only state. Anything but 0 needs attention.",
        });
    }
    if let Some(value) = number("percentage_used") {
        attributes.push(HealthAttr {
            label: "life used",
            value: format!("{value}%"),
            // 100% 는 "수명 예상치를 다 썼다"는 뜻이며 즉시 고장은 아니다.
            concern: value >= 90,
            explain: "The drive's own estimate of how much of its write endurance is gone. Past 100% it still works, but replacement planning should start.",
        });
    }
    if let Some(value) = number("available_spare") {
        attributes.push(HealthAttr {
            label: "spare blocks",
            value: format!("{value}%"),
            concern: value < 20,
            explain: "Spare blocks left for replacing worn ones. Falling towards zero means the drive is running out of room to heal itself.",
        });
    }
    if let Some(value) = number("media_errors") {
        attributes.push(HealthAttr {
            label: "media errors",
            value: value.to_string(),
            concern: value > 0,
            explain: "Unrecoverable data errors detected by the drive. Should stay at zero.",
        });
    }
    if let Some(value) = number("num_err_log_entries") {
        attributes.push(HealthAttr {
            label: "error log entries",
            value: value.to_string(),
            concern: false,
            explain: "How many entries the drive's error log holds. Useful as history, not a verdict.",
        });
    }
    if let Some(value) = field("temperature") {
        attributes.push(HealthAttr {
            label: "temperature",
            value,
            concern: false,
            explain: "Controller temperature. NVMe drives throttle when hot, which shows up as sudden slowness.",
        });
    }
    if let Some(value) = number("unsafe_shutdowns") {
        attributes.push(HealthAttr {
            label: "unsafe shutdowns",
            value: value.to_string(),
            concern: false,
            explain: "Power lost before the drive was told to stop. A high count hints at power problems or hard resets.",
        });
    }

    NvmeLog {
        healthy: critical.map(|c| c == 0),
        attributes,
    }
}

/// 모든 드라이브의 건전성을 조회한다. 느리므로 배경 작업으로 돌린다.
pub fn read_all(ctx: &ProbeCtx) -> Vec<DriveHealth> {
    let mounts = super::mounts::mounts(ctx);
    let devices = super::blockdev::devices(ctx, &mounts);
    drives(&devices)
        .into_iter()
        .map(|(device, block, kind, model)| match kind {
            DriveKind::Sata => read_sata(&device, &block, &model),
            DriveKind::Nvme => read_nvme(&device, &block, &model),
        })
        .collect()
}
