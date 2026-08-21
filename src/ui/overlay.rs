//! 도움말과 검색 오버레이.

use super::theme::Theme;
use super::{centered, widgets};
use crate::app::state::App;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

pub fn help(frame: &mut Frame, area: Rect, th: Theme) {
    let rect = centered(area, 74, 76);
    frame.render_widget(Clear, rect);

    let lines = vec![
        Line::from(Span::styled(
            "syschk - guided, read-only diagnosis".to_string(),
            th.title(),
        )),
        widgets::blank(),
        Line::from("Pick what you are trying to do. syschk gathers the evidence,"),
        Line::from("explains what it means, and tells you what is still unknown."),
        widgets::blank(),
        widgets::section("What it will never do", th),
        Line::from("  It does not change anything: no config edits, no service"),
        Line::from("  restarts, no package installs, no repairs. Every command it"),
        Line::from("  runs is checked against a read-only policy first. Fixing is"),
        Line::from("  left to you - deliberately, so a diagnosis cannot break the"),
        Line::from("  machine you are diagnosing."),
        widgets::blank(),
        widgets::section("Keys", th),
        key_line("↑ ↓ / j k", "move", th),
        key_line("⏎", "open the selected item", th),
        key_line("esc / q", "back, or quit from the home screen", th),
        key_line("1 - 14", "jump straight to a screen from home", th),
        key_line("/", "search by symptom: slow, disk full, frozen", th),
        key_line("c", "show the commands behind a screen", th),
        key_line("t", "tools: what is missing and how to install it", th),
        key_line("?", "this help", th),
        key_line("ctrl-c", "quit from anywhere", th),
        widgets::blank(),
        widgets::section("Command line", th),
        Line::from("  syschk doctor    which diagnosis tools are present"),
        Line::from("  syschk tasks     list everything syschk can look into"),
        Line::from("  syschk check     one-shot summary, exit code for scripts"),
        widgets::blank(),
        Line::from(Span::styled(
            "Press any key to close.".to_string(),
            th.dim(),
        )),
    ];

    frame.render_widget(
        Paragraph::new(lines)
            .block(Block::default().borders(Borders::ALL).title(" Help "))
            .wrap(Wrap { trim: false }),
        rect,
    );
}

fn key_line(key: &str, desc: &str, th: Theme) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("  {key:<12}"), th.key()),
        Span::raw(desc.to_string()),
    ])
}

pub fn search(frame: &mut Frame, area: Rect, app: &App, query: &str, cursor: usize, th: Theme) {
    let rect = centered(area, 78, 70);
    frame.render_widget(Clear, rect);

    let hits = app.search_hits();
    let mut lines = vec![
        Line::from(vec![
            Span::styled("Describe the symptom: ", th.dim()),
            Span::styled(query.to_string(), th.accent()),
            Span::styled("▏".to_string(), th.accent()),
        ]),
        widgets::blank(),
    ];

    if query.trim().is_empty() {
        lines.push(Line::from(Span::styled(
            "Try: slow, disk full, frozen, port, reboot, memory, dns, service failed".to_string(),
            th.dim(),
        )));
    } else if hits.is_empty() {
        lines.push(Line::from(Span::styled(
            "Nothing matches that wording. Try a plainer word.".to_string(),
            th.warn(),
        )));
    } else {
        for (i, hit) in hits.iter().take(14).enumerate() {
            let selected = i == cursor;
            lines.push(Line::from(vec![
                Span::styled(
                    format!(" {} ", if selected { th.glyphs.arrow } else { " " }),
                    th.accent(),
                ),
                Span::styled(
                    format!("{:<44}", hit.task.title),
                    if selected {
                        th.selected()
                    } else {
                        Default::default()
                    },
                ),
                Span::styled(
                    format!("{}. {}", hit.task.screen.number(), hit.task.screen.tag()),
                    th.dim(),
                ),
            ]));
        }
        if hits.len() > 14 {
            lines.push(Line::from(Span::styled(
                format!("   … {} more matches", hits.len() - 14),
                th.dim(),
            )));
        }
    }

    frame.render_widget(
        Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Search by symptom "),
            )
            .wrap(Wrap { trim: false }),
        rect,
    );
}
