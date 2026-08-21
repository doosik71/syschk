//! 도구 설치 여부 탐지. 외부 명령을 실행하지 않고 PATH 와 파일 존재만 본다.

use super::{Applicability, Tool, ToolStatus, registry};
use crate::util::exec::find_in_path;
use std::collections::BTreeMap;

/// 도구 하나의 상태를 판단한다.
pub fn status_of(tool: &Tool) -> ToolStatus {
    if let Applicability::IfPathExists(path) = tool.applicability
        && !std::path::Path::new(path).exists()
    {
        return ToolStatus::NotApplicable("no matching hardware or service on this system");
    }
    for bin in tool.binaries {
        if let Some(p) = find_in_path(bin) {
            return ToolStatus::Installed(p);
        }
    }
    ToolStatus::Missing
}

/// 전체 도구 상태를 한 번에 조사한다.
pub fn scan() -> Inventory {
    let map = registry::tools()
        .iter()
        .map(|t| (t.id, status_of(t)))
        .collect();
    Inventory { map }
}

/// 조사 결과.
#[derive(Clone, Debug)]
pub struct Inventory {
    map: BTreeMap<&'static str, ToolStatus>,
}

impl Inventory {
    pub fn get(&self, id: &str) -> Option<&ToolStatus> {
        self.map.get(id)
    }

    pub fn installed(&self) -> usize {
        self.map
            .values()
            .filter(|s| matches!(s, ToolStatus::Installed(_)))
            .count()
    }

    pub fn missing(&self) -> usize {
        self.map.values().filter(|s| s.is_missing()).count()
    }

    pub fn not_applicable(&self) -> usize {
        self.map
            .values()
            .filter(|s| matches!(s, ToolStatus::NotApplicable(_)))
            .count()
    }

    /// 특정 작업을 수행하는 데 빠져 있는 도구 목록.
    pub fn missing_for(&self, tool_ids: &[&str]) -> Vec<&'static Tool> {
        tool_ids
            .iter()
            .filter_map(|id| registry::by_id(id))
            .filter(|t| self.get(t.id).map(ToolStatus::is_missing).unwrap_or(false))
            .collect()
    }
}
