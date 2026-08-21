//! 판정 계층. 수치를 사람이 읽을 수 있는 결론으로 바꾼다.
//!
//! 판정에는 반드시 **근거 수치**와 **한 줄 설명**이 함께 붙는다. 설명은 사용자가
//! 이 앱을 쓰면서 지표의 의미를 자연스럽게 익히도록 하기 위한 것이다.

pub mod bottleneck;
pub mod rules;

pub use bottleneck::{Assessment, Axis, Metrics, assess};
pub use rules::{Finding, Verdict};
