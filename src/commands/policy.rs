//! `syschk policy` — 읽기 전용 정책 공개.
//!
//! "이 앱이 정말 시스템을 안 바꾸는가"를 사용자가 직접 확인할 수 있어야 한다.

use crate::tasks::registry;
use crate::util::exec::{ReadOnlyCommand, allowlisted_programs};

pub fn run() -> i32 {
    let programs = allowlisted_programs();
    println!("Read-only command policy");
    println!(
        "syschk can only run programs on this list, and only with arguments that\n\
         cannot change the system. Anything else is refused before it runs.\n"
    );
    println!("{} programs allowed:", programs.len());
    for chunk in programs.chunks(8) {
        println!("  {}", chunk.join("  "));
    }

    // 카탈로그에 등록된 모든 명령을 정책으로 검증해 보고한다.
    let mut checked = 0;
    let mut rejected = Vec::new();
    for task in registry::tasks() {
        for cmd in task.commands {
            checked += 1;
            if let Err(e) = ReadOnlyCommand::parse(cmd) {
                rejected.push(format!("{} :: {cmd} :: {e}", task.id));
            }
        }
    }

    println!("\ncatalogue check: {checked} command(s) declared by tasks");
    if rejected.is_empty() {
        println!("  all of them pass the read-only policy");
        0
    } else {
        println!("  {} would be refused:", rejected.len());
        for r in &rejected {
            println!("    {r}");
        }
        1
    }
}
