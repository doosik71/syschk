//! 수집 계층.
//!
//! 수집기는 [`Probe`] 하나를 구현하고 레지스트리에 등록한다. 실행할 명령을 스스로
//! 노출하므로 근거 표시와 보고서 첨부가 자동으로 따라온다.
//!
//! 수집 실패는 예외가 아니라 [`Availability`] 의 한 상태다. 도구가 없거나 권한이 없거나
//! 값이 신뢰할 수 없어도 앱은 계속 동작하고, 그 사실을 사용자에게 그대로 알린다.

pub mod system;

use crate::util::exec::CommandOutput;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// 수집 결과를 왜 쓸 수 없는지 — 또는 쓸 수 있는지.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Availability {
    /// 정상 수집.
    Ok,
    /// 필요한 도구가 설치되어 있지 않다. 도구 안내로 연결된다.
    NotInstalled { tool: &'static str },
    /// 권한이 필요하다.
    NeedsPrivilege { hint: String },
    /// 이 시스템에는 해당하지 않는다.
    Unsupported { reason: String },
    /// 값을 얻었지만 신뢰할 수 없다. 수치를 노출하지 않는다.
    Untrusted { reason: String },
    /// 출력 형식이 예상과 달라 해석하지 못했다. 원본만 보여준다.
    ParseFailed { reason: String },
}

impl Availability {
    pub fn is_ok(&self) -> bool {
        matches!(self, Availability::Ok)
    }

    /// 사용자에게 보여줄 한 줄 설명.
    pub fn message(&self) -> String {
        match self {
            Availability::Ok => "collected".into(),
            Availability::NotInstalled { tool } => {
                format!("not available - {tool} is not installed")
            }
            Availability::NeedsPrivilege { hint } => format!("needs privileges - {hint}"),
            Availability::Unsupported { reason } => format!("not applicable here - {reason}"),
            Availability::Untrusted { reason } => {
                format!("value not trustworthy - {reason}")
            }
            Availability::ParseFailed { reason } => format!("could not be read - {reason}"),
        }
    }
}

/// 이름과 값의 쌍. M1 에서 축별 전용 타입이 추가된다.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Field {
    pub label: String,
    pub value: String,
}

impl Field {
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
        }
    }
}

/// 수집된 값.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProbeData {
    Empty,
    Fields(Vec<Field>),
}

impl ProbeData {
    pub fn field(&self, label: &str) -> Option<&str> {
        match self {
            ProbeData::Fields(f) => f
                .iter()
                .find(|x| x.label == label)
                .map(|x| x.value.as_str()),
            ProbeData::Empty => None,
        }
    }
}

/// 수집 결과.
#[derive(Clone, Debug)]
pub struct ProbeResult {
    pub probe: &'static str,
    pub data: ProbeData,
    /// 원본 출력. 보고서에 그대로 실리고, 해석 실패 시 화면에 표시된다.
    pub raw: Vec<CommandOutput>,
    pub availability: Availability,
    pub elapsed: Duration,
}

impl ProbeResult {
    pub fn ok(probe: &'static str, data: ProbeData) -> Self {
        Self {
            probe,
            data,
            raw: Vec::new(),
            availability: Availability::Ok,
            elapsed: Duration::ZERO,
        }
    }

    pub fn unavailable(probe: &'static str, availability: Availability) -> Self {
        Self {
            probe,
            data: ProbeData::Empty,
            raw: Vec::new(),
            availability,
            elapsed: Duration::ZERO,
        }
    }
}

/// 수집 문맥. 시험에서 `/proc` 대신 픽스처를 가리킬 수 있도록 루트를 분리한다.
#[derive(Clone, Debug)]
pub struct ProbeCtx {
    root: PathBuf,
}

impl Default for ProbeCtx {
    fn default() -> Self {
        Self {
            root: PathBuf::from("/"),
        }
    }
}

impl ProbeCtx {
    /// 시험용. 픽스처 디렉터리를 루트로 삼는다.
    pub fn with_root(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn path(&self, p: &str) -> PathBuf {
        self.root.join(p.trim_start_matches('/'))
    }

    /// 파일을 읽는다. 없거나 권한이 없으면 `None`.
    pub fn read(&self, p: &str) -> Option<String> {
        std::fs::read_to_string(self.path(p)).ok()
    }

    pub fn exists(&self, p: &str) -> bool {
        Path::new(&self.path(p)).exists()
    }
}

/// 하나의 수집기.
pub trait Probe: Send + Sync {
    /// 안정적인 식별자.
    fn id(&self) -> &'static str;

    /// 사용자에게 보여줄 한 줄 설명.
    fn describe(&self) -> &'static str;

    /// 이 수집기가 사용하는 명령. 근거로 노출되며 전부 읽기 전용이어야 한다.
    fn commands(&self) -> Vec<&'static str> {
        Vec::new()
    }

    /// 필요한 도구 id.
    fn required_tools(&self) -> &'static [&'static str] {
        &[]
    }

    fn run(&self, ctx: &ProbeCtx) -> ProbeResult;
}

/// 등록된 수집기 전체.
pub fn probes() -> Vec<Box<dyn Probe>> {
    vec![
        Box::new(system::Identity),
        Box::new(system::Uptime),
        Box::new(system::LoadAverage),
    ]
}

pub fn by_id(id: &str) -> Option<Box<dyn Probe>> {
    probes().into_iter().find(|p| p.id() == id)
}
