//! 압박 지표(PSI, `/proc/pressure/*`).
//!
//! "무엇 때문에 느린가"를 가장 직접적으로 말해주는 값이다. 사용률과 달리
//! **일이 실제로 지연되고 있는 시간의 비율**을 나타내므로, 축 사이 비교가 가능하다.
//! 커널 설정(CONFIG_PSI)에 따라 없을 수 있으므로 항상 `Option` 으로 다룬다.

use super::ProbeCtx;

/// 한 축의 압박 지표.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pressure {
    /// 일부 작업이 지연된 시간 비율(최근 10초).
    pub some_avg10: f32,
    pub some_avg60: f32,
    /// 모든 작업이 멈춘 시간 비율(최근 10초). CPU 축에는 없다.
    pub full_avg10: f32,
}

impl Pressure {
    fn parse(text: &str) -> Self {
        let mut p = Pressure::default();
        for line in text.lines() {
            let full = line.starts_with("full");
            for token in line.split_whitespace() {
                let Some((key, value)) = token.split_once('=') else {
                    continue;
                };
                let Ok(v) = value.parse::<f32>() else {
                    continue;
                };
                match (full, key) {
                    (false, "avg10") => p.some_avg10 = v,
                    (false, "avg60") => p.some_avg60 = v,
                    (true, "avg10") => p.full_avg10 = v,
                    _ => {}
                }
            }
        }
        p
    }
}

/// CPU·메모리·I/O 압박 지표 묶음.
#[derive(Clone, Copy, Debug, Default)]
pub struct PressureSet {
    pub cpu: Option<Pressure>,
    pub memory: Option<Pressure>,
    pub io: Option<Pressure>,
}

impl PressureSet {
    pub fn read(ctx: &ProbeCtx) -> Self {
        Self {
            cpu: ctx.read("/proc/pressure/cpu").map(|t| Pressure::parse(&t)),
            memory: ctx
                .read("/proc/pressure/memory")
                .map(|t| Pressure::parse(&t)),
            io: ctx.read("/proc/pressure/io").map(|t| Pressure::parse(&t)),
        }
    }

    /// 커널이 압박 지표를 제공하지 않는 경우.
    pub fn available(&self) -> bool {
        self.cpu.is_some() || self.memory.is_some() || self.io.is_some()
    }
}
