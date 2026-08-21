//! 삭제했는데 돌아오지 않는 공간.
//!
//! 파일을 지워도 그 파일을 열고 있는 프로세스가 있으면 공간이 반환되지 않는다.
//! 초보자가 가장 당황하는 상황이다 — `rm` 을 했는데 `df` 가 그대로다.
//!
//! `lsof +L1` 이 하는 일을 `/proc/<pid>/fd` 를 직접 읽어서 한다. 커널은 삭제된 파일의
//! 링크 대상 뒤에 ` (deleted)` 를 붙여 준다.
//!
//! 다른 사용자의 프로세스는 볼 수 없다. 그 경우 "여기까지만 보였다"고 알린다.

use super::{Availability, Field, Probe, ProbeCtx, ProbeData, ProbeResult};

/// 삭제되었지만 아직 열려 있는 파일 하나.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeletedFile {
    pub pid: u32,
    /// 프로세스 이름(명령줄이 아니라 `comm`).
    pub process: String,
    /// 지워진 파일의 경로.
    pub path: String,
    /// 아직 붙잡고 있는 바이트. 읽을 수 없으면 `None`.
    pub bytes: Option<u64>,
}

/// 조사 결과.
#[derive(Clone, Debug, Default)]
pub struct DeletedFiles {
    pub files: Vec<DeletedFile>,
    /// 볼 수 있었던 프로세스 수.
    pub processes_scanned: u32,
    /// 권한이 없어 들여다보지 못한 프로세스 수.
    pub processes_denied: u32,
}

impl DeletedFiles {
    /// 붙잡혀 있는 총 바이트(읽을 수 있었던 것만).
    pub fn held_bytes(&self) -> u64 {
        self.files.iter().filter_map(|f| f.bytes).sum()
    }

    /// 프로세스별 합계를 큰 것부터.
    pub fn by_process(&self) -> Vec<(u32, String, u64, usize)> {
        let mut grouped: Vec<(u32, String, u64, usize)> = Vec::new();
        for file in &self.files {
            match grouped.iter_mut().find(|g| g.0 == file.pid) {
                Some(existing) => {
                    existing.2 += file.bytes.unwrap_or(0);
                    existing.3 += 1;
                }
                None => grouped.push((file.pid, file.process.clone(), file.bytes.unwrap_or(0), 1)),
            }
        }
        grouped.sort_by(|a, b| b.2.cmp(&a.2));
        grouped
    }

    /// 권한 때문에 결과가 불완전한가.
    pub fn partial(&self) -> bool {
        self.processes_denied > 0
    }
}

/// 삭제되었지만 열려 있는 파일을 찾는다.
pub fn scan(ctx: &ProbeCtx) -> DeletedFiles {
    let mut result = DeletedFiles::default();
    let Ok(dir) = std::fs::read_dir(ctx.path("/proc")) else {
        return result;
    };

    for entry in dir.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        let Ok(pid) = name.parse::<u32>() else {
            continue;
        };

        let fd_dir = ctx.path(&format!("/proc/{pid}/fd"));
        let Ok(fds) = std::fs::read_dir(&fd_dir) else {
            result.processes_denied += 1;
            continue;
        };
        result.processes_scanned += 1;

        let process = ctx
            .read(&format!("/proc/{pid}/comm"))
            .map(|c| c.trim().to_string())
            .unwrap_or_else(|| format!("pid {pid}"));

        for fd in fds.flatten() {
            let fd_path = fd.path();
            let Ok(target) = std::fs::read_link(&fd_path) else {
                continue;
            };
            let target = target.to_string_lossy().to_string();
            let Some(path) = target.strip_suffix(" (deleted)") else {
                continue;
            };
            // 메모리 매핑이나 임시 소켓은 공간과 무관하다.
            if path.starts_with("/memfd:") || path.starts_with("/dev/") {
                continue;
            }
            // 링크를 따라가면 아직 살아 있는 파일에 닿으므로 크기를 알 수 있다.
            let bytes = std::fs::metadata(&fd_path).ok().map(|m| m.len());
            result.files.push(DeletedFile {
                pid,
                process: process.clone(),
                path: path.to_string(),
                bytes,
            });
        }
    }

    result
        .files
        .sort_by(|a, b| b.bytes.unwrap_or(0).cmp(&a.bytes.unwrap_or(0)));
    result
}

pub struct DeletedFilesProbe;

impl Probe for DeletedFilesProbe {
    fn id(&self) -> &'static str {
        "storage.deleted"
    }

    fn describe(&self) -> &'static str {
        "Files that were deleted but are still held open, so their space has not come back"
    }

    fn commands(&self) -> Vec<&'static str> {
        vec!["ls /proc/PID/fd"]
    }

    fn run(&self, ctx: &ProbeCtx) -> ProbeResult {
        let found = scan(ctx);
        if found.processes_scanned == 0 {
            return ProbeResult::unavailable(
                "storage.deleted",
                Availability::ParseFailed {
                    reason: "/proc could not be listed".into(),
                },
            );
        }
        let mut result = ProbeResult::ok(
            "storage.deleted",
            ProbeData::Fields(vec![
                Field::new("files", found.files.len().to_string()),
                Field::new("held_bytes", found.held_bytes().to_string()),
                Field::new("denied", found.processes_denied.to_string()),
            ]),
        );
        // 권한이 없으면 결과가 불완전하다는 사실을 숨기지 않는다.
        if found.partial() {
            result.availability = Availability::NeedsPrivilege {
                hint: format!(
                    "{} process(es) could not be inspected - run with sudo to see all of them",
                    found.processes_denied
                ),
            };
        }
        result
    }
}
