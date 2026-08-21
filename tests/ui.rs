//! 화면 렌더링. 레이아웃이 깨지거나 패닉하지 않는지, 핵심 문구가 보이는지 확인한다.

mod common;

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use syschk::app::state::App;
use syschk::tasks::{MENU, Screen};
use syschk::ui;

fn terminal(w: u16, h: u16) -> Terminal<TestBackend> {
    Terminal::new(TestBackend::new(w, h)).expect("test terminal")
}

fn press(app: &mut App, code: KeyCode) {
    app.on_key(KeyEvent::new(code, KeyModifiers::NONE));
}

#[test]
fn home_screen_shows_the_question_and_the_menu() {
    let mut term = terminal(120, 34);
    let app = App::new();
    term.draw(|f| ui::draw(f, &app)).unwrap();
    let screen = common::rendered(&term);

    assert!(screen.contains("What do you want to do?"));
    assert!(screen.contains("syschk"));
    assert!(screen.contains("read-only"));
    // 메뉴 첫 항목과 마지막 항목이 보인다.
    assert!(screen.contains("See what the system is doing right now"));
    assert!(screen.contains("Get the tools diagnosis needs"));
}

#[test]
fn every_screen_renders() {
    for screen in MENU {
        let mut term = terminal(120, 34);
        let mut app = App::new();
        app.screen = screen;
        term.draw(|f| ui::draw(f, &app)).unwrap();
        let out = common::rendered(&term);
        assert!(
            out.contains(&format!("{}.", screen.number())) || screen == Screen::Tools,
            "screen {} did not render its heading",
            screen.number()
        );
        // 도구 화면은 작업이 아니라 도구 목록을 보여준다.
        if screen == Screen::Tools {
            assert!(out.contains("Tools diagnosis needs"));
            continue;
        }
        // 그 밖의 화면은 첫 작업 제목이 목록에 보인다.
        if let Some(first) = screen.tasks().first() {
            let head: String = first.title.chars().take(20).collect();
            assert!(
                out.contains(&head),
                "screen {} does not list its first task",
                screen.number()
            );
        }
    }
}

#[test]
fn narrow_terminal_still_renders() {
    let mut term = terminal(80, 24);
    let app = App::new();
    term.draw(|f| ui::draw(f, &app)).unwrap();
    let out = common::rendered(&term);
    assert!(out.contains("What do you want to do?"));
}

#[test]
fn number_keys_jump_including_two_digit_menu_items() {
    let mut app = App::new();
    press(&mut app, KeyCode::Char('4'));
    assert_eq!(app.cursor(), 3);
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.screen, Screen::Storage);

    press(&mut app, KeyCode::Esc);
    assert_eq!(app.screen, Screen::Home);

    // "1" 다음 "4" → 14번 항목.
    press(&mut app, KeyCode::Char('1'));
    assert_eq!(app.cursor(), 0);
    press(&mut app, KeyCode::Char('4'));
    assert_eq!(app.cursor(), 13);
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.screen, Screen::Tools);
}

#[test]
fn escape_goes_back_then_quits() {
    let mut app = App::new();
    press(&mut app, KeyCode::Enter);
    assert_ne!(app.screen, Screen::Home);
    press(&mut app, KeyCode::Esc);
    assert_eq!(app.screen, Screen::Home);
    assert!(!app.quit);
    press(&mut app, KeyCode::Esc);
    assert!(app.quit);
}

#[test]
fn commands_are_hidden_until_asked_for() {
    let mut app = App::new();
    app.screen = Screen::Storage;
    let mut term = terminal(120, 34);
    term.draw(|f| ui::draw(f, &app)).unwrap();
    let collapsed = common::rendered(&term);
    assert!(collapsed.contains("press c to show them"));

    press(&mut app, KeyCode::Char('c'));
    term.draw(|f| ui::draw(f, &app)).unwrap();
    let expanded = common::rendered(&term);
    assert!(expanded.contains("read-only"));
    assert!(expanded.contains("df -hT") || expanded.contains("df -i"));
}

#[test]
fn help_overlay_states_the_non_destructive_promise() {
    let mut app = App::new();
    press(&mut app, KeyCode::Char('?'));
    let mut term = terminal(120, 40);
    term.draw(|f| ui::draw(f, &app)).unwrap();
    let out = common::rendered(&term);
    assert!(out.contains("Help"));
    assert!(out.contains("does not change anything"));
    // 아무 키나 누르면 닫힌다.
    press(&mut app, KeyCode::Char('x'));
    assert!(app.overlay.is_none());
}

#[test]
fn search_overlay_finds_a_task_and_jumps_to_it() {
    let mut app = App::new();
    press(&mut app, KeyCode::Char('/'));
    for ch in "disk full".chars() {
        press(&mut app, KeyCode::Char(ch));
    }
    let mut term = terminal(120, 34);
    term.draw(|f| ui::draw(f, &app)).unwrap();
    let out = common::rendered(&term);
    assert!(out.contains("Search by symptom"));
    assert!(out.contains("Which filesystem is full"));

    press(&mut app, KeyCode::Enter);
    assert!(app.overlay.is_none());
    assert_eq!(app.screen, Screen::Storage);
    assert_eq!(
        app.selected_task().map(|t| t.id),
        Some("storage.which-full")
    );
}

#[test]
fn tools_screen_shows_status_and_install_guidance() {
    let mut app = App::new();
    press(&mut app, KeyCode::Char('t'));
    assert_eq!(app.screen, Screen::Tools);
    let mut term = terminal(120, 40);
    term.draw(|f| ui::draw(f, &app)).unwrap();
    let out = common::rendered(&term);
    assert!(out.contains("Tools diagnosis needs"));
    assert!(out.contains("installed") || out.contains("missing") || out.contains("n/a"));
}

#[test]
fn moving_the_cursor_wraps_around() {
    let mut app = App::new();
    press(&mut app, KeyCode::Up);
    assert_eq!(app.cursor(), MENU.len() - 1);
    press(&mut app, KeyCode::Down);
    assert_eq!(app.cursor(), 0);
}

// ── M1: 실시간 화면 ────────────────────────────────────────────────

/// 표본 두 개를 모아 비율이 계산된 상태를 만든다.
fn warmed_app(screen: Screen) -> App {
    let mut app = App::new();
    app.screen = screen;
    app.sampler.tick();
    std::thread::sleep(std::time::Duration::from_millis(120));
    app.sampler.tick();
    app
}

#[test]
fn live_screen_shows_the_four_axes_and_a_process_table() {
    let app = warmed_app(Screen::Live);
    let mut term = terminal(140, 40);
    term.draw(|f| ui::draw(f, &app)).unwrap();
    let out = common::rendered(&term);

    for axis in ["cpu", "memory", "disk", "network"] {
        assert!(out.contains(axis), "live view is missing the {axis} panel");
    }
    assert!(out.contains("load"), "load average should be on screen");
    assert!(out.contains("Per-core usage") || out.contains("How it reads"));
}

#[test]
fn live_screen_reports_real_numbers() {
    let app = warmed_app(Screen::Live);
    assert!(app.sampler.cores > 0, "core count should be detected");
    assert!(app.sampler.memory.total > 0, "memory total should be read");
    assert!(!app.sampler.procs.is_empty(), "processes should be listed");
    assert!(!app.sampler.warming_up(), "two samples were taken");
    // 사용률은 0~100 범위를 벗어나지 않는다.
    assert!((0.0..=100.0).contains(&app.sampler.cpu.busy));
    assert!((0.0..=100.0).contains(&app.sampler.memory.used_pct()));
}

#[test]
fn process_list_can_be_sorted_and_pinned() {
    let mut app = warmed_app(Screen::Live);
    // "Who is using the CPU right now" 로 이동하면 CPU 정렬이 된다.
    press(&mut app, KeyCode::Down);
    assert_eq!(app.sort, syschk::app::state::ProcSort::Cpu);

    press(&mut app, KeyCode::Char('s'));
    assert_eq!(app.sort, syschk::app::state::ProcSort::Memory);
    let by_mem = app.sorted_procs();
    if by_mem.len() > 1 {
        assert!(
            by_mem[0].rss_kb >= by_mem[1].rss_kb,
            "memory sort must be descending"
        );
    }

    // 고정한 프로세스는 정렬과 무관하게 맨 위로 온다.
    let target = app.sorted_procs().last().map(|p| p.pid).unwrap();
    app.row_cursor = app.sorted_procs().len() - 1;
    press(&mut app, KeyCode::Char('p'));
    assert_eq!(app.pinned, Some(target));
    assert_eq!(app.sorted_procs()[0].pid, target);

    press(&mut app, KeyCode::Char('p'));
    assert_eq!(app.pinned, None, "pressing p again unpins");
}

#[test]
fn freezing_holds_the_values_still() {
    let mut app = warmed_app(Screen::Live);
    press(&mut app, KeyCode::Char('f'));
    assert!(app.frozen);
    let samples = app.sampler.samples;
    app.maybe_sample();
    assert_eq!(app.sampler.samples, samples, "frozen means no new samples");

    press(&mut app, KeyCode::Char('f'));
    assert!(!app.frozen);
}

#[test]
fn other_screens_do_not_sample_at_all() {
    let mut app = App::new();
    app.screen = Screen::Storage;
    let samples = app.sampler.samples;
    app.maybe_sample();
    assert_eq!(
        app.sampler.samples, samples,
        "sampling only happens on the live screens"
    );
}

#[test]
fn slow_screen_names_a_bottleneck_with_evidence() {
    let app = warmed_app(Screen::Slow);
    let mut term = terminal(140, 40);
    term.draw(|f| ui::draw(f, &app)).unwrap();
    let out = common::rendered(&term);

    assert!(out.contains("Why is it slow"));
    // 결론 문장 중 하나가 보여야 한다.
    assert!(
        out.contains("bottleneck") || out.contains("saturated") || out.contains("Measuring"),
        "the verdict headline is missing"
    );
    assert!(out.contains("cpu") && out.contains("memory") && out.contains("disk"));
}

#[test]
fn slow_screen_explains_what_the_numbers_mean() {
    let mut app = warmed_app(Screen::Slow);
    // 두 번째 항목: "Am I running out of CPU"
    press(&mut app, KeyCode::Down);
    let mut term = terminal(140, 40);
    term.draw(|f| ui::draw(f, &app)).unwrap();
    let out = common::rendered(&term);
    assert!(out.contains("Evidence"), "evidence section is missing");
    assert!(
        out.contains("What these numbers mean"),
        "the explanation that teaches the metric is missing"
    );
}

#[test]
fn slow_screen_offers_the_equivalent_commands_to_type() {
    let mut app = warmed_app(Screen::Slow);
    press(&mut app, KeyCode::Char('c'));
    let mut term = terminal(140, 44);
    term.draw(|f| ui::draw(f, &app)).unwrap();
    let out = common::rendered(&term);
    assert!(out.contains("What syschk reads"));
    assert!(
        out.contains("Try it yourself"),
        "learning commands should be offered"
    );
}

#[test]
fn planned_tasks_say_so_instead_of_pretending() {
    let mut app = warmed_app(Screen::Slow);
    // "Was it always like this - compare with the past" 는 M3 예정이다.
    app.goto_task("slow.compare-past");
    let mut term = terminal(140, 40);
    term.draw(|f| ui::draw(f, &app)).unwrap();
    let out = common::rendered(&term);
    assert!(out.contains("Not built yet"), "planned work must be honest");
    assert!(out.contains("M3"));
}

/// 화면을 텍스트로 덤프한다. 사람이 눈으로 확인하기 위한 것이므로 기본 실행에서는 건너뛴다.
///
/// `cargo test --test ui -- --ignored --nocapture screenshot` 로 본다.
#[test]
#[ignore = "prints screens for visual inspection"]
fn screenshot() {
    for screen in [Screen::Home, Screen::Live, Screen::Slow, Screen::Tools] {
        let mut app = warmed_app(screen);
        if screen == Screen::Live {
            press(&mut app, KeyCode::Down); // "Who is using the CPU right now"
        }
        if screen == Screen::Slow {
            press(&mut app, KeyCode::Down); // "Am I running out of CPU"
            press(&mut app, KeyCode::Char('c'));
        }
        let mut term = terminal(150, 40);
        term.draw(|f| ui::draw(f, &app)).unwrap();
        println!("\n=== {:?} ===\n{}", screen, common::rendered(&term));
    }
}
