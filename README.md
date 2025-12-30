# Claude Autonomous Engineering CLI

用 Rust 编写的统一命令行工具，简化 Claude Code 的自主工程系统。

## 特性

- 🔍 **自动检测项目根目录** - 支持 git submodule 场景
- 🚀 **一键初始化** - `claude-autonomous init`
- 🪝 **统一 Hook 运行** - `claude-autonomous hook <name>`
- 📊 **状态查看** - `claude-autonomous status`

## 安装

```bash
# 编译
cargo build --release

# 安装到系统
sudo cp target/release/claude-autonomous /usr/local/bin/
sudo chmod +x /usr/local/bin/claude-autonomous

# 验证
claude-autonomous --help
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
├── settings.json      # Hook 配置（使用 CLI 命令）
├── CLAUDE.md          # 项目规则
├── hooks/
│   ├── inject_state.py
│   ├── codex_review_gate.py
│   ├── progress_sync.py
│   └── loop_driver.py
└── status/
    └── memory.json
```

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

## 开发

```bash
# 开发模式运行
cargo run -- init
cargo run -- hook inject_state
cargo run -- status

# 测试
cargo test

# 发布构建
cargo build --release
```

## License

MIT
