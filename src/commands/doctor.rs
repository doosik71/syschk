//! `syschk doctor` — 진단 도구 설치 상태와 설치 안내.
//!
//! 종료 코드: 0 = 빠진 도구 없음, 1 = 빠진 도구 있음.

use crate::tools::detect;
use crate::tools::registry;
use crate::tools::{Bundle, ToolStatus};

pub fn run(bundle_filter: Option<&str>, only_missing: bool) -> i32 {
    let inventory = detect::scan();
    let wanted: Option<Bundle> = bundle_filter.and_then(parse_bundle);
    if bundle_filter.is_some() && wanted.is_none() {
        eprintln!(
            "unknown bundle '{}'. valid: {}",
            bundle_filter.unwrap_or(""),
            Bundle::ALL
                .iter()
                .map(|b| b.label())
                .collect::<Vec<_>>()
                .join(", ")
        );
        return 2;
    }

    println!("Diagnosis tools on this system");
    println!("syschk shows install commands; it never installs anything itself.\n");

    let mut missing_pkgs: Vec<&str> = Vec::new();

    for bundle in Bundle::ALL {
        if let Some(w) = wanted
            && w != bundle
        {
            continue;
        }
        let tools = registry::in_bundle(bundle);
        let rows: Vec<_> = tools
            .iter()
            .filter(|t| {
                !only_missing
                    || inventory
                        .get(t.id)
                        .map(ToolStatus::is_missing)
                        .unwrap_or(false)
            })
            .collect();
        if rows.is_empty() {
            continue;
        }
        println!("[{}] {}", bundle.label(), bundle.why());
        for tool in rows {
            let status = inventory
                .get(tool.id)
                .cloned()
                .unwrap_or(ToolStatus::Missing);
            let mark = match status {
                ToolStatus::Installed(_) => "ok     ",
                ToolStatus::Missing => "MISSING",
                ToolStatus::NotApplicable(_) => "n/a    ",
            };
            println!("  {mark} {:<24} {}", tool.package, tool.purpose);
            if status.is_missing() {
                missing_pkgs.push(tool.package);
                let blocked = registry::tasks_needing(tool.id);
                if !blocked.is_empty() {
                    println!(
                        "          without it you cannot: {}",
                        blocked
                            .iter()
                            .take(3)
                            .map(|t| t.title)
                            .collect::<Vec<_>>()
                            .join("; ")
                    );
                }
                if let Some(note) = tool.post_install {
                    println!("          note: {note}");
                }
                if let Some(fallback) = tool.without_it {
                    println!("          without installing: {fallback}");
                }
            }
        }
        println!();
    }

    println!(
        "summary: {} installed, {} missing, {} not applicable here",
        inventory.installed(),
        inventory.missing(),
        inventory.not_applicable()
    );

    if !missing_pkgs.is_empty() {
        println!("\nto install everything reported missing above, run this yourself:");
        println!("  sudo apt install -y {}", missing_pkgs.join(" "));
        return 1;
    }
    0
}

fn parse_bundle(name: &str) -> Option<Bundle> {
    Bundle::ALL
        .into_iter()
        .find(|b| b.label().eq_ignore_ascii_case(name))
}
