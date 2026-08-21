//! 판정과 근거.

/// 판정 결과.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Verdict {
    /// 아직 판단할 자료가 부족하다(표본 1개 등).
    Unknown,
    Ok,
    Warn,
    Critical,
}

impl Verdict {
    pub fn label(self) -> &'static str {
        match self {
            Verdict::Unknown => "measuring",
            Verdict::Ok => "ok",
            Verdict::Warn => "warning",
            Verdict::Critical => "critical",
        }
    }
}

/// 한 축에 대한 판정.
#[derive(Clone, Debug, PartialEq)]
pub struct Finding {
    /// 사용자에게 보이는 축 이름.
    pub axis: &'static str,
    pub verdict: Verdict,
    /// 결론 한 줄.
    pub headline: String,
    /// 그 결론의 근거 수치.
    pub evidence: Vec<String>,
    /// 이 지표가 무엇인지에 대한 설명. 쓰면서 배우게 하는 부분이다.
    pub learn: &'static str,
    /// 이어서 볼 작업 id.
    pub next: Option<&'static str>,
}

impl Finding {
    pub fn new(axis: &'static str, verdict: Verdict, headline: impl Into<String>) -> Self {
        Self {
            axis,
            verdict,
            headline: headline.into(),
            evidence: Vec::new(),
            learn: "",
            next: None,
        }
    }

    pub fn evidence(mut self, line: impl Into<String>) -> Self {
        self.evidence.push(line.into());
        self
    }

    pub fn learn(mut self, text: &'static str) -> Self {
        self.learn = text;
        self
    }

    pub fn next(mut self, task_id: &'static str) -> Self {
        self.next = Some(task_id);
        self
    }
}

// ── 임계값 ────────────────────────────────────────────────────────
// 판정 기준을 한곳에 모아 둔다. 근거 문구도 이 값을 인용한다.

/// 부하 / 논리코어. 1.0 이면 코어를 정확히 채운 상태다.
pub const LOAD_PER_CORE_WARN: f32 = 0.8;
pub const LOAD_PER_CORE_CRITICAL: f32 = 2.0;
/// CPU 가 실제로 일한 비율.
pub const CPU_BUSY_WARN: f32 = 85.0;
/// 가상화 환경에서 하이퍼바이저에 빼앗긴 시간.
pub const CPU_STEAL_WARN: f32 = 5.0;
/// PSI: 일부 작업이 지연된 시간 비율.
pub const PSI_WARN: f32 = 20.0;
pub const PSI_CRITICAL: f32 = 50.0;
pub const PSI_MEMORY_WARN: f32 = 10.0;
/// 남은 메모리 비율.
pub const MEM_AVAILABLE_WARN: f32 = 15.0;
pub const MEM_AVAILABLE_CRITICAL: f32 = 5.0;
/// 초당 스왑 아웃 페이지 수.
pub const SWAP_OUT_WARN: f32 = 50.0;
/// 장치 사용률과 평균 대기시간.
pub const DISK_UTIL_WARN: f32 = 80.0;
pub const DISK_AWAIT_WARN_MS: f32 = 20.0;
/// CPU 가 저장장치를 기다린 비율.
pub const IOWAIT_WARN: f32 = 15.0;

// ── 저장 공간 ─────────────────────────────────────────────────────

/// 파일시스템 사용률.
pub const SPACE_WARN_PCT: f32 = 90.0;
pub const SPACE_CRITICAL_PCT: f32 = 95.0;
/// 남은 절대량이 이보다 적으면 경고한다.
///
/// 단독으로 쓰면 작은 파일시스템(`/boot/efi`, `/run/lock`)이 늘 걸린다. 그래서
/// [`SPACE_TIGHT_PCT`] 이상 차 있을 때만 함께 적용한다.
pub const SPACE_FREE_WARN_BYTES: u64 = 1024 * 1024 * 1024;
/// 절대 여유량 규칙을 적용할 최소 사용률.
pub const SPACE_TIGHT_PCT: f32 = 75.0;
/// inode 사용률. 용량과 무관하게 파일 생성이 막힌다.
pub const INODE_WARN_PCT: f32 = 85.0;
pub const INODE_CRITICAL_PCT: f32 = 90.0;
/// 지운 파일이 붙잡고 있는 양.
pub const DELETED_HELD_WARN_BYTES: u64 = 1024 * 1024 * 1024;
/// 로그가 파일시스템 사용량에서 차지하는 비중.
pub const LOG_SHARE_WARN_PCT: f32 = 25.0;
/// 로그 절대 크기.
pub const LOG_SIZE_WARN_BYTES: u64 = 5 * 1024 * 1024 * 1024;
