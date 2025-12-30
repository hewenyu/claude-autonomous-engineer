# Claude Autonomous Engineering CLI

**纯 Rust 实现的自主工程工具 - 零 Python 依赖**

一个完整用 Rust 重写的 Claude Code 自主工程系统，所有 hooks 和 agents 都嵌入在 2MB 的单一二进制文件中。

## ✨ 特性

- ⚡ **零依赖** - 纯 Rust 实现，无需 Python 运行时
- 📦 **资源嵌入** - 5 个 agents 和所有模板编译进二进制
- 🚀 **极速启动** - 启动时间 < 50ms（vs Python 200ms+）
- 🔍 **智能根目录检测** - 完美支持 git submodule
- 🪝 **4 个内置 Hooks** - 状态注入、进度同步、代码审查、循环驱动
- 📊 **丰富状态显示** - 彩色终端输出，一目了然
- 🔧 **诊断工具** - `doctor` 命令检查环境配置
- 📦 **系统级安装** - 支持 deb/rpm 包和 cargo install

## 📥 安装

### 方式 1: Cargo Install（推荐）

```bash
cargo install claude-autonomous
```

### 方式 2: DEB 包（Debian/Ubuntu）

```bash
# 下载 .deb 包
wget https://github.com/hewenyu/claude-autonomous-engineer/releases/latest/download/claude-autonomous_1.0.0_amd64.deb

# 安装
sudo dpkg -i claude-autonomous_1.0.0_amd64.deb
```

### 方式 3: RPM 包（Fedora/RHEL/CentOS）

```bash
# 下载 .rpm 包
wget https://github.com/hewenyu/claude-autonomous-engineer/releases/latest/download/claude-autonomous-1.0.0-1.x86_64.rpm

# 安装
sudo rpm -i claude-autonomous-1.0.0-1.x86_64.rpm
```

### 方式 4: 从源码编译

```bash
git clone https://github.com/hewenyu/claude-autonomous-engineer.git
cd claude-autonomous-engineer
cargo build --release
sudo cp target/release/claude-autonomous /usr/local/bin/
```

### 验证安装

```bash
claude-autonomous --version
claude-autonomous agents  # 查看嵌入的 5 个 agents
```

## 🚀 快速开始

### 1. 初始化项目

```bash
cd your-project
claude-autonomous init --name "My Project"
```

这会创建完整的目录结构并安装所有资源：

```
.claude/
├── agents/                        # 5 个 agent 定义文件
│   ├── project-architect-supervisor.md
│   ├── codex-reviewer.md
│   ├── code-executor.md
│   ├── prd-generator.md
│   └── visual-designer.md
├── status/
│   ├── memory.json               # 状态管理
│   ├── ROADMAP.md                # 任务路线图（需手动创建）
│   ├── api_contract.yaml         # API 契约模板
│   ├── error_history.json        # 错误历史
│   └── decisions.log             # 决策日志
├── phases/                        # 阶段计划目录
└── settings.json                  # Hook 配置

CLAUDE.md                          # 项目指令（项目根目录）
```

### 2. 查看嵌入的 Agents

```bash
claude-autonomous agents
```

输出：
```
📦 Embedded Agents:

  • code-executor
  • codex-reviewer
  • prd-generator
  • project-architect-supervisor
  • visual-designer

✓ 5 embedded agents available
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

## 📚 命令参考

### 初始化

| 命令 | 说明 |
|------|------|
| `claude-autonomous init` | 初始化项目，创建 .claude 目录和所有资源 |
| `claude-autonomous init --name "Name"` | 指定项目名称初始化 |
| `claude-autonomous init --force` | 强制覆盖已有配置 |

### 信息查看

| 命令 | 说明 |
|------|------|
| `claude-autonomous status` | 显示项目状态和进度（解析 ROADMAP.md） |
| `claude-autonomous agents` | 列出所有嵌入的 agents |
| `claude-autonomous root` | 显示项目根目录路径 |
| `claude-autonomous doctor` | 诊断环境，检查配置文件完整性 |

### Hook 执行（由 Claude Code 调用）

| 命令 | 说明 |
|------|------|
| `claude-autonomous hook inject_state` | 注入当前状态到 Claude 上下文 |
| `claude-autonomous hook progress_sync` | 同步 ROADMAP.md 进度到 memory.json |
| `claude-autonomous hook codex_review_gate` | Git commit 前代码审查 |
| `claude-autonomous hook loop_driver` | 控制自主循环继续/停止 |

## 🔧 开发

### 本地开发

```bash
# 开发模式运行
cargo run -- init
cargo run -- hook inject_state
cargo run -- status
cargo run -- agents
cargo run -- doctor

# 测试
cargo test --all

# 发布构建（优化大小）
cargo build --release
```

### 打包

```bash
# 安装打包工具
cargo install cargo-deb
cargo install cargo-rpm

# 构建 DEB 包
cargo deb

# 构建 RPM 包
cargo rpm build

# 生成的包位于：
# - target/debian/claude-autonomous_1.0.0_amd64.deb
# - target/release/rpmbuild/RPMS/x86_64/claude-autonomous-1.0.0-1.x86_64.rpm
```

### 发布到 crates.io

```bash
# 登录
cargo login

# 发布（dry-run）
cargo publish --dry-run

# 正式发布
cargo publish
```

## 🏗️ 技术架构

- **语言**: 100% Rust（零 Python 依赖）
- **核心模块**:
  - `utils` - Git/JSON/文件系统工具
  - `state` - Markdown/YAML/JSON 解析和状态同步
  - `context` - 智能上下文管理（80K/40K/30K token 预算）
  - `hooks` - 4 个 hook 的纯 Rust 实现
  - `templates` - rust-embed 资源嵌入
  - `project` - 项目初始化和根目录查找

- **性能**:
  - 二进制大小: 2.0MB
  - 启动时间: < 50ms
  - Hook 执行: inject_state < 50ms, 其他 < 30ms
  - 内存占用: < 20MB

## 📄 License

MIT License - See [LICENSE](LICENSE) for details

## 🤝 贡献

欢迎提交 Issue 和 Pull Request！

## 🔗 相关资源

- [Claude Code Documentation](https://claude.com/claude-code)
- [Rust Programming Language](https://www.rust-lang.org/)
- [cargo-deb](https://github.com/kornelski/cargo-deb)
- [cargo-rpm](https://github.com/ruuda/cargo-rpm)
