//! syschk — 우분투 시스템 상태 점검과 장애 원인 분석을 돕는 TUI.
//!
//! 이 크레이트는 네 계층으로 나뉜다.
//!
//! * [`tasks`] — 사용자가 보는 "무엇을 하고 싶은가"(화면과 작업)
//! * [`tools`] — 진단에 필요한 외부 도구와 설치 안내
//! * [`collect`] — 자료 수집기(`Probe`)와 가용성 처리
//! * [`ui`] / [`app`] — 표시와 상태 전이
//!
//! 세 계층 모두 "등록만으로 확장"되도록 설계했다. 새 기능은 레지스트리에 항목을
//! 추가하는 일로 끝나며, 기존 화면 코드를 고치지 않는다.
//!
//! 목적은 **정밀하지만 비파괴적인 진단**이다. 시스템을 바꾸는 명령은 실행하지 않으며,
//! 이 원칙은 [`util::exec`] 의 읽기 전용 정책으로 강제된다.

pub mod app;
pub mod cli;
pub mod collect;
pub mod commands;
pub mod tasks;
pub mod tools;
pub mod ui;
pub mod util;
