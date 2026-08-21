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
