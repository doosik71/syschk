//! 블록 장치 구성.
//!
//! `lsblk` 없이 `/sys/block` 을 직접 읽는다. 장치·파티션·회전 여부·모델을 얻고,
//! 장치 번호로 마운트 지점과 연결한다. 소프트웨어 RAID 는 `/proc/mdstat` 에서 읽는다.

use super::mounts::Mount;
use super::{Availability, Field, Probe, ProbeCtx, ProbeData, ProbeResult};

const SECTOR_BYTES: u64 = 512;

/// 파티션 한 건.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Partition {
    pub name: String,
    pub size_bytes: u64,
    /// 마운트되어 있으면 그 지점과 파일시스템.
    pub mount_point: Option<String>,
    pub fstype: Option<String>,
    pub read_only: bool,
}

/// 전체 디스크 한 대.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockDevice {
    pub name: String,
    pub size_bytes: u64,
    /// 회전 디스크(HDD)인가. 성능 기대치가 완전히 다르다.
    pub rotational: bool,
    pub removable: bool,
    pub model: String,
    pub partitions: Vec<Partition>,
    /// 이 장치를 재료로 쓰는 상위 장치(RAID·LVM 등).
    pub used_by: Vec<String>,
    /// 파티션 없이 통째로 마운트된 경우.
    pub mount_point: Option<String>,
    pub fstype: Option<String>,
}

impl BlockDevice {
    pub fn kind(&self) -> &'static str {
        if self.rotational { "HDD" } else { "SSD" }
    }
}

fn read_trimmed(ctx: &ProbeCtx, path: &str) -> Option<String> {
    ctx.read(path).map(|s| s.trim().to_string())
}

fn read_u64(ctx: &ProbeCtx, path: &str) -> Option<u64> {
    read_trimmed(ctx, path)?.parse().ok()
}

/// `/sys/block/<dev>/dev` 의 `major:minor`.
fn device_number(ctx: &ProbeCtx, path: &str) -> Option<(u32, u32)> {
    let raw = read_trimmed(ctx, path)?;
    let (major, minor) = raw.split_once(':')?;
    Some((major.parse().ok()?, minor.parse().ok()?))
}

/// 진단에서 의미 없는 가상 장치.
fn is_noise(name: &str) -> bool {
    name.starts_with("loop") || name.starts_with("ram") || name.starts_with("zram")
}

/// 장치 목록을 읽는다. 마운트 정보를 함께 넘기면 마운트 지점까지 채운다.
pub fn devices(ctx: &ProbeCtx, mounts: &[Mount]) -> Vec<BlockDevice> {
    let Ok(dir) = std::fs::read_dir(ctx.path("/sys/block")) else {
        return Vec::new();
    };
    let find_mount = |major: u32, minor: u32| -> Option<&Mount> {
        mounts.iter().find(|m| m.major == major && m.minor == minor)
    };

    let mut out = Vec::new();
    for entry in dir.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if is_noise(&name) {
            continue;
        }
        let base = format!("/sys/block/{name}");
        let size_bytes = read_u64(ctx, &format!("{base}/size")).unwrap_or(0) * SECTOR_BYTES;
        if size_bytes == 0 {
            continue;
        }

        // 모델 이름은 장치 종류에 따라 다른 파일에 있다.
        let model = read_trimmed(ctx, &format!("{base}/device/model"))
            .or_else(|| read_trimmed(ctx, &format!("{base}/dm/name")))
            .or_else(|| read_trimmed(ctx, &format!("{base}/device/name")))
            .unwrap_or_else(|| "unknown".into());

        let (mount_point, fstype) = device_number(ctx, &format!("{base}/dev"))
            .and_then(|(maj, min)| find_mount(maj, min))
            .map(|m| (Some(m.target.clone()), Some(m.fstype.clone())))
            .unwrap_or((None, None));

        // 이 장치를 재료로 쓰는 상위 장치(RAID·LVM).
        let used_by = std::fs::read_dir(ctx.path(&format!("{base}/holders")))
            .map(|d| {
                d.flatten()
                    .map(|e| {
                        let holder = e.file_name().to_string_lossy().to_string();
                        read_trimmed(ctx, &format!("/sys/block/{holder}/dm/name")).unwrap_or(holder)
                    })
                    .collect()
            })
            .unwrap_or_default();

        let mut partitions = Vec::new();
        if let Ok(children) = std::fs::read_dir(ctx.path(&base)) {
            for child in children.flatten() {
                let part_name = child.file_name().to_string_lossy().to_string();
                let part_base = format!("{base}/{part_name}");
                // 파티션 디렉터리만 `partition` 파일을 갖는다.
                if !ctx.exists(&format!("{part_base}/partition")) {
                    continue;
                }
                let part_size =
                    read_u64(ctx, &format!("{part_base}/size")).unwrap_or(0) * SECTOR_BYTES;
                let mount = device_number(ctx, &format!("{part_base}/dev"))
                    .and_then(|(maj, min)| find_mount(maj, min));
                partitions.push(Partition {
                    name: part_name,
                    size_bytes: part_size,
                    mount_point: mount.map(|m| m.target.clone()),
                    fstype: mount.map(|m| m.fstype.clone()),
                    read_only: mount.is_some_and(|m| m.is_read_only()),
                });
            }
        }
        partitions.sort_by(|a, b| a.name.cmp(&b.name));

        out.push(BlockDevice {
            name,
            size_bytes,
            rotational: read_u64(ctx, &format!("{base}/queue/rotational")) == Some(1),
            removable: read_u64(ctx, &format!("{base}/removable")) == Some(1),
            model,
            partitions,
            used_by,
            mount_point,
            fstype,
        });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// 소프트웨어 RAID 배열 한 건.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RaidArray {
    pub name: String,
    pub level: String,
    /// `[UU]` 처럼 커널이 보고하는 구성원 상태.
    pub state: String,
    pub members: Vec<String>,
    /// 재구성·검사 진행 중이면 그 설명.
    pub progress: Option<String>,
}

impl RaidArray {
    /// 구성원 중 하나라도 빠져 있는가(`[U_]`).
    pub fn degraded(&self) -> bool {
        self.state.contains('_')
    }
}

/// `/proc/mdstat` 를 파싱한다.
pub fn raid_arrays(ctx: &ProbeCtx) -> Vec<RaidArray> {
    let Some(text) = ctx.read("/proc/mdstat") else {
        return Vec::new();
    };
    let mut out: Vec<RaidArray> = Vec::new();
    for line in text.lines() {
        // 배열 헤더: `md0 : active raid1 sdb1[1] sda1[0]`
        if let Some((name, rest)) = line.split_once(" : ")
            && name.starts_with("md")
        {
            let mut tokens = rest.split_whitespace();
            let _active = tokens.next();
            let level = tokens.next().unwrap_or("unknown").to_string();
            let members = tokens.map(|t| t.to_string()).collect();
            out.push(RaidArray {
                name: name.trim().to_string(),
                level,
                state: String::new(),
                members,
                progress: None,
            });
            continue;
        }
        let Some(current) = out.last_mut() else {
            continue;
        };
        // 상태 줄: `      4189184 blocks super 1.2 [2/2] [UU]`
        if let Some(start) = line.rfind('[')
            && line.trim_end().ends_with(']')
            && line[start..].chars().all(|c| "[]U_".contains(c))
        {
            current.state = line[start..].trim().to_string();
        }
        if line.contains("recovery =") || line.contains("resync =") || line.contains("check =") {
            current.progress = Some(line.trim().to_string());
        }
    }
    out
}

pub struct BlockDeviceProbe;

impl Probe for BlockDeviceProbe {
    fn id(&self) -> &'static str {
        "storage.layout"
    }

    fn describe(&self) -> &'static str {
        "Drives, partitions, what is mounted where, and RAID state"
    }

    fn commands(&self) -> Vec<&'static str> {
        vec![
            "cat /sys/block/DEVICE/size",
            "cat /sys/block/DEVICE/queue/rotational",
            "cat /proc/mdstat",
        ]
    }

    fn run(&self, ctx: &ProbeCtx) -> ProbeResult {
        let mounts = super::mounts::mounts(ctx);
        let devices = devices(ctx, &mounts);
        if devices.is_empty() {
            return ProbeResult::unavailable(
                "storage.layout",
                Availability::ParseFailed {
                    reason: "/sys/block could not be listed".into(),
                },
            );
        }
        ProbeResult::ok(
            "storage.layout",
            ProbeData::Fields(
                devices
                    .iter()
                    .map(|d| {
                        Field::new(
                            d.name.clone(),
                            format!("{} {} partitions", d.kind(), d.partitions.len()),
                        )
                    })
                    .collect(),
            ),
        )
    }
}
