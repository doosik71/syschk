//! 재사용 위젯.

use super::theme::Theme;
use crate::collect::Availability;
use crate::tasks::TaskState;
use crate::tools::ToolStatus;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph, Wrap};

/// 커서 위치를 화면 안에 유지하며 한 줄 항목 목록을 그린다.
pub fn scroll_list(
    frame: &mut Frame,
    area: Rect,
    block: Block<'_>,
    lines: Vec<Line<'static>>,
    cursor: usize,
) {
    let inner_height = area.height.saturating_sub(2) as usize; // 테두리 제외
    // 커서를 화면 중앙 부근에 두되, 목록의 처음과 끝에서는 넘치지 않게 한다.
    let offset = if inner_height == 0 || lines.len() <= inner_height {
        0
    } else {
        cursor
            .saturating_sub(inner_height / 2)
            .min(lines.len() - inner_height)
    };
    let visible: Vec<Line> = lines.into_iter().skip(offset).collect();
    frame.render_widget(Paragraph::new(visible).block(block), area);
}

/// 표처럼 줄바꿈 없이 그린다. 넘치는 부분은 잘린다.
pub fn table(frame: &mut Frame, area: Rect, block: Block<'_>, lines: Vec<Line<'static>>) {
    frame.render_widget(Paragraph::new(lines).block(block), area);
}

/// 여러 줄 상세 내용을 그린다.
pub fn detail(frame: &mut Frame, area: Rect, block: Block<'_>, lines: Vec<Line<'static>>) {
    frame.render_widget(
        Paragraph::new(lines)
            .block(block)
            .wrap(Wrap { trim: false }),
        area,
    );
}

/// `label: value` 한 줄.
pub fn kv(label: &str, value: impl Into<String>, th: Theme) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label:<12}"), th.dim()),
        Span::raw(value.into()),
    ])
}

/// 작업 상태 배지.
pub fn task_badge(state: TaskState, milestone: &str, th: Theme) -> Span<'static> {
    match state {
        TaskState::Ready => Span::styled("ready", th.ok()),
        TaskState::Planned => Span::styled(format!("planned {milestone}"), th.dim()),
    }
}

/// 도구 상태 배지.
pub fn tool_badge(status: &ToolStatus, th: Theme) -> Span<'static> {
    let g = th.glyphs;
    match status {
        ToolStatus::Installed(_) => Span::styled(format!("{} installed", g.ok), th.ok()),
        ToolStatus::Missing => Span::styled(format!("{} missing", g.missing), th.warn()),
        ToolStatus::NotApplicable(_) => Span::styled(format!("{} n/a", g.na), th.dim()),
    }
}

/// 수집 가용성 배지.
pub fn availability_badge(a: &Availability, th: Theme) -> Span<'static> {
    let g = th.glyphs;
    match a {
        Availability::Ok => Span::styled(format!("{} ok", g.ok), th.ok()),
        Availability::NotInstalled { .. } => {
            Span::styled(format!("{} tool missing", g.missing), th.warn())
        }
        Availability::NeedsPrivilege { .. } => {
            Span::styled(format!("{} needs privileges", g.warn), th.warn())
        }
        Availability::Unsupported { .. } => Span::styled(format!("{} n/a", g.na), th.dim()),
        Availability::Untrusted { .. } => {
            Span::styled(format!("{} not trustworthy", g.warn), th.bad())
        }
        Availability::ParseFailed { .. } => {
            Span::styled(format!("{} unreadable", g.warn), th.bad())
        }
    }
}

/// 구분선 겸 소제목.
pub fn section(title: &str, th: Theme) -> Line<'static> {
    Line::from(Span::styled(title.to_string(), th.accent()))
}

pub fn blank() -> Line<'static> {
    Line::from("")
}

/// 막대 게이지. 값과 임계 색을 함께 보여준다.
pub fn gauge(value_pct: f32, width: usize, th: Theme) -> Span<'static> {
    let width = width.max(4);
    let filled = ((value_pct / 100.0) * width as f32)
        .round()
        .clamp(0.0, width as f32) as usize;
    let (full, empty) = if th.glyphs.ok == "✔" {
        ('█', '░')
    } else {
        ('#', '.')
    };
    let bar: String = std::iter::repeat_n(full, filled)
        .chain(std::iter::repeat_n(empty, width - filled))
        .collect();
    let style = if value_pct >= 90.0 {
        th.bad()
    } else if value_pct >= 70.0 {
        th.warn()
    } else {
        th.ok()
    };
    Span::styled(bar, style)
}

/// 추세를 한 줄로 그린다. 유니코드가 안 되면 ASCII 로 떨어진다.
pub fn sparkline(values: &[f32], max: f32, width: usize, th: Theme) -> Span<'static> {
    if values.is_empty() || width == 0 {
        return Span::styled(" ".repeat(width), th.dim());
    }
    let levels: &[char] = if th.glyphs.ok == "✔" {
        &['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█']
    } else {
        &['_', '.', '.', ':', ':', '|', '|', '|']
    };
    let max = max.max(f32::EPSILON);
    let start = values.len().saturating_sub(width);
    let mut out = String::new();
    for v in &values[start..] {
        let idx = ((v / max) * (levels.len() - 1) as f32).round();
        let idx = idx.clamp(0.0, (levels.len() - 1) as f32) as usize;
        out.push(levels[idx]);
    }
    Span::styled(out, th.accent())
}

/// 판정 배지.
pub fn verdict_badge(v: crate::analyze::Verdict, th: Theme) -> Span<'static> {
    use crate::analyze::Verdict;
    let g = th.glyphs;
    match v {
        Verdict::Ok => Span::styled(format!("{} ok      ", g.ok), th.ok()),
        Verdict::Warn => Span::styled(format!("{} warning ", g.warn), th.warn()),
        Verdict::Critical => Span::styled(format!("{} critical", g.missing), th.bad()),
        Verdict::Unknown => Span::styled(format!("{} measuring", g.na), th.dim()),
    }
}
