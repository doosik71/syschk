//! `syschk check` — 한 번 점검하고 요약을 출력한다. 스크립트·cron 용.
//!
//! 종료 코드: 0 = 확인된 문제 없음, 1 = 주의 항목 있음, 2 = 점검 자체를 못 함.
//! M1 이후 각 축의 판정이 이 요약에 더해진다.

use crate::collect::{ProbeCtx, probes};
use crate::tools::detect;

pub fn run() -> i32 {
    let ctx = ProbeCtx::default();
    let mut failures = 0;

    println!("syschk check (read-only)");
    for probe in probes() {
        let result = probe.run(&ctx);
        if result.availability.is_ok() {
            if let crate::collect::ProbeData::Fields(fields) = &result.data {
                let rendered = fields
                    .iter()
                    .filter(|f| !f.value.is_empty())
                    .map(|f| format!("{}={}", f.label, f.value))
                    .collect::<Vec<_>>()
                    .join(" ");
                println!("  ok      {:<18} {rendered}", probe.id());
            }
        } else {
            failures += 1;
            println!(
                "  skipped {:<18} {}",
                probe.id(),
                result.availability.message()
            );
        }
    }

    let inventory = detect::scan();
    println!(
        "  tools   {} installed, {} missing, {} n/a",
        inventory.installed(),
        inventory.missing(),
        inventory.not_applicable()
    );
    if inventory.missing() > 0 {
        println!("          run 'syschk doctor' to see what the missing ones would give you");
    }

    println!(
        "\nHealth verdicts per axis (cpu, memory, disk, network) arrive with the\n\
         collectors in M1. What is above is what syschk can state today."
    );

    if failures > 0 { 1 } else { 0 }
}
