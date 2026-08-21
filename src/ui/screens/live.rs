//! 실시간 관찰 화면 (1번).
//!
//! 위쪽은 네 축의 현재 상태, 아래쪽은 선택한 작업에 따라 달라지는 상세다.
//! 각 값 옆에는 그 값이 무엇인지 짧은 설명이 붙는다 — 쓰다 보면 지표를 익히게 된다.

use crate::app::state::App;
use crate::collect::process::ProcRow;
use crate::ui::theme::Theme;
use crate::ui::widgets;
use crate::util::fmt::{bytes_per_sec, kib, pct, truncate};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders};

pub fn draw(frame: &mut Frame, area: Rect, app: &App, th: Theme) {
    let [top, bottom] = Layout::vertical([Constraint::Length(7), Constraint::Min(6)]).areas(area);
    draw_axes(frame, top, app, th);

    let [left, right] =
        Layout::horizontal([Constraint::Length(38), Constraint::Min(30)]).areas(bottom);
    draw_task_list(frame, left, app, th);
    draw_content(frame, right, app, th);
}

/// CPU · 메모리 · 디스크 · 네트워크 네 축을 나란히 보여준다.
fn draw_axes(frame: &mut Frame, area: Rect, app: &App, th: Theme) {
    let cols = Layout::horizontal([
        Constraint::Ratio(1, 4),
        Constraint::Ratio(1, 4),
        Constraint::Ratio(1, 4),
        Constraint::Ratio(1, 4),
    ])
    .split(area);
    let s = &app.sampler;
    let bar_width = (cols[0].width as usize).saturating_sub(14).clamp(6, 24);

    // CPU
    let load_per_core = if s.cores == 0 {
        s.load.one
    } else {
        s.load.one / s.cores as f32
    };
    box_with(
        frame,
        cols[0],
        " cpu ",
        vec![
            Line::from(vec![
                widgets::gauge(s.cpu.busy, bar_width, th),
                Span::raw(format!(" {}", pct(s.cpu.busy))),
            ]),
            Line::from(Span::styled(
                format!(
                    "user {} sys {} io {}",
                    pct(s.cpu.user),
                    pct(s.cpu.system),
                    pct(s.cpu.iowait)
                ),
                th.dim(),
            )),
            Line::from(Span::styled(
                format!(
                    "load {:.2} / {} cores = {:.2}",
                    s.load.one, s.cores, load_per_core
                ),
                th.dim(),
            )),
            Line::from(widgets::sparkline(
                s.cpu_trend.values(),
                100.0,
                bar_width + 6,
                th,
            )),
        ],
        th,
    );

    // 메모리
    box_with(
        frame,
        cols[1],
        " memory ",
        vec![
            Line::from(vec![
                widgets::gauge(s.memory.used_pct(), bar_width, th),
                Span::raw(format!(" {}", pct(s.memory.used_pct()))),
            ]),
            Line::from(Span::styled(
                format!("{} used of {}", kib(s.memory.used()), kib(s.memory.total)),
                th.dim(),
            )),
            Line::from(Span::styled(
                format!(
                    "cache {}  swap {}",
                    kib(s.memory.cached + s.memory.buffers),
                    kib(s.memory.swap_used())
                ),
                th.dim(),
            )),
            Line::from(widgets::sparkline(
                s.mem_trend.values(),
                100.0,
                bar_width + 6,
                th,
            )),
        ],
        th,
    );

    // 디스크
    let busiest = s
        .disks
        .iter()
        .max_by(|a, b| {
            a.util_pct
                .partial_cmp(&b.util_pct)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .cloned();
    let (disk_name, disk_util, disk_await) = busiest
        .map(|d| (d.name, d.util_pct, d.await_ms))
        .unwrap_or_else(|| ("-".to_string(), 0.0, 0.0));
    box_with(
        frame,
        cols[2],
        " disk ",
        vec![
            Line::from(vec![
                widgets::gauge(disk_util, bar_width, th),
                Span::raw(format!(" {}", pct(disk_util))),
            ]),
            Line::from(Span::styled(
                format!("busiest {disk_name}  wait {disk_await:.1}ms"),
                th.dim(),
            )),
            Line::from(Span::styled(
                format!(
                    "{}  {} waiting",
                    if s.disk_throughput() < 1.0 {
                        "idle".to_string()
                    } else {
                        bytes_per_sec(s.disk_throughput())
                    },
                    s.blocked
                ),
                th.dim(),
            )),
            Line::from(widgets::sparkline(
                s.disk_trend.values(),
                100.0,
                bar_width + 6,
                th,
            )),
        ],
        th,
    );

    // 네트워크
    let (net_name, rx, tx, errors) = s
        .nets
        .first()
        .map(|n| (n.name.clone(), n.rx_bytes, n.tx_bytes, n.errors_total))
        .unwrap_or_else(|| ("-".to_string(), 0.0, 0.0, 0));
    let drops: u64 = s.nets.iter().map(|n| n.drops_total).sum();
    let peak = s.net_trend.peak().max(1.0);
    box_with(
        frame,
        cols[3],
        " network ",
        vec![
            Line::from(Span::raw(format!(
                "{}  {}",
                net_name,
                bytes_per_sec(rx + tx)
            ))),
            Line::from(Span::styled(format!("in {}", bytes_per_sec(rx)), th.dim())),
            Line::from(Span::styled(
                format!("out {}  err {} drop {}", bytes_per_sec(tx), errors, drops),
                th.dim(),
            )),
            Line::from(widgets::sparkline(
                s.net_trend.values(),
                peak,
                bar_width + 6,
                th,
            )),
        ],
        th,
    );
}

fn box_with(frame: &mut Frame, area: Rect, title: &str, lines: Vec<Line<'static>>, th: Theme) {
    let _ = th;
    widgets::detail(
        frame,
        area,
        Block::default()
            .borders(Borders::ALL)
            .title(title.to_string()),
        lines,
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
            Line::from(vec![
                Span::styled(
                    format!(" {} ", if selected { th.glyphs.arrow } else { " " }),
                    th.accent(),
                ),
                Span::styled(
                    truncate(task.title, 32),
                    if selected {
                        th.selected()
                    } else {
                        Default::default()
                    },
                ),
            ])
        })
        .collect();
    widgets::scroll_list(
        frame,
        area,
        Block::default()
            .borders(Borders::ALL)
            .title(" 1. Right now "),
        rows,
        cursor,
    );
}

fn draw_content(frame: &mut Frame, area: Rect, app: &App, th: Theme) {
    let task_id = app.selected_task().map(|t| t.id).unwrap_or("live.overview");
    let title = match task_id {
        "live.cpu" => " Processes by CPU ",
        "live.memory" => " Processes by memory ",
        "live.disk-io" => " Disk activity ",
        "live.network" => " Interfaces ",
        "live.stuck" => " Stuck processes ",
        "live.freeze" => " Hold still and inspect ",
        _ => " Overview ",
    };

    let mut lines: Vec<Line> = Vec::new();
    if app.sampler.warming_up() {
        lines.push(Line::from(Span::styled(
            "Measuring - rates need two samples, one second apart.".to_string(),
            th.dim(),
        )));
        lines.push(widgets::blank());
    }
    if app.frozen {
        lines.push(Line::from(Span::styled(
            "FROZEN - values are held still. Press f to resume.".to_string(),
            th.warn(),
        )));
        lines.push(widgets::blank());
    }

    match task_id {
        "live.overview" => overview(&mut lines, app, th),
        "live.cpu" | "live.memory" | "live.disk-io" => {
            process_table(&mut lines, app, th, area.width as usize)
        }
        "live.network" => interfaces(&mut lines, app, th),
        "live.stuck" => stuck(&mut lines, app, th),
        "live.freeze" => freeze(&mut lines, app, th, area.width as usize),
        _ => {}
    }

    widgets::table(
        frame,
        area,
        Block::default()
            .borders(Borders::ALL)
            .title(title.to_string()),
        lines,
    );
}

fn overview(lines: &mut Vec<Line<'static>>, app: &App, th: Theme) {
    let s = &app.sampler;

    // 코어별 사용률.
    lines.push(widgets::section("Per-core usage", th));
    let per_core = &s.cpu.per_core;
    if per_core.is_empty() {
        lines.push(Line::from(Span::styled(
            "  measuring".to_string(),
            th.dim(),
        )));
    } else {
        for chunk in per_core.chunks(4) {
            let mut spans = vec![Span::raw("  ")];
            for (i, v) in chunk.iter().enumerate() {
                spans.push(Span::styled(format!("{:>5} ", pct(*v)), th.dim()));
                spans.push(widgets::gauge(*v, 8, th));
                if i + 1 < chunk.len() {
                    spans.push(Span::raw("  "));
                }
            }
            lines.push(Line::from(spans));
        }
    }

    lines.push(widgets::blank());
    lines.push(widgets::section("Right now", th));
    lines.push(widgets::kv("processes", s.procs.len().to_string(), th));
    lines.push(widgets::kv(
        "runnable",
        format!("{} waiting for a cpu", s.running),
        th,
    ));
    lines.push(widgets::kv(
        "blocked",
        format!("{} stuck waiting on storage", s.blocked),
        th,
    ));

    // 판정 요약.
    let assessment = s.assessment();
    lines.push(widgets::blank());
    lines.push(widgets::section("How it reads", th));
    lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled(assessment.headline.clone(), th.accent()),
    ]));
    for f in &assessment.findings {
        lines.push(Line::from(vec![
            Span::raw("  "),
            widgets::verdict_badge(f.verdict, th),
            Span::styled(format!("  {:<8}", f.axis), th.dim()),
            Span::raw(f.headline.clone()),
        ]));
    }
    lines.push(widgets::blank());
    lines.push(Line::from(Span::styled(
        "  Press 2 from home for the full reasoning behind these verdicts.".to_string(),
        th.dim(),
    )));
}

fn process_table(lines: &mut Vec<Line<'static>>, app: &App, th: Theme, width: usize) {
    lines.push(Line::from(vec![
        Span::styled(format!("sorted by {} ", app.sort.label()), th.accent()),
        Span::styled(
            "(s to change, J/K to move, p to pin, f to freeze)".to_string(),
            th.dim(),
        ),
    ]));
    lines.push(Line::from(Span::styled(
        format!(
            "  {:>7} {:<9} {:>6} {:>6} {:>8} {:>9} {:>9}  {}",
            "PID", "USER", "CPU%", "MEM%", "RSS", "READ/s", "WRITE/s", "COMMAND"
        ),
        th.dim(),
    )));

    // 테두리 2 + 표식 1 + 고정 열 64 를 뺀 나머지가 명령줄 자리다.
    let cmd_width = width.saturating_sub(68).max(12);
    let rows = app.sorted_procs();
    for (i, p) in rows.iter().take(24).enumerate() {
        lines.push(proc_line(p, i == app.row_cursor, app.pinned, cmd_width, th));
    }
    if rows.is_empty() {
        lines.push(Line::from(Span::styled(
            "  measuring".to_string(),
            th.dim(),
        )));
    }
    if !app.sampler.io_visible {
        lines.push(widgets::blank());
        lines.push(Line::from(Span::styled(
            "Per-process I/O for other users needs privileges - shown as '-' rather than 0."
                .to_string(),
            th.dim(),
        )));
    }
}

fn proc_line(
    p: &ProcRow,
    selected: bool,
    pinned: Option<u32>,
    cmd_width: usize,
    th: Theme,
) -> Line<'static> {
    let io = |v: Option<f32>| match v {
        Some(x) => bytes_per_sec(x),
        None => "-".to_string(),
    };
    let marker = if pinned == Some(p.pid) {
        th.glyphs.arrow
    } else if selected {
        ">"
    } else {
        " "
    };
    let body = format!(
        "{:>7} {:<9} {:>6} {:>6} {:>8} {:>9} {:>9}  {}",
        p.pid,
        truncate(&p.user, 9),
        format!("{:.1}", p.cpu_pct),
        format!("{:.1}", p.mem_pct),
        kib(p.rss_kb),
        io(p.read_bps),
        io(p.write_bps),
        truncate(&p.cmd, cmd_width)
    );
    let style = if selected {
        th.selected()
    } else if p.is_blocked() {
        th.warn()
    } else {
        Default::default()
    };
    Line::from(vec![
        Span::styled(marker.to_string(), th.accent()),
        Span::styled(body, style),
    ])
}

fn interfaces(lines: &mut Vec<Line<'static>>, app: &App, th: Theme) {
    lines.push(Line::from(Span::styled(
        format!(
            "  {:<12} {:>12} {:>12} {:>9} {:>9} {:>7} {:>7}",
            "INTERFACE", "IN/s", "OUT/s", "PKT IN/s", "PKT OUT/s", "ERRORS", "DROPS"
        ),
        th.dim(),
    )));
    if app.sampler.nets.is_empty() {
        lines.push(Line::from(Span::styled(
            "  measuring".to_string(),
            th.dim(),
        )));
    }
    for n in &app.sampler.nets {
        let style = if n.errors_total > 0 {
            th.warn()
        } else {
            Default::default()
        };
        lines.push(Line::from(Span::styled(
            format!(
                "  {:<12} {:>12} {:>12} {:>9.0} {:>9.0} {:>7} {:>7}",
                truncate(&n.name, 12),
                bytes_per_sec(n.rx_bytes),
                bytes_per_sec(n.tx_bytes),
                n.rx_packets,
                n.tx_packets,
                n.errors_total,
                n.drops_total
            ),
            style,
        )));
    }
    lines.push(widgets::blank());
    lines.push(Line::from(Span::styled(
        "Errors mean the link itself is failing and should stay at zero. Drops also count"
            .to_string(),
        th.dim(),
    )));
    lines.push(Line::from(Span::styled(
        "packets nobody was listening for, so a steady trickle of drops is normal.".to_string(),
        th.dim(),
    )));
}

fn stuck(lines: &mut Vec<Line<'static>>, app: &App, th: Theme) {
    let blocked = app.sampler.blocked_procs();
    if blocked.is_empty() {
        lines.push(Line::from(Span::styled(
            "Nothing is stuck. No process is in uninterruptible wait.".to_string(),
            th.ok(),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            format!("{} process(es) are stuck waiting", blocked.len()),
            th.warn(),
        )));
        lines.push(widgets::blank());
        for p in blocked {
            lines.push(Line::from(format!(
                "  {:>7} {:<10} waiting at {}",
                p.pid,
                truncate(&p.user, 10),
                p.wchan.clone().unwrap_or_else(|| "unknown".into())
            )));
            lines.push(Line::from(Span::styled(
                format!("          {}", truncate(&p.cmd, 70)),
                th.dim(),
            )));
        }
    }
    lines.push(widgets::blank());
    lines.push(widgets::section("What this means", th));
    lines.push(Line::from(
        "  A process in state D is waiting on the kernel - almost always storage or a".to_string(),
    ));
    lines.push(Line::from(
        "  network filesystem. It cannot be interrupted, so it ignores Ctrl-C and shows up"
            .to_string(),
    ));
    lines.push(Line::from(
        "  as a frozen program. Several at once usually means the storage path is the".to_string(),
    ));
    lines.push(Line::from("  problem, not the programs.".to_string()));
}

fn freeze(lines: &mut Vec<Line<'static>>, app: &App, th: Theme, width: usize) {
    lines.push(Line::from(vec![
        Span::styled("state  ", th.dim()),
        if app.frozen {
            Span::styled("frozen".to_string(), th.warn())
        } else {
            Span::styled("live".to_string(), th.ok())
        },
    ]));
    lines.push(widgets::kv("sort", app.sort.label(), th));
    lines.push(widgets::kv(
        "pinned",
        app.pinned
            .map(|p| p.to_string())
            .unwrap_or_else(|| "none".into()),
        th,
    ));
    lines.push(widgets::kv(
        "interval",
        format!("{:.1}s", app.sampler.interval.as_secs_f32()),
        th,
    ));
    lines.push(widgets::kv(
        "sampling cost",
        format!(
            "{:.0}ms per sample over {} processes",
            app.sampler.cost.as_secs_f32() * 1000.0,
            app.sampler.procs.len()
        ),
        th,
    ));
    lines.push(widgets::blank());
    lines.push(widgets::section("Keys", th));
    lines.push(Line::from("  f  freeze and resume".to_string()));
    lines.push(Line::from("  s  change sort order".to_string()));
    lines.push(Line::from(
        "  J / K  move down and up the process list".to_string(),
    ));
    lines.push(Line::from(
        "  p  pin the highlighted process to the top".to_string(),
    ));
    lines.push(widgets::blank());

    if let Some(p) = app.selected_proc() {
        lines.push(widgets::section("Highlighted process", th));
        lines.push(widgets::kv("pid", p.pid.to_string(), th));
        lines.push(widgets::kv("parent", p.ppid.to_string(), th));
        lines.push(widgets::kv("user", p.user.clone(), th));
        lines.push(widgets::kv("state", p.state.to_string(), th));
        lines.push(widgets::kv("threads", p.threads.to_string(), th));
        lines.push(widgets::kv("cpu", format!("{:.1}%", p.cpu_pct), th));
        lines.push(widgets::kv(
            "memory",
            format!("{} ({:.1}%)", kib(p.rss_kb), p.mem_pct),
            th,
        ));
        lines.push(widgets::kv(
            "command",
            truncate(&p.cmd, width.saturating_sub(16).max(20)),
            th,
        ));
        lines.push(widgets::blank());
        lines.push(Line::from(Span::styled(
            "Screen 7 goes deeper on one process: open files, limits, why it waits.".to_string(),
            th.dim(),
        )));
    }
}
