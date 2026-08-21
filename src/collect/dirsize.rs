//! 디렉터리 용량 조사 — "무엇이 공간을 먹고 있나".
//!
//! `du` 와 같은 계산을 직접 한다. 세 가지를 `du` 처럼 지킨다.
//!
//! * 실제 점유 블록(`st_blocks`)을 센다. 희소 파일은 파일 크기보다 적게 차지한다.
//! * 하드링크로 연결된 같은 파일은 한 번만 센다.
//! * 다른 파일시스템으로 넘어가지 않는다(`du -x`). 넘어가면 같은 공간을 두 번 세게 된다.
//!
//! 큰 트리는 오래 걸리므로 시간 예산을 받고, 예산을 넘기면 "여기까지"라고 알린다.
//! 조용히 잘라 놓고 다 센 척하지 않는다.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// 조사한 디렉터리의 바로 아래 항목 하나.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub name: String,
    pub path: PathBuf,
    pub bytes: u64,
    pub is_dir: bool,
    /// 디렉터리인 경우 그 안의 항목 수.
    pub items: u64,
}

/// 조사 결과.
#[derive(Clone, Debug, Default)]
pub struct DirScan {
    pub path: PathBuf,
    pub total_bytes: u64,
    /// 바로 아래 항목을 큰 것부터.
    pub entries: Vec<Entry>,
    pub files_counted: u64,
    /// 권한이 없어 들어가 보지 못한 디렉터리 수.
    pub denied: u64,
    /// 다른 파일시스템이라 건너뛴 디렉터리(마운트 지점).
    pub crossed_mounts: Vec<String>,
    /// 시간 예산을 넘겨 중단했는가. 이 경우 합계는 하한값이다.
    pub truncated: bool,
    pub elapsed: Duration,
}

impl DirScan {
    /// 부모 디렉터리 경로.
    pub fn parent(&self) -> Option<PathBuf> {
        self.path.parent().map(Path::to_path_buf)
    }
}

/// 조사 중 상태.
struct Walk {
    root_device: u64,
    deadline: Instant,
    seen_hardlinks: HashSet<(u64, u64)>,
    files: u64,
    denied: u64,
    crossed: Vec<String>,
    truncated: bool,
}

/// 한 디렉터리를 조사한다. `budget` 을 넘기면 중단하고 `truncated` 로 알린다.
pub fn scan(path: &Path, budget: Duration) -> DirScan {
    let started = Instant::now();
    let mut result = DirScan {
        path: path.to_path_buf(),
        ..Default::default()
    };

    let Ok(root_meta) = std::fs::metadata(path) else {
        result.elapsed = started.elapsed();
        return result;
    };
    use std::os::unix::fs::MetadataExt;

    let mut walk = Walk {
        root_device: root_meta.dev(),
        deadline: started + budget,
        seen_hardlinks: HashSet::new(),
        files: 0,
        denied: 0,
        crossed: Vec::new(),
        truncated: false,
    };

    let Ok(dir) = std::fs::read_dir(path) else {
        result.denied = 1;
        result.elapsed = started.elapsed();
        return result;
    };

    for child in dir.flatten() {
        let child_path = child.path();
        let Ok(meta) = std::fs::symlink_metadata(&child_path) else {
            continue;
        };
        if meta.is_dir() {
            if meta.dev() != walk.root_device {
                walk.crossed.push(child_path.display().to_string());
                continue;
            }
            let (bytes, items) = walk_dir(&child_path, &mut walk);
            result.entries.push(Entry {
                name: child.file_name().to_string_lossy().to_string(),
                path: child_path,
                bytes,
                is_dir: true,
                items,
            });
        } else if !meta.is_symlink() {
            let bytes = file_bytes(&meta, &mut walk);
            walk.files += 1;
            result.entries.push(Entry {
                name: child.file_name().to_string_lossy().to_string(),
                path: child_path,
                bytes,
                is_dir: false,
                items: 0,
            });
        }
        if Instant::now() >= walk.deadline {
            walk.truncated = true;
            break;
        }
    }

    result.total_bytes = result.entries.iter().map(|e| e.bytes).sum();
    result.entries.sort_by(|a, b| b.bytes.cmp(&a.bytes));
    result.files_counted = walk.files;
    result.denied = walk.denied;
    result.crossed_mounts = walk.crossed;
    result.truncated = walk.truncated;
    result.elapsed = started.elapsed();
    result
}

/// 디렉터리 하나를 재귀로 합산한다. `(바이트, 항목 수)`.
fn walk_dir(path: &Path, walk: &mut Walk) -> (u64, u64) {
    use std::os::unix::fs::MetadataExt;

    if Instant::now() >= walk.deadline {
        walk.truncated = true;
        return (0, 0);
    }
    let Ok(dir) = std::fs::read_dir(path) else {
        walk.denied += 1;
        return (0, 0);
    };

    let mut bytes = 0;
    let mut items = 0;
    for child in dir.flatten() {
        let Ok(meta) = std::fs::symlink_metadata(child.path()) else {
            continue;
        };
        items += 1;
        if meta.is_dir() {
            if meta.dev() != walk.root_device {
                walk.crossed.push(child.path().display().to_string());
                continue;
            }
            let (sub_bytes, sub_items) = walk_dir(&child.path(), walk);
            bytes += sub_bytes;
            items += sub_items;
        } else if !meta.is_symlink() {
            bytes += file_bytes(&meta, walk);
            walk.files += 1;
        }
        if Instant::now() >= walk.deadline {
            walk.truncated = true;
            break;
        }
    }
    (bytes, items)
}

/// 파일이 실제로 차지하는 바이트. 하드링크는 한 번만 센다.
fn file_bytes(meta: &std::fs::Metadata, walk: &mut Walk) -> u64 {
    use std::os::unix::fs::MetadataExt;
    if meta.nlink() > 1 && !walk.seen_hardlinks.insert((meta.dev(), meta.ino())) {
        return 0;
    }
    meta.blocks() * 512
}
