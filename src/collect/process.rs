//! 프로세스 목록.
//!
//! `/proc/<pid>/stat` 의 누적 CPU 시간을 두 표본으로 나눠 사용률을 만든다.
//! `top` 이 하는 계산과 같다(`USER_HZ` = 100 을 가정하며, 이는 리눅스에서 고정값이다).
//!
//! 다른 사용자의 프로세스는 I/O 통계를 읽을 수 없다. 그런 경우 값을 0 으로 꾸미지 않고
//! "읽을 수 없음"으로 남긴다(원칙: 모르는 것은 모른다고 말한다).

use super::{Availability, Field, Probe, ProbeCtx, ProbeData, ProbeResult};
use std::collections::{HashMap, HashSet};

/// 리눅스의 clock tick. `/proc` 의 CPU 시간 단위다.
const USER_HZ: f32 = 100.0;

/// 프로세스 하나의 누적값.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProcTimes {
    pub utime: u64,
    pub stime: u64,
    pub read_bytes: Option<u64>,
    pub write_bytes: Option<u64>,
}

/// 프로세스 하나의 한 시점 상태.
#[derive(Clone, Debug)]
pub struct ProcInfo {
    pub pid: u32,
    pub ppid: u32,
    pub uid: u32,
    pub state: char,
    pub rss_kb: u64,
    pub threads: u32,
    pub cmd: String,
    /// D 상태(중단 불가 대기)일 때 커널이 기다리는 지점.
    pub wchan: Option<String>,
    pub times: ProcTimes,
}

/// 한 시점의 전체 프로세스 목록.
#[derive(Clone, Debug, Default)]
pub struct ProcSnapshot {
    pub procs: Vec<ProcInfo>,
    /// I/O 통계를 하나도 읽을 수 없었다면(비권한 실행) 그 사실을 알린다.
    pub io_readable: bool,
}

/// 표본마다 다시 읽지 않아도 되는 값을 기억한다.
///
/// 프로세스가 수백 개인 시스템에서 1초마다 모든 파일을 다시 읽으면 진단 도구가
/// 진단 대상에 부담을 준다(NFR-1). 명령줄은 프로세스가 사는 동안 바뀌지 않고,
/// 권한이 없어 못 읽는 I/O 통계는 다음에도 못 읽는다.
#[derive(Debug, Default)]
pub struct ProcCache {
    /// pid → (시작 시각, 명령줄, uid). 시작 시각이 다르면 pid 가 재사용된 것이다.
    identity: HashMap<u32, (u64, String, u32)>,
    /// I/O 통계를 읽을 수 없던 pid.
    io_denied: HashSet<u32>,
}

impl ProcCache {
    /// 사라진 프로세스의 항목을 정리한다.
    fn retain(&mut self, live: &[ProcInfo]) {
        let alive: HashSet<u32> = live.iter().map(|p| p.pid).collect();
        self.identity.retain(|pid, _| alive.contains(pid));
        self.io_denied.retain(|pid| alive.contains(pid));
    }
}

impl ProcSnapshot {
    /// 캐시 없이 읽는다(수집기와 시험에서 쓴다).
    pub fn read(ctx: &ProbeCtx) -> Option<Self> {
        let mut cache = ProcCache::default();
        Self::read_cached(ctx, &mut cache)
    }

    pub fn read_cached(ctx: &ProbeCtx, cache: &mut ProcCache) -> Option<Self> {
        let dir = std::fs::read_dir(ctx.path("/proc")).ok()?;
        let mut procs = Vec::new();
        let mut io_readable = false;
        for entry in dir.flatten() {
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            let Ok(pid) = name.parse::<u32>() else {
                continue;
            };
            let Some(info) = read_one(ctx, pid, cache) else {
                continue;
            };
            if info.times.read_bytes.is_some() {
                io_readable = true;
            }
            procs.push(info);
        }
        cache.retain(&procs);
        (!procs.is_empty()).then_some(ProcSnapshot { procs, io_readable })
    }
}

fn read_one(ctx: &ProbeCtx, pid: u32, cache: &mut ProcCache) -> Option<ProcInfo> {
    let stat = ctx.read(&format!("/proc/{pid}/stat"))?;
    // comm 에 공백과 괄호가 들어갈 수 있으므로 마지막 ')' 뒤부터 파싱한다.
    let close = stat.rfind(')')?;
    let comm = stat.get(stat.find('(')? + 1..close)?.to_string();
    let rest: Vec<&str> = stat.get(close + 2..)?.split_whitespace().collect();
    if rest.len() < 22 {
        return None;
    }
    let num = |i: usize| rest.get(i).and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);

    let state = rest[0].chars().next().unwrap_or('?');
    let ppid = num(1) as u32;
    let utime = num(11);
    let stime = num(12);
    let threads = num(17) as u32;
    let starttime = num(19);
    // 필드 24(rss)는 페이지 단위다.
    let rss_pages = num(21);

    // 명령줄과 소유자는 프로세스가 사는 동안 바뀌지 않으므로 한 번만 읽는다.
    let (cmd, uid) = match cache.identity.get(&pid) {
        Some((cached_start, cmd, uid)) if *cached_start == starttime => (cmd.clone(), *uid),
        _ => {
            let cmd = ctx
                .read(&format!("/proc/{pid}/cmdline"))
                .map(|raw| {
                    raw.split('\0')
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| format!("[{comm}]"));
            let uid = uid_of(ctx, pid);
            cache.identity.insert(pid, (starttime, cmd.clone(), uid));
            (cmd, uid)
        }
    };

    // 다른 사용자의 프로세스는 읽을 수 없다. 그 사실을 Option 으로 남기고,
    // 한 번 거부된 pid 는 다시 시도하지 않는다.
    let (read_bytes, write_bytes) = if cache.io_denied.contains(&pid) {
        (None, None)
    } else {
        match ctx.read(&format!("/proc/{pid}/io")) {
            Some(io) => {
                let field = |key: &str| {
                    io.lines()
                        .find_map(|l| l.strip_prefix(key))
                        .and_then(|v| v.trim().parse::<u64>().ok())
                };
                (field("read_bytes:"), field("write_bytes:"))
            }
            None => {
                cache.io_denied.insert(pid);
                (None, None)
            }
        }
    };

    let wchan = if state == 'D' {
        ctx.read(&format!("/proc/{pid}/wchan"))
            .map(|w| w.trim().to_string())
            .filter(|w| !w.is_empty() && w != "0")
    } else {
        None
    };

    Some(ProcInfo {
        pid,
        ppid,
        uid,
        state,
        rss_kb: rss_pages * 4, // 페이지 크기 4KiB 가정(x86_64/aarch64 기본)
        threads,
        cmd,
        wchan,
        times: ProcTimes {
            utime,
            stime,
            read_bytes,
            write_bytes,
        },
    })
}

fn uid_of(ctx: &ProbeCtx, pid: u32) -> u32 {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata(ctx.path(&format!("/proc/{pid}")))
        .map(|m| m.uid())
        .unwrap_or(0)
}

/// uid → 사용자 이름. `/etc/passwd` 를 한 번만 읽는다(외부 명령 없이).
pub fn user_names(ctx: &ProbeCtx) -> HashMap<u32, String> {
    let mut map = HashMap::new();
    if let Some(text) = ctx.read("/etc/passwd") {
        for line in text.lines() {
            let f: Vec<&str> = line.split(':').collect();
            if f.len() > 2
                && let Ok(uid) = f[2].parse::<u32>()
            {
                map.insert(uid, f[0].to_string());
            }
        }
    }
    map
}

/// 표시용 프로세스 한 줄.
#[derive(Clone, Debug, PartialEq)]
pub struct ProcRow {
    pub pid: u32,
    pub ppid: u32,
    pub user: String,
    pub state: char,
    pub cpu_pct: f32,
    pub mem_pct: f32,
    pub rss_kb: u64,
    /// 권한이 없어 읽지 못하면 `None`.
    pub read_bps: Option<f32>,
    pub write_bps: Option<f32>,
    pub threads: u32,
    pub cmd: String,
    pub wchan: Option<String>,
}

impl ProcRow {
    /// I/O 를 기다리며 멈춰 있는가.
    pub fn is_blocked(&self) -> bool {
        self.state == 'D'
    }

    pub fn io_bps(&self) -> f32 {
        self.read_bps.unwrap_or(0.0) + self.write_bps.unwrap_or(0.0)
    }
}

/// 두 표본에서 표시용 목록을 만든다.
pub fn rows(
    prev: &ProcSnapshot,
    now: &ProcSnapshot,
    secs: f32,
    mem_total_kb: u64,
    users: &HashMap<u32, String>,
) -> Vec<ProcRow> {
    if secs <= 0.0 {
        return Vec::new();
    }
    let before: HashMap<u32, &ProcTimes> = prev.procs.iter().map(|p| (p.pid, &p.times)).collect();

    now.procs
        .iter()
        .map(|p| {
            let cpu_pct = before
                .get(&p.pid)
                .map(|old| {
                    let ticks =
                        (p.times.utime + p.times.stime).saturating_sub(old.utime + old.stime);
                    ticks as f32 / USER_HZ / secs * 100.0
                })
                .unwrap_or(0.0);
            let rate = |now_v: Option<u64>, old_v: Option<u64>| match (now_v, old_v) {
                (Some(n), Some(o)) => Some(n.saturating_sub(o) as f32 / secs),
                (Some(_), None) => Some(0.0),
                _ => None,
            };
            let old_io = before.get(&p.pid);
            ProcRow {
                pid: p.pid,
                ppid: p.ppid,
                user: users
                    .get(&p.uid)
                    .cloned()
                    .unwrap_or_else(|| p.uid.to_string()),
                state: p.state,
                cpu_pct,
                mem_pct: if mem_total_kb == 0 {
                    0.0
                } else {
                    p.rss_kb as f32 / mem_total_kb as f32 * 100.0
                },
                rss_kb: p.rss_kb,
                read_bps: rate(p.times.read_bytes, old_io.and_then(|o| o.read_bytes)),
                write_bps: rate(p.times.write_bytes, old_io.and_then(|o| o.write_bytes)),
                threads: p.threads,
                cmd: p.cmd.clone(),
                wchan: p.wchan.clone(),
            }
        })
        .collect()
}

pub struct ProcessProbe;

impl Probe for ProcessProbe {
    fn id(&self) -> &'static str {
        "process.list"
    }

    fn describe(&self) -> &'static str {
        "Per-process CPU, memory, I/O and wait state"
    }

    fn commands(&self) -> Vec<&'static str> {
        vec![
            "cat /proc/PID/stat",
            "cat /proc/PID/io",
            "cat /proc/PID/wchan",
        ]
    }

    fn run(&self, ctx: &ProbeCtx) -> ProbeResult {
        let Some(snap) = ProcSnapshot::read(ctx) else {
            return ProbeResult::unavailable(
                "process.list",
                Availability::ParseFailed {
                    reason: "/proc could not be listed".into(),
                },
            );
        };
        let blocked = snap.procs.iter().filter(|p| p.state == 'D').count();
        ProbeResult::ok(
            "process.list",
            ProbeData::Fields(vec![
                Field::new("processes", snap.procs.len().to_string()),
                Field::new("blocked", blocked.to_string()),
                Field::new(
                    "io_visible",
                    if snap.io_readable { "yes" } else { "no" }.to_string(),
                ),
            ]),
        )
    }
}
