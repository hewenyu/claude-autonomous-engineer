//! 状态转换 Hooks
//!
//! 允许在状态转换前后执行自定义逻辑

use super::{MachineState, StateId};
use anyhow::Result;
use serde_json::Value;
use std::path::PathBuf;

/// 状态转换上下文
#[derive(Debug, Clone)]
pub struct TransitionContext {
    /// 项目根目录
    pub project_root: PathBuf,
    /// 源状态
    pub from_state: StateId,
    /// 目标状态
    pub to_state: StateId,
    /// 任务 ID（可选）
    pub task_id: Option<String>,
    /// 元数据（可选）
    pub metadata: Option<Value>,
}

/// Hook 决策
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookDecision {
    /// 允许转换
    Allow,
    /// 阻止转换
    Block(String), // 原因
    /// 修改目标状态
    Modify(StateId), // 新的目标状态
}

/// PreTransition Hook 特征
///
/// 在状态转换之前执行，可以阻止或修改转换
pub trait PreTransitionHook: Send + Sync {
    /// Hook 名称
    fn name(&self) -> &str;

    /// 执行 hook
    ///
    /// 返回 `HookDecision` 决定是否允许转换
    fn execute(&self, context: &TransitionContext) -> Result<HookDecision>;
}

/// PostTransition Hook 特征
///
/// 在状态转换之后执行，不能阻止转换
pub trait PostTransitionHook: Send + Sync {
    /// Hook 名称
    fn name(&self) -> &str;

    /// 执行 hook
    ///
    /// 接收转换后的完整状态
    fn execute(&self, context: &TransitionContext, new_state: &MachineState) -> Result<()>;
}

/// 状态转换 Hook 管理器
pub struct TransitionHookManager {
    pre_hooks: Vec<Box<dyn PreTransitionHook>>,
    post_hooks: Vec<Box<dyn PostTransitionHook>>,
}

impl Default for TransitionHookManager {
    fn default() -> Self {
        Self::new()
    }
}

impl TransitionHookManager {
    /// 创建新的 Hook 管理器
    pub fn new() -> Self {
        TransitionHookManager {
            pre_hooks: Vec::new(),
            post_hooks: Vec::new(),
        }
    }

    /// 注册 PreTransition Hook
    pub fn register_pre_hook(&mut self, hook: Box<dyn PreTransitionHook>) {
        self.pre_hooks.push(hook);
    }

    /// 注册 PostTransition Hook
    pub fn register_post_hook(&mut self, hook: Box<dyn PostTransitionHook>) {
        self.post_hooks.push(hook);
    }

    /// 执行所有 PreTransition Hooks
    ///
    /// 返回最终的 HookDecision：
    /// - 任何 hook 返回 Block → 阻止转换
    /// - 任何 hook 返回 Modify → 修改目标状态
    /// - 所有 hooks 返回 Allow → 允许转换
    pub fn run_pre_hooks(&self, context: &TransitionContext) -> Result<HookDecision> {
        let mut final_decision = HookDecision::Allow;

        for hook in &self.pre_hooks {
            let decision = hook.execute(context)?;

            match decision {
                HookDecision::Block(reason) => {
                    // 遇到 Block，立即停止并返回
                    println!("🚫 PreTransition hook '{}' blocked transition: {}", hook.name(), reason);
                    return Ok(HookDecision::Block(reason));
                }
                HookDecision::Modify(new_state) => {
                    // 修改目标状态
                    println!("🔄 PreTransition hook '{}' modified target state to {}", hook.name(), new_state.as_str());
                    final_decision = HookDecision::Modify(new_state);
                }
                HookDecision::Allow => {
                    // 继续
                }
            }
        }

        Ok(final_decision)
    }

    /// 执行所有 PostTransition Hooks
    pub fn run_post_hooks(
        &self,
        context: &TransitionContext,
        new_state: &MachineState,
    ) -> Result<()> {
        for hook in &self.post_hooks {
            if let Err(e) = hook.execute(context, new_state) {
                eprintln!(
                    "⚠️  PostTransition hook '{}' failed: {}",
                    hook.name(),
                    e
                );
                // Post hooks 失败不影响状态转换，只记录错误
            }
        }

        Ok(())
    }

    /// 清空所有 hooks
    pub fn clear(&mut self) {
        self.pre_hooks.clear();
        self.post_hooks.clear();
    }
}

// ═══════════════════════════════════════════════════════════════════
// 内置 Hooks
// ═══════════════════════════════════════════════════════════════════

/// 工作流验证 Hook
///
/// 验证状态转换是否符合工作流规则
pub struct WorkflowValidationHook;

impl PreTransitionHook for WorkflowValidationHook {
    fn name(&self) -> &str {
        "workflow_validation"
    }

    fn execute(&self, context: &TransitionContext) -> Result<HookDecision> {
        use crate::state_machine::WorkflowEngine;

        // 验证转换是否合法
        match WorkflowEngine::validate_transition(context.from_state, context.to_state) {
            Ok(_) => Ok(HookDecision::Allow),
            Err(e) => Ok(HookDecision::Block(format!(
                "Invalid transition: {}",
                e
            ))),
        }
    }
}

/// 日志记录 Hook
///
/// 记录所有状态转换到日志文件
pub struct LoggingHook;

impl PostTransitionHook for LoggingHook {
    fn name(&self) -> &str {
        "logging"
    }

    fn execute(&self, context: &TransitionContext, new_state: &MachineState) -> Result<()> {
        let log_file = context.project_root.join(".claude/status/state_transitions.log");

        let log_entry = format!(
            "[{}] {} → {} | Task: {} | Timestamp: {}\n",
            chrono::Utc::now().to_rfc3339(),
            context.from_state.as_str(),
            context.to_state.as_str(),
            context.task_id.as_deref().unwrap_or("-"),
            new_state.timestamp
        );

        // 追加到日志文件
        use std::fs::OpenOptions;
        use std::io::Write;

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file)?;

        file.write_all(log_entry.as_bytes())?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestPreHook {
        decision: HookDecision,
    }

    impl PreTransitionHook for TestPreHook {
        fn name(&self) -> &str {
            "test_pre"
        }

        fn execute(&self, _context: &TransitionContext) -> Result<HookDecision> {
            Ok(self.decision.clone())
        }
    }

    struct TestPostHook;

    impl PostTransitionHook for TestPostHook {
        fn name(&self) -> &str {
            "test_post"
        }

        fn execute(&self, _context: &TransitionContext, _new_state: &MachineState) -> Result<()> {
            Ok(())
        }
    }

    #[test]
    fn test_hook_manager_allow() {
        let mut manager = TransitionHookManager::new();

        manager.register_pre_hook(Box::new(TestPreHook {
            decision: HookDecision::Allow,
        }));
        manager.register_post_hook(Box::new(TestPostHook));

        let context = TransitionContext {
            project_root: PathBuf::from("/tmp"),
            from_state: StateId::Idle,
            to_state: StateId::Planning,
            task_id: None,
            metadata: None,
        };

        let decision = manager.run_pre_hooks(&context).unwrap();
        assert_eq!(decision, HookDecision::Allow);

        // Post hooks 也应能正常执行
        let state = MachineState::default();
        manager.run_post_hooks(&context, &state).unwrap();
    }

    #[test]
    fn test_hook_manager_block() {
        let mut manager = TransitionHookManager::new();

        manager.register_pre_hook(Box::new(TestPreHook {
            decision: HookDecision::Block("Test block".to_string()),
        }));

        let context = TransitionContext {
            project_root: PathBuf::from("/tmp"),
            from_state: StateId::Idle,
            to_state: StateId::Completed, // 非法转换
            task_id: None,
            metadata: None,
        };

        let decision = manager.run_pre_hooks(&context).unwrap();
        assert!(matches!(decision, HookDecision::Block(_)));
    }

    #[test]
    fn test_hook_manager_modify() {
        let mut manager = TransitionHookManager::new();

        manager.register_pre_hook(Box::new(TestPreHook {
            decision: HookDecision::Modify(StateId::Coding),
        }));

        let context = TransitionContext {
            project_root: PathBuf::from("/tmp"),
            from_state: StateId::Planning,
            to_state: StateId::Testing,
            task_id: None,
            metadata: None,
        };

        let decision = manager.run_pre_hooks(&context).unwrap();
        assert_eq!(decision, HookDecision::Modify(StateId::Coding));
    }

    #[test]
    fn test_workflow_validation_hook() {
        let hook = WorkflowValidationHook;

        // 合法转换
        let context = TransitionContext {
            project_root: PathBuf::from("/tmp"),
            from_state: StateId::Planning,
            to_state: StateId::Coding,
            task_id: None,
            metadata: None,
        };

        let decision = hook.execute(&context).unwrap();
        assert_eq!(decision, HookDecision::Allow);

        // 非法转换
        let context_invalid = TransitionContext {
            project_root: PathBuf::from("/tmp"),
            from_state: StateId::Planning,
            to_state: StateId::Completed,
            task_id: None,
            metadata: None,
        };

        let decision = hook.execute(&context_invalid).unwrap();
        assert!(matches!(decision, HookDecision::Block(_)));
    }
}
