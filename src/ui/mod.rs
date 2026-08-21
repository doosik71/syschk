//! 화면 그리기. 상태를 읽어 표시만 하며, 상태를 바꾸지 않는다.

pub mod overlay;
pub mod screens;
pub mod theme;
pub mod widgets;

use crate::app::state::{App, Overlay};
use crate::tasks::Screen;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use theme::Theme;

pub fn draw(frame: &mut Frame, app: &App) {
    let th = Theme::detect();
    let area = frame.area();
    let [header, body, footer] = Layout::vertical([
        Constraint::Length(4),
        Constraint::Min(5),
        Constraint::Length(1),
    ])
    .areas(area);

    draw_header(frame, header, app, th);
    match app.screen {
        Screen::Home => screens::home::draw(frame, body, app, th),
        Screen::Tools => screens::tools::draw(frame, body, app, th),
        _ => screens::task_list::draw(frame, body, app, th),
    }
    draw_footer(frame, footer, app, th);

    match &app.overlay {
        Some(Overlay::Help) => overlay::help(frame, area, th),
        Some(Overlay::Search { query, cursor }) => {
            overlay::search(frame, area, app, query, *cursor, th)
        }
        None => {}
    }
}

fn draw_header(frame: &mut Frame, area: Rect, app: &App, th: Theme) {
    let h = &app.header;
    let g = th.glyphs;

    let mut first = vec![
        Span::styled("syschk", th.title()),
        Span::raw(format!(" {} ", g.bullet)),
        Span::raw(if h.hostname.is_empty() {
            "unknown host".to_string()
        } else {
            h.hostname.clone()
        }),
    ];
    if !h.os.is_empty() {
        first.push(Span::raw(format!(" {} ", g.bullet)));
        first.push(Span::styled(h.os.clone(), th.dim()));
    }
    if !h.kernel.is_empty() {
        first.push(Span::raw(format!(" {} kernel ", g.bullet)));
        first.push(Span::styled(h.kernel.clone(), th.dim()));
    }

    let mut second = Vec::new();
    if !h.uptime.is_empty() {
        second.push(Span::raw(format!("up {}", h.uptime)));
    }
    if !h.load.is_empty() {
        second.push(Span::raw(format!("  load {}", h.load)));
        if !h.cores.is_empty() {
            second.push(Span::styled(format!(" / {} cores", h.cores), th.dim()));
        }
    }
    let inv = &app.inventory;
    second.push(Span::raw("   tools "));
    second.push(Span::styled(
        format!("{} {}", g.ok, inv.installed()),
        th.ok(),
    ));
    second.push(Span::raw("  "));
    second.push(Span::styled(
        format!("{} {} missing", g.missing, inv.missing()),
        if inv.missing() > 0 {
            th.warn()
        } else {
            th.dim()
        },
    ));
    second.push(Span::styled(
        format!("  {} {} n/a", g.na, inv.not_applicable()),
        th.dim(),
    ));
    second.push(Span::styled(
        "   read-only: syschk never changes this system",
        th.dim(),
    ));

    let block = Block::default().borders(Borders::ALL);
    let text = vec![Line::from(first), Line::from(second)];
    frame.render_widget(Paragraph::new(text).block(block), area);
}

fn draw_footer(frame: &mut Frame, area: Rect, app: &App, th: Theme) {
    let hints: &[(&str, &str)] = if app.overlay.is_some() {
        &[("esc", "close"), ("↑↓", "move"), ("⏎", "open")]
    } else if app.screen == Screen::Home {
        &[
            ("↑↓", "move"),
            ("1-14", "jump"),
            ("⏎", "open"),
            ("/", "search"),
            ("t", "tools"),
            ("?", "help"),
            ("q", "quit"),
        ]
    } else {
        &[
            ("↑↓", "move"),
            ("c", "commands"),
            ("/", "search"),
            ("t", "tools"),
            ("?", "help"),
            ("esc", "back"),
        ]
    };

    let mut spans = Vec::new();
    if let Some(msg) = &app.status {
        spans.push(Span::styled(msg.clone(), th.warn()));
        spans.push(Span::raw("   "));
    }
    for (key, label) in hints {
        spans.push(Span::styled(format!(" {key} "), th.key()));
        spans.push(Span::styled(format!("{label}  "), th.dim()));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

/// 오버레이용 가운데 사각형.
pub fn centered(area: Rect, width_pct: u16, height_pct: u16) -> Rect {
    let [_, mid, _] = Layout::vertical([
        Constraint::Percentage((100 - height_pct) / 2),
        Constraint::Percentage(height_pct),
        Constraint::Percentage((100 - height_pct) / 2),
    ])
    .areas(area);
    let [_, center, _] = Layout::horizontal([
        Constraint::Percentage((100 - width_pct) / 2),
        Constraint::Percentage(width_pct),
        Constraint::Percentage((100 - width_pct) / 2),
    ])
    .areas(mid);
    center
}

/// 배경을 지우는 빈 문단(오버레이 아래 내용이 비치지 않게).
pub fn clear(frame: &mut Frame, area: Rect) {
    frame.render_widget(Paragraph::new("").style(Style::default()), area);
}
