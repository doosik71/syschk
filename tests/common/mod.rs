//! 시험 공용 헬퍼.
//!
//! 시험 바이너리마다 쓰는 헬퍼가 달라 일부는 사용되지 않는다.
#![allow(dead_code)]

use ratatui::Terminal;
use ratatui::backend::TestBackend;

/// 렌더된 화면을 문자열로 만든다.
pub fn rendered(terminal: &Terminal<TestBackend>) -> String {
    let buffer = terminal.backend().buffer();
    let area = buffer.area();
    let mut out = String::new();
    for y in 0..area.height {
        for x in 0..area.width {
            out.push_str(buffer[(x, y)].symbol());
        }
        out.push('\n');
    }
    out
}

/// 픽스처 디렉터리 경로.
pub fn fixture_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}
