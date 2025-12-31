//! Git 驱动的状态机模块
//!
//! 使用 Git commits + tags 作为状态快照，提供：
//! - 状态转换（自动 commit + tag）
//! - 状态回滚（git checkout）
//! - 状态历史查询
//! - 工作流编排

pub mod git_state;
pub mod hooks;
pub mod visualizer;
pub mod workflow;

// 重导出核心类型
pub use git_state::GitStateMachine;
pub use hooks::*;
pub use visualizer::*;
pub use workflow::*;

use serde::{Deserialize, Serialize};

// ═══════════════════════════════════════════════════════════════════
// 状态定义
// ═══════════════════════════════════════════════════════════════════

/// 状态 ID 枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StateId {
    /// 空闲状态
    Idle,
    /// 规划阶段
    Planning,
    /// 编码阶段
    Coding,
    /// 测试阶段
    Testing,
    /// 审查阶段
    Reviewing,
    /// 完成状态
    Completed,
    /// 阻塞状态
    Blocked,
}

impl StateId {
    /// 转换为字符串
    pub fn as_str(&self) -> &'static str {
        match self {
            StateId::Idle => "idle",
            StateId::Planning => "planning",
            StateId::Coding => "coding",
            StateId::Testing => "testing",
            StateId::Reviewing => "reviewing",
            StateId::Completed => "completed",
            StateId::Blocked => "blocked",
        }
    }

    /// 从字符串解析
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "idle" => Some(StateId::Idle),
            "planning" => Some(StateId::Planning),
            "coding" => Some(StateId::Coding),
            "testing" => Some(StateId::Testing),
            "reviewing" => Some(StateId::Reviewing),
            "completed" => Some(StateId::Completed),
            "blocked" => Some(StateId::Blocked),
            _ => None,
        }
    }

    /// 获取显示图标
    pub fn icon(&self) -> &'static str {
        match self {
            StateId::Idle => "⏸️",
            StateId::Planning => "📝",
            StateId::Coding => "💻",
            StateId::Testing => "🧪",
            StateId::Reviewing => "🔍",
            StateId::Completed => "✅",
            StateId::Blocked => "🚫",
        }
    }
}

impl std::fmt::Display for StateId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for StateId {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        StateId::parse(s).ok_or(())
    }
}

/// 状态机状态（存储在 .claude/status/state.json）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachineState {
    /// 状态 ID
    pub state_id: StateId,
    /// 关联的任务 ID
    pub task_id: Option<String>,
    /// 阶段信息
    pub phase: Option<String>,
    /// 时间戳
    pub timestamp: String,
    /// 额外元数据
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

impl Default for MachineState {
    fn default() -> Self {
        MachineState {
            state_id: StateId::Idle,
            task_id: None,
            phase: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
            metadata: None,
        }
    }
}

impl MachineState {
    /// 创建新状态
    pub fn new(state_id: StateId, task_id: Option<String>) -> Self {
        MachineState {
            state_id,
            task_id,
            phase: None,
            timestamp: chrono::Utc::now().to_rfc3339(),
            metadata: None,
        }
    }

    /// 带阶段的状态
    pub fn with_phase(mut self, phase: String) -> Self {
        self.phase = Some(phase);
        self
    }

    /// 带元数据的状态
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }
}

/// 状态快照（从 Git tag 中提取）
#[derive(Debug, Clone)]
pub struct StateSnapshot {
    /// Git tag 名称
    pub tag: String,
    /// Commit SHA
    pub commit_sha: String,
    /// Commit 消息
    pub message: String,
    /// 时间戳（Unix timestamp）
    pub timestamp: i64,
    /// 解析的状态信息
    pub state: Option<MachineState>,
}

impl StateSnapshot {
    /// 从 tag 名称解析状态信息
    ///
    /// Tag 格式: state-{timestamp}-{state_id}-{task_id}
    /// 例如: state-20251231-120000-planning-TASK-001
    pub fn parse_tag_info(&self) -> Option<(StateId, Option<String>)> {
        let parts: Vec<&str> = self.tag.strip_prefix("state-")?.split('-').collect();

        if parts.len() < 3 {
            return None;
        }

        // parts[0-1] = timestamp (YYYYMMDD-HHMMSS)
        // parts[2] = state_id
        // parts[3..] = task_id (可能包含连字符)

        let state_id = StateId::parse(parts[2])?;

        let task_id = if parts.len() > 3 && parts[3] != "none" {
            Some(parts[3..].join("-"))
        } else {
            None
        };

        Some((state_id, task_id))
    }

    /// 获取格式化的时间字符串
    pub fn formatted_time(&self) -> String {
        use chrono::{TimeZone, Utc};

        match Utc.timestamp_opt(self.timestamp, 0).single() {
            Some(dt) => dt.format("%Y-%m-%d %H:%M:%S").to_string(),
            None => "Unknown".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_id_conversion() {
        assert_eq!(StateId::Planning.as_str(), "planning");
        assert_eq!(StateId::parse("coding"), Some(StateId::Coding));
        assert_eq!(StateId::parse("COMPLETED"), Some(StateId::Completed));
        assert_eq!(StateId::parse("invalid"), None);
    }

    #[test]
    fn test_state_snapshot_parse() {
        let snapshot = StateSnapshot {
            tag: "state-20251231-120000-planning-TASK-001".to_string(),
            commit_sha: "abc123".to_string(),
            message: "state: planning | task: TASK-001".to_string(),
            timestamp: 1735646400,
            state: None,
        };

        let (state_id, task_id) = snapshot.parse_tag_info().unwrap();
        assert_eq!(state_id, StateId::Planning);
        assert_eq!(task_id, Some("TASK-001".to_string()));
    }

    #[test]
    fn test_state_snapshot_parse_with_hyphens() {
        let snapshot = StateSnapshot {
            tag: "state-20251231-120000-coding-TASK-001-SUBTASK-A".to_string(),
            commit_sha: "def456".to_string(),
            message: "".to_string(),
            timestamp: 0,
            state: None,
        };

        let (state_id, task_id) = snapshot.parse_tag_info().unwrap();
        assert_eq!(state_id, StateId::Coding);
        assert_eq!(task_id, Some("TASK-001-SUBTASK-A".to_string()));
    }

    #[test]
    fn test_machine_state_default() {
        let state = MachineState::default();
        assert_eq!(state.state_id, StateId::Idle);
        assert!(state.task_id.is_none());
    }

    #[test]
    fn test_machine_state_builders() {
        let state = MachineState::new(StateId::Coding, Some("TASK-001".to_string()))
            .with_phase("implementation".to_string())
            .with_metadata(serde_json::json!({"retry": 1}));

        assert_eq!(state.state_id, StateId::Coding);
        assert_eq!(state.task_id, Some("TASK-001".to_string()));
        assert_eq!(state.phase, Some("implementation".to_string()));
        assert!(state.metadata.is_some());
    }
}
