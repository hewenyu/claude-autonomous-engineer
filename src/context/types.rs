// Core Data Structures
// 核心数据结构定义

use serde::{Deserialize, Serialize};

/// Memory 结构 (memory.json)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    pub project: String,
    pub version: String,
    pub mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_phase: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_task: Option<TaskInfo>,
    pub progress: Progress,
    pub next_action: NextAction,
    #[serde(default)]
    pub error_history: Vec<ErrorRecord>,
    #[serde(default)]
    pub decisions_log: Vec<String>,
    #[serde(default)]
    pub active_files: Vec<String>,
    #[serde(default)]
    pub working_context: WorkingContext,
}

/// 任务信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskInfo {
    pub id: String,
    pub name: String,
    pub status: String,
    #[serde(default)]
    pub retry_count: u32,
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
}

fn default_max_retries() -> u32 {
    5
}

/// 进度信息
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Progress {
    #[serde(default)]
    pub tasks_total: u32,
    #[serde(default)]
    pub tasks_completed: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_phase: Option<String>,
    #[serde(default)]
    pub completed: Vec<String>,
    #[serde(default)]
    pub in_progress: Vec<String>,
    #[serde(default)]
    pub blocked: Vec<String>,
    #[serde(default)]
    pub pending: Vec<String>,
}

/// 下一步行动
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NextAction {
    pub action: String,
    pub target: String,
    pub reason: String,
}

impl Default for NextAction {
    fn default() -> Self {
        Self {
            action: "INITIALIZE".to_string(),
            target: "Run project-architect-supervisor".to_string(),
            reason: "System initialized, awaiting project plan".to_string(),
        }
    }
}

/// 工作上下文
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkingContext {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_function: Option<String>,
    #[serde(default)]
    pub pending_tests: Vec<String>,
    #[serde(default)]
    pub pending_implementations: Vec<String>,
}

/// 错误记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorRecord {
    pub task: String,
    pub error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attempted_fix: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolution: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
}

/// 任务状态
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskStatus {
    Pending,     // - [ ]
    InProgress,  // - [>] 或 - [~]
    Completed,   // - [x] 或 - [X]
    Blocked,     // - [!]
}

impl TaskStatus {
    pub fn from_marker(marker: &str) -> Option<Self> {
        match marker {
            "[ ]" => Some(Self::Pending),
            "[>]" | "[~]" => Some(Self::InProgress),
            "[x]" | "[X]" => Some(Self::Completed),
            "[!]" => Some(Self::Blocked),
            _ => None,
        }
    }

    pub fn to_marker(&self) -> &str {
        match self {
            Self::Pending => "[ ]",
            Self::InProgress => "[>]",
            Self::Completed => "[x]",
            Self::Blocked => "[!]",
        }
    }
}

/// 任务项
#[derive(Debug, Clone)]
pub struct Task {
    pub raw_line: String,
    pub status: TaskStatus,
    pub content: String,
}

impl Task {
    pub fn parse(line: &str) -> Option<Self> {
        let trimmed = line.trim();
        if !trimmed.starts_with("- [") {
            return None;
        }

        // 提取状态标记
        let marker_end = trimmed.find(']')?;
        let marker = &trimmed[2..=marker_end];
        let status = TaskStatus::from_marker(marker)?;

        // 提取内容
        let content = trimmed[marker_end + 1..].trim().to_string();

        Some(Task {
            raw_line: line.to_string(),
            status,
            content,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_parse() {
        let task = Task::parse("- [ ] Task 1").unwrap();
        assert_eq!(task.status, TaskStatus::Pending);
        assert_eq!(task.content, "Task 1");

        let task = Task::parse("- [x] Completed task").unwrap();
        assert_eq!(task.status, TaskStatus::Completed);
        assert_eq!(task.content, "Completed task");
    }
}
