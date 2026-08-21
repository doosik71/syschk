//! 병목 특정 화면 (2번).
//!
//! "무엇 때문에 느린가"에 한 문장으로 답하고, 그 답의 근거를 축별로 펼친다.
//! 각 축에는 지표 설명이 붙어 있어, 답을 읽는 과정에서 지표의 의미를 알게 된다.

use crate::app::state::App;
use crate::tasks::TaskState;
use crate::ui::theme::Theme;
use crate::ui::widgets;
use crate::util::fmt::{bytes_per_sec, kib, truncate};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders};

pub fn draw(frame: &mut Frame, area: Rect, app: &App, th: Theme) {
    let [banner, body] = Layout::vertical([Constraint::Length(4), Constraint::Min(6)]).areas(area);
    draw_banner(frame, banner, app, th);

    let [left, right] =
        Layout::horizontal([Constraint::Length(38), Constraint::Min(30)]).areas(body);
    draw_task_list(frame, left, app, th);
    draw_detail(frame, right, app, th);
}

fn draw_banner(frame: &mut Frame, area: Rect, app: &App, th: Theme) {
    let assessment = app.sampler.assessment();
    let mut spans = vec![
        widgets::verdict_badge(assessment.worst(), th),
        Span::raw("  "),
    ];
    spans.push(Span::styled(assessment.headline.clone(), th.title()));

    let axes: Vec<Span> = assessment
        .findings
        .iter()
        .flat_map(|f| {
            vec![
                Span::styled(format!(" {} ", f.axis), th.dim()),
                widgets::verdict_badge(f.verdict, th),
                Span::raw("  "),
            ]
        })
        .collect();

    widgets::detail(
        frame,
        area,
        Block::default()
            .borders(Borders::ALL)
            .title(" 2. Why is it slow "),
        vec![Line::from(spans), Line::from(axes)],
    );
}

fn draw_task_list(frame: &mut Frame, area: Rect, app: &App, th: Theme) {
    let cursor = app.cursor();
    let rows: Vec<Line> = app
        .screen
        .tasks()
        .iter()
        .enumerate()
        .map(|(i, task)| {
            let selected = i == cursor;
            let mut spans = vec![
                Span::styled(
                    format!(" {} ", if selected { th.glyphs.arrow } else { " " }),
                    th.accent(),
                ),
                Span::styled(
                    truncate(task.title, 30),
                    if selected {
                        th.selected()
                    } else {
                        Default::default()
                    },
                ),
            ];
            if task.state == TaskState::Planned {
                spans.push(Span::styled(format!(" {}", task.milestone), th.dim()));
            }
            Line::from(spans)
        })
        .collect();
    widgets::scroll_list(
        frame,
        area,
        Block::default().borders(Borders::ALL).title(" Look at "),
        rows,
        cursor,
    );
}

fn draw_detail(frame: &mut Frame, area: Rect, app: &App, th: Theme) {
    let task = app.selected_task();
    let task_id = task.map(|t| t.id).unwrap_or("slow.verdict");
    let assessment = app.sampler.assessment();
    let mut lines: Vec<Line> = Vec::new();

    if let Some(t) = task
        && t.state == TaskState::Planned
    {
        lines.push(Line::from(Span::styled(
            format!("Not built yet - arrives in {}.", t.milestone),
            th.dim(),
        )));
        lines.push(widgets::blank());
        lines.push(Line::from(t.answers.to_string()));
        widgets::detail(
            frame,
            area,
            Block::default().borders(Borders::ALL).title(" Detail "),
            lines,
        );
        return;
    }

    match task_id {
        "slow.verdict" => {
            lines.push(Line::from(Span::styled(
                assessment.headline.clone(),
                th.title(),
            )));
            lines.push(widgets::blank());
            for f in &assessment.findings {
                lines.push(Line::from(vec![
                    widgets::verdict_badge(f.verdict, th),
                    Span::styled(format!("  {:<8}", f.axis), th.dim()),
                    Span::raw(f.headline.clone()),
                ]));
            }
            lines.push(widgets::blank());
            lines.push(widgets::section("What to do with this", th));
            lines.push(Line::from(
                "  Pick the axis on the left to see the numbers behind its verdict.".to_string(),
            ));
            lines.push(Line::from(
                "  \"Is one program to blame\" ranks the heaviest processes on each axis."
                    .to_string(),
            ));
            lines.push(widgets::blank());
            lines.push(Line::from(Span::styled(
                "syschk stops at understanding: it does not kill processes or change limits."
                    .to_string(),
                th.dim(),
            )));
        }
        "slow.cpu" | "slow.memory" | "slow.disk" => {
            let axis = match task_id {
                "slow.cpu" => "cpu",
                "slow.memory" => "memory",
                _ => "disk",
            };
            if let Some(f) = assessment.finding(axis) {
                lines.push(Line::from(vec![
                    widgets::verdict_badge(f.verdict, th),
                    Span::raw("  "),
                    Span::styled(f.headline.clone(), th.title()),
                ]));
                lines.push(widgets::blank());
                lines.push(widgets::section("Evidence", th));
                for e in &f.evidence {
                    lines.push(Line::from(format!("  {e}")));
                }
                lines.push(widgets::blank());
                lines.push(widgets::section("What these numbers mean", th));
                for chunk in wrap(f.learn, 76) {
                    lines.push(Line::from(Span::styled(format!("  {chunk}"), th.dim())));
                }
            }
        }
        "slow.culprit" => culprit(&mut lines, app, th),
        _ => {}
    }

    // 근거 명령과 "직접 해보기".
    if let Some(t) = task {
        lines.push(widgets::blank());
        if app.show_commands {
            if !t.commands.is_empty() {
                lines.push(widgets::section("What syschk reads", th));
                for c in t.commands {
                    lines.push(Line::from(Span::styled(format!("  {c}"), th.accent())));
                }
            }
            if !t.learn.is_empty() {
                lines.push(widgets::section("Try it yourself", th));
                for c in t.learn {
                    lines.push(Line::from(Span::styled(format!("  {c}"), th.accent())));
                }
            }
        } else if !t.commands.is_empty() || !t.learn.is_empty() {
            lines.push(Line::from(Span::styled(
                "press c to see what syschk reads, and the commands you can run yourself"
                    .to_string(),
                th.dim(),
            )));
        }
    }

    widgets::detail(
        frame,
        area,
        Block::default().borders(Borders::ALL).title(" Detail "),
        lines,
    );
}

fn culprit(lines: &mut Vec<Line<'static>>, app: &App, th: Theme) {
    let mut by_cpu: Vec<_> = app.sampler.procs.iter().collect();
    by_cpu.sort_by(|a, b| {
        b.cpu_pct
            .partial_cmp(&a.cpu_pct)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut by_mem: Vec<_> = app.sampler.procs.iter().collect();
    by_mem.sort_by(|a, b| b.rss_kb.cmp(&a.rss_kb));
    let mut by_io: Vec<_> = app.sampler.procs.iter().collect();
    by_io.sort_by(|a, b| {
        b.io_bps()
            .partial_cmp(&a.io_bps())
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    lines.push(widgets::section("Heaviest on CPU", th));
    for p in by_cpu.iter().take(5) {
        lines.push(Line::from(format!(
            "  {:>6.1}%  {:>7} {:<10} {}",
            p.cpu_pct,
            p.pid,
            truncate(&p.user, 10),
            truncate(&p.cmd, 44)
        )));
    }
    lines.push(widgets::blank());
    lines.push(widgets::section("Heaviest on memory", th));
    for p in by_mem.iter().take(5) {
        lines.push(Line::from(format!(
            "  {:>7}  {:>7} {:<10} {}",
            kib(p.rss_kb),
            p.pid,
            truncate(&p.user, 10),
            truncate(&p.cmd, 44)
        )));
    }
    lines.push(widgets::blank());
    lines.push(widgets::section("Heaviest on disk", th));
    if app.sampler.io_visible {
        for p in by_io.iter().take(5) {
            lines.push(Line::from(format!(
                "  {:>9}  {:>7} {:<10} {}",
                bytes_per_sec(p.io_bps()),
                p.pid,
                truncate(&p.user, 10),
                truncate(&p.cmd, 42)
            )));
        }
    } else {
        lines.push(Line::from(Span::styled(
            "  per-process I/O is not readable without privileges".to_string(),
            th.dim(),
        )));
    }
    lines.push(widgets::blank());
    lines.push(Line::from(Span::styled(
        "A heavy process is not automatically the problem - compare it against the verdict"
            .to_string(),
        th.dim(),
    )));
    lines.push(Line::from(Span::styled(
        "for that axis before concluding anything.".to_string(),
        th.dim(),
    )));
}

/// 설명 문장을 폭에 맞춰 나눈다.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        if line.len() + word.len() + 1 > width {
            out.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() {
        out.push(line);
    }
    out
}
