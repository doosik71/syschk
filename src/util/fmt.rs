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

/// KiB 값을 사람이 읽는 크기로.
pub fn kib(kb: u64) -> String {
    bytes(kb.saturating_mul(1024))
}

/// 바이트 값을 사람이 읽는 크기로.
pub fn bytes(b: u64) -> String {
    const UNITS: [&str; 5] = ["B", "K", "M", "G", "T"];
    let mut value = b as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{}B", b)
    } else if value >= 100.0 {
        format!("{value:.0}{}", UNITS[unit])
    } else {
        format!("{value:.1}{}", UNITS[unit])
    }
}

/// 초당 바이트.
pub fn bytes_per_sec(b: f32) -> String {
    if b < 1.0 {
        return "0".into();
    }
    format!("{}/s", bytes(b as u64))
}

/// 백분율.
pub fn pct(v: f32) -> String {
    if v >= 99.95 {
        "100%".into()
    } else if v >= 10.0 {
        format!("{v:.0}%")
    } else {
        format!("{v:.1}%")
    }
}
