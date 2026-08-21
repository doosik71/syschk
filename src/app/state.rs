//! 앱 상태와 키 입력 처리.
//!
//! 상태 전이는 모두 이곳에 모여 있고, UI 는 상태를 읽어서 그리기만 한다.
//! 덕분에 화면을 추가할 때 이벤트 루프를 건드릴 필요가 없다.

use super::jobs::Jobs;
use super::sampler::Sampler;
use super::storage::StorageState;
use crate::collect::process::ProcRow;
use crate::collect::{ProbeCtx, ProbeData};
use crate::tasks::{MENU, Screen, Task, registry, search};
use crate::tools::detect::{self, Inventory};
use crate::tools::registry as tool_registry;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use std::collections::HashMap;
use std::time::Instant;

/// 프로세스 목록 정렬 기준.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProcSort {
    Cpu,
    Memory,
    Io,
    Pid,
}

impl ProcSort {
    pub fn label(self) -> &'static str {
        match self {
            ProcSort::Cpu => "cpu",
            ProcSort::Memory => "memory",
            ProcSort::Io => "disk i/o",
            ProcSort::Pid => "pid",
        }
    }

    fn next(self) -> Self {
        match self {
            ProcSort::Cpu => ProcSort::Memory,
            ProcSort::Memory => ProcSort::Io,
            ProcSort::Io => ProcSort::Pid,
            ProcSort::Pid => ProcSort::Cpu,
        }
    }
}

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
    ///
    /// 필요한 세 개만 골라 돌린다. 전체 수집기를 돌리면 시작할 때 디스크와 로그까지
    /// 뒤지게 되어 앱이 뜨는 데만 몇 초가 걸린다.
    pub fn collect(ctx: &ProbeCtx) -> Self {
        let mut h = Header::default();
        let probes: Vec<Box<dyn crate::collect::Probe>> = vec![
            Box::new(crate::collect::system::Identity),
            Box::new(crate::collect::system::Uptime),
            Box::new(crate::collect::system::LoadAverage),
        ];
        for probe in probes {
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
    /// 실시간 지표 표본 추출기. 실시간 화면에서만 동작한다.
    pub sampler: Sampler,
    pub sort: ProcSort,
    /// 화면 정지. 값을 자세히 보려고 멈춘 상태.
    pub frozen: bool,
    /// 고정 관찰 중인 프로세스.
    pub pinned: Option<u32>,
    /// 프로세스·표 안의 커서.
    pub row_cursor: usize,
    /// 저장 공간 화면의 자료.
    pub storage: StorageState,
    /// 배경 수집 작업.
    pub jobs: Jobs,
    /// 수집 문맥. 배경 작업에 넘긴다.
    ctx: ProbeCtx,
    /// 두 자리 메뉴 번호 입력 버퍼("1" 다음 "4" → 14번).
    digits: String,
    pub status: Option<String>,
    pub quit: bool,
}

impl App {
    pub fn new() -> Self {
        let ctx = ProbeCtx::default();
        let header = Header::collect(&ctx);
        Self {
            screen: Screen::Home,
            cursors: HashMap::new(),
            overlay: None,
            show_commands: false,
            header,
            inventory: detect::scan(),
            sampler: Sampler::new(ctx.clone()),
            sort: ProcSort::Cpu,
            frozen: false,
            pinned: None,
            row_cursor: 0,
            storage: StorageState::default(),
            jobs: Jobs::new(),
            ctx,
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

    /// 실시간 표본이 필요한 화면인가.
    pub fn live_active(&self) -> bool {
        matches!(self.screen, Screen::Live | Screen::Slow)
    }

    /// 필요하면 표본을 뜬다. 다른 화면에서는 아무 것도 읽지 않는다.
    pub fn maybe_sample(&mut self) {
        if self.live_active() && !self.frozen && self.sampler.due(Instant::now()) {
            self.sampler.tick();
        }
    }

    /// 화면이 요구하는 자료를 챙긴다. 느린 것은 배경 작업으로 넘긴다.
    pub fn poll_collectors(&mut self) {
        for output in self.jobs.drain() {
            self.storage.apply(output);
        }
        if self.screen != Screen::Storage {
            return;
        }
        if !self.storage.loaded {
            self.storage.load_fast(&self.ctx);
        }
        let Some(task) = self.selected_task() else {
            return;
        };
        for job in self.storage.jobs_for(task.id) {
            self.jobs.request(job, &self.ctx);
        }
    }

    /// 저장 공간 화면에서 선택된 디렉터리 항목.
    pub fn selected_dir_entry(&self) -> Option<crate::collect::dirsize::Entry> {
        self.storage
            .dir
            .as_ref()?
            .entries
            .get(self.row_cursor)
            .cloned()
    }

    /// 디렉터리를 파고든다.
    fn descend_directory(&mut self) {
        let Some(entry) = self.selected_dir_entry() else {
            return;
        };
        if !entry.is_dir {
            self.status = Some(format!("{} is a file, not a directory", entry.name));
            return;
        }
        self.row_cursor = 0;
        self.storage
            .descend(entry.path.clone(), &mut self.jobs, &self.ctx);
    }

    /// 디렉터리 한 단계 위로.
    fn ascend_directory(&mut self) -> bool {
        self.row_cursor = 0;
        self.storage.ascend(&mut self.jobs, &self.ctx)
    }

    /// 저장 공간 화면에서 디렉터리를 파고드는 작업을 보고 있는가.
    fn browsing_directories(&self) -> bool {
        self.screen == Screen::Storage
            && self.selected_task().map(|t| t.id) == Some("storage.what-fills")
    }

    /// 정렬과 고정을 반영한 프로세스 목록.
    pub fn sorted_procs(&self) -> Vec<&ProcRow> {
        let mut rows: Vec<&ProcRow> = self.sampler.procs.iter().collect();
        match self.sort {
            ProcSort::Cpu => rows.sort_by(|a, b| {
                b.cpu_pct
                    .partial_cmp(&a.cpu_pct)
                    .unwrap_or(std::cmp::Ordering::Equal)
            }),
            ProcSort::Memory => rows.sort_by(|a, b| b.rss_kb.cmp(&a.rss_kb)),
            ProcSort::Io => rows.sort_by(|a, b| {
                b.io_bps()
                    .partial_cmp(&a.io_bps())
                    .unwrap_or(std::cmp::Ordering::Equal)
            }),
            ProcSort::Pid => rows.sort_by(|a, b| a.pid.cmp(&b.pid)),
        }
        // 고정한 프로세스는 항상 맨 위에 둔다.
        if let Some(pid) = self.pinned
            && let Some(idx) = rows.iter().position(|p| p.pid == pid)
        {
            let row = rows.remove(idx);
            rows.insert(0, row);
        }
        rows
    }

    /// 프로세스 표에서 커서가 가리키는 행.
    pub fn selected_proc(&self) -> Option<ProcRow> {
        self.sorted_procs()
            .get(self.row_cursor)
            .map(|p| (*p).clone())
    }

    /// Live 화면의 작업에 맞는 기본 정렬. 사용자가 `s` 로 언제든 바꿀 수 있다.
    fn sync_sort_with_task(&mut self) {
        if self.screen != Screen::Live {
            return;
        }
        let Some(task) = self.selected_task() else {
            return;
        };
        match task.id {
            "live.cpu" => self.sort = ProcSort::Cpu,
            "live.memory" => self.sort = ProcSort::Memory,
            "live.disk-io" => self.sort = ProcSort::Io,
            _ => {}
        }
    }

    fn move_row(&mut self, delta: isize) {
        // 표의 길이는 화면에 따라 다르다.
        let n = match self.screen {
            Screen::Storage => self
                .storage
                .dir
                .as_ref()
                .map(|d| d.entries.len())
                .unwrap_or(0) as isize,
            _ => self.sampler.procs.len() as isize,
        };
        if n == 0 {
            self.row_cursor = 0;
            return;
        }
        let next = (self.row_cursor as isize + delta).rem_euclid(n);
        self.row_cursor = next as usize;
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
            // ── 실시간 화면 전용 ──────────────────────────────────
            KeyCode::Char('s') if self.live_active() => {
                self.sort = self.sort.next();
                self.row_cursor = 0;
                self.status = Some(format!("sorted by {}", self.sort.label()));
            }
            KeyCode::Char('f') if self.live_active() => {
                self.frozen = !self.frozen;
                self.status = Some(
                    if self.frozen {
                        "frozen - values held still. press f to resume"
                    } else {
                        "live again"
                    }
                    .to_string(),
                );
            }
            KeyCode::Char('p') if self.live_active() => match self.selected_proc() {
                Some(row) if self.pinned == Some(row.pid) => {
                    self.pinned = None;
                    self.status = Some(format!("unpinned {}", row.pid));
                }
                Some(row) => {
                    self.pinned = Some(row.pid);
                    // 고정한 행은 맨 위로 올라가므로 커서도 따라간다.
                    self.row_cursor = 0;
                    self.status = Some(format!("pinned {} ({})", row.pid, row.user));
                }
                None => {}
            },
            KeyCode::Char('J') if self.live_active() || self.screen == Screen::Storage => {
                self.move_row(1)
            }
            KeyCode::Char('K') if self.live_active() || self.screen == Screen::Storage => {
                self.move_row(-1)
            }
            KeyCode::Down | KeyCode::Up
                if (self.live_active() || self.screen == Screen::Storage)
                    && key.modifiers.contains(KeyModifiers::SHIFT) =>
            {
                self.move_row(if key.code == KeyCode::Down { 1 } else { -1 });
            }
            // ── 저장 공간 화면 전용 ───────────────────────────────
            KeyCode::Char('r') if self.screen == Screen::Storage => {
                self.storage.reset();
                self.row_cursor = 0;
                self.status = Some("re-reading storage state".into());
            }
            KeyCode::Char('t') => {
                self.digits.clear();
                self.screen = Screen::Tools;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.digits.clear();
                self.move_cursor(-1);
                self.sync_sort_with_task();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.digits.clear();
                self.move_cursor(1);
                self.sync_sort_with_task();
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
                    self.row_cursor = 0;
                    self.sync_sort_with_task();
                } else if self.browsing_directories() {
                    // 디렉터리를 파고든다.
                    self.descend_directory();
                }
            }
            KeyCode::Backspace => {
                self.digits.clear();
                // 디렉터리를 보고 있으면 한 단계 위로, 위가 없으면 홈으로.
                if self.browsing_directories() && self.ascend_directory() {
                    return;
                }
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
