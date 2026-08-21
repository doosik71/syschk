//! 이벤트 루프. 입력이 없으면 잠들어 있으므로 유휴 부하가 거의 없다(NFR-1).

use super::state::App;
use anyhow::Result;
use ratatui::crossterm::event::{self, Event};
use std::time::Duration;

/// TUI 를 실행한다. 종료 시 터미널 상태를 반드시 되돌린다.
pub fn run() -> Result<()> {
    let mut terminal = ratatui::init();
    let result = event_loop(&mut terminal);
    ratatui::restore();
    result
}

fn event_loop(terminal: &mut ratatui::DefaultTerminal) -> Result<()> {
    let mut app = App::new();
    loop {
        // 실시간 화면에서는 정해진 간격으로 표본을 뜨고, 다른 화면에서는 아무 것도 읽지 않는다.
        app.maybe_sample();
        // 배경 작업 결과를 받고, 이 화면에 필요한 수집을 시작한다.
        app.poll_collectors();
        terminal.draw(|frame| crate::ui::draw(frame, &app))?;

        // 갱신이 필요한 동안에는 짧게, 그 밖에는 입력만 기다린다.
        let wait = if (app.live_active() && !app.frozen) || app.jobs.busy() {
            Duration::from_millis(250)
        } else {
            Duration::from_millis(1000)
        };
        if event::poll(wait)? {
            match event::read()? {
                Event::Key(key) => app.on_key(key),
                Event::Resize(_, _) => {}
                _ => {}
            }
        }
        if app.quit {
            return Ok(());
        }
    }
}
