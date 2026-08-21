//! 앱 상태와 키 입력 처리.
//!
//! 상태 전이는 모두 이곳에 모여 있고, UI 는 상태를 읽어서 그리기만 한다.
//! 덕분에 화면을 추가할 때 이벤트 루프를 건드릴 필요가 없다.

use crate::collect::{ProbeCtx, ProbeData};
use crate::tasks::{MENU, Screen, Task, registry, search};
use crate::tools::detect::{self, Inventory};
use crate::tools::registry as tool_registry;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use std::collections::HashMap;

/// 화면 위에 덮이는 창.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Overlay {
    Help,
    Search { query: String, cursor: usize },
}

/// 헤더에 표시할 시스템 정보. 시작 시 한 번 수집한다.
#[derive(Clone, Debug, Default)]
pub struct Header {
    pub hostname: String,
    pub os: String,
    pub kernel: String,
    pub uptime: String,
    pub load: String,
    pub cores: String,
}

impl Header {
    /// `/proc` 기반 수집기로 채운다. 실패해도 빈 값으로 남기고 계속한다.
    pub fn collect(ctx: &ProbeCtx) -> Self {
        let mut h = Header::default();
        for probe in crate::collect::probes() {
            let result = probe.run(ctx);
            if !result.availability.is_ok() {
                continue;
            }
            let ProbeData::Fields(_) = &result.data else {
                continue;
            };
            match probe.id() {
                "system.identity" => {
                    h.hostname = result.data.field("hostname").unwrap_or_default().into();
                    h.os = result.data.field("os").unwrap_or_default().into();
                    h.kernel = result.data.field("kernel").unwrap_or_default().into();
                }
                "system.uptime" => {
                    h.uptime = result.data.field("uptime").unwrap_or_default().into();
                }
                "system.load" => {
                    h.load = format!(
                        "{} {} {}",
                        result.data.field("load1").unwrap_or("-"),
                        result.data.field("load5").unwrap_or("-"),
                        result.data.field("load15").unwrap_or("-"),
                    );
                    h.cores = result.data.field("cores").unwrap_or_default().into();
                }
                _ => {}
            }
        }
        h
    }
}

/// 앱 전체 상태.
pub struct App {
    pub screen: Screen,
    /// 화면별 커서 위치. 돌아왔을 때 자리를 기억한다.
    cursors: HashMap<Screen, usize>,
    pub overlay: Option<Overlay>,
    /// 근거 명령 펼침 여부(원칙: 명령은 기억 대상이 아니라 근거).
    pub show_commands: bool,
    pub header: Header,
    pub inventory: Inventory,
    /// 두 자리 메뉴 번호 입력 버퍼("1" 다음 "4" → 14번).
    digits: String,
    pub status: Option<String>,
    pub quit: bool,
}

impl App {
    pub fn new() -> Self {
        let ctx = ProbeCtx::default();
        Self {
            screen: Screen::Home,
            cursors: HashMap::new(),
            overlay: None,
            show_commands: false,
            header: Header::collect(&ctx),
            inventory: detect::scan(),
            digits: String::new(),
            status: None,
            quit: false,
        }
    }

    /// 현재 화면의 항목 수.
    pub fn item_count(&self) -> usize {
        match self.screen {
            Screen::Home => MENU.len(),
            Screen::Tools => tool_registry::tools().len(),
            s => s.tasks().len(),
        }
    }

    pub fn cursor(&self) -> usize {
        let n = self.item_count();
        let c = self.cursors.get(&self.screen).copied().unwrap_or(0);
        if n == 0 { 0 } else { c.min(n - 1) }
    }

    fn set_cursor(&mut self, value: usize) {
        let n = self.item_count();
        if n == 0 {
            return;
        }
        self.cursors.insert(self.screen, value.min(n - 1));
    }

    fn move_cursor(&mut self, delta: isize) {
        let n = self.item_count() as isize;
        if n == 0 {
            return;
        }
        let cur = self.cursor() as isize;
        let next = (cur + delta).rem_euclid(n);
        self.set_cursor(next as usize);
    }

    /// 홈에서 선택된 화면.
    pub fn selected_screen(&self) -> Screen {
        MENU[self.cursor().min(MENU.len() - 1)]
    }

    /// 작업 화면에서 선택된 작업.
    pub fn selected_task(&self) -> Option<&'static Task> {
        self.screen.tasks().get(self.cursor()).copied()
    }

    /// 도구 화면에서 선택된 도구.
    pub fn selected_tool(&self) -> Option<&'static crate::tools::Tool> {
        tool_registry::tools().get(self.cursor())
    }

    /// 검색 결과(오버레이가 검색일 때).
    pub fn search_hits(&self) -> Vec<search::Hit> {
        match &self.overlay {
            Some(Overlay::Search { query, .. }) => search::find(query),
            _ => Vec::new(),
        }
    }

    /// 어떤 화면에 미설치 도구가 걸려 있는지 — 홈 메뉴의 주의 표시에 쓴다.
    pub fn screen_missing_tools(&self, screen: Screen) -> usize {
        let mut ids: Vec<&str> = screen
            .tasks()
            .iter()
            .flat_map(|t| t.tools.iter().copied())
            .collect();
        ids.sort_unstable();
        ids.dedup();
        self.inventory.missing_for(&ids).len()
    }

    pub fn on_key(&mut self, key: KeyEvent) {
        if key.kind == KeyEventKind::Release {
            return;
        }
        // Ctrl-C 는 어디서든 종료.
        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('c')) {
            self.quit = true;
            return;
        }
        self.status = None;

        if self.overlay.is_some() {
            self.on_key_overlay(key);
            return;
        }

        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => {
                if self.screen == Screen::Home {
                    self.quit = true;
                } else {
                    self.screen = Screen::Home;
                }
            }
            KeyCode::Char('?') => self.overlay = Some(Overlay::Help),
            KeyCode::Char('/') => {
                self.overlay = Some(Overlay::Search {
                    query: String::new(),
                    cursor: 0,
                })
            }
            KeyCode::Char('c') => {
                self.show_commands = !self.show_commands;
            }
            KeyCode::Char('t') => {
                self.digits.clear();
                self.screen = Screen::Tools;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.digits.clear();
                self.move_cursor(-1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.digits.clear();
                self.move_cursor(1);
            }
            KeyCode::PageUp => {
                self.digits.clear();
                self.move_cursor(-5);
            }
            KeyCode::PageDown => {
                self.digits.clear();
                self.move_cursor(5);
            }
            KeyCode::Home => {
                self.digits.clear();
                self.set_cursor(0);
            }
            KeyCode::End => {
                self.digits.clear();
                let n = self.item_count();
                self.set_cursor(n.saturating_sub(1));
            }
            KeyCode::Enter => {
                self.digits.clear();
                if self.screen == Screen::Home {
                    self.screen = self.selected_screen();
                }
            }
            KeyCode::Backspace => {
                self.digits.clear();
                self.screen = Screen::Home;
            }
            KeyCode::Char(ch) if ch.is_ascii_digit() => self.on_digit(ch),
            _ => {}
        }
    }

    /// 홈에서 번호로 화면을 고른다. 두 자리(10~14)도 이어서 입력할 수 있다.
    fn on_digit(&mut self, ch: char) {
        if self.screen != Screen::Home {
            return;
        }
        self.digits.push(ch);
        if let Ok(n) = self.digits.parse::<usize>()
            && n >= 1
            && n <= MENU.len()
        {
            self.set_cursor(n - 1);
            // 더 긴 번호가 될 수 없으면 버퍼를 비운다.
            if n * 10 > MENU.len() {
                self.digits.clear();
            }
            return;
        }
        // 유효하지 않은 조합이면 마지막 입력만 남긴다.
        self.digits = ch.to_string();
        if let Ok(n) = self.digits.parse::<usize>()
            && n >= 1
            && n <= MENU.len()
        {
            self.set_cursor(n - 1);
        }
    }

    fn on_key_overlay(&mut self, key: KeyEvent) {
        let Some(overlay) = self.overlay.clone() else {
            return;
        };
        match overlay {
            Overlay::Help => {
                self.overlay = None;
            }
            Overlay::Search { mut query, cursor } => match key.code {
                KeyCode::Esc => self.overlay = None,
                KeyCode::Backspace => {
                    query.pop();
                    self.overlay = Some(Overlay::Search { query, cursor: 0 });
                }
                KeyCode::Up => {
                    let n = search::find(&query).len();
                    let next = if n == 0 { 0 } else { (cursor + n - 1) % n };
                    self.overlay = Some(Overlay::Search {
                        query,
                        cursor: next,
                    });
                }
                KeyCode::Down => {
                    let n = search::find(&query).len();
                    let next = if n == 0 { 0 } else { (cursor + 1) % n };
                    self.overlay = Some(Overlay::Search {
                        query,
                        cursor: next,
                    });
                }
                KeyCode::Enter => {
                    let hits = search::find(&query);
                    if let Some(hit) = hits.get(cursor) {
                        let task_id = hit.task.id;
                        self.overlay = None;
                        self.goto_task(task_id);
                    } else {
                        self.overlay = None;
                    }
                }
                KeyCode::Char(ch) => {
                    query.push(ch);
                    self.overlay = Some(Overlay::Search { query, cursor: 0 });
                }
                _ => {}
            },
        }
    }

    /// 작업 id 로 해당 화면과 커서 위치까지 이동한다.
    pub fn goto_task(&mut self, task_id: &str) {
        let Some(task) = registry::by_id(task_id) else {
            return;
        };
        self.screen = task.screen;
        if let Some(idx) = task.screen.tasks().iter().position(|t| t.id == task_id) {
            self.set_cursor(idx);
        }
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
