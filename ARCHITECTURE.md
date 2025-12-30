# Claude Autonomous Engineering CLI - Architecture Design

## 🎯 目标

将 Python hooks (.claude/hooks + .claude/lib) 完全迁移到 Rust 二进制,实现:
1. ✅ 零 Python 依赖 - 纯 Rust 实现
2. ✅ 单二进制部署 - 可部署到 /usr/bin/
3. ✅ 自包含 - 内嵌所有模板 (agents, hooks, settings)
4. ✅ 向后兼容 - 保持 JSON 输出格式不变

## 📦 模块结构

```
src/
├── main.rs                 # CLI 入口点
├── lib.rs                  # 库根模块,导出公共接口
│
├── cli/                    # CLI 接口层
│   ├── mod.rs              # CLI 命令路由
│   ├── init.rs             # init 命令实现
│   ├── hook.rs             # hook 命令实现
│   └── status.rs           # status 命令实现
│
├── context/                # 上下文管理器 (移植 context_manager.py)
│   ├── mod.rs              # 上下文管理器核心
│   ├── types.rs            # 数据类型定义 (Memory, Task, etc.)
│   ├── memory.rs           # memory.json 读写
│   ├── roadmap.rs          # ROADMAP.md 解析
│   ├── contract.rs         # api_contract.yaml 处理
│   ├── errors.rs           # error_history.json 处理
│   ├── structure.rs        # 项目结构扫描
│   └── builder.rs          # 上下文构建器
│
├── hooks/                  # Hook 实现 (移植 .claude/hooks/*.py)
│   ├── mod.rs              # Hook 路由
│   ├── inject_state.rs     # UserPromptSubmit hook
│   ├── progress_sync.rs    # PostToolUse hook
│   ├── codex_review.rs     # PreToolUse hook
│   └── loop_driver.rs      # Stop hook
│
├── templates/              # 模板资源
│   ├── mod.rs              # 模板管理器
│   ├── agents.rs           # Agent markdown 模板 (嵌入)
│   ├── settings.rs         # settings.json 生成
│   └── files.rs            # 其他文件模板
│
└── utils/                  # 工具函数
    ├── mod.rs
    ├── project_root.rs     # 项目根查找逻辑
    ├── git.rs              # Git 操作封装
    └── format.rs           # 文本格式化工具
```

## 🔧 核心数据结构

### Memory (memory.json)

```rust
#[derive(Debug, Serialize, Deserialize)]
struct Memory {
    project: String,
    version: String,
    mode: String,
    current_phase: Option<String>,
    current_task: Option<TaskInfo>,
    progress: Progress,
    next_action: NextAction,
    error_history: Vec<ErrorRecord>,
    decisions_log: Vec<String>,
    active_files: Vec<String>,
    working_context: WorkingContext,
}

#[derive(Debug, Serialize, Deserialize)]
struct TaskInfo {
    id: String,
    name: String,
    status: String,
    retry_count: u32,
    max_retries: u32,
}

#[derive(Debug, Serialize, Deserialize)]
struct Progress {
    tasks_total: u32,
    tasks_completed: u32,
    current_phase: Option<String>,
    completed: Vec<String>,
    in_progress: Vec<String>,
    blocked: Vec<String>,
    pending: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct NextAction {
    action: String,
    target: String,
    reason: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct WorkingContext {
    current_file: Option<String>,
    current_function: Option<String>,
    pending_tests: Vec<String>,
    pending_implementations: Vec<String>,
}
```

### Roadmap (ROADMAP.md)

```rust
#[derive(Debug)]
struct Roadmap {
    raw_content: String,
    pending: Vec<Task>,
    in_progress: Vec<Task>,
    completed: Vec<Task>,
    blocked: Vec<Task>,
}

#[derive(Debug)]
struct Task {
    raw_line: String,
    status: TaskStatus,
    content: String,
}

#[derive(Debug)]
enum TaskStatus {
    Pending,      // - [ ]
    InProgress,   // - [>]
    Completed,    // - [x]
    Blocked,      // - [!]
}
```

### Context Builder

```rust
pub struct ContextBuilder {
    project_root: PathBuf,
    mode: ContextMode,
    memory: Option<Memory>,
    roadmap: Option<Roadmap>,
    contract: Option<String>,
    // ... 其他字段
}

pub enum ContextMode {
    Autonomous,   // 完整上下文 (inject_state)
    Review,       // 代码审查上下文 (codex_review)
    Task,         // 任务执行上下文
}

impl ContextBuilder {
    pub fn new(project_root: PathBuf) -> Self;
    pub fn mode(mut self, mode: ContextMode) -> Self;
    pub fn with_memory(mut self) -> Result<Self>;
    pub fn with_roadmap(mut self) -> Result<Self>;
    pub fn with_contract(mut self) -> Result<Self>;
    pub fn with_errors(mut self, task_filter: Option<&str>) -> Result<Self>;
    pub fn with_active_files(mut self, max_files: usize) -> Result<Self>;
    pub fn build(self) -> Result<String>;
}
```

## 🎯 Hook 实现细节

### 1. inject_state (UserPromptSubmit)

**输入**: 用户提交的 prompt (通过 stdin)
**输出**: JSON with `hookSpecificOutput.additionalContext`

```rust
pub fn inject_state(project_root: &Path) -> Result<serde_json::Value> {
    let context = ContextBuilder::new(project_root.to_path_buf())
        .mode(ContextMode::Autonomous)
        .with_memory()?
        .with_roadmap()?
        .with_contract()?
        .with_errors(None)?
        .with_active_files(5)?
        .with_structure()?
        .build()?;

    Ok(json!({
        "hookSpecificOutput": {
            "additionalContext": context
        }
    }))
}
```

### 2. progress_sync (PostToolUse)

**输入**: Tool use 信息 (Write/Edit)
**输出**: JSON with `status`
**逻辑**:
- 检测 ROADMAP.md / TASK-xxx.md 的修改
- 解析任务状态变化
- 自动同步到 memory.json

```rust
pub fn progress_sync(input: &HookInput) -> Result<serde_json::Value> {
    // 检测修改的文件
    let modified_file = input.get_modified_file();

    if modified_file.ends_with("ROADMAP.md") {
        sync_roadmap_to_memory()?;
    } else if modified_file.contains("TASK-") {
        sync_task_to_memory(modified_file)?;
    }

    Ok(json!({"status": "ok"}))
}
```

### 3. codex_review_gate (PreToolUse)

**输入**: Bash 命令
**输出**: JSON with `decision` (allow/block)
**逻辑**:
- 拦截 `git commit` / `git push`
- 调用 Codex API 审查变更
- 基于 API contract + task spec 验证

```rust
pub fn codex_review_gate(input: &HookInput) -> Result<serde_json::Value> {
    let command = input.get_command();

    // 只拦截 git commit/push
    if !command.contains("git commit") && !command.contains("git push") {
        return Ok(json!({"decision": "allow"}));
    }

    // TODO: 实现 Codex 审查逻辑
    // 1. 获取 staged files
    // 2. 构建审查上下文
    // 3. 调用 Codex API
    // 4. 解析结果 -> allow/block

    Ok(json!({"decision": "allow"}))
}
```

### 4. loop_driver (Stop)

**输入**: Stop 请求
**输出**: JSON with `decision` (allow/block)
**逻辑**:
- 检查 ROADMAP 是否还有 pending tasks
- 如果有,阻止停止

```rust
pub fn loop_driver(project_root: &Path) -> Result<serde_json::Value> {
    let roadmap = Roadmap::load(project_root)?;

    if !roadmap.pending.is_empty() {
        return Ok(json!({
            "decision": "block",
            "reason": format!("[Loop] {} tasks remaining. Continue working!", roadmap.pending.len())
        }));
    }

    Ok(json!({"decision": "allow"}))
}
```

## 📝 模板嵌入策略

使用 `include_str!` 宏将模板文件嵌入到二进制中:

```rust
// src/templates/agents.rs
pub const PROJECT_ARCHITECT: &str = include_str!("../../templates/agents/project-architect-supervisor.md");
pub const CODE_EXECUTOR: &str = include_str!("../../templates/agents/code-executor.md");
pub const CODEX_REVIEWER: &str = include_str!("../../templates/agents/codex-reviewer.md");
pub const PRD_GENERATOR: &str = include_str!("../../templates/agents/prd-generator.md");
pub const VISUAL_DESIGNER: &str = include_str!("../../templates/agents/visual-designer.md");

pub fn write_all_agents(agents_dir: &Path) -> Result<()> {
    fs::write(agents_dir.join("project-architect-supervisor.md"), PROJECT_ARCHITECT)?;
    fs::write(agents_dir.join("code-executor.md"), CODE_EXECUTOR)?;
    fs::write(agents_dir.join("codex-reviewer.md"), CODEX_REVIEWER)?;
    fs::write(agents_dir.join("prd-generator.md"), PRD_GENERATOR)?;
    fs::write(agents_dir.join("visual-designer.md"), VISUAL_DESIGNER)?;
    Ok(())
}
```

## 🛠️ 依赖项

```toml
[dependencies]
clap = { version = "4.4", features = ["derive"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
serde_yaml = "0.9"
anyhow = "1.0"
colored = "2.0"
dirs = "5.0"
walkdir = "2.5"           # 文件遍历
regex = "1.10"            # 正则表达式
once_cell = "1.19"        # 懒加载静态变量
chrono = "0.4"            # 时间处理
```

## 🚀 部署流程

### 编译

```bash
cargo build --release
```

### 安装到 /usr/bin

```bash
# install.sh
#!/bin/bash
set -e

echo "📦 Building claude-autonomous..."
cargo build --release

echo "📋 Installing to /usr/local/bin..."
sudo cp target/release/claude-autonomous /usr/local/bin/

echo "✅ Installation complete!"
echo "Run: claude-autonomous --version"
```

### 使用

```bash
# 初始化项目
cd /path/to/project
claude-autonomous init --name my-project

# 运行 hooks (由 Claude Code 自动调用)
claude-autonomous hook inject_state
claude-autonomous hook progress_sync
claude-autonomous hook codex_review_gate
claude-autonomous hook loop_driver

# 查看状态
claude-autonomous status

# 查看项目根
claude-autonomous root
```

## ✅ 迁移清单

- [ ] 创建模块目录结构
- [ ] 实现 context/types.rs (数据结构)
- [ ] 实现 context/memory.rs (memory.json 读写)
- [ ] 实现 context/roadmap.rs (ROADMAP.md 解析)
- [ ] 实现 context/builder.rs (上下文构建器)
- [ ] 实现 hooks/inject_state.rs
- [ ] 实现 hooks/progress_sync.rs
- [ ] 实现 hooks/codex_review.rs
- [ ] 实现 hooks/loop_driver.rs
- [ ] 从 .claude/agents/ 复制模板文件到 templates/agents/
- [ ] 实现 templates/agents.rs (模板嵌入)
- [ ] 更新 init 命令支持 agents 初始化
- [ ] 更新 Cargo.toml 依赖
- [ ] 创建 install.sh 脚本
- [ ] 测试完整流程
- [ ] 移除 Python 脚本依赖

## 🎯 实现优先级

1. **Phase 1** (核心功能):
   - ✅ 数据结构定义
   - ✅ memory.json 读写
   - ✅ ROADMAP 解析
   - ✅ inject_state hook
   - ✅ loop_driver hook

2. **Phase 2** (进度追踪):
   - progress_sync hook
   - TASK-xxx.md 解析
   - 自动同步逻辑

3. **Phase 3** (代码审查):
   - codex_review_gate hook
   - Git diff 提取
   - Codex API 集成

4. **Phase 4** (完善):
   - 模板嵌入
   - 安装脚本
   - 文档更新
