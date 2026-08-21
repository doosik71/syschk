//! 배경 작업.
//!
//! `/proc` 읽기는 밀리초 단위로 끝나지만, 디렉터리 용량 측정이나 드라이브 자기 진단은
//! 초 단위로 걸린다. 그런 수집을 UI 스레드에서 하면 화면이 멈춘다(NFR-2).
//!
//! 여기서는 표준 스레드와 채널만 쓴다. 비동기 런타임을 들이지 않은 이유는 이 작업들이
//! 네트워크 대기가 아니라 **블로킹 파일 작업**이고, 동시에 도는 수가 한 손에 꼽히기
//! 때문이다. 스레드 하나가 곧 작업 하나라 추적도 쉽다.

use crate::collect::deleted::DeletedFiles;
use crate::collect::dirsize::DirScan;
use crate::collect::logspace::LogFootprint;
use crate::collect::smart::DriveHealth;
use crate::collect::storage_errors::StorageErrors;
use crate::collect::{ProbeCtx, deleted, dirsize, logspace, smart, storage_errors};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Duration;

/// 배경으로 돌릴 작업.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Job {
    /// 디렉터리 용량 측정.
    DirScan(PathBuf),
    /// 모든 드라이브의 자기 진단(SMART).
    Drives,
    /// 로그 점유량.
    Logs,
    /// 지웠지만 열려 있는 파일.
    Deleted,
    /// 커널 로그의 저장장치 오류.
    KernelErrors,
}

impl Job {
    /// 진행 중임을 표시할 문구.
    pub fn describe(&self) -> String {
        match self {
            Job::DirScan(path) => format!("measuring {}", path.display()),
            Job::Drives => "asking each drive for its own health report".into(),
            Job::Logs => "measuring log space".into(),
            Job::Deleted => "looking for deleted files still held open".into(),
            Job::KernelErrors => "searching the kernel log for storage errors".into(),
        }
    }

    /// 같은 작업이 이미 돌고 있는지 판단할 열쇠. 경로가 다른 측정은 다른 작업이다.
    fn key(&self) -> String {
        match self {
            Job::DirScan(path) => format!("dir:{}", path.display()),
            Job::Drives => "drives".into(),
            Job::Logs => "logs".into(),
            Job::Deleted => "deleted".into(),
            Job::KernelErrors => "errors".into(),
        }
    }
}

/// 작업 결과.
#[derive(Debug)]
pub enum JobOutput {
    DirScan(DirScan),
    Drives(Vec<DriveHealth>),
    Logs(LogFootprint),
    Deleted(DeletedFiles),
    KernelErrors(StorageErrors),
}

/// 배경 작업 관리자.
pub struct Jobs {
    tx: Sender<(String, JobOutput)>,
    rx: Receiver<(String, JobOutput)>,
    running: Vec<(String, Job)>,
}

impl Default for Jobs {
    fn default() -> Self {
        Self::new()
    }
}

impl Jobs {
    pub fn new() -> Self {
        let (tx, rx) = channel();
        Self {
            tx,
            rx,
            running: Vec::new(),
        }
    }

    /// 작업을 시작한다. 같은 작업이 이미 돌고 있으면 아무 것도 하지 않는다.
    pub fn request(&mut self, job: Job, ctx: &ProbeCtx) {
        let key = job.key();
        if self.running.iter().any(|(k, _)| *k == key) {
            return;
        }
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        let spawned = job.clone();
        let key_for_thread = key.clone();
        // 스레드를 만들 수 없는 상황에서도 앱은 계속 동작해야 한다.
        let handle = std::thread::Builder::new()
            .name("syschk-collect".into())
            .spawn(move || {
                let output = run(&spawned, &ctx);
                // 수신 측이 이미 사라졌다면(종료 중) 조용히 버린다.
                let _ = tx.send((key_for_thread, output));
            });
        if handle.is_ok() {
            self.running.push((key, job));
        }
    }

    /// 끝난 작업 결과를 가져온다.
    pub fn drain(&mut self) -> Vec<JobOutput> {
        let mut out = Vec::new();
        while let Ok((key, output)) = self.rx.try_recv() {
            self.running.retain(|(k, _)| *k != key);
            out.push(output);
        }
        out
    }

    pub fn busy(&self) -> bool {
        !self.running.is_empty()
    }

    /// 진행 중인 작업 설명. 사용자에게 무엇을 기다리는지 알린다.
    pub fn in_progress(&self) -> Vec<String> {
        self.running.iter().map(|(_, job)| job.describe()).collect()
    }
}

fn run(job: &Job, ctx: &ProbeCtx) -> JobOutput {
    match job {
        // 거대한 트리에서 무한정 돌지 않도록 예산을 둔다. 중단되면 결과가 하한값임을 알린다.
        Job::DirScan(path) => JobOutput::DirScan(dirsize::scan(path, Duration::from_secs(8))),
        Job::Drives => JobOutput::Drives(smart::read_all(ctx)),
        Job::Logs => JobOutput::Logs(logspace::footprint(ctx, Duration::from_secs(10))),
        Job::Deleted => JobOutput::Deleted(deleted::scan(ctx)),
        Job::KernelErrors => JobOutput::KernelErrors(storage_errors::scan(0)),
    }
}
