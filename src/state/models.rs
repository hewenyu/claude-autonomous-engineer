//! 状态数据模型
//!
//! 定义 Memory, RoadmapData, TaskItem 等数据结构

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// Memory 数据结构 - 对应 .claude/status/memory.json
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Memory {
    // Template metadata fields (keep as-is in memory.json)
    #[serde(rename = "_schema_version", default)]
    pub schema_version: Option<String>,

    #[serde(rename = "_description", default)]
    pub description: Option<String>,

    #[serde(rename = "_last_updated", default)]
    pub last_updated: Option<String>,

    #[serde(default)]
    pub project: String,

    #[serde(default)]
    pub version: String,

    #[serde(default)]
    pub mode: String,

    #[serde(default)]
    pub session: Session,

    #[serde(default)]
    pub current_task: CurrentTask,

    #[serde(default)]
    pub working_context: WorkingContext,

    #[serde(default)]
    pub active_files: Vec<String>,

    #[serde(default)]
    pub progress: Progress,

    #[serde(default)]
    pub error_state: ErrorState,

    #[serde(default)]
    pub checkpoints: Vec<Value>,

    #[serde(default)]
    pub next_action: NextAction,

    #[serde(default)]
    pub contract_hash: Option<String>,

    #[serde(default)]
    pub warnings: Vec<String>,

    // Preserve forward-compatible/unknown fields to avoid wiping user state.
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// 会话信息（对应 memory.json.session）
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Session {
    #[serde(default)]
    pub started_at: Option<String>,

    #[serde(default)]
    pub loop_count: u64,

    #[serde(default)]
    pub last_context_compression_at: Option<String>,

    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// 当前任务
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CurrentTask {
    pub id: Option<String>,
    pub name: Option<String>,
    #[serde(default = "default_task_status")]
    pub status: String,
    pub phase: Option<String>,
    pub started_at: Option<String>,

    #[serde(default)]
    pub retry_count: u32,

    #[serde(default = "default_max_retries")]
    pub max_retries: u32,

    pub acceptance_progress: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_updated: Option<String>,

    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

fn default_max_retries() -> u32 {
    5
}

fn default_task_status() -> String {
    "PENDING".to_string()
}

/// 工作上下文
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WorkingContext {
    pub current_file: Option<String>,
    pub current_function: Option<String>,
    pub pending_tests: Vec<String>,
    pub pending_implementations: Vec<String>,

    #[serde(default)]
    pub modified_files: Vec<String>,

    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
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

    #[serde(default)]
    pub tasks_skipped: usize,

    pub current_phase: Option<String>,
    pub current_phase_name: Option<String>,
    pub current_phase_status: Option<String>,

    #[serde(default)]
    pub phases_completed: usize,

    #[serde(default)]
    pub phases_total: usize,

    pub last_synced: Option<String>,

    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// 下一步行动
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NextAction {
    pub action: String,
    pub target: Option<String>,
    pub reason: Option<String>,

    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// 错误状态（对应 memory.json.error_state）
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ErrorState {
    #[serde(default)]
    pub last_error: Option<String>,

    #[serde(default)]
    pub last_error_at: Option<String>,

    #[serde(default)]
    pub error_count: u64,

    #[serde(default)]
    pub blocked: bool,

    #[serde(default)]
    pub block_reason: Option<String>,

    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Roadmap 数据结构
#[derive(Debug, Clone, Default)]
pub struct RoadmapData {
    pub pending: Vec<TaskItem>,
    pub in_progress: Vec<TaskItem>,
    pub completed: Vec<TaskItem>,
    pub blocked: Vec<TaskItem>,
    pub skipped: Vec<TaskItem>,
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
        self.pending.is_empty() && self.in_progress.is_empty() && self.blocked.is_empty()
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

/// Review 重试状态 - 用于跟踪 codex review 的失败尝试
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReviewRetryState {
    /// 当前任务 ID
    #[serde(default)]
    pub current_task_id: String,

    /// 连续失败次数
    #[serde(default)]
    pub consecutive_failures: u32,

    /// 最后失败时间戳
    #[serde(default)]
    pub last_failure_timestamp: String,

    /// 最后一次 staged files 的 hash（用于检测代码是否有实质性修改）
    #[serde(default)]
    pub last_staged_files_hash: String,

    /// 失败原因列表（保留历史记录）
    #[serde(default)]
    pub failure_reasons: Vec<String>,
}

// ═══════════════════════════════════════════════════════════════════
// Story 相关数据结构（新增）
// ═══════════════════════════════════════════════════════════════════

/// Story 索引数据 - 从 INDEX.md 解析
#[derive(Debug, Clone)]
pub struct StoryIndexData {
    pub draft: Vec<StoryItem>,
    pub reviewing: Vec<StoryItem>,
    pub confirmed: Vec<StoryItem>,
    pub archived: Vec<StoryItem>,
    pub total: usize,
}

impl StoryIndexData {
    /// 检查是否有已确认的stories
    pub fn has_confirmed(&self) -> bool {
        !self.confirmed.is_empty()
    }

    /// 检查是否还有未确认的stories（draft或reviewing）
    pub fn has_unconfirmed(&self) -> bool {
        !self.draft.is_empty() || !self.reviewing.is_empty()
    }

    /// 获取确认进度百分比
    pub fn confirmation_progress(&self) -> f64 {
        if self.total == 0 {
            return 0.0;
        }
        (self.confirmed.len() as f64 / self.total as f64) * 100.0
    }
}

/// Story 项
#[derive(Debug, Clone)]
pub struct StoryItem {
    pub line: String,
    pub id: Option<String>,
    pub title: Option<String>,
    pub priority: Option<String>,
}
