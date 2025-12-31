//! 任务状态追踪器
//!
//! 检测任务状态转换，用于触发差异化的代码审查策略

use crate::state::CurrentTask;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// 任务状态快照
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSnapshot {
    pub status: String,
    pub snapshot_time: String,
    pub task_id: String,
}

/// 状态转换类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransitionType {
    /// 开始任务 (PENDING → IN_PROGRESS)
    StartTask,
    /// 完成任务 (IN_PROGRESS → COMPLETED)
    CompleteTask,
    /// 阻塞任务 (IN_PROGRESS → BLOCKED)
    BlockTask,
    /// 解除阻塞 (BLOCKED → IN_PROGRESS)
    UnblockTask,
    /// 内部进展（状态未变）
    InternalProgress,
}

/// 任务状态追踪器
pub struct TaskStateTracker {
    snapshots: HashMap<String, TaskSnapshot>,
    snapshots_file: std::path::PathBuf,
}

impl TaskStateTracker {
    /// 加载状态追踪器
    pub fn load(project_root: &Path) -> Result<Self> {
        let snapshots_file = project_root.join(".claude/status/task_snapshots.json");

        let snapshots = if snapshots_file.exists() {
            let content = std::fs::read_to_string(&snapshots_file)?;
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            HashMap::new()
        };

        Ok(Self {
            snapshots,
            snapshots_file,
        })
    }

    /// 检测是否发生状态转换
    pub fn detect_transition(&self, current: &CurrentTask) -> bool {
        if let Some(task_id) = &current.id {
            if let Some(snapshot) = self.snapshots.get(task_id) {
                snapshot.status != current.status
            } else {
                // 第一次记录，不算转换
                false
            }
        } else {
            false
        }
    }

    /// 识别转换类型
    pub fn classify_transition(&self, current: &CurrentTask) -> TransitionType {
        let task_id = match &current.id {
            Some(id) => id,
            None => return TransitionType::InternalProgress,
        };

        let previous_status = self
            .snapshots
            .get(task_id)
            .map(|s| s.status.as_str())
            .unwrap_or("UNKNOWN");

        match (previous_status, current.status.as_str()) {
            ("PENDING", "IN_PROGRESS") => TransitionType::StartTask,
            ("IN_PROGRESS", "COMPLETED") => TransitionType::CompleteTask,
            ("IN_PROGRESS", "BLOCKED") => TransitionType::BlockTask,
            ("BLOCKED", "IN_PROGRESS") => TransitionType::UnblockTask,
            _ => TransitionType::InternalProgress,
        }
    }

    /// 获取前一个快照
    pub fn get_previous_snapshot(&self, task_id: &str) -> Option<&TaskSnapshot> {
        self.snapshots.get(task_id)
    }

    /// 更新快照
    pub fn update_snapshot(&mut self, current: &CurrentTask) -> Result<()> {
        if let Some(task_id) = &current.id {
            let snapshot = TaskSnapshot {
                status: current.status.clone(),
                snapshot_time: chrono::Utc::now().to_rfc3339(),
                task_id: task_id.clone(),
            };

            self.snapshots.insert(task_id.clone(), snapshot);
            self.save()?;
        }

        Ok(())
    }

    /// 保存快照到文件
    fn save(&self) -> Result<()> {
        // 确保目录存在
        if let Some(parent) = self.snapshots_file.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(&self.snapshots)?;
        std::fs::write(&self.snapshots_file, content)?;

        Ok(())
    }

    /// 清除所有快照
    #[allow(dead_code)]
    pub fn clear(&mut self) -> Result<()> {
        self.snapshots.clear();
        self.save()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_detect_transition() {
        let temp = TempDir::new().unwrap();
        let mut tracker = TaskStateTracker {
            snapshots: HashMap::new(),
            snapshots_file: temp.path().join("task_snapshots.json"),
        };

        let task = CurrentTask {
            id: Some("TASK-001".to_string()),
            name: Some("Test".to_string()),
            status: "IN_PROGRESS".to_string(),
            phase: None,
            started_at: None,
            retry_count: 0,
            max_retries: 5,
            acceptance_progress: None,
            last_updated: None,
        };

        // 第一次，不算转换
        assert!(!tracker.detect_transition(&task));

        // 更新快照
        tracker.update_snapshot(&task).unwrap();

        // 状态未变，不算转换
        assert!(!tracker.detect_transition(&task));

        // 状态改变
        let mut completed_task = task.clone();
        completed_task.status = "COMPLETED".to_string();
        assert!(tracker.detect_transition(&completed_task));
    }

    #[test]
    fn test_classify_transition() {
        let temp = TempDir::new().unwrap();
        let mut tracker = TaskStateTracker {
            snapshots: HashMap::new(),
            snapshots_file: temp.path().join("task_snapshots.json"),
        };

        // 设置初始快照
        let snapshot = TaskSnapshot {
            status: "IN_PROGRESS".to_string(),
            snapshot_time: chrono::Utc::now().to_rfc3339(),
            task_id: "TASK-001".to_string(),
        };
        tracker.snapshots.insert("TASK-001".to_string(), snapshot);

        let task = CurrentTask {
            id: Some("TASK-001".to_string()),
            name: Some("Test".to_string()),
            status: "COMPLETED".to_string(),
            phase: None,
            started_at: None,
            retry_count: 0,
            max_retries: 5,
            acceptance_progress: None,
            last_updated: None,
        };

        assert_eq!(
            tracker.classify_transition(&task),
            TransitionType::CompleteTask
        );
    }
}
