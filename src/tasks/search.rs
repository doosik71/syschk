//! 증상 표현으로 작업을 찾는다.
//!
//! 사용자는 도구 이름이 아니라 "slow", "disk full", "frozen" 같은 말로 검색한다.
//! 작업의 제목·별칭·설명·화면 제목을 모두 색인한다.

use super::{Task, registry};

/// 검색 결과 한 건.
#[derive(Clone, Debug)]
pub struct Hit {
    pub task: &'static Task,
    pub score: u32,
}

/// 질의에 맞는 작업을 점수 순으로 돌려준다.
pub fn find(query: &str) -> Vec<Hit> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Vec::new();
    }
    let terms: Vec<&str> = q.split_whitespace().collect();

    let mut hits: Vec<Hit> = registry::tasks()
        .iter()
        .filter_map(|task| {
            let score = score(task, &terms);
            (score > 0).then_some(Hit { task, score })
        })
        .collect();

    // 점수 내림차순, 동점이면 화면 번호와 제목 순으로 안정화한다.
    hits.sort_by(|a, b| {
        b.score
            .cmp(&a.score)
            .then_with(|| a.task.screen.number().cmp(&b.task.screen.number()))
            .then_with(|| a.task.title.cmp(b.task.title))
    });
    hits
}

fn score(task: &Task, terms: &[&str]) -> u32 {
    let title = task.title.to_lowercase();
    let answers = task.answers.to_lowercase();
    let screen = task.screen.title().to_lowercase();

    let mut total = 0;
    for term in terms {
        let mut best = 0;
        // 별칭 완전 일치가 가장 강한 신호다.
        if task.aliases.iter().any(|a| a.eq_ignore_ascii_case(term)) {
            best = best.max(100);
        }
        if task.aliases.iter().any(|a| a.contains(term)) {
            best = best.max(60);
        }
        if title.split_whitespace().any(|w| w == *term) {
            best = best.max(50);
        }
        if title.contains(term) {
            best = best.max(30);
        }
        if screen.contains(term) {
            best = best.max(20);
        }
        if answers.contains(term) {
            best = best.max(10);
        }
        if task.id.contains(term) {
            best = best.max(15);
        }
        // 하나라도 걸리지 않는 낱말이 있으면 결과에서 제외한다(AND 검색).
        if best == 0 {
            return 0;
        }
        total += best;
    }
    total
}
