//! 홈 화면 — "무엇을 하시겠습니까?"
//!
//! 도구별 분류가 아니라 사용자의 목적을 나열한다.

use crate::app::state::App;
use crate::tasks::{MENU, TaskState};
use crate::ui::theme::Theme;
use crate::ui::widgets;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders};

pub fn draw(frame: &mut Frame, area: Rect, app: &App, th: Theme) {
    let [left, right] =
        Layout::horizontal([Constraint::Percentage(58), Constraint::Percentage(42)]).areas(area);

    let g = th.glyphs;
    let cursor = app.cursor();
    let mut rows: Vec<Line> = Vec::new();
    for (i, screen) in MENU.iter().enumerate() {
        let selected = i == cursor;
        let missing = app.screen_missing_tools(*screen);
        let mut spans = vec![
            Span::styled(
                format!(" {:>2}  ", i + 1),
                if selected { th.selected() } else { th.dim() },
            ),
            Span::styled(
                format!("{:<44}", screen.title()),
                if selected {
                    th.selected()
                } else {
                    Default::default()
                },
            ),
            Span::styled(format!("{:<24}", screen.tag()), th.dim()),
        ];
        if missing > 0 {
            spans.push(Span::styled(
                format!("{} {missing} tool(s) missing", g.warn),
                th.warn(),
            ));
        }
        rows.push(Line::from(spans));
    }

    widgets::scroll_list(
        frame,
        left,
        Block::default()
            .borders(Borders::ALL)
            .title(" What do you want to do? "),
        rows,
        cursor,
    );

    // 오른쪽: 선택된 화면 설명.
    let screen = app.selected_screen();
    let tasks = screen.tasks();
    let ready = tasks.iter().filter(|t| t.state == TaskState::Ready).count();
    let missing = app.screen_missing_tools(screen);

    let mut lines = vec![
        Line::from(Span::styled(screen.title().to_string(), th.title())),
        widgets::blank(),
        Line::from(screen.blurb().to_string()),
        widgets::blank(),
        widgets::kv("things to do", format!("{}", tasks.len()), th),
        widgets::kv("working now", format!("{ready} of {}", tasks.len()), th),
        widgets::kv(
            "tools missing",
            if missing == 0 {
                "none".to_string()
            } else {
                format!("{missing} - press t for guidance")
            },
            th,
        ),
        widgets::blank(),
        widgets::section("Inside this screen", th),
    ];
    for task in tasks.iter().take(8) {
        lines.push(Line::from(vec![
            Span::styled(format!(" {} ", th.glyphs.bullet), th.dim()),
            Span::raw(task.title.to_string()),
        ]));
    }
    if tasks.len() > 8 {
        lines.push(Line::from(Span::styled(
            format!("   … and {} more", tasks.len() - 8),
            th.dim(),
        )));
    }
    lines.push(widgets::blank());
    lines.push(Line::from(Span::styled(
        "Not sure where to look? Press / and describe the symptom.".to_string(),
        th.dim(),
    )));

    widgets::detail(
        frame,
        right,
        Block::default().borders(Borders::ALL).title(" Details "),
        lines,
    );
}
