//! 저장 공간·저장장치 화면 (4번).
//!
//! 위쪽은 파일시스템 사용률 요약, 아래쪽은 선택한 작업에 따라 달라지는 상세다.
//! "꽉 찼다"의 원인 다섯 가지를 구분해 주는 것이 이 화면의 핵심이다.

use crate::analyze::storage as rules;
use crate::app::state::App;
use crate::tasks::TaskState;
use crate::ui::theme::Theme;
use crate::ui::widgets;
use crate::util::fmt::{bytes, pct, truncate};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders};

pub fn draw(frame: &mut Frame, area: Rect, app: &App, th: Theme) {
    let mount_rows = app.storage.mounts.len().min(5) as u16;
    let [top, bottom] =
        Layout::vertical([Constraint::Length(mount_rows + 3), Constraint::Min(6)]).areas(area);

    draw_mounts(frame, top, app, th);

    let [left, right] =
        Layout::horizontal([Constraint::Length(38), Constraint::Min(30)]).areas(bottom);
    draw_task_list(frame, left, app, th);
    draw_content(frame, right, app, th);
}

/// 파일시스템 사용률 요약. 꽉 찬 것부터.
fn draw_mounts(frame: &mut Frame, area: Rect, app: &App, th: Theme) {
    let mut lines = vec![Line::from(Span::styled(
        format!(
            "  {:<22} {:<8} {:>8} {:>8} {:>6}  {:<12} {:>7}",
            "MOUNT", "TYPE", "SIZE", "AVAIL", "USE%", "USED", "INODE%"
        ),
        th.dim(),
    ))];

    if app.storage.mounts.is_empty() {
        lines.push(Line::from(Span::styled(
            "  reading mount table…".to_string(),
            th.dim(),
        )));
    }

    for mount in app.storage.mounts.iter().take(5) {
        let Some(usage) = mount.usage else {
            lines.push(Line::from(Span::styled(
                format!("  {:<22} could not be measured", mount.mount.target),
                th.dim(),
            )));
            continue;
        };
        let used = usage.used_pct();
        let style = if used >= 95.0 {
            th.bad()
        } else if used >= 90.0 {
            th.warn()
        } else {
            Default::default()
        };
        let inode = usage
            .inodes_used_pct()
            .map(pct)
            .unwrap_or_else(|| "-".into());
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(
                format!(
                    "{:<22} {:<8} {:>8} {:>8} {:>6}  ",
                    truncate(&mount.mount.target, 22),
                    truncate(&mount.mount.fstype, 8),
                    bytes(usage.total_bytes),
                    bytes(usage.available_bytes),
                    pct(used)
                ),
                style,
            ),
            widgets::gauge(used, 12, th),
            Span::styled(format!(" {inode:>6}"), th.dim()),
        ]));
    }

    widgets::table(
        frame,
        area,
        Block::default()
            .borders(Borders::ALL)
            .title(" 4. Storage - filesystems, fullest first "),
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

fn draw_content(frame: &mut Frame, area: Rect, app: &App, th: Theme) {
    let task = app.selected_task();
    let task_id = task.map(|t| t.id).unwrap_or("storage.which-full");
    let mut lines: Vec<Line> = Vec::new();

    // 진행 중인 배경 작업을 알린다. 멈춘 것처럼 보이지 않게.
    for note in app.jobs.in_progress() {
        lines.push(Line::from(Span::styled(format!("… {note}"), th.accent())));
    }
    if !app.jobs.in_progress().is_empty() {
        lines.push(widgets::blank());
    }

    match task_id {
        "storage.which-full" => which_full(&mut lines, app, th),
        "storage.what-fills" => what_fills(&mut lines, app, th, area.width as usize),
        "storage.deleted-held" => deleted_held(&mut lines, app, th),
        "storage.inodes" => inodes(&mut lines, app, th),
        "storage.logs" => logs(&mut lines, app, th),
        "storage.drive-failing" => drives(&mut lines, app, th),
        "storage.errors" => errors(&mut lines, app, th),
        "storage.layout" => layout(&mut lines, app, th),
        "storage.fs-health" => fs_health(&mut lines, app, th),
        _ => {}
    }

    // 근거 명령과 직접 해보기.
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

    widgets::table(
        frame,
        area,
        Block::default().borders(Borders::ALL).title(" Detail "),
        lines,
    );
}

/// 어디가 꽉 찼고, 왜 그런지.
fn which_full(lines: &mut Vec<Line<'static>>, app: &App, th: Theme) {
    let findings = rules::space_findings(&app.storage.mounts);
    // 마운트는 꽉 찬 순서로 정렬되어 있다. 판정이 같으면 더 꽉 찬 쪽을 보여준다.
    let worst = findings.iter().reduce(|best, current| {
        if current.verdict > best.verdict {
            current
        } else {
            best
        }
    });

    match worst {
        Some(f) => {
            lines.push(Line::from(vec![
                widgets::verdict_badge(f.verdict, th),
                Span::raw("  "),
                Span::styled(f.headline.clone(), th.title()),
            ]));
            lines.push(widgets::blank());
            for e in &f.evidence {
                lines.push(Line::from(format!("  {e}")));
            }
        }
        None => lines.push(Line::from(Span::styled(
            "reading filesystems…".to_string(),
            th.dim(),
        ))),
    }

    // 원인 좁히기.
    if let Some(diagnosis) = app.storage.diagnosis() {
        lines.push(widgets::blank());
        lines.push(widgets::section("Why it is full", th));
        if diagnosis.cause == rules::FullCause::NotFull {
            lines.push(Line::from(
                "  Nothing is close to full, so there is nothing to explain.".to_string(),
            ));
        } else {
            lines.push(Line::from(vec![
                Span::raw("  most likely  "),
                Span::styled(diagnosis.cause.label().to_string(), th.warn()),
                Span::styled(format!("  on {}", diagnosis.target), th.dim()),
            ]));
            lines.push(widgets::blank());
            for finding in &diagnosis.findings {
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    widgets::verdict_badge(finding.verdict, th),
                    Span::raw("  "),
                    Span::raw(finding.headline.clone()),
                ]));
                for e in &finding.evidence {
                    lines.push(Line::from(Span::styled(format!("      {e}"), th.dim())));
                }
            }
            lines.push(widgets::blank());
            lines.push(widgets::section("What you can do", th));
            for chunk in wrap(diagnosis.cause.what_to_do(), 76) {
                lines.push(Line::from(format!("  {chunk}")));
            }
        }
    }
    if let Some(f) = worst
        && !f.learn.is_empty()
    {
        lines.push(widgets::blank());
        lines.push(widgets::section("What these numbers mean", th));
        for chunk in wrap(f.learn, 76) {
            lines.push(Line::from(Span::styled(format!("  {chunk}"), th.dim())));
        }
    }
}

/// 디렉터리 용량 드릴다운.
fn what_fills(lines: &mut Vec<Line<'static>>, app: &App, th: Theme, width: usize) {
    let Some(scan) = &app.storage.dir else {
        lines.push(Line::from(Span::styled(
            format!("measuring {}…", app.storage.dir_path.display()),
            th.dim(),
        )));
        lines.push(widgets::blank());
        lines.push(Line::from(
            "Sizes count blocks actually occupied and never cross into another".to_string(),
        ));
        lines.push(Line::from(
            "filesystem, the same way du -x does.".to_string(),
        ));
        return;
    };

    lines.push(Line::from(vec![
        Span::styled(format!("{}", scan.path.display()), th.title()),
        Span::styled(format!("   {}", bytes(scan.total_bytes)), th.accent()),
    ]));
    lines.push(Line::from(Span::styled(
        format!(
            "{} files measured in {:.1}s{}   (⏎ enter, backspace up, J/K move)",
            scan.files_counted,
            scan.elapsed.as_secs_f32(),
            if scan.truncated {
                " - stopped early, totals are lower bounds"
            } else {
                ""
            }
        ),
        th.dim(),
    )));
    lines.push(widgets::blank());

    let name_width = width.saturating_sub(30).max(16);
    for (i, entry) in scan.entries.iter().take(20).enumerate() {
        let share = if scan.total_bytes == 0 {
            0.0
        } else {
            entry.bytes as f32 / scan.total_bytes as f32 * 100.0
        };
        let selected = i == app.row_cursor;
        lines.push(Line::from(vec![
            Span::styled(
                format!("{} ", if selected { th.glyphs.arrow } else { " " }),
                th.accent(),
            ),
            Span::styled(
                format!(
                    "{:>8}  {:<width$}",
                    bytes(entry.bytes),
                    truncate(
                        &format!("{}{}", entry.name, if entry.is_dir { "/" } else { "" }),
                        name_width
                    ),
                    width = name_width
                ),
                if selected {
                    th.selected()
                } else {
                    Default::default()
                },
            ),
            widgets::gauge(share, 10, th),
        ]));
    }
    if scan.entries.is_empty() {
        lines.push(Line::from(Span::styled(
            "  nothing here".to_string(),
            th.dim(),
        )));
    }

    if scan.denied > 0 {
        lines.push(widgets::blank());
        lines.push(Line::from(Span::styled(
            format!(
                "{} director(ies) could not be opened without privileges - the total is a lower bound",
                scan.denied
            ),
            th.warn(),
        )));
    }
    if !scan.crossed_mounts.is_empty() {
        lines.push(Line::from(Span::styled(
            format!(
                "skipped {} mount point(s) belonging to another filesystem",
                scan.crossed_mounts.len()
            ),
            th.dim(),
        )));
    }
}

/// 지웠는데 돌아오지 않은 공간.
fn deleted_held(lines: &mut Vec<Line<'static>>, app: &App, th: Theme) {
    let Some(deleted) = &app.storage.deleted else {
        lines.push(Line::from(Span::styled(
            "looking through open files…".to_string(),
            th.dim(),
        )));
        return;
    };

    let finding = rules::deleted_finding(deleted);
    lines.push(Line::from(vec![
        widgets::verdict_badge(finding.verdict, th),
        Span::raw("  "),
        Span::styled(finding.headline.clone(), th.title()),
    ]));
    lines.push(widgets::blank());

    if deleted.files.is_empty() {
        lines.push(Line::from(
            "  Nothing is holding deleted files. If space is missing, it is being used".to_string(),
        ));
        lines.push(Line::from("  by files that still exist.".to_string()));
    } else {
        lines.push(Line::from(Span::styled(
            format!(
                "  {:>9}  {:>7} {:<16} {}",
                "SIZE", "PID", "PROCESS", "DELETED FILE"
            ),
            th.dim(),
        )));
        for file in deleted.files.iter().take(14) {
            lines.push(Line::from(format!(
                "  {:>9}  {:>7} {:<16} {}",
                file.bytes.map(bytes).unwrap_or_else(|| "-".into()),
                file.pid,
                truncate(&file.process, 16),
                truncate(&file.path, 46)
            )));
        }
    }

    if deleted.partial() {
        lines.push(widgets::blank());
        lines.push(Line::from(Span::styled(
            format!(
                "{} process(es) belong to other users and could not be inspected. Run with sudo",
                deleted.processes_denied
            ),
            th.warn(),
        )));
        lines.push(Line::from(Span::styled(
            "to see all of them - the total above is a lower bound.".to_string(),
            th.warn(),
        )));
    }

    lines.push(widgets::blank());
    lines.push(widgets::section("What these numbers mean", th));
    for chunk in wrap(finding.learn, 76) {
        lines.push(Line::from(Span::styled(format!("  {chunk}"), th.dim())));
    }
    lines.push(widgets::blank());
    lines.push(widgets::section("What you can do", th));
    for chunk in wrap(rules::FullCause::DeletedButHeld.what_to_do(), 76) {
        lines.push(Line::from(format!("  {chunk}")));
    }
}

/// inode 고갈.
fn inodes(lines: &mut Vec<Line<'static>>, app: &App, th: Theme) {
    let findings = rules::inode_findings(&app.storage.mounts);
    if findings.is_empty() {
        lines.push(Line::from(Span::styled(
            "reading filesystems…".to_string(),
            th.dim(),
        )));
        return;
    }
    let worst = findings.iter().max_by_key(|f| f.verdict).unwrap();
    lines.push(Line::from(vec![
        widgets::verdict_badge(worst.verdict, th),
        Span::raw("  "),
        Span::styled(worst.headline.clone(), th.title()),
    ]));
    lines.push(widgets::blank());

    lines.push(Line::from(Span::styled(
        format!(
            "  {:<24} {:>12} {:>12} {:>7}  {:>10}",
            "MOUNT", "INODES USED", "TOTAL", "USE%", "SPACE FREE"
        ),
        th.dim(),
    )));
    for mount in &app.storage.mounts {
        let Some(usage) = mount.usage else { continue };
        let inode_pct = usage.inodes_used_pct();
        let style = match inode_pct {
            Some(p) if p >= 90.0 => th.bad(),
            Some(p) if p >= 85.0 => th.warn(),
            _ => Default::default(),
        };
        lines.push(Line::from(Span::styled(
            format!(
                "  {:<24} {:>12} {:>12} {:>7}  {:>10}",
                truncate(&mount.mount.target, 24),
                usage.inodes_used(),
                if usage.inodes_total == 0 {
                    "n/a".to_string()
                } else {
                    usage.inodes_total.to_string()
                },
                inode_pct.map(pct).unwrap_or_else(|| "-".into()),
                bytes(usage.available_bytes)
            ),
            style,
        )));
    }

    lines.push(widgets::blank());
    lines.push(widgets::section("What these numbers mean", th));
    for chunk in wrap(worst.learn, 76) {
        lines.push(Line::from(Span::styled(format!("  {chunk}"), th.dim())));
    }
    lines.push(widgets::blank());
    lines.push(widgets::section("What you can do", th));
    for chunk in wrap(rules::FullCause::InodesExhausted.what_to_do(), 76) {
        lines.push(Line::from(format!("  {chunk}")));
    }
}

/// 로그 점유.
fn logs(lines: &mut Vec<Line<'static>>, app: &App, th: Theme) {
    let Some(footprint) = &app.storage.logs else {
        lines.push(Line::from(Span::styled(
            "measuring log space…".to_string(),
            th.dim(),
        )));
        return;
    };
    let var = app
        .storage
        .mounts
        .iter()
        .find(|m| m.mount.target == "/var")
        .or_else(|| app.storage.mounts.iter().find(|m| m.mount.target == "/"));
    let finding = rules::log_finding(footprint, var);

    lines.push(Line::from(vec![
        widgets::verdict_badge(finding.verdict, th),
        Span::raw("  "),
        Span::styled(finding.headline.clone(), th.title()),
    ]));
    lines.push(widgets::blank());
    for e in &finding.evidence {
        lines.push(Line::from(format!("  {e}")));
    }
    if !footprint.availability.is_ok() {
        lines.push(widgets::blank());
        lines.push(Line::from(Span::styled(
            format!("  {}", footprint.availability.message()),
            th.warn(),
        )));
    }

    if let Some(scan) = &footprint.var_log {
        lines.push(widgets::blank());
        lines.push(widgets::section("Inside /var/log", th));
        for entry in scan.entries.iter().take(10) {
            lines.push(Line::from(format!(
                "  {:>8}  {}{}",
                bytes(entry.bytes),
                truncate(&entry.name, 40),
                if entry.is_dir { "/" } else { "" }
            )));
        }
    }

    lines.push(widgets::blank());
    lines.push(widgets::section("What these numbers mean", th));
    for chunk in wrap(finding.learn, 76) {
        lines.push(Line::from(Span::styled(format!("  {chunk}"), th.dim())));
    }
    lines.push(widgets::blank());
    lines.push(widgets::section("What you can do", th));
    for chunk in wrap(rules::FullCause::Logs.what_to_do(), 76) {
        lines.push(Line::from(format!("  {chunk}")));
    }
}

/// 드라이브 자기 진단.
fn drives(lines: &mut Vec<Line<'static>>, app: &App, th: Theme) {
    if !app.storage.drives_loaded {
        lines.push(Line::from(Span::styled(
            "asking each drive for its own health report…".to_string(),
            th.dim(),
        )));
        lines.push(widgets::blank());
        lines.push(Line::from(
            "This reads only. It never starts a self-test, which would put load on the drive."
                .to_string(),
        ));
        return;
    }
    let findings = rules::drive_findings(&app.storage.drives);
    if findings.is_empty() {
        lines.push(Line::from(
            "No drives on this system report self-diagnosis data.".to_string(),
        ));
        return;
    }
    for finding in &findings {
        lines.push(Line::from(vec![
            widgets::verdict_badge(finding.verdict, th),
            Span::raw("  "),
            Span::styled(finding.headline.clone(), th.title()),
        ]));
        for e in &finding.evidence {
            let style = if e.starts_with("! ") {
                th.warn()
            } else {
                th.dim()
            };
            lines.push(Line::from(Span::styled(format!("     {e}"), style)));
        }
        lines.push(widgets::blank());
    }

    // 값의 뜻을 함께 싣는다.
    let explained: Vec<&crate::collect::smart::HealthAttr> = app
        .storage
        .drives
        .iter()
        .flat_map(|d| d.attributes.iter())
        .filter(|a| a.concern)
        .collect();
    if !explained.is_empty() {
        lines.push(widgets::section("What the flagged values mean", th));
        for attr in explained.iter().take(4) {
            lines.push(Line::from(Span::styled(
                format!("  {}", attr.label),
                th.warn(),
            )));
            for chunk in wrap(attr.explain, 74) {
                lines.push(Line::from(Span::styled(format!("    {chunk}"), th.dim())));
            }
        }
    } else if let Some(finding) = findings.first() {
        lines.push(widgets::section("What these numbers mean", th));
        for chunk in wrap(finding.learn, 76) {
            lines.push(Line::from(Span::styled(format!("  {chunk}"), th.dim())));
        }
    }
}

/// 커널이 남긴 저장장치 오류.
fn errors(lines: &mut Vec<Line<'static>>, app: &App, th: Theme) {
    let Some(errors) = &app.storage.errors else {
        lines.push(Line::from(Span::styled(
            "searching the kernel log…".to_string(),
            th.dim(),
        )));
        return;
    };
    let findings = rules::filesystem_findings(&[], Some(errors), &[]);
    for finding in &findings {
        lines.push(Line::from(vec![
            widgets::verdict_badge(finding.verdict, th),
            Span::raw("  "),
            Span::styled(finding.headline.clone(), th.title()),
        ]));
        lines.push(widgets::blank());
        for e in &finding.evidence {
            lines.push(Line::from(Span::styled(format!("  {e}"), th.dim())));
        }
        lines.push(widgets::blank());
        lines.push(widgets::section("What these numbers mean", th));
        for chunk in wrap(finding.learn, 76) {
            lines.push(Line::from(Span::styled(format!("  {chunk}"), th.dim())));
        }
    }

    if !errors.hits.is_empty() {
        lines.push(widgets::blank());
        lines.push(widgets::section("Patterns found", th));
        for hit in &errors.hits {
            lines.push(Line::from(vec![
                Span::styled(format!("  {} x ", hit.count), th.warn()),
                Span::raw(hit.pattern.to_string()),
            ]));
            for chunk in wrap(hit.meaning, 72) {
                lines.push(Line::from(Span::styled(format!("      {chunk}"), th.dim())));
            }
        }
    }
}

/// 장치 구성.
fn layout(lines: &mut Vec<Line<'static>>, app: &App, th: Theme) {
    if app.storage.devices.is_empty() {
        lines.push(Line::from(Span::styled(
            "reading /sys/block…".to_string(),
            th.dim(),
        )));
        return;
    }
    for device in &app.storage.devices {
        lines.push(Line::from(vec![
            Span::styled(format!("{:<10}", device.name), th.title()),
            Span::raw(format!(
                "{:>9}  {:<4}  {}",
                bytes(device.size_bytes),
                device.kind(),
                truncate(device.model.trim(), 34)
            )),
        ]));
        if let Some(mount) = &device.mount_point {
            lines.push(Line::from(Span::styled(
                format!(
                    "           mounted at {mount} ({})",
                    device.fstype.clone().unwrap_or_default()
                ),
                th.dim(),
            )));
        }
        for part in &device.partitions {
            let mount = part
                .mount_point
                .clone()
                .unwrap_or_else(|| "not mounted".into());
            lines.push(Line::from(vec![
                Span::styled(format!("  └─ {:<12}", part.name), th.dim()),
                Span::raw(format!(
                    "{:>9}  {:<8} {}",
                    bytes(part.size_bytes),
                    part.fstype.clone().unwrap_or_else(|| "-".into()),
                    mount
                )),
                if part.read_only {
                    Span::styled("  read-only".to_string(), th.warn())
                } else {
                    Span::raw("")
                },
            ]));
        }
        if !device.used_by.is_empty() {
            lines.push(Line::from(Span::styled(
                format!("           used by {}", device.used_by.join(", ")),
                th.dim(),
            )));
        }
    }

    if !app.storage.raid.is_empty() {
        lines.push(widgets::blank());
        lines.push(widgets::section("Software RAID", th));
        for finding in rules::filesystem_findings(&[], None, &app.storage.raid) {
            lines.push(Line::from(vec![
                Span::raw("  "),
                widgets::verdict_badge(finding.verdict, th),
                Span::raw("  "),
                Span::raw(finding.headline.clone()),
            ]));
            for e in &finding.evidence {
                lines.push(Line::from(Span::styled(format!("      {e}"), th.dim())));
            }
        }
    }

    lines.push(widgets::blank());
    lines.push(Line::from(Span::styled(
        "HDD and SSD are not interchangeable when judging performance: 20ms of wait is".to_string(),
        th.dim(),
    )));
    lines.push(Line::from(Span::styled(
        "ordinary for a spinning disk and alarming for an SSD.".to_string(),
        th.dim(),
    )));
}

/// 파일시스템 상태.
fn fs_health(lines: &mut Vec<Line<'static>>, app: &App, th: Theme) {
    if app.storage.filesystems.is_empty() {
        lines.push(Line::from(Span::styled(
            "reading mount options…".to_string(),
            th.dim(),
        )));
        return;
    }

    let findings = rules::filesystem_findings(
        &app.storage.filesystems,
        app.storage.errors.as_ref(),
        &app.storage.raid,
    );
    let problems: Vec<_> = findings
        .iter()
        .filter(|f| f.verdict >= crate::analyze::Verdict::Warn)
        .collect();

    if problems.is_empty() {
        lines.push(Line::from(vec![
            widgets::verdict_badge(crate::analyze::Verdict::Ok, th),
            Span::raw("  "),
            Span::styled(
                "No filesystem has flagged a problem".to_string(),
                th.title(),
            ),
        ]));
    } else {
        for finding in problems {
            lines.push(Line::from(vec![
                widgets::verdict_badge(finding.verdict, th),
                Span::raw("  "),
                Span::styled(finding.headline.clone(), th.title()),
            ]));
            for e in &finding.evidence {
                lines.push(Line::from(Span::styled(format!("     {e}"), th.dim())));
            }
        }
    }

    lines.push(widgets::blank());
    lines.push(Line::from(Span::styled(
        format!(
            "  {:<22} {:<8} {:<6} {:>10}  {:<28}",
            "MOUNT", "TYPE", "MODE", "RESERVED", "OPTIONS"
        ),
        th.dim(),
    )));
    for fs in &app.storage.filesystems {
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::raw(format!(
                "{:<22} {:<8} ",
                truncate(&fs.target, 22),
                truncate(&fs.fstype, 8)
            )),
            if fs.read_only {
                Span::styled("ro    ".to_string(), th.warn())
            } else {
                Span::styled("rw    ".to_string(), th.dim())
            },
            Span::raw(format!(
                "{:>10}  {}",
                bytes(fs.reserved_bytes),
                truncate(&fs.notable_options.join(","), 28)
            )),
        ]));
        if let Some(count) = fs.ext4_errors
            && count > 0
        {
            lines.push(Line::from(Span::styled(
                format!("      ext4 has recorded {count} error(s) on this filesystem"),
                th.warn(),
            )));
        }
    }

    lines.push(widgets::blank());
    lines.push(widgets::section("What these numbers mean", th));
    for chunk in wrap(
        "'Reserved' is space only root can use - normally 5% on ext4. It is why a filesystem can \
         show free space and still refuse writes from a normal user. 'errors=remount-ro' means the \
         kernel will make the filesystem read-only rather than risk more damage.",
        76,
    ) {
        lines.push(Line::from(Span::styled(format!("  {chunk}"), th.dim())));
    }
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
