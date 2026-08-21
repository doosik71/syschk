//! 목적 중심 계층. 사용자가 보는 "무엇을 하고 싶은가"의 단위.
//!
//! 화면(`Screen`)과 작업(`Task`)은 [`registry`] 에 선언되어 있고, 메뉴 표시·검색 색인·
//! 필요 도구 역참조·보고서 목차가 모두 이 선언에서 파생된다. 새 기능 추가는
//! 레지스트리에 항목을 등록하는 일로 끝난다(기존 화면 코드 수정 없음).

pub mod registry;
pub mod search;

/// 최상위 화면. 도구별 분류가 아니라 사용자의 목적으로 나눈다.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Screen {
    Home,
    Live,
    Slow,
    Halt,
    Storage,
    Network,
    Services,
    Process,
    Hardware,
    Security,
    Updates,
    Logs,
    Readiness,
    Report,
    Tools,
}

/// 홈 메뉴에 표시되는 순서.
pub const MENU: [Screen; 14] = [
    Screen::Live,
    Screen::Slow,
    Screen::Halt,
    Screen::Storage,
    Screen::Network,
    Screen::Services,
    Screen::Process,
    Screen::Hardware,
    Screen::Security,
    Screen::Updates,
    Screen::Logs,
    Screen::Readiness,
    Screen::Report,
    Screen::Tools,
];

impl Screen {
    /// 홈 메뉴 번호(1..=14). 홈은 0.
    pub fn number(self) -> usize {
        MENU.iter().position(|s| *s == self).map_or(0, |i| i + 1)
    }

    /// 홈 메뉴에 보이는 문구. 사용자의 목적을 그대로 적는다.
    pub fn title(self) -> &'static str {
        match self {
            Screen::Home => "What do you want to do?",
            Screen::Live => "See what the system is doing right now",
            Screen::Slow => "Something is slow - find out why",
            Screen::Halt => "It froze or rebooted unexpectedly",
            Screen::Storage => "Disk is full, or a drive looks bad",
            Screen::Network => "Network is down or slow",
            Screen::Services => "A service failed, or boot is slow",
            Screen::Process => "Look into one program",
            Screen::Hardware => "Check whether the hardware is healthy",
            Screen::Security => "See who logged in and what is exposed",
            Screen::Updates => "Check updates and package state",
            Screen::Logs => "Search the logs",
            Screen::Readiness => "Be ready to catch the cause next time",
            Screen::Report => "Save what I found as a document",
            Screen::Tools => "Get the tools diagnosis needs",
        }
    }

    /// 홈 메뉴 오른쪽에 붙는 짧은 분류.
    pub fn tag(self) -> &'static str {
        match self {
            Screen::Home => "",
            Screen::Live => "live view",
            Screen::Slow => "find the bottleneck",
            Screen::Halt => "after the fact",
            Screen::Storage => "space and drive health",
            Screen::Network => "reachability and speed",
            Screen::Services => "units and boot",
            Screen::Process => "one process in depth",
            Screen::Hardware => "cpu, ram, disk, gpu, sensors",
            Screen::Security => "logins and exposure",
            Screen::Updates => "packages and reboots",
            Screen::Logs => "time, unit, pattern",
            Screen::Readiness => "instrumentation",
            Screen::Report => "markdown and json",
            Screen::Tools => "install guidance",
        }
    }

    /// 화면 안에서 보여줄 한 줄 안내.
    pub fn blurb(self) -> &'static str {
        match self {
            Screen::Home => {
                "Pick what you are trying to do. syschk only reads - it never changes your system."
            }
            Screen::Live => "Watch current activity and see which processes are behind it.",
            Screen::Slow => "Narrow the slowdown to one axis: cpu, memory, disk or network.",
            Screen::Halt => {
                "Pin down when it stopped, then rule causes in or out with what was recorded."
            }
            Screen::Storage => "Separate 'space is used up' from 'the drive is failing'.",
            Screen::Network => "Walk the path outward and find the first step that breaks.",
            Screen::Services => "See which units failed and what the boot time went into.",
            Screen::Process => "Everything about one process: usage, files, limits, why it waits.",
            Screen::Hardware => {
                "Inventory and health, with untrustworthy sensor values called out as such."
            }
            Screen::Security => {
                "Who is connected, who tried, and what the outside world can reach."
            }
            Screen::Updates => "What is pending, what needs a reboot, and what changed recently.",
            Screen::Logs => "Find the entries that matter and fold away the repeating noise.",
            Screen::Readiness => {
                "Check whether this machine can even record the cause of a hard stop."
            }
            Screen::Report => "Keep the evidence: findings, verdicts and the exact commands used.",
            Screen::Tools => "See what is missing, what it would give you, and how to install it.",
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            Screen::Home => "home",
            Screen::Live => "live",
            Screen::Slow => "slow",
            Screen::Halt => "halt",
            Screen::Storage => "storage",
            Screen::Network => "network",
            Screen::Services => "services",
            Screen::Process => "process",
            Screen::Hardware => "hardware",
            Screen::Security => "security",
            Screen::Updates => "updates",
            Screen::Logs => "logs",
            Screen::Readiness => "readiness",
            Screen::Report => "report",
            Screen::Tools => "tools",
        }
    }

    /// 이 화면에 속한 작업 목록.
    pub fn tasks(self) -> Vec<&'static Task> {
        registry::tasks()
            .iter()
            .filter(|t| t.screen == self)
            .collect()
    }
}

/// 작업의 구현 상태. 사용자에게 "무엇이 지금 되는지"를 정직하게 알린다.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum TaskState {
    /// 지금 동작한다.
    Ready,
    /// 아직 구현 전. 어느 마일스톤에서 오는지 함께 표시한다.
    Planned,
}

impl TaskState {
    pub fn label(self) -> &'static str {
        match self {
            TaskState::Ready => "ready",
            TaskState::Planned => "planned",
        }
    }
}

/// 하나의 작업 = 메뉴 한 줄.
#[derive(Clone, Debug)]
pub struct Task {
    /// 안정적인 식별자. 보고서·설정·시험에서 참조한다.
    pub id: &'static str,
    pub screen: Screen,
    /// 메뉴에 보이는 문구. 사용자의 표현으로 적는다.
    pub title: &'static str,
    /// 앱이 무엇을 답해주는지.
    pub answers: &'static str,
    /// 증상 표현 별칭. 검색 색인에 들어간다.
    pub aliases: &'static [&'static str],
    /// 필요한 도구 id. [`crate::tools`] 레지스트리를 참조한다.
    pub tools: &'static [&'static str],
    /// 이 작업이 사용하는 명령. 근거로 노출되며, 전부 읽기 전용이어야 한다.
    pub commands: &'static [&'static str],
    pub state: TaskState,
    /// 구현이 오는 마일스톤.
    pub milestone: &'static str,
}
