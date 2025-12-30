# Claude Autonomous Engineering CLI

> **Zero Python Dependencies** | **Single Binary Deployment** | **Self-Contained**

用 Rust 编写的统一命令行工具，完全重写了 Claude Code 的自主工程系统。

## ✨ 特性

- ✅ **零 Python 依赖** - 纯 Rust 实现，无需安装 Python
- ✅ **单二进制部署** - 可部署到 `/usr/bin/` 或 `/usr/local/bin/`
- ✅ **内嵌模板** - Agent 配置内置于二进制中
- 🔍 **自动检测项目根目录** - 支持 git submodule 场景
- 🚀 **一键初始化** - `claude-autonomous init`
- 🪝 **统一 Hook 运行** - `claude-autonomous hook <name>`
- 📊 **状态查看** - `claude-autonomous status`
- 🔄 **自动上下文注入** - 完整的状态管理
- 🛑 **循环控制** - 防止任务未完成时停止

## 📦 安装

### 快速安装

```bash
git clone https://github.com/your-username/claude-autonomous-engineer.git
cd claude-autonomous-engineer
./install.sh
```

### 手动安装

```bash
# 编译
cargo build --release

# 安装到系统
sudo cp target/release/claude-autonomous /usr/local/bin/
sudo chmod +x /usr/local/bin/claude-autonomous

# 验证
claude-autonomous --version
```

### 自定义安装位置

```bash
# 安装到用户目录
INSTALL_DIR=$HOME/.local/bin ./install.sh

# 添加到 PATH (添加到 ~/.bashrc 或 ~/.zshrc)
export PATH="$HOME/.local/bin:$PATH"
```

## 使用

### 初始化项目

```bash
cd your-project
claude-autonomous init --name "My Project"
```

这会创建：
```
.claude/
├── settings.json      # Hook 配置（使用 Rust CLI 命令）
├── CLAUDE.md          # 项目规则
├── agents/            # Agent 模板（从二进制嵌入）
│   ├── project-architect-supervisor.md
│   ├── code-executor.md
│   ├── codex-reviewer.md
│   ├── prd-generator.md
│   └── visual-designer.md
├── hooks/             # 兼容 Python (可选)
├── lib/               # 兼容 Python (可选)
├── status/
│   └── memory.json    # 当前状态
└── phases/            # 任务组织
```

**注意**: 新版本不再需要 Python hooks，所有功能都在 Rust 二进制中实现。

### 生成的 settings.json

使用 CLI 后，`settings.json` 变得非常简洁：

```json
{
  "hooks": {
    "UserPromptSubmit": [{
      "matcher": "*",
      "hooks": [{
        "type": "command",
        "command": "claude-autonomous hook inject_state"
      }]
    }],
    "PreToolUse": [{
      "matcher": "Bash",
      "hooks": [{
        "type": "command",
        "command": "claude-autonomous hook codex_review_gate"
      }]
    }],
    "PostToolUse": [{
      "matcher": "Write|Edit|Create",
      "hooks": [{
        "type": "command",
        "command": "claude-autonomous hook progress_sync"
      }]
    }],
    "Stop": [{
      "matcher": "*",
      "hooks": [{
        "type": "command",
        "command": "claude-autonomous hook loop_driver"
      }]
    }]
  }
}
```

对比之前的复杂 bash 命令：
```json
"command": "bash -c 'R=$(git rev-parse --show-toplevel 2>/dev/null || pwd); while [ ! -d \"$R/.claude\" ] && [ \"$R\" != \"/\" ]; do R=$(dirname \"$R\"); done; S=$(git rev-parse --show-superproject-working-tree 2>/dev/null); [ -n \"$S\" ] && [ -d \"$S/.claude\" ] && R=\"$S\"; if [ -f \"$R/.claude/hooks/inject_state.py\" ]; then cd \"$R\" && python3 .claude/hooks/inject_state.py; else echo \"{\\\"hookSpecificOutput\\\":{\\\"additionalContext\\\":\\\"\\\"}}\"; fi'"
```

### 查看状态

```bash
claude-autonomous status
```

输出：
```
╔══════════════════════════════════════════════════════════════════╗
║              Claude Autonomous Engineering Status                 ║
╚══════════════════════════════════════════════════════════════════╝

📁 Project Root: /home/user/my-project

🧠 Current State:
   Project: My Project
   Phase: Phase 1 - Core
   Task: TASK-001

📋 Progress:
   ✓ Completed: 5
   ▶ In Progress: 1
   ○ Pending: 10
```

### 查看项目根目录

```bash
# 即使在 submodule 中也能找到正确的根目录
cd my-project/submodule
claude-autonomous root
# 输出: /home/user/my-project
```

## Submodule 支持

CLI 会按以下顺序查找 `.claude` 目录：

1. **git superproject** - 优先检查父项目（处理 submodule）
2. **当前目录**
3. **git 仓库根目录**
4. **向上遍历父目录**

这意味着：
- 在 `my-project/` 中执行 → 找到 `my-project/.claude`
- 在 `my-project/submodule/` 中执行 → 找到 `my-project/.claude`
- 在 `my-project/submodule/deep/path/` 中执行 → 找到 `my-project/.claude`

## 命令参考

| 命令 | 说明 |
|------|------|
| `claude-autonomous init` | 初始化 .claude 目录 |
| `claude-autonomous init --name "Name"` | 指定项目名称初始化 |
| `claude-autonomous init --force` | 强制覆盖已有配置 |
| `claude-autonomous hook <name>` | 运行指定的 hook |
| `claude-autonomous root` | 显示项目根目录 |
| `claude-autonomous status` | 显示当前状态 |
| `claude-autonomous gen-settings` | 生成 settings.json |

## 🛠️ 架构

### 模块结构

```
src/
├── main.rs                 # CLI 入口点
├── lib.rs                  # 库根模块
├── context/                # 上下文管理 (移植自 context_manager.py)
│   ├── types.rs            # 数据结构
│   ├── memory.rs           # memory.json 处理
│   ├── roadmap.rs          # ROADMAP.md 解析
│   ├── builder.rs          # 上下文构建器
│   ├── contract.rs         # API 契约处理
│   ├── errors.rs           # 错误历史
│   └── structure.rs        # 项目扫描
├── hooks/                  # Hook 实现 (移植自 .claude/hooks/*.py)
│   ├── inject_state.rs     # UserPromptSubmit
│   ├── loop_driver.rs      # Stop
│   ├── progress_sync.rs    # PostToolUse
│   └── codex_review.rs     # PreToolUse
├── templates/              # 嵌入的模板
│   └── agents.rs           # Agent markdown 文件
└── utils/                  # 工具函数
    ├── project_root.rs     # 项目根查找
    └── format.rs           # 文本格式化
```

### Hook 流程

```
UserPromptSubmit → inject_state
   ↓
   注入: memory.json + ROADMAP + API contract + errors

PostToolUse (Write/Edit) → progress_sync
   ↓
   同步: ROADMAP 变化 → memory.json

PreToolUse (Bash) → codex_review_gate
   ↓
   审查: git commit/push (TODO: 集成 Codex API)

Stop → loop_driver
   ↓
   阻止如果: ROADMAP 有待处理任务
```

详细架构文档请查看 [ARCHITECTURE.md](./ARCHITECTURE.md)

## 🔧 开发

```bash
# 开发模式运行
cargo run -- init
cargo run -- hook inject_state
cargo run -- status

# 测试
cargo test

# 发布构建
cargo build --release

# Lint
cargo clippy

# 格式化
cargo fmt
```

## 🎯 从 Python 版本迁移

如果你正在使用 Python 版本:

1. **备份 `.claude` 目录**
2. 运行 `claude-autonomous init --force` (保留 settings.json)
3. 更新 `.claude/settings.json` 使用 `claude-autonomous hook <name>` 而非 Python 脚本
4. 可选: 删除 Python 依赖和 hooks/*.py 文件

### Settings 对比

**之前 (Python):**
```json
{
  "command": "bash -c 'python3 .claude/hooks/inject_state.py'"
}
```

**现在 (Rust):**
```json
{
  "command": "claude-autonomous hook inject_state"
}
```

## 📊 性能对比

| 指标 | Python 版本 | Rust 版本 |
|------|------------|----------|
| 二进制大小 | N/A (需要 Python) | ~3MB |
| 启动时间 | ~100-200ms | ~5-10ms |
| 内存占用 | ~30-50MB | ~2-5MB |
| 依赖 | Python 3.x + 库 | 无 |

## 📝 License

MIT

## 🙏 致谢

- 使用 [Rust](https://www.rust-lang.org/) 构建
- 由 [Claude Code](https://claude.com/claude-code) 驱动
- 原始概念来自 Autonomous Engineering System

---

**用 ❤️ 和 Claude 构建**
