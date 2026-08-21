//! `syschk check` — 한 번 점검하고 요약을 출력한다. 스크립트·cron 용.
//!
//! 종료 코드
//! * 0 — 확인된 문제 없음
//! * 1 — 주의(warning) 항목 있음
//! * 2 — 심각(critical) 항목 있음
//! * 3 — 점검 자체를 하지 못함

use crate::analyze::Verdict;
use crate::analyze::storage as storage_rules;
use crate::app::sampler::Sampler;
use crate::collect::{ProbeCtx, mounts};
use crate::tools::detect;
use crate::util::fmt::duration_human;
use std::time::Duration;

pub fn run() -> i32 {
    let ctx = ProbeCtx::default();

    // 비율 지표는 두 표본이 필요하다. 1초를 기다린다.
    let mut sampler = Sampler::new(ctx.clone());
    sampler.tick();
    std::thread::sleep(Duration::from_secs(1));
    sampler.tick();

    if sampler.warming_up() {
        eprintln!("could not sample /proc - is this a Linux system?");
        return 3;
    }

    let uptime = ctx
        .read("/proc/uptime")
        .and_then(|t| {
            t.split_whitespace()
                .next()
                .and_then(|v| v.parse::<f64>().ok())
        })
        .map(|s| duration_human(s as u64))
        .unwrap_or_else(|| "unknown".into());
    let host = ctx
        .read("/proc/sys/kernel/hostname")
        .map(|h| h.trim().to_string())
        .unwrap_or_else(|| "unknown".into());

    println!("syschk check (read-only)  host {host}  up {uptime}");

    let assessment = sampler.assessment();
    println!("\n{}", assessment.headline);
    for f in &assessment.findings {
        let mark = match f.verdict {
            Verdict::Ok => "ok      ",
            Verdict::Warn => "WARNING ",
            Verdict::Critical => "CRITICAL",
            Verdict::Unknown => "unknown ",
        };
        println!("  {mark} {:<8} {}", f.axis, f.headline);
        for e in &f.evidence {
            println!("           {e}");
        }
    }

    // 저장 공간은 statvfs 만 쓰므로 비용이 거의 없다. 여기서 함께 본다.
    let mount_usage = mounts::usage(&ctx);
    let mut storage_findings = storage_rules::space_findings(&mount_usage);
    storage_findings.extend(storage_rules::inode_findings(&mount_usage));
    let storage_worst = storage_findings
        .iter()
        .map(|f| f.verdict)
        .max()
        .unwrap_or(Verdict::Ok);
    println!("\nStorage");
    let notable: Vec<_> = storage_findings
        .iter()
        .filter(|f| f.verdict >= Verdict::Warn)
        .collect();
    if notable.is_empty() {
        println!(
            "  ok       space    every filesystem has room and inodes to spare ({} checked)",
            mount_usage.len()
        );
    } else {
        for f in notable {
            let mark = match f.verdict {
                Verdict::Critical => "CRITICAL",
                Verdict::Warn => "WARNING ",
                _ => "ok      ",
            };
            println!("  {mark} {:<8} {}", f.axis, f.headline);
            for e in &f.evidence {
                println!("           {e}");
            }
        }
    }

    let inventory = detect::scan();
    println!(
        "\ntools: {} installed, {} missing, {} not applicable",
        inventory.installed(),
        inventory.missing(),
        inventory.not_applicable()
    );
    if inventory.missing() > 0 {
        println!("run 'syschk doctor' to see what the missing ones would give you");
    }

    match assessment.worst().max(storage_worst) {
        Verdict::Critical => 2,
        Verdict::Warn => 1,
        _ => 0,
    }
}
