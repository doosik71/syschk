//! 작업 화면 — 화면에 속한 작업 목록과 선택된 작업의 상세.
//!
//! 상단 판정 요약 / 중단 근거 자료 / 하단 다음 단계라는 3단 구성은 M1 이후 각 화면이
//! 실제 자료로 채운다. M0 에서는 작업의 상태, 필요 도구, 사용될 명령을 보여준다.

use crate::app::state::App;
use crate::tools::{ToolStatus, registry as tool_registry};
use crate::ui::theme::Theme;
use crate::ui::widgets;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders};

pub fn draw(frame: &mut Frame, area: Rect, app: &App, th: Theme) {
    let [left, right] =
        Layout::horizontal([Constraint::Percentage(48), Constraint::Percentage(52)]).areas(area);

    let screen = app.screen;
    let tasks = screen.tasks();
    let cursor = app.cursor();

    let rows: Vec<Line> = tasks
        .iter()
        .enumerate()
        .map(|(i, task)| {
            let selected = i == cursor;
            Line::from(vec![
                Span::styled(
                    format!(" {} ", if selected { th.glyphs.arrow } else { " " }),
                    th.accent(),
                ),
                Span::styled(
                    format!("{:<46}", task.title),
                    if selected {
                        th.selected()
                    } else {
                        Default::default()
                    },
                ),
                widgets::task_badge(task.state, task.milestone, th),
            ])
        })
        .collect();

    widgets::scroll_list(
        frame,
        left,
        Block::default().borders(Borders::ALL).title(format!(
            " {}. {} ",
            screen.number(),
            screen.title()
        )),
        rows,
        cursor,
    );

    let mut lines: Vec<Line> = Vec::new();
    if let Some(task) = app.selected_task() {
        lines.push(Line::from(Span::styled(task.title.to_string(), th.title())));
        lines.push(widgets::blank());
        lines.push(Line::from(vec![
            Span::styled("What you get   ", th.dim()),
            Span::raw(task.answers.to_string()),
        ]));
        lines.push(widgets::blank());
        lines.push(Line::from(vec![
            Span::styled("Status         ", th.dim()),
            widgets::task_badge(task.state, task.milestone, th),
        ]));

        // 필요 도구와 그 상태.
        lines.push(widgets::blank());
        lines.push(widgets::section("Needs these tools", th));
        if task.tools.is_empty() {
            lines.push(Line::from(Span::styled(
                "  nothing extra - reads /proc and /sys only".to_string(),
                th.dim(),
            )));
        } else {
            for id in task.tools {
                let Some(tool) = tool_registry::by_id(id) else {
                    continue;
                };
                let status = app
                    .inventory
                    .get(id)
                    .cloned()
                    .unwrap_or(ToolStatus::Missing);
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    widgets::tool_badge(&status, th),
                    Span::raw(format!("  {}", tool.package)),
                    Span::styled(format!("  {}", tool.purpose), th.dim()),
                ]));
            }
            let missing = app.inventory.missing_for(task.tools);
            if !missing.is_empty() {
                lines.push(widgets::blank());
                lines.push(Line::from(Span::styled(
                    format!("  Press t for install guidance ({} missing)", missing.len()),
                    th.warn(),
                )));
            }
        }

        // 근거 명령. 기본은 접힘(원칙: 명령은 기억 대상이 아니라 근거).
        lines.push(widgets::blank());
        if task.commands.is_empty() {
            lines.push(Line::from(Span::styled(
                "No external commands - this works on collected data.".to_string(),
                th.dim(),
            )));
        } else if app.show_commands {
            lines.push(widgets::section("Commands it runs (read-only)", th));
            for cmd in task.commands {
                lines.push(Line::from(Span::styled(format!("  {cmd}"), th.accent())));
            }
        } else {
            lines.push(Line::from(Span::styled(
                format!(
                    "{} read-only command(s) behind this - press c to show them",
                    task.commands.len()
                ),
                th.dim(),
            )));
        }
    } else {
        lines.push(Line::from("Nothing here yet."));
    }

    widgets::detail(
        frame,
        right,
        Block::default().borders(Borders::ALL).title(" Details "),
        lines,
    );
}
