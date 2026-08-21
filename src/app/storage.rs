//! 저장 공간 화면의 상태.
//!
//! 빠른 자료(마운트·장치 구성)는 화면에 들어올 때 바로 읽고, 느린 자료(디렉터리 용량,
//! 드라이브 자기 진단, 로그 측정)는 [`super::jobs`] 로 배경에서 받아온다.
//! 어떤 작업을 볼 때 무엇이 필요한지는 작업 id 로 결정한다.

use super::jobs::{Job, JobOutput, Jobs};
use crate::analyze::storage::{self, SpaceDiagnosis};
use crate::collect::ProbeCtx;
use crate::collect::blockdev::{self, BlockDevice, RaidArray};
use crate::collect::deleted::DeletedFiles;
use crate::collect::dirsize::DirScan;
use crate::collect::fsinfo::{self, FsHealth};
use crate::collect::logspace::LogFootprint;
use crate::collect::mounts::{self, MountUsage};
use crate::collect::smart::DriveHealth;
use crate::collect::storage_errors::StorageErrors;
use std::path::PathBuf;

/// 저장 공간 화면이 들고 있는 자료.
#[derive(Default)]
pub struct StorageState {
    pub mounts: Vec<MountUsage>,
    pub devices: Vec<BlockDevice>,
    pub raid: Vec<RaidArray>,
    pub filesystems: Vec<FsHealth>,
    /// 디렉터리 용량 측정 결과와 현재 보고 있는 경로.
    pub dir: Option<DirScan>,
    pub dir_path: PathBuf,
    pub drives: Vec<DriveHealth>,
    pub drives_loaded: bool,
    pub logs: Option<LogFootprint>,
    pub deleted: Option<DeletedFiles>,
    pub errors: Option<StorageErrors>,
    /// 빠른 자료를 한 번이라도 읽었는가.
    pub loaded: bool,
}

impl StorageState {
    /// 빠른 자료를 읽는다. `statvfs` 와 `/sys/block` 뿐이라 즉시 끝난다.
    pub fn load_fast(&mut self, ctx: &ProbeCtx) {
        let mount_list = mounts::mounts(ctx);
        self.mounts = mounts::usage(ctx);
        self.devices = blockdev::devices(ctx, &mount_list);
        self.raid = blockdev::raid_arrays(ctx);
        self.filesystems = fsinfo::health(ctx, &self.mounts);
        if self.dir_path.as_os_str().is_empty() {
            // 기본 조사 지점은 가장 꽉 찬 파일시스템이다.
            self.dir_path = self
                .mounts
                .first()
                .map(|m| PathBuf::from(&m.mount.target))
                .unwrap_or_else(|| PathBuf::from("/"));
        }
        self.loaded = true;
    }

    /// 이 작업을 보려면 어떤 배경 작업이 필요한가.
    pub fn jobs_for(&self, task_id: &str) -> Vec<Job> {
        match task_id {
            "storage.what-fills" if self.dir.is_none() => {
                vec![Job::DirScan(self.dir_path.clone())]
            }
            "storage.deleted-held" if self.deleted.is_none() => vec![Job::Deleted],
            "storage.logs" if self.logs.is_none() => vec![Job::Logs],
            "storage.drive-failing" if !self.drives_loaded => vec![Job::Drives],
            "storage.errors" if self.errors.is_none() => vec![Job::KernelErrors],
            // 원인을 좁히려면 세 가지가 함께 필요하다.
            "storage.which-full" => {
                let mut jobs = Vec::new();
                if self.deleted.is_none() {
                    jobs.push(Job::Deleted);
                }
                if self.logs.is_none() {
                    jobs.push(Job::Logs);
                }
                if self.dir.is_none() {
                    jobs.push(Job::DirScan(self.dir_path.clone()));
                }
                jobs
            }
            _ => Vec::new(),
        }
    }

    /// 배경 작업 결과를 반영한다.
    pub fn apply(&mut self, output: JobOutput) {
        match output {
            JobOutput::DirScan(scan) => {
                self.dir_path = scan.path.clone();
                self.dir = Some(scan);
            }
            JobOutput::Drives(drives) => {
                self.drives = drives;
                self.drives_loaded = true;
            }
            JobOutput::Logs(logs) => self.logs = Some(logs),
            JobOutput::Deleted(deleted) => self.deleted = Some(deleted),
            JobOutput::KernelErrors(errors) => self.errors = Some(errors),
        }
    }

    /// 디렉터리를 파고든다.
    pub fn descend(&mut self, path: PathBuf, jobs: &mut Jobs, ctx: &ProbeCtx) {
        self.dir_path = path;
        self.dir = None;
        jobs.request(Job::DirScan(self.dir_path.clone()), ctx);
    }

    /// 한 단계 위로 올라간다. 마운트 지점보다 위로는 가지 않는다.
    pub fn ascend(&mut self, jobs: &mut Jobs, ctx: &ProbeCtx) -> bool {
        let Some(parent) = self.dir_path.parent().map(PathBuf::from) else {
            return false;
        };
        if parent.as_os_str().is_empty() {
            return false;
        }
        self.descend(parent, jobs, ctx);
        true
    }

    /// 가장 꽉 찬 파일시스템.
    pub fn fullest(&self) -> Option<&MountUsage> {
        self.mounts.first()
    }

    /// 원인 좁히기 결과.
    pub fn diagnosis(&self) -> Option<SpaceDiagnosis> {
        let mount = self.fullest()?;
        Some(storage::diagnose_full(
            mount,
            self.deleted.as_ref(),
            self.logs.as_ref(),
            self.dir.as_ref(),
        ))
    }

    /// 다시 읽는다.
    pub fn reset(&mut self) {
        let dir_path = std::mem::take(&mut self.dir_path);
        *self = StorageState {
            dir_path,
            ..Default::default()
        };
    }
}
