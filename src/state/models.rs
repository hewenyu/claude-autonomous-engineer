//! 状态数据模型
//!
//! 定义 Memory, RoadmapData, TaskItem 等数据结构

use serde::{Deserialize, Serialize};

/// Memory 数据结构 - 对应 .claude/status/memory.json
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Memory {
    #[serde(default)]
    pub project: String,

    #[serde(default)]
    pub version: String,

    #[serde(default)]
    pub mode: String,

    #[serde(default)]
    pub current_phase: Option<String>,

    #[serde(default)]
    pub current_task: CurrentTask,

    #[serde(default)]
    pub working_context: WorkingContext,

    #[serde(default)]
    pub active_files: Vec<String>,

    #[serde(default)]
    pub progress: Progress,

    #[serde(default)]
    pub next_action: NextAction,

    #[serde(default)]
    pub error_history: Vec<ErrorRecord>,

    #[serde(default)]
    pub decisions_log: Vec<String>,
}

/// 当前任务
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CurrentTask {
    pub id: Option<String>,
    pub name: Option<String>,
    pub status: String,
    pub phase: Option<String>,
    pub started_at: Option<String>,

    #[serde(default)]
    pub retry_count: u32,

    #[serde(default = "default_max_retries")]
    pub max_retries: u32,

    pub acceptance_progress: Option<String>,
    pub last_updated: Option<String>,
}

fn default_max_retries() -> u32 {
    5
}

/// 工作上下文
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkingContext {
    pub current_file: Option<String>,
    pub current_function: Option<String>,
    pub pending_tests: Vec<String>,
    pub pending_implementations: Vec<String>,
}

/// 进度信息
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Progress {
    #[serde(default)]
    pub tasks_completed: usize,

    #[serde(default)]
    pub tasks_total: usize,

    #[serde(default)]
    pub tasks_pending: usize,

    #[serde(default)]
    pub tasks_in_progress: usize,

    pub current_phase: Option<String>,
    pub current_phase_name: Option<String>,
    pub current_phase_status: Option<String>,
    pub last_synced: Option<String>,
}

/// 下一步行动
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NextAction {
    pub action: String,
    pub target: Option<String>,
    pub reason: Option<String>,
}

/// 错误记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorRecord {
    pub task: String,
    pub error: String,
    pub attempted_fix: Option<String>,
    pub resolution: Option<String>,
    pub timestamp: Option<String>,
}

/// Roadmap 数据结构
#[derive(Debug, Clone, Default)]
pub struct RoadmapData {
    pub pending: Vec<TaskItem>,
    pub in_progress: Vec<TaskItem>,
    pub completed: Vec<TaskItem>,
    pub blocked: Vec<TaskItem>,
    pub current_phase: Option<String>,
    pub total: usize,
}

impl RoadmapData {
    /// 查找当前应该执行的任务
    pub fn find_current_task(&self) -> Option<&TaskItem> {
        // 优先返回进行中的任务
        if !self.in_progress.is_empty() {
            return Some(&self.in_progress[0]);
        }
        // 否则返回第一个待处理的任务
        if !self.pending.is_empty() {
            return Some(&self.pending[0]);
        }
        None
    }

    /// 检查是否全部完成
    pub fn is_complete(&self) -> bool {
        self.pending.is_empty() && self.in_progress.is_empty()
    }
}

/// 任务项
#[derive(Debug, Clone)]
pub struct TaskItem {
    pub line: String,
    pub id: Option<String>,
}

/// 任务详情 - 从 TASK-xxx.md 解析
#[derive(Debug, Clone)]
pub struct TaskDetail {
    pub id: String,
    pub name: String,
    pub status: String,
    pub phase: Option<String>,
    pub dependencies: Vec<String>,
    pub acceptance_criteria: Vec<String>,
}

/// 阶段计划 - 从 PHASE_PLAN.md 解析
#[derive(Debug, Clone)]
pub struct PhasePlan {
    pub phase_num: String,
    pub phase_name: String,
    pub status: String,
}
