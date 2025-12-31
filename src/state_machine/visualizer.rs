//! 状态机可视化器 - 生成状态转换图

use super::{StateId, StateSnapshot};
use colored::Colorize;

/// 状态机可视化器
pub struct StateVisualizer;

impl StateVisualizer {
    /// 生成状态列表表格
    pub fn render_state_list(snapshots: &[StateSnapshot], current_tag: Option<&str>) -> String {
        let mut output = String::new();

        output.push_str("📊 State Machine History:\n\n");

        // 表格头部
        output.push_str("┌────────────────────────────────────────────────────────────────────┐\n");
        output.push_str("│ Tag                                      │ State     │ Task        │\n");
        output.push_str("├────────────────────────────────────────────────────────────────────┤\n");

        // 表格行
        for snapshot in snapshots {
            let tag_display = if snapshot.tag.len() > 40 {
                format!("{}...", &snapshot.tag[..37])
            } else {
                format!("{:40}", snapshot.tag)
            };

            let (state_id, task_id) = snapshot.parse_tag_info().unwrap_or((StateId::Idle, None));

            let state_str = format!("{} {}", state_id.icon(), state_id.as_str());

            let task_str = task_id.as_deref().unwrap_or("-");

            let current_marker = if Some(snapshot.tag.as_str()) == current_tag {
                " ← Current"
            } else {
                ""
            };

            output.push_str(&format!(
                "│ {} │ {:9} │ {:11} │{}\n",
                tag_display, state_str, task_str, current_marker
            ));
        }

        output.push_str("└────────────────────────────────────────────────────────────────────┘\n");

        // 统计信息
        output.push_str(&format!("\nTotal Transitions: {}\n", snapshots.len()));

        // 计算回滚次数（检测状态后退）
        let rollbacks = Self::count_rollbacks(snapshots);
        if rollbacks > 0 {
            output.push_str(&format!("Rollbacks: {} ", rollbacks));
            output.push_str(&Self::detect_rollback_pattern(snapshots));
            output.push('\n');
        }

        output
    }

    /// 生成状态转换图（ASCII 艺术）
    pub fn render_transition_graph(
        snapshots: &[StateSnapshot],
        task_id: Option<&str>,
    ) -> String {
        let mut output = String::new();

        output.push_str("📈 State Transition Graph");
        if let Some(tid) = task_id {
            output.push_str(&format!(" for {}", tid));
        }
        output.push_str(":\n\n");

        // 过滤指定任务的快照
        let filtered: Vec<_> = if let Some(tid) = task_id {
            snapshots
                .iter()
                .filter(|s| {
                    s.parse_tag_info()
                        .and_then(|(_, t)| t)
                        .as_deref()
                        == Some(tid)
                })
                .collect()
        } else {
            snapshots.iter().collect()
        };

        if filtered.is_empty() {
            output.push_str("    No state transitions found.\n");
            return output;
        }

        // 开始节点
        output.push_str("    Start\n");
        output.push_str("      │\n");

        // 遍历状态转换
        for (idx, snapshot) in filtered.iter().rev().enumerate() {
            let (state_id, _) = snapshot.parse_tag_info().unwrap_or((StateId::Idle, None));

            // 状态节点
            let time_str = Self::format_short_time(snapshot.timestamp);
            let tag_suffix = Self::extract_tag_suffix(&snapshot.tag);

            output.push_str(&format!(
                "      ▼\n  {:9} {} ({})\n",
                state_id.as_str(),
                "─".repeat(30),
                time_str
            ));

            output.push_str(&format!(
                "      │{:>38}tag: ...{}\n",
                "", tag_suffix
            ));

            // 检测是否有提交或特殊事件
            if idx < filtered.len() - 1 {
                let next_snapshot = filtered[idx + 1];
                let (next_state, _) =
                    next_snapshot.parse_tag_info().unwrap_or((StateId::Idle, None));

                if Self::is_rollback_transition(state_id, next_state) {
                    output.push_str("      │\n");
                    output.push_str(&format!(
                        "      {} ROLLBACK\n",
                        "✗".to_string().red().bold()
                    ));
                } else if Self::is_success_transition(state_id, next_state) {
                    output.push_str("      │\n");
                    output.push_str(&format!(
                        "      {} PASSED\n",
                        "✓".to_string().green().bold()
                    ));
                }

                // 添加连接线
                output.push_str("      │\n");
            }
        }

        // 结束节点
        output.push_str("      ▼\n");
        output.push_str("    End\n");

        output
    }

    /// 生成简化的状态流程图
    pub fn render_compact_flow(snapshots: &[StateSnapshot]) -> String {
        let mut output = String::new();

        output.push_str("State Flow: ");

        for (idx, snapshot) in snapshots.iter().rev().enumerate() {
            let (state_id, _) = snapshot.parse_tag_info().unwrap_or((StateId::Idle, None));

            output.push_str(&format!("{} {}", state_id.icon(), state_id.as_str()));

            if idx < snapshots.len() - 1 {
                output.push_str(" → ");
            }
        }

        output.push('\n');

        output
    }

    // ═══════════════════════════════════════════════════════════════════
    // 辅助函数
    // ═══════════════════════════════════════════════════════════════════

    /// 计算回滚次数
    fn count_rollbacks(snapshots: &[StateSnapshot]) -> usize {
        let mut count = 0;

        for i in 0..snapshots.len().saturating_sub(1) {
            let (current_state, _) = snapshots[i]
                .parse_tag_info()
                .unwrap_or((StateId::Idle, None));
            let (prev_state, _) = snapshots[i + 1]
                .parse_tag_info()
                .unwrap_or((StateId::Idle, None));

            if Self::is_rollback_transition(current_state, prev_state) {
                count += 1;
            }
        }

        count
    }

    /// 检测回滚模式
    fn detect_rollback_pattern(snapshots: &[StateSnapshot]) -> String {
        let mut patterns = Vec::new();

        for i in 0..snapshots.len().saturating_sub(1) {
            let (current_state, _) = snapshots[i]
                .parse_tag_info()
                .unwrap_or((StateId::Idle, None));
            let (prev_state, _) = snapshots[i + 1]
                .parse_tag_info()
                .unwrap_or((StateId::Idle, None));

            if Self::is_rollback_transition(current_state, prev_state) {
                patterns.push(format!("{} → {}", prev_state.as_str(), current_state.as_str()));
            }
        }

        if patterns.is_empty() {
            String::new()
        } else {
            format!("({})", patterns.join(", "))
        }
    }

    /// 判断是否是回滚转换
    fn is_rollback_transition(from: StateId, to: StateId) -> bool {
        matches!(
            (from, to),
            (StateId::Testing, StateId::Coding)
                | (StateId::Reviewing, StateId::Coding)
                | (StateId::Reviewing, StateId::Testing)
        )
    }

    /// 判断是否是成功转换
    fn is_success_transition(from: StateId, to: StateId) -> bool {
        matches!(
            (from, to),
            (StateId::Planning, StateId::Coding)
                | (StateId::Coding, StateId::Testing)
                | (StateId::Testing, StateId::Reviewing)
                | (StateId::Reviewing, StateId::Completed)
        )
    }

    /// 格式化简短时间
    fn format_short_time(timestamp: i64) -> String {
        use chrono::{TimeZone, Utc};

        match Utc.timestamp_opt(timestamp, 0).single() {
            Some(dt) => dt.format("%m-%d %H:%M").to_string(),
            None => "??-?? ??:??".to_string(),
        }
    }

    /// 提取 tag 后缀（最后 20 个字符）
    fn extract_tag_suffix(tag: &str) -> &str {
        if tag.len() > 20 {
            &tag[tag.len() - 20..]
        } else {
            tag
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_snapshot(tag: &str, timestamp: i64) -> StateSnapshot {
        StateSnapshot {
            tag: tag.to_string(),
            commit_sha: "abc123".to_string(),
            message: "test".to_string(),
            timestamp,
            state: None,
        }
    }

    #[test]
    fn test_render_state_list() {
        let snapshots = vec![
            create_test_snapshot("state-20251231-120000-planning-TASK-001", 1000),
            create_test_snapshot("state-20251231-130000-coding-TASK-001", 2000),
        ];

        let output = StateVisualizer::render_state_list(&snapshots, None);

        assert!(output.contains("State Machine History"));
        assert!(output.contains("planning"));
        assert!(output.contains("coding"));
        assert!(output.contains("Total Transitions: 2"));
    }

    #[test]
    fn test_render_compact_flow() {
        let snapshots = vec![
            create_test_snapshot("state-20251231-120000-planning-TASK-001", 1000),
            create_test_snapshot("state-20251231-130000-coding-TASK-001", 2000),
            create_test_snapshot("state-20251231-140000-testing-TASK-001", 3000),
        ];

        let output = StateVisualizer::render_compact_flow(&snapshots);

        assert!(output.contains("planning"));
        assert!(output.contains("coding"));
        assert!(output.contains("testing"));
        assert!(output.contains("→"));
    }

    #[test]
    fn test_count_rollbacks() {
        let snapshots = vec![
            create_test_snapshot("state-20251231-140000-testing-TASK-001", 3000),
            create_test_snapshot("state-20251231-130000-coding-TASK-001", 2000), // 回滚
            create_test_snapshot("state-20251231-120000-planning-TASK-001", 1000),
        ];

        let count = StateVisualizer::count_rollbacks(&snapshots);
        assert_eq!(count, 1);
    }

    #[test]
    fn test_is_rollback_transition() {
        assert!(StateVisualizer::is_rollback_transition(
            StateId::Testing,
            StateId::Coding
        ));
        assert!(StateVisualizer::is_rollback_transition(
            StateId::Reviewing,
            StateId::Coding
        ));
        assert!(!StateVisualizer::is_rollback_transition(
            StateId::Coding,
            StateId::Testing
        ));
    }

    #[test]
    fn test_is_success_transition() {
        assert!(StateVisualizer::is_success_transition(
            StateId::Planning,
            StateId::Coding
        ));
        assert!(StateVisualizer::is_success_transition(
            StateId::Coding,
            StateId::Testing
        ));
        assert!(!StateVisualizer::is_success_transition(
            StateId::Testing,
            StateId::Coding
        ));
    }
}
