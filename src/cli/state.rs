//! 状态机 CLI 命令实现

use crate::project::find_project_root;
use crate::state_machine::{GitStateMachine, StateId, StateVisualizer, WorkflowEngine};
use anyhow::{Context, Result};
use colored::Colorize;

/// 列出所有状态快照
pub fn list_states() -> Result<()> {
    let project_root = find_project_root()
        .context("No .claude directory found. Run 'claude-autonomous init' first.")?;
    let state_machine = GitStateMachine::new(&project_root)?;

    let snapshots = state_machine.list_states()?;

    if snapshots.is_empty() {
        println!("No state snapshots found.");
        println!("💡 Tip: Use state transitions to create checkpoints automatically.");
        return Ok(());
    }

    // 获取当前状态的 tag（如果存在）
    let current_state = state_machine.current_state().ok();
    let current_tag = snapshots.iter().find(|s| {
        s.state
            .as_ref()
            .and_then(|state| current_state.as_ref().map(|cs| state.state_id == cs.state_id))
            .unwrap_or(false)
    });

    let output = StateVisualizer::render_state_list(
        &snapshots,
        current_tag.map(|s| s.tag.as_str()),
    );

    println!("{}", output);

    Ok(())
}

/// 显示当前状态
pub fn show_current_state() -> Result<()> {
    let project_root = find_project_root()
        .context("No .claude directory found. Run 'claude-autonomous init' first.")?;
    let state_machine = GitStateMachine::new(&project_root)?;

    let state = state_machine.current_state()?;

    println!("📊 Current State:\n");
    println!("  State:     {} {}", state.state_id.icon(), state.state_id.as_str());
    println!(
        "  Task ID:   {}",
        state.task_id.as_deref().unwrap_or("-")
    );
    println!(
        "  Phase:     {}",
        state.phase.as_deref().unwrap_or("-")
    );
    println!("  Timestamp: {}", state.timestamp);

    println!("\n📝 Description:");
    println!("  {}", WorkflowEngine::state_description(state.state_id));

    // 显示可能的后继状态
    let next_states = WorkflowEngine::next_states(state.state_id);
    if !next_states.is_empty() {
        println!("\n🔄 Possible Next States:");
        for next in &next_states {
            let icon = if WorkflowEngine::recommend_next_state(state.state_id) == Some(*next) {
                "→".green().bold()
            } else {
                "→".normal()
            };

            println!("  {} {} {}", icon, next.icon(), next.as_str());
        }
    }

    Ok(())
}

/// 回滚到指定 tag
pub fn rollback_to_tag(tag: &str) -> Result<()> {
    let project_root = find_project_root()
        .context("No .claude directory found. Run 'claude-autonomous init' first.")?;
    let state_machine = GitStateMachine::new(&project_root)?;

    // 验证 tag 存在
    let snapshots = state_machine.list_states()?;
    let snapshot = snapshots
        .iter()
        .find(|s| s.tag == tag)
        .context("Tag not found")?;

    // 显示回滚目标
    let (state_id, task_id) = snapshot.parse_tag_info().unwrap_or((StateId::Idle, None));

    println!("🔄 Rolling back to:");
    println!("  Tag:   {}", tag);
    println!("  State: {} {}", state_id.icon(), state_id.as_str());
    println!("  Task:  {}", task_id.as_deref().unwrap_or("-"));
    println!("  Time:  {}", snapshot.formatted_time());

    // 执行回滚
    state_machine.rollback_to_tag(tag)?;

    println!("\n✅ Rollback completed successfully!");
    println!("💡 Tip: Run 'claude-autonomous state current' to verify the new state.");

    Ok(())
}

/// 显示状态转换图
pub fn show_state_graph(task_id: Option<&str>) -> Result<()> {
    let project_root = find_project_root()
        .context("No .claude directory found. Run 'claude-autonomous init' first.")?;
    let state_machine = GitStateMachine::new(&project_root)?;

    let snapshots = state_machine.list_states()?;

    if snapshots.is_empty() {
        println!("No state transitions found.");
        return Ok(());
    }

    let output = StateVisualizer::render_transition_graph(&snapshots, task_id);
    println!("{}", output);

    // 也显示简化流程图
    println!("\n{}", StateVisualizer::render_compact_flow(&snapshots));

    Ok(())
}

/// 手动创建状态转换
pub fn transition_to(state_str: &str, task_id: Option<&str>) -> Result<()> {
    let project_root = find_project_root()
        .context("No .claude directory found. Run 'claude-autonomous init' first.")?;
    let state_machine = GitStateMachine::new(&project_root)?;

    // 解析目标状态
    let target_state = StateId::from_str(state_str)
        .context(format!("Invalid state: {}", state_str))?;

    // 获取当前状态
    let current_state = state_machine.current_state()?;

    // 验证转换合法性
    WorkflowEngine::validate_transition(current_state.state_id, target_state)
        .context("Invalid state transition")?;

    println!("🔄 State Transition:");
    println!(
        "  From: {} {}",
        current_state.state_id.icon(),
        current_state.state_id.as_str()
    );
    println!("  To:   {} {}", target_state.icon(), target_state.as_str());

    if let Some(tid) = task_id {
        println!("  Task: {}", tid);
    }

    // 执行转换
    let tag = state_machine.transition_to(target_state, task_id, None)?;

    println!("\n✅ Transition completed!");
    println!("  Created tag: {}", tag);

    Ok(())
}

/// 显示工作流帮助
pub fn show_workflow_help() -> Result<()> {
    println!("📋 State Machine Workflow Guide\n");

    println!("Available States:");
    let states = [
        StateId::Idle,
        StateId::Planning,
        StateId::Coding,
        StateId::Testing,
        StateId::Reviewing,
        StateId::Completed,
        StateId::Blocked,
    ];

    for state in &states {
        println!(
            "  {} {:10} - {}",
            state.icon(),
            state.as_str(),
            WorkflowEngine::state_description(*state)
        );
    }

    println!("\n🔄 Standard Workflow:");
    println!("  Idle → Planning → Coding → Testing → Reviewing → Completed");

    println!("\n📝 Common Commands:");
    println!("  claude-autonomous state current         # Show current state");
    println!("  claude-autonomous state list            # List all state snapshots");
    println!("  claude-autonomous state graph           # Show transition graph");
    println!("  claude-autonomous state rollback <tag>  # Rollback to a previous state");
    println!("  claude-autonomous state transition <state> [--task-id <id>]  # Manual transition");

    Ok(())
}
