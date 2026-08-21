//! 색상과 글리프. 유니코드·색상 미지원 터미널로 폴백한다(NFR-6).

use ratatui::style::{Color, Modifier, Style};

/// 상태 표시용 글리프 묶음.
#[derive(Clone, Copy, Debug)]
pub struct Glyphs {
    pub ok: &'static str,
    pub warn: &'static str,
    pub missing: &'static str,
    pub na: &'static str,
    pub bullet: &'static str,
    pub arrow: &'static str,
}

const UNICODE: Glyphs = Glyphs {
    ok: "✔",
    warn: "⚠",
    missing: "✖",
    na: "–",
    bullet: "·",
    arrow: "›",
};

const ASCII: Glyphs = Glyphs {
    ok: "+",
    warn: "!",
    missing: "x",
    na: "-",
    bullet: "*",
    arrow: ">",
};

/// 테마. 현재는 터미널 능력으로만 결정한다.
#[derive(Clone, Copy, Debug)]
pub struct Theme {
    pub glyphs: Glyphs,
}

impl Theme {
    pub fn detect() -> Self {
        Self {
            glyphs: if unicode_capable() { UNICODE } else { ASCII },
        }
    }

    pub fn title(self) -> Style {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    }

    pub fn dim(self) -> Style {
        Style::default().add_modifier(Modifier::DIM)
    }

    pub fn selected(self) -> Style {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    }

    pub fn ok(self) -> Style {
        Style::default().fg(Color::Green)
    }

    pub fn warn(self) -> Style {
        Style::default().fg(Color::Yellow)
    }

    pub fn bad(self) -> Style {
        Style::default().fg(Color::Red)
    }

    pub fn accent(self) -> Style {
        Style::default().fg(Color::Cyan)
    }

    pub fn key(self) -> Style {
        Style::default()
            .fg(Color::Magenta)
            .add_modifier(Modifier::BOLD)
    }
}

fn unicode_capable() -> bool {
    ["LC_ALL", "LC_CTYPE", "LANG"].iter().any(|var| {
        std::env::var(var)
            .map(|v| v.to_uppercase().contains("UTF-8") || v.to_uppercase().contains("UTF8"))
            .unwrap_or(false)
    })
}
