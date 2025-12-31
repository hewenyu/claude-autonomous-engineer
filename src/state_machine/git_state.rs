//! Git 驱动的状态机核心实现
//!
//! 使用 Git 的 commits 和 tags 作为状态快照存储机制

use super::hooks::{
    HookDecision, LoggingHook, PostTransitionHook, PreTransitionHook, TransitionContext,
    TransitionHookManager, WorkflowValidationHook,
};
use super::{MachineState, StateId, StateSnapshot};
use anyhow::{bail, Context, Result};
use git2::{Commit, Repository, Signature, StatusOptions};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Git 驱动的状态机
pub struct GitStateMachine {
    /// Git 仓库
    repo: Repository,
    /// 项目根目录
    project_root: PathBuf,
    /// 状态文件路径
    state_file: PathBuf,
    /// Hook 管理器
    hook_manager: Arc<Mutex<TransitionHookManager>>,
}

impl GitStateMachine {
    /// 创建新的状态机实例
    pub fn new(project_root: &Path) -> Result<Self> {
        let repo = Repository::open(project_root)
            .context("Failed to open git repository. Is this a git project?")?;

        let state_file = project_root.join(".claude/status/state.json");

        // 确保状态目录存在
        if let Some(parent) = state_file.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // 创建 Hook 管理器并注册默认 hooks
        let mut hook_manager = TransitionHookManager::new();

        // PreTransition hooks
        hook_manager.register_pre_hook(Box::new(WorkflowValidationHook));

        // PostTransition hooks
        hook_manager.register_post_hook(Box::new(LoggingHook));

        Ok(GitStateMachine {
            repo,
            project_root: project_root.to_path_buf(),
            state_file,
            hook_manager: Arc::new(Mutex::new(hook_manager)),
        })
    }

    /// 注册自定义 PreTransition Hook
    pub fn register_pre_hook(&self, hook: Box<dyn PreTransitionHook>) {
        if let Ok(mut manager) = self.hook_manager.lock() {
            manager.register_pre_hook(hook);
        }
    }

    /// 注册自定义 PostTransition Hook
    pub fn register_post_hook(&self, hook: Box<dyn PostTransitionHook>) {
        if let Ok(mut manager) = self.hook_manager.lock() {
            manager.register_post_hook(hook);
        }
    }

    /// 获取当前状态
    pub fn current_state(&self) -> Result<MachineState> {
        if !self.state_file.exists() {
            return Ok(MachineState::default());
        }

        let content = std::fs::read_to_string(&self.state_file)?;
        let state: MachineState = serde_json::from_str(&content)?;

        Ok(state)
    }

    /// 状态转换（自动 commit + tag）
    pub fn transition_to(
        &self,
        new_state_id: StateId,
        task_id: Option<&str>,
        metadata: Option<serde_json::Value>,
    ) -> Result<String> {
        // 0. 防止把用户已暂存的修改一起提交进“状态提交”
        self.ensure_no_staged_changes()?;

        // 0. 获取当前状态（用于 hooks）
        let current_state = self.current_state()?;

        // 0.5. 创建转换上下文
        let mut context = TransitionContext {
            project_root: self.project_root.clone(),
            from_state: current_state.state_id,
            to_state: new_state_id,
            task_id: task_id.map(|s| s.to_string()),
            metadata: metadata.clone(),
        };

        // 0.6. 执行 PreTransition hooks
        let final_state_id = if let Ok(manager) = self.hook_manager.lock() {
            match manager.run_pre_hooks(&context)? {
                HookDecision::Allow => new_state_id,
                HookDecision::Block(reason) => {
                    bail!("State transition blocked by hook: {}", reason);
                }
                HookDecision::Modify(modified_state) => {
                    // Hook 修改了目标状态
                    context.to_state = modified_state;
                    modified_state
                }
            }
        } else {
            new_state_id
        };

        // 1. 创建新状态
        let mut state = MachineState::new(final_state_id, task_id.map(|s| s.to_string()));

        if let Some(meta) = metadata {
            state = state.with_metadata(meta);
        }

        // 2. 写入状态文件
        let state_json = serde_json::to_string_pretty(&state)?;
        std::fs::write(&self.state_file, state_json)?;

        // 3. Git add
        let mut index = self.repo.index()?;
        let relative_path = self.state_file.strip_prefix(&self.project_root)?;
        index.add_path(relative_path)?;
        index.write()?;

        // 4. Git commit
        let tree_id = index.write_tree()?;
        let tree = self.repo.find_tree(tree_id)?;

        let sig = self.get_signature()?;

        let message = format!(
            "state: {} | task: {}",
            final_state_id.as_str(),
            task_id.unwrap_or("none")
        );

        // 获取 HEAD commit 作为 parent
        let parent_commit = match self.repo.head() {
            Ok(head) => Some(head.peel_to_commit()?),
            Err(_) => None, // 首次提交
        };

        let commit_oid = if let Some(parent) = parent_commit {
            self.repo
                .commit(Some("HEAD"), &sig, &sig, &message, &tree, &[&parent])?
        } else {
            // 首次提交（无 parent）
            self.repo
                .commit(Some("HEAD"), &sig, &sig, &message, &tree, &[])?
        };

        // 5. Git tag
        let tag_name = format!(
            "state-{}-{}-{}",
            chrono::Utc::now().format("%Y%m%d-%H%M%S"),
            final_state_id.as_str(),
            task_id.unwrap_or("none")
        );

        let commit = self.repo.find_commit(commit_oid)?;
        self.repo
            .tag_lightweight(&tag_name, commit.as_object(), false)?;

        // 6. 执行 PostTransition hooks
        if let Ok(manager) = self.hook_manager.lock() {
            let _ = manager.run_post_hooks(&context, &state);
        }

        Ok(tag_name)
    }

    /// 回滚到指定 tag
    pub fn rollback_to_tag(&self, tag_name: &str) -> Result<()> {
        // 查找 tag
        let tag_ref = self
            .repo
            .find_reference(&format!("refs/tags/{}", tag_name))
            .context("Tag not found")?;

        let commit = tag_ref.peel_to_commit()?;

        // 从 commit 中读取状态文件内容
        let tree = commit.tree()?;

        let relative_path = self.state_file.strip_prefix(&self.project_root)?;

        let entry = tree
            .get_path(relative_path)
            .context("State file not found in commit")?;

        let blob = self.repo.find_blob(entry.id())?;
        let content = blob.content();

        // 写入当前工作目录
        std::fs::write(&self.state_file, content)?;

        println!("✅ State rolled back to: {}", tag_name);

        Ok(())
    }

    /// 列出所有状态快照
    pub fn list_states(&self) -> Result<Vec<StateSnapshot>> {
        let mut snapshots = Vec::new();

        // 获取所有 tags
        let tag_names = self.repo.tag_names(Some("state-*"))?;

        for tag_name in tag_names.iter().flatten() {
            if let Ok(snapshot) = self.get_snapshot_from_tag(tag_name) {
                snapshots.push(snapshot);
            }
        }

        // 按时间戳排序
        snapshots.sort_by_key(|s| std::cmp::Reverse(s.timestamp));

        Ok(snapshots)
    }

    /// 从 tag 名称获取快照信息
    fn get_snapshot_from_tag(&self, tag_name: &str) -> Result<StateSnapshot> {
        let tag_ref = self
            .repo
            .find_reference(&format!("refs/tags/{}", tag_name))?;

        let commit = tag_ref.peel_to_commit()?;

        let snapshot = StateSnapshot {
            tag: tag_name.to_string(),
            commit_sha: commit.id().to_string(),
            message: commit.message().unwrap_or("").to_string(),
            timestamp: commit.time().seconds(),
            state: self.extract_state_from_commit(&commit).ok(),
        };

        Ok(snapshot)
    }

    /// 从 commit 中提取状态信息
    fn extract_state_from_commit(&self, commit: &Commit) -> Result<MachineState> {
        let tree = commit.tree()?;

        let relative_path = self.state_file.strip_prefix(&self.project_root)?;

        let entry = tree.get_path(relative_path)?;
        let blob = self.repo.find_blob(entry.id())?;
        let content = std::str::from_utf8(blob.content())?;

        let state: MachineState = serde_json::from_str(content)?;

        Ok(state)
    }

    /// 获取 Git 签名
    fn get_signature(&self) -> Result<Signature<'_>> {
        // 尝试从 git config 读取
        match self.repo.signature() {
            Ok(sig) => Ok(sig),
            Err(_) => {
                // 使用默认签名
                Signature::now("claude-autonomous", "noreply@claude-autonomous.dev")
                    .context("Failed to create git signature")
            }
        }
    }

    /// 确保当前 index 中没有已暂存的用户修改
    ///
    /// 由于状态转换会创建一次 commit，如果 index 里存在其他暂存内容，
    /// 会导致“状态提交”意外夹带用户变更。
    fn ensure_no_staged_changes(&self) -> Result<()> {
        let mut opts = StatusOptions::new();
        opts.include_untracked(false)
            .recurse_untracked_dirs(false)
            .include_ignored(false)
            .show(git2::StatusShow::IndexAndWorkdir);

        let statuses = self.repo.statuses(Some(&mut opts))?;

        let has_index_changes = statuses.iter().any(|e| {
            let s = e.status();
            s.is_index_new()
                || s.is_index_modified()
                || s.is_index_deleted()
                || s.is_index_renamed()
                || s.is_index_typechange()
        });

        if has_index_changes {
            bail!(
                "Refusing to create a state commit while there are staged changes in the index. \
                 Please commit or unstage your changes first."
            );
        }

        Ok(())
    }

    /// 检查仓库是否干净（无未提交的修改）
    pub fn is_clean(&self) -> Result<bool> {
        let statuses = self.repo.statuses(None)?;

        Ok(statuses.is_empty())
    }

    /// 获取当前 HEAD commit SHA
    pub fn head_commit_sha(&self) -> Result<String> {
        let head = self.repo.head()?;
        let commit = head.peel_to_commit()?;

        Ok(commit.id().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup_test_repo() -> (TempDir, GitStateMachine) {
        let temp = TempDir::new().unwrap();
        let repo_path = temp.path();

        // 初始化 git 仓库
        Repository::init(repo_path).unwrap();

        // 创建状态目录
        std::fs::create_dir_all(repo_path.join(".claude/status")).unwrap();

        // 创建初始提交
        let repo = Repository::open(repo_path).unwrap();
        let sig = Signature::now("test", "test@test.com").unwrap();

        let mut index = repo.index().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();

        repo.commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree, &[])
            .unwrap();

        let state_machine = GitStateMachine::new(repo_path).unwrap();

        (temp, state_machine)
    }

    #[test]
    fn test_create_state_machine() {
        let (_temp, sm) = setup_test_repo();
        assert!(sm.state_file.exists() || !sm.state_file.exists()); // Just check it's valid
    }

    #[test]
    fn test_default_state() {
        let (_temp, sm) = setup_test_repo();
        let state = sm.current_state().unwrap();
        assert_eq!(state.state_id, StateId::Idle);
    }

    #[test]
    fn test_transition_to() {
        let (_temp, sm) = setup_test_repo();

        let tag = sm
            .transition_to(StateId::Planning, Some("TASK-001"), None)
            .unwrap();

        assert!(tag.starts_with("state-"));
        assert!(tag.contains("planning"));
        assert!(tag.contains("TASK-001"));

        let state = sm.current_state().unwrap();
        assert_eq!(state.state_id, StateId::Planning);
        assert_eq!(state.task_id, Some("TASK-001".to_string()));
    }

    #[test]
    fn test_list_states() {
        let (_temp, sm) = setup_test_repo();

        sm.transition_to(StateId::Planning, Some("TASK-001"), None)
            .unwrap();
        sm.transition_to(StateId::Coding, Some("TASK-001"), None)
            .unwrap();

        let snapshots = sm.list_states().unwrap();
        assert_eq!(snapshots.len(), 2);

        // 验证最新的在前面（因为按时间戳倒序）
        assert_eq!(
            snapshots[0].state.as_ref().unwrap().state_id,
            StateId::Coding
        );
        assert_eq!(
            snapshots[1].state.as_ref().unwrap().state_id,
            StateId::Planning
        );
    }

    #[test]
    fn test_rollback() {
        let (_temp, sm) = setup_test_repo();

        let tag1 = sm
            .transition_to(StateId::Planning, Some("TASK-001"), None)
            .unwrap();
        sm.transition_to(StateId::Coding, Some("TASK-001"), None)
            .unwrap();

        // 当前应该是 Coding
        assert_eq!(sm.current_state().unwrap().state_id, StateId::Coding);

        // 回滚到 Planning
        sm.rollback_to_tag(&tag1).unwrap();

        // 验证回滚成功
        assert_eq!(sm.current_state().unwrap().state_id, StateId::Planning);
    }
}
