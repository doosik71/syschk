//! 도구 준비 화면 — 무엇이 없고, 그것이 무엇을 해주며, 어떻게 설치하는지.
//!
//! 설치 명령은 보여주기만 한다. syschk 는 시스템을 변경하지 않는다.

use crate::app::state::App;
use crate::tools::{Bundle, ToolStatus, registry as tool_registry};
use crate::ui::theme::Theme;
use crate::ui::widgets;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders};

pub fn draw(frame: &mut Frame, area: Rect, app: &App, th: Theme) {
    let [left, right] =
        Layout::horizontal([Constraint::Percentage(44), Constraint::Percentage(56)]).areas(area);

    let cursor = app.cursor();
    let rows: Vec<Line> = tool_registry::tools()
        .iter()
        .enumerate()
        .map(|(i, tool)| {
            let selected = i == cursor;
            let status = app
                .inventory
                .get(tool.id)
                .cloned()
                .unwrap_or(ToolStatus::Missing);
            Line::from(vec![
                Span::raw(" "),
                widgets::tool_badge(&status, th),
                Span::raw("  "),
                Span::styled(
                    format!("{:<22}", tool.package),
                    if selected {
                        th.selected()
                    } else {
                        Default::default()
                    },
                ),
                Span::styled(format!("{:<12}", tool.bundle.label()), th.dim()),
            ])
        })
        .collect();

    widgets::scroll_list(
        frame,
        left,
        Block::default()
            .borders(Borders::ALL)
            .title(" 14. Tools diagnosis needs "),
        rows,
        cursor,
    );

    let mut lines: Vec<Line> = Vec::new();
    if let Some(tool) = app.selected_tool() {
        let status = app
            .inventory
            .get(tool.id)
            .cloned()
            .unwrap_or(ToolStatus::Missing);

        lines.push(Line::from(Span::styled(
            tool.package.to_string(),
            th.title(),
        )));
        lines.push(widgets::blank());
        lines.push(Line::from(tool.purpose.to_string()));
        lines.push(widgets::blank());
        lines.push(Line::from(vec![
            Span::styled("Status         ", th.dim()),
            widgets::tool_badge(&status, th),
        ]));
        lines.push(widgets::kv("bundle", tool.bundle.label(), th));
        lines.push(widgets::kv("provides", tool.binaries.join(", "), th));
        if tool.preinstalled {
            lines.push(Line::from(Span::styled(
                "Usually present on Ubuntu already.".to_string(),
                th.dim(),
            )));
        }

        match &status {
            ToolStatus::Installed(path) => {
                lines.push(widgets::blank());
                lines.push(Line::from(Span::styled(
                    format!("Found at {}", path.display()),
                    th.dim(),
                )));
            }
            ToolStatus::NotApplicable(reason) => {
                lines.push(widgets::blank());
                lines.push(Line::from(Span::styled(
                    format!("Not needed here: {reason}"),
                    th.dim(),
                )));
            }
            ToolStatus::Missing => {
                // 없으면 무엇을 못 하는지 역참조로 보여준다.
                let blocked = tool_registry::tasks_needing(tool.id);
                lines.push(widgets::blank());
                if blocked.is_empty() {
                    lines.push(Line::from(Span::styled(
                        "Nothing in syschk depends on it yet.".to_string(),
                        th.dim(),
                    )));
                } else {
                    lines.push(widgets::section(
                        &format!("Without it you cannot ({})", blocked.len()),
                        th,
                    ));
                    for task in blocked.iter().take(6) {
                        lines.push(Line::from(vec![
                            Span::styled(format!("  {} ", th.glyphs.bullet), th.dim()),
                            Span::raw(task.title.to_string()),
                        ]));
                    }
                    if blocked.len() > 6 {
                        lines.push(Line::from(Span::styled(
                            format!("    … and {} more", blocked.len() - 6),
                            th.dim(),
                        )));
                    }
                }

                lines.push(widgets::blank());
                lines.push(widgets::section("To install it, run this yourself", th));
                lines.push(Line::from(Span::styled(
                    format!("  {}", tool.install_command()),
                    th.accent(),
                )));
                lines.push(Line::from(Span::styled(
                    "  syschk shows the command but never runs it.".to_string(),
                    th.dim(),
                )));
            }
        }

        if let Some(note) = tool.post_install {
            lines.push(widgets::blank());
            lines.push(widgets::section("Worth knowing", th));
            lines.push(Line::from(format!("  {note}")));
        }
        if let Some(fallback) = tool.without_it {
            lines.push(widgets::blank());
            lines.push(widgets::section("Without installing anything", th));
            lines.push(Line::from(format!("  {fallback}")));
        }

        // 묶음 단위 안내.
        lines.push(widgets::blank());
        lines.push(widgets::section(
            &format!("Bundle: {}", tool.bundle.label()),
            th,
        ));
        lines.push(Line::from(format!("  {}", tool.bundle.why())));
        let pkgs = bundle_packages(tool.bundle, app);
        if !pkgs.is_empty() {
            lines.push(Line::from(Span::styled(
                format!("  sudo apt install -y {}", pkgs.join(" ")),
                th.accent(),
            )));
        }
    } else {
        lines.push(Line::from("No tools registered."));
    }

    widgets::detail(
        frame,
        right,
        Block::default().borders(Borders::ALL).title(" Details "),
        lines,
    );
}

/// 묶음에서 아직 없는 패키지만 모은다.
fn bundle_packages(bundle: Bundle, app: &App) -> Vec<&'static str> {
    tool_registry::in_bundle(bundle)
        .into_iter()
        .filter(|t| {
            app.inventory
                .get(t.id)
                .map(ToolStatus::is_missing)
                .unwrap_or(false)
        })
        .map(|t| t.package)
        .collect()
}
