//! 도구 안내 계층.
//!
//! 초보 사용자가 가장 흔하게 막히는 지점은 "필요한 명령이 설치되어 있지 않다"이다.
//! 이 계층은 도구가 무엇을 해주는지, 없으면 어떤 작업을 못 하는지, 어떻게 설치하는지를
//! 알려준다. **설치는 앱이 실행하지 않는다** — 명령을 보여주고 사용자가 직접 실행한다
//! (syschk 는 시스템을 변경하지 않는다).

pub mod detect;
pub mod registry;

/// 권장 묶음. 한 번에 무엇을 갖추면 되는지 안내한다.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Bundle {
    Core,
    Storage,
    Network,
    Hardware,
    Diagnostics,
    Updates,
    Containers,
    Advanced,
}

impl Bundle {
    pub const ALL: [Bundle; 8] = [
        Bundle::Core,
        Bundle::Storage,
        Bundle::Network,
        Bundle::Hardware,
        Bundle::Diagnostics,
        Bundle::Updates,
        Bundle::Containers,
        Bundle::Advanced,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Bundle::Core => "core",
            Bundle::Storage => "storage",
            Bundle::Network => "network",
            Bundle::Hardware => "hardware",
            Bundle::Diagnostics => "diagnostics",
            Bundle::Updates => "updates",
            Bundle::Containers => "containers",
            Bundle::Advanced => "advanced",
        }
    }

    pub fn why(self) -> &'static str {
        match self {
            Bundle::Core => "Needed by most checks. Install this first.",
            Bundle::Storage => "Drive health and layout: failing disks, RAID, LVM.",
            Bundle::Network => "Reachability, name resolution and link quality.",
            Bundle::Hardware => "Inventory, memory errors, sensors, PCIe links.",
            Bundle::Diagnostics => "Per-process depth and update state.",
            Bundle::Updates => "Package state, pending updates, restart needs.",
            Bundle::Containers => "Only if you run containers or virtual machines.",
            Bundle::Advanced => "Deep profiling. Can add load - use deliberately.",
        }
    }
}

/// 이 시스템에 해당하는 도구인지 판단하는 조건.
#[derive(Copy, Clone, Debug)]
pub enum Applicability {
    /// 항상 해당한다.
    Always,
    /// 이 경로가 있을 때만 의미가 있다(예: NVIDIA 드라이버).
    IfPathExists(&'static str),
}

/// 하나의 진단 도구.
#[derive(Clone, Debug)]
pub struct Tool {
    /// 안정적인 식별자. 작업(`Task.tools`)에서 참조한다.
    pub id: &'static str,
    /// apt 패키지 이름.
    pub package: &'static str,
    /// 이 패키지가 제공하는 실행 파일. 설치 여부 판단에 쓴다.
    pub binaries: &'static [&'static str],
    /// 이 도구가 무엇을 해주는지, 비전문가에게 한 줄로.
    pub purpose: &'static str,
    pub bundle: Bundle,
    /// 우분투 기본 설치 여부. 보통 설치되어 있으면 안내를 덜 강조한다.
    pub preinstalled: bool,
    /// 설치 후 알아야 할 제약(예: 성능 이력은 설치 시점 이후만 조회 가능).
    pub post_install: Option<&'static str>,
    /// 설치하지 않고 얻을 수 있는 대체 경로.
    pub without_it: Option<&'static str>,
    pub applicability: Applicability,
}

impl Tool {
    /// 사용자가 그대로 실행할 수 있는 설치 명령. 앱은 실행하지 않는다.
    pub fn install_command(&self) -> String {
        format!("sudo apt install -y {}", self.package)
    }
}

/// 도구의 현재 상태.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ToolStatus {
    /// 설치되어 있다.
    Installed(std::path::PathBuf),
    /// 설치되어 있지 않다.
    Missing,
    /// 이 시스템에는 해당하지 않는다(예: NVIDIA GPU 없음).
    NotApplicable(&'static str),
}

impl ToolStatus {
    pub fn label(&self) -> &'static str {
        match self {
            ToolStatus::Installed(_) => "installed",
            ToolStatus::Missing => "missing",
            ToolStatus::NotApplicable(_) => "n/a",
        }
    }

    pub fn is_missing(&self) -> bool {
        matches!(self, ToolStatus::Missing)
    }
}
