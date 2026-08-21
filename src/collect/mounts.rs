//! 마운트 목록과 사용량.
//!
//! 마운트 목록은 `/proc/self/mountinfo` 에서 읽고, 용량과 inode 수는 `statvfs` 로 얻는다.
//! 커널은 여유 용량을 `/proc` 로 노출하지 않기 때문에 이 한 가지만 libc 를 쓴다.
//!
//! 초보자가 가장 많이 겪는 두 가지 함정을 이 모듈이 구분해 준다.
//!
//! * **예약 블록** — `free` 는 남아 있는데 쓸 수 없는 공간. root 를 위해 예약되어 있다.
//! * **inode 고갈** — 용량은 남았는데 파일을 못 만드는 상태.

use super::{Availability, Field, Probe, ProbeCtx, ProbeData, ProbeResult};
use std::path::Path;

/// 마운트 한 건.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mount {
    /// 장치(또는 그에 준하는 출처).
    pub source: String,
    /// 마운트 지점.
    pub target: String,
    pub fstype: String,
    /// 마운트 단위 옵션과 수퍼블록 옵션을 합친 것.
    ///
    /// `errors=remount-ro` 처럼 중요한 값이 수퍼블록 쪽에만 있는 경우가 있어,
    /// `findmnt` 와 같이 양쪽을 함께 다룬다.
    pub options: Vec<String>,
    /// 장치 번호. 블록 장치와 연결할 때 쓴다.
    pub major: u32,
    pub minor: u32,
}

impl Mount {
    /// 읽기 전용으로 마운트되어 있는가. 커널이 오류를 만나 강제 전환한 결과일 수 있다.
    pub fn is_read_only(&self) -> bool {
        self.options.iter().any(|o| o == "ro")
    }

    /// 사용자가 실제로 신경 쓰는 파일시스템인가.
    ///
    /// 커널 가상 파일시스템과 snap 이미지(squashfs)는 용량이 항상 100% 이므로 제외한다.
    /// 그대로 보여주면 "꽉 찬 파일시스템" 목록이 노이즈로 뒤덮인다.
    pub fn is_user_filesystem(&self) -> bool {
        const PSEUDO: &[&str] = &[
            "autofs",
            "bpf",
            "binfmt_misc",
            "cgroup",
            "cgroup2",
            "configfs",
            "debugfs",
            "devpts",
            "devtmpfs",
            "efivarfs",
            "fuse.gvfsd-fuse",
            "fuse.portal",
            "fusectl",
            "hugetlbfs",
            "mqueue",
            "proc",
            "pstore",
            "ramfs",
            "securityfs",
            "sysfs",
            "tracefs",
        ];
        if PSEUDO.contains(&self.fstype.as_str()) {
            return false;
        }
        // snap 패키지는 읽기 전용 이미지라 늘 100% 다.
        if self.fstype == "squashfs" || self.target.starts_with("/snap/") {
            return false;
        }
        true
    }
}

/// `statvfs` 로 얻은 파일시스템 사용량.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Usage {
    pub total_bytes: u64,
    /// 완전히 빈 공간(예약 블록 포함).
    pub free_bytes: u64,
    /// 일반 사용자가 실제로 쓸 수 있는 공간.
    pub available_bytes: u64,
    pub inodes_total: u64,
    pub inodes_free: u64,
}

impl Usage {
    pub fn used_bytes(&self) -> u64 {
        self.total_bytes.saturating_sub(self.free_bytes)
    }

    /// `df` 가 보여주는 사용률과 같은 정의(예약 블록을 사용 중으로 본다).
    pub fn used_pct(&self) -> f32 {
        let denominator = self.used_bytes() + self.available_bytes;
        if denominator == 0 {
            0.0
        } else {
            self.used_bytes() as f32 / denominator as f32 * 100.0
        }
    }

    /// root 에게만 남겨진 예약 공간. "남았는데 쓸 수 없는" 양이다.
    pub fn reserved_bytes(&self) -> u64 {
        self.free_bytes.saturating_sub(self.available_bytes)
    }

    pub fn inodes_used(&self) -> u64 {
        self.inodes_total.saturating_sub(self.inodes_free)
    }

    /// inode 사용률. inode 개념이 없는 파일시스템(btrfs 등)은 `None`.
    pub fn inodes_used_pct(&self) -> Option<f32> {
        if self.inodes_total == 0 {
            None
        } else {
            Some(self.inodes_used() as f32 / self.inodes_total as f32 * 100.0)
        }
    }
}

/// 마운트 + 사용량. 사용량을 얻지 못하면 그 사실을 남긴다.
#[derive(Clone, Debug)]
pub struct MountUsage {
    pub mount: Mount,
    pub usage: Option<Usage>,
}

impl MountUsage {
    pub fn used_pct(&self) -> f32 {
        self.usage.map_or(0.0, |u| u.used_pct())
    }
}

/// `/proc/self/mountinfo` 를 파싱한다.
///
/// 형식: `36 35 98:0 /mnt1 /mnt2 rw,noatime master:1 - ext3 /dev/root rw,errors=continue`
/// 선택 필드가 가변이라 `-` 를 기준으로 앞뒤를 나눈다.
pub fn parse_mountinfo(text: &str) -> Vec<Mount> {
    let mut out = Vec::new();
    for line in text.lines() {
        let Some((head, tail)) = line.split_once(" - ") else {
            continue;
        };
        let head: Vec<&str> = head.split_whitespace().collect();
        let tail: Vec<&str> = tail.split_whitespace().collect();
        if head.len() < 6 || tail.len() < 2 {
            continue;
        }
        let (major, minor) = head[2]
            .split_once(':')
            .and_then(|(a, b)| Some((a.parse().ok()?, b.parse().ok()?)))
            .unwrap_or((0, 0));
        let mut options: Vec<String> = head[5].split(',').map(str::to_string).collect();
        if let Some(super_options) = tail.get(2) {
            for option in super_options.split(',') {
                if !options.iter().any(|o| o == option) {
                    options.push(option.to_string());
                }
            }
        }
        out.push(Mount {
            source: unescape(tail[1]),
            target: unescape(head[4]),
            fstype: tail[0].to_string(),
            options,
            major,
            minor,
        });
    }
    out
}

/// mountinfo 는 공백 등을 8진 이스케이프로 적는다(`\040`).
fn unescape(s: &str) -> String {
    if !s.contains('\\') {
        return s.to_string();
    }
    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 3 < bytes.len() {
            let digits = &s[i + 1..i + 4];
            if let Ok(code) = u8::from_str_radix(digits, 8) {
                out.push(code as char);
                i += 4;
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

/// 마운트 목록을 읽는다.
pub fn mounts(ctx: &ProbeCtx) -> Vec<Mount> {
    ctx.read("/proc/self/mountinfo")
        .map(|t| parse_mountinfo(&t))
        .unwrap_or_default()
}

/// 한 경로의 파일시스템 사용량을 `statvfs` 로 읽는다.
pub fn usage_of(path: &Path) -> Option<Usage> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let c_path = CString::new(path.as_os_str().as_bytes()).ok()?;
    // SAFETY: 유효한 C 문자열과 초기화된 구조체 포인터를 넘긴다. 실패는 반환값으로 확인한다.
    let stat = unsafe {
        let mut stat: libc::statvfs = std::mem::zeroed();
        if libc::statvfs(c_path.as_ptr(), &mut stat) != 0 {
            return None;
        }
        stat
    };
    // f_frsize 가 0 인 파일시스템도 있어 f_bsize 로 보완한다.
    let block = if stat.f_frsize > 0 {
        stat.f_frsize as u64
    } else {
        stat.f_bsize as u64
    };
    Some(Usage {
        total_bytes: stat.f_blocks as u64 * block,
        free_bytes: stat.f_bfree as u64 * block,
        available_bytes: stat.f_bavail as u64 * block,
        inodes_total: stat.f_files as u64,
        inodes_free: stat.f_ffree as u64,
    })
}

/// 사용자가 신경 쓰는 파일시스템의 사용량을 모은다.
///
/// 같은 파일시스템이 여러 곳에 마운트된 경우(bind mount) 첫 번째만 남긴다.
pub fn usage(ctx: &ProbeCtx) -> Vec<MountUsage> {
    let mut seen = Vec::new();
    let mut out = Vec::new();
    for mount in mounts(ctx) {
        if !mount.is_user_filesystem() {
            continue;
        }
        let key = (mount.major, mount.minor);
        if seen.contains(&key) {
            continue;
        }
        seen.push(key);
        let usage = usage_of(&ctx.path(&mount.target));
        // 크기가 0 인 것은 볼 것이 없다.
        if usage.map(|u| u.total_bytes) == Some(0) {
            continue;
        }
        out.push(MountUsage { mount, usage });
    }
    out.sort_by(|a, b| {
        b.used_pct()
            .partial_cmp(&a.used_pct())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    out
}

pub struct MountProbe;

impl Probe for MountProbe {
    fn id(&self) -> &'static str {
        "storage.mounts"
    }

    fn describe(&self) -> &'static str {
        "Space and inode usage per mounted filesystem"
    }

    fn commands(&self) -> Vec<&'static str> {
        vec!["cat /proc/self/mountinfo"]
    }

    fn run(&self, ctx: &ProbeCtx) -> ProbeResult {
        let list = usage(ctx);
        if list.is_empty() {
            return ProbeResult::unavailable(
                "storage.mounts",
                Availability::ParseFailed {
                    reason: "no mounted filesystems could be read".into(),
                },
            );
        }
        ProbeResult::ok(
            "storage.mounts",
            ProbeData::Fields(
                list.iter()
                    .map(|m| {
                        Field::new(m.mount.target.clone(), format!("{:.1}% used", m.used_pct()))
                    })
                    .collect(),
            ),
        )
    }
}
