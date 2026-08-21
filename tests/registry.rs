//! 카탈로그 불변식.
//!
//! 작업·도구는 등록만으로 확장되므로, 등록 자료의 정합성을 시험이 지킨다.

use std::collections::HashSet;
use syschk::tasks::{MENU, TaskState, registry as tasks};
use syschk::tools::registry as tools;

#[test]
fn task_ids_are_unique() {
    let mut seen = HashSet::new();
    for task in tasks::tasks() {
        assert!(seen.insert(task.id), "duplicate task id: {}", task.id);
    }
}

#[test]
fn every_screen_has_tasks() {
    for screen in MENU {
        assert!(
            !screen.tasks().is_empty(),
            "screen {} ({}) has no tasks",
            screen.number(),
            screen.title()
        );
    }
}

#[test]
fn every_task_belongs_to_a_menu_screen() {
    for task in tasks::tasks() {
        assert!(
            MENU.contains(&task.screen),
            "task {} points at a screen that is not in the menu",
            task.id
        );
    }
}

#[test]
fn task_tools_exist_in_tool_registry() {
    for task in tasks::tasks() {
        for id in task.tools {
            assert!(
                tools::by_id(id).is_some(),
                "task {} needs unknown tool '{}'",
                task.id,
                id
            );
        }
    }
}

#[test]
fn tasks_are_searchable() {
    for task in tasks::tasks() {
        assert!(
            !task.aliases.is_empty(),
            "task {} has no symptom aliases, so search cannot find it",
            task.id
        );
        assert!(!task.answers.is_empty(), "task {} answers nothing", task.id);
    }
}

/// 화면에 표시되는 문구는 영어로 유지한다(초기 단계에서는 다국어를 지원하지 않는다).
#[test]
fn user_facing_text_is_ascii() {
    for task in tasks::tasks() {
        assert!(
            task.title.is_ascii(),
            "task title is not ASCII English: {}",
            task.title
        );
        assert!(
            task.answers.is_ascii(),
            "task answers text is not ASCII English: {}",
            task.id
        );
    }
    for screen in MENU {
        assert!(screen.title().is_ascii(), "screen title not ASCII");
        assert!(screen.tag().is_ascii(), "screen tag not ASCII");
        assert!(screen.blurb().is_ascii(), "screen blurb not ASCII");
    }
    for tool in tools::tools() {
        assert!(
            tool.purpose.is_ascii(),
            "tool purpose not ASCII: {}",
            tool.id
        );
    }
}

#[test]
fn tool_registry_is_consistent() {
    let mut seen = HashSet::new();
    for tool in tools::tools() {
        assert!(seen.insert(tool.id), "duplicate tool id: {}", tool.id);
        assert!(!tool.package.is_empty(), "tool {} has no package", tool.id);
        assert!(
            !tool.binaries.is_empty(),
            "tool {} provides no binaries, so it can never be detected",
            tool.id
        );
        assert!(
            tool.install_command().starts_with("sudo apt install"),
            "unexpected install command for {}",
            tool.id
        );
    }
}

#[test]
fn ready_tasks_are_reported_as_progress() {
    let (ready, total) = tasks::progress();
    assert!(total > 50, "task catalogue looks too small: {total}");
    assert!(ready > 0, "nothing is marked ready");
    assert_eq!(
        ready,
        tasks::tasks()
            .iter()
            .filter(|t| t.state == TaskState::Ready)
            .count()
    );
}

/// M0 에서 동작해야 하는 것: 도구 준비 화면의 작업들.
#[test]
fn tool_screen_tasks_are_ready_in_m0() {
    let ready: Vec<&str> = syschk::tasks::Screen::Tools
        .tasks()
        .iter()
        .filter(|t| t.state == TaskState::Ready)
        .map(|t| t.id)
        .collect();
    assert!(ready.contains(&"tools.inventory"));
    assert!(ready.contains(&"tools.how-to-install"));
    assert!(ready.contains(&"tools.without-installing"));
}
