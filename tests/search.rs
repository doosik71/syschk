//! 증상 표현으로 작업을 찾을 수 있어야 한다(비전문가가 쓰는 말로).

use syschk::tasks::search;

fn top_ids(query: &str, n: usize) -> Vec<&'static str> {
    search::find(query)
        .into_iter()
        .take(n)
        .map(|h| h.task.id)
        .collect()
}

#[test]
fn plain_symptom_words_find_the_right_task() {
    let cases: &[(&str, &str)] = &[
        ("disk full", "storage.which-full"),
        ("no space", "storage.which-full"),
        ("slow", "slow.verdict"),
        ("frozen", "halt.when"),
        ("oom", "slow.memory"),
        ("ports", "network.listening"),
        ("dns", "network.dns"),
        ("service failed", "services.failed"),
        ("reboot required", "updates.reboot-needed"),
        ("brute force", "security.failed-logins"),
        ("memory leak", "process.memory-growth"),
        ("smart", "storage.drive-failing"),
        ("gpu", "hardware.gpu"),
        ("kdump", "halt.crash-record"),
        ("install", "tools.how-to-install"),
    ];
    for (query, expected) in cases {
        let hits = top_ids(query, 5);
        assert!(
            hits.contains(expected),
            "searching \"{query}\" should surface {expected}, got {hits:?}"
        );
    }
}

#[test]
fn nonsense_returns_nothing() {
    assert!(search::find("zzzzqqq").is_empty());
    assert!(search::find("").is_empty());
    assert!(search::find("   ").is_empty());
}

#[test]
fn multiple_words_narrow_the_result() {
    let broad = search::find("disk").len();
    let narrow = search::find("disk full").len();
    assert!(
        narrow <= broad,
        "adding a word should not widen the result set"
    );
    // 두 낱말 모두 걸려야 결과에 남는다(AND 검색).
    for hit in search::find("disk failing") {
        let haystack = format!(
            "{} {} {} {}",
            hit.task.title,
            hit.task.answers,
            hit.task.id,
            hit.task.aliases.join(" ")
        )
        .to_lowercase();
        assert!(haystack.contains("disk") || haystack.contains("drive"));
    }
}

#[test]
fn results_are_ordered_by_relevance() {
    let hits = search::find("disk full");
    assert!(!hits.is_empty());
    let scores: Vec<u32> = hits.iter().map(|h| h.score).collect();
    let mut sorted = scores.clone();
    sorted.sort_unstable_by(|a, b| b.cmp(a));
    assert_eq!(scores, sorted, "hits must come back best-first");
}
