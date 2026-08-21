//! 표시용 서식 헬퍼.

/// 초를 "3d 4h 12m" 형태로 만든다.
pub fn duration_human(secs: u64) -> String {
    let d = secs / 86_400;
    let h = (secs % 86_400) / 3_600;
    let m = (secs % 3_600) / 60;
    if d > 0 {
        format!("{d}d {h}h {m}m")
    } else if h > 0 {
        format!("{h}h {m}m")
    } else {
        format!("{m}m")
    }
}

/// 문자열을 폭에 맞게 자른다(말줄임 포함).
pub fn truncate(s: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= width {
        return s.to_string();
    }
    if width <= 1 {
        return chars[..width].iter().collect();
    }
    let mut out: String = chars[..width - 1].iter().collect();
    out.push('…');
    out
}
