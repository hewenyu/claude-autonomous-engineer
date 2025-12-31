//! 工作流引擎 - 定义状态转换规则

use super::StateId;
use anyhow::{bail, Result};

/// 状态转换规则引擎
pub struct WorkflowEngine;

impl WorkflowEngine {
    /// 验证状态转换是否合法
    pub fn validate_transition(from: StateId, to: StateId) -> Result<()> {
        let valid = match (from, to) {
            // 从 Idle 可以到任何状态
            (StateId::Idle, _) => true,

            // Planning 的合法转换
            (StateId::Planning, StateId::Coding) => true,
            (StateId::Planning, StateId::Blocked) => true,

            // Coding 的合法转换
            (StateId::Coding, StateId::Testing) => true,
            (StateId::Coding, StateId::Blocked) => true,
            (StateId::Coding, StateId::Reviewing) => true, // 可以直接进入审查

            // Testing 的合法转换
            (StateId::Testing, StateId::Coding) => true, // 测试失败回到编码
            (StateId::Testing, StateId::Reviewing) => true,
            (StateId::Testing, StateId::Blocked) => true,

            // Reviewing 的合法转换
            (StateId::Reviewing, StateId::Completed) => true,
            (StateId::Reviewing, StateId::Coding) => true, // 审查未通过回到编码
            (StateId::Reviewing, StateId::Blocked) => true,

            // Blocked 可以回到任何进行中的状态
            (StateId::Blocked, StateId::Planning) => true,
            (StateId::Blocked, StateId::Coding) => true,
            (StateId::Blocked, StateId::Testing) => true,
            (StateId::Blocked, StateId::Reviewing) => true,

            // Completed 可以回到 Idle 或开始新任务
            (StateId::Completed, StateId::Idle) => true,
            (StateId::Completed, StateId::Planning) => true,

            // 同状态不算转换
            (s1, s2) if s1 == s2 => true,

            // 其他转换不合法
            _ => false,
        };

        if !valid {
            bail!(
                "Invalid state transition: {} → {}",
                from.as_str(),
                to.as_str()
            );
        }

        Ok(())
    }

    /// 获取某个状态的所有可能后继状态
    pub fn next_states(from: StateId) -> Vec<StateId> {
        match from {
            StateId::Idle => vec![StateId::Planning],
            StateId::Planning => vec![StateId::Coding, StateId::Blocked],
            StateId::Coding => vec![
                StateId::Testing,
                StateId::Reviewing,
                StateId::Blocked,
            ],
            StateId::Testing => vec![
                StateId::Coding,
                StateId::Reviewing,
                StateId::Blocked,
            ],
            StateId::Reviewing => vec![
                StateId::Completed,
                StateId::Coding,
                StateId::Blocked,
            ],
            StateId::Blocked => vec![
                StateId::Planning,
                StateId::Coding,
                StateId::Testing,
                StateId::Reviewing,
            ],
            StateId::Completed => vec![StateId::Idle, StateId::Planning],
        }
    }

    /// 推荐下一个状态（基于标准工作流）
    pub fn recommend_next_state(from: StateId) -> Option<StateId> {
        match from {
            StateId::Idle => Some(StateId::Planning),
            StateId::Planning => Some(StateId::Coding),
            StateId::Coding => Some(StateId::Testing),
            StateId::Testing => Some(StateId::Reviewing),
            StateId::Reviewing => Some(StateId::Completed),
            StateId::Blocked => None, // 需要人工决定
            StateId::Completed => Some(StateId::Idle),
        }
    }

    /// 获取状态描述
    pub fn state_description(state: StateId) -> &'static str {
        match state {
            StateId::Idle => "空闲状态，无活动任务",
            StateId::Planning => "规划阶段，设计架构和任务分解",
            StateId::Coding => "编码阶段，实现功能代码",
            StateId::Testing => "测试阶段，运行测试并验证功能",
            StateId::Reviewing => "审查阶段，代码审查和质量检查",
            StateId::Completed => "任务完成，等待下一个任务",
            StateId::Blocked => "阻塞状态，等待外部条件或人工介入",
        }
    }

    /// 判断状态是否是终止状态
    pub fn is_terminal_state(state: StateId) -> bool {
        matches!(state, StateId::Completed | StateId::Blocked)
    }

    /// 判断状态是否是活跃状态（需要持续工作）
    pub fn is_active_state(state: StateId) -> bool {
        matches!(
            state,
            StateId::Planning | StateId::Coding | StateId::Testing | StateId::Reviewing
        )
    }
}

/// 预定义工作流模板
pub struct WorkflowTemplate;

impl WorkflowTemplate {
    /// 标准开发工作流
    ///
    /// Idle → Planning → Coding → Testing → Reviewing → Completed
    pub fn standard_workflow() -> Vec<StateId> {
        vec![
            StateId::Idle,
            StateId::Planning,
            StateId::Coding,
            StateId::Testing,
            StateId::Reviewing,
            StateId::Completed,
        ]
    }

    /// 快速原型工作流（跳过测试）
    ///
    /// Idle → Planning → Coding → Reviewing → Completed
    pub fn rapid_prototype_workflow() -> Vec<StateId> {
        vec![
            StateId::Idle,
            StateId::Planning,
            StateId::Coding,
            StateId::Reviewing,
            StateId::Completed,
        ]
    }

    /// TDD 工作流（测试先行）
    ///
    /// Idle → Planning → Testing → Coding → Testing → Reviewing → Completed
    pub fn tdd_workflow() -> Vec<StateId> {
        vec![
            StateId::Idle,
            StateId::Planning,
            StateId::Testing, // 先写测试
            StateId::Coding,
            StateId::Testing, // 再运行测试
            StateId::Reviewing,
            StateId::Completed,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_transitions() {
        assert!(WorkflowEngine::validate_transition(StateId::Idle, StateId::Planning).is_ok());
        assert!(WorkflowEngine::validate_transition(StateId::Planning, StateId::Coding).is_ok());
        assert!(WorkflowEngine::validate_transition(StateId::Coding, StateId::Testing).is_ok());
        assert!(
            WorkflowEngine::validate_transition(StateId::Testing, StateId::Reviewing).is_ok()
        );
        assert!(
            WorkflowEngine::validate_transition(StateId::Reviewing, StateId::Completed).is_ok()
        );
    }

    #[test]
    fn test_invalid_transitions() {
        // Planning 不能直接到 Completed
        assert!(
            WorkflowEngine::validate_transition(StateId::Planning, StateId::Completed).is_err()
        );

        // Coding 不能直接到 Completed
        assert!(WorkflowEngine::validate_transition(StateId::Coding, StateId::Completed).is_err());

        // Testing 不能直接到 Completed
        assert!(
            WorkflowEngine::validate_transition(StateId::Testing, StateId::Completed).is_err()
        );
    }

    #[test]
    fn test_rollback_transitions() {
        // 测试失败可以回到编码
        assert!(WorkflowEngine::validate_transition(StateId::Testing, StateId::Coding).is_ok());

        // 审查失败可以回到编码
        assert!(
            WorkflowEngine::validate_transition(StateId::Reviewing, StateId::Coding).is_ok()
        );
    }

    #[test]
    fn test_blocked_transitions() {
        // 任何活跃状态都可以进入阻塞
        assert!(WorkflowEngine::validate_transition(StateId::Planning, StateId::Blocked).is_ok());
        assert!(WorkflowEngine::validate_transition(StateId::Coding, StateId::Blocked).is_ok());

        // 阻塞可以回到任何状态
        assert!(WorkflowEngine::validate_transition(StateId::Blocked, StateId::Coding).is_ok());
    }

    #[test]
    fn test_next_states() {
        let next = WorkflowEngine::next_states(StateId::Coding);
        assert!(next.contains(&StateId::Testing));
        assert!(next.contains(&StateId::Reviewing));
        assert!(next.contains(&StateId::Blocked));
    }

    #[test]
    fn test_recommend_next_state() {
        assert_eq!(
            WorkflowEngine::recommend_next_state(StateId::Planning),
            Some(StateId::Coding)
        );
        assert_eq!(
            WorkflowEngine::recommend_next_state(StateId::Coding),
            Some(StateId::Testing)
        );
    }

    #[test]
    fn test_state_classification() {
        assert!(WorkflowEngine::is_terminal_state(StateId::Completed));
        assert!(WorkflowEngine::is_terminal_state(StateId::Blocked));

        assert!(WorkflowEngine::is_active_state(StateId::Coding));
        assert!(WorkflowEngine::is_active_state(StateId::Testing));

        assert!(!WorkflowEngine::is_active_state(StateId::Completed));
    }

    #[test]
    fn test_workflow_templates() {
        let standard = WorkflowTemplate::standard_workflow();
        assert_eq!(standard.len(), 6);
        assert_eq!(standard[0], StateId::Idle);
        assert_eq!(standard[5], StateId::Completed);

        let rapid = WorkflowTemplate::rapid_prototype_workflow();
        assert_eq!(rapid.len(), 5);
        assert!(!rapid.contains(&StateId::Testing));
    }
}
