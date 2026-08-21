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
        terminal.draw(|frame| crate::ui::draw(frame, &app))?;

        // 입력을 기다린다. 타임아웃은 창 크기 변경 등에 대한 재그리기 여유.
        if event::poll(Duration::from_millis(500))? {
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
