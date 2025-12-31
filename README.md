# Claude Autonomous Engineer

**让 Claude Code 真正实现自主工程 - 纯 Rust 实现，单一二进制**

这是一个完整的 Claude Code 自主工程系统，将所有 hooks 和 agents 打包进单一的 Rust 二进制文件（仅 2MB）。通过智能的上下文注入、自动进度同步和代码审查，让 Claude 能够真正自主地完成复杂的工程任务。

## 🎯 这个工具解决什么问题？

### 传统 Claude Code 的痛点

```
你: "帮我实现用户认证系统"
Claude: [写了一些代码]
Claude: "完成了！"

你: "还有很多功能没做啊..."
Claude: "抱歉，我忘记了之前的计划" ❌
```

### 使用 Claude Autonomous Engineer

```
你: "帮我实现用户认证系统"

[系统自动注入上下文]
Claude: "我会先设计架构并生成任务列表..."
       [生成 ROADMAP.md - 20个任务]
       [生成 API 契约]

Claude: "现在开始 TASK-001: 实现用户注册..."
       [写代码 → 写测试 → 自动审查 → 提交]
       ✓ TASK-001 完成

Claude: "继续 TASK-002: 实现登录功能..."
       [自动继续下一个任务]
       ✓ TASK-002 完成

... [持续执行，直到所有任务完成] ✓
```

## ✨ 核心特性

### 🧠 智能上下文注入
每次交互前自动注入：
- **当前状态** (memory.json) - 正在做什么任务，当前进度
- **任务清单** (ROADMAP.md) - 还有哪些待完成
- **API 契约** (api_contract.yaml) - 函数签名和接口规范
- **错误历史** (error_history.json) - 避免重复失败
- **活跃文件** - 正在编辑的代码

### 🔄 自动进度同步
修改 Markdown 文件时自动更新状态：
```
你修改: ROADMAP.md
  - [x] TASK-001: 用户注册  ← 标记为完成

系统自动: memory.json 更新
  {
    "current_task": "TASK-002",
    "progress": { "tasks_completed": 1 }
  }
```

### 🛡️ Git Commit 前自动审查
```bash
git commit -m "实现用户注册"

[系统自动触发审查]
→ 检查是否符合 API 契约
→ 检查是否有测试
→ 检查代码质量

✓ 审查通过 → 允许提交
✗ 审查失败 → 阻止提交 + 反馈问题
```

### 🔁 自主循环控制
```
Claude: "这个任务完成了"

[loop_driver hook 自动检查]
→ ROADMAP 还有 pending 任务吗？
  - 有 → "继续下一个任务"
  - 没有 → "所有任务完成！"
```

## 📦 安装

### 方式 1: Cargo Install（推荐）

```bash
cargo install claude-autonomous
```

### 方式 2: DEB 包（Debian/Ubuntu）

```bash
wget https://github.com/hewenyu/claude-autonomous-engineer/releases/latest/download/claude-autonomous_1.0.2_amd64.deb
sudo dpkg -i claude-autonomous_1.0.2_amd64.deb
```

### 方式 3: RPM 包（Fedora/RHEL/CentOS）

```bash
wget https://github.com/hewenyu/claude-autonomous-engineer/releases/latest/download/claude-autonomous-1.0.2-1.x86_64.rpm
sudo rpm -i claude-autonomous-1.0.2-1.x86_64.rpm
```

### 验证安装

```bash
claude-autonomous --version
# claude-autonomous 1.0.2
```

## 🚀 快速开始

### 第一步：初始化项目

在你的项目根目录运行：

```bash
cd my-project
claude-autonomous init --name "My Awesome Project"
```

这会创建完整的目录结构：

```
my-project/
├── CLAUDE.md                          # 项目指令（告诉 Claude 如何工作）
└── .claude/
    ├── settings.json                  # Hook 配置
    ├── agents/                        # 5 个 agent 定义
    │   ├── project-architect-supervisor.md
    │   ├── code-executor.md
    │   ├── codex-reviewer.md
    │   ├── prd-generator.md
    │   └── visual-designer.md
    ├── status/                        # 状态管理（唯一真相来源）
    │   ├── memory.json                # 当前状态
    │   ├── ROADMAP.md                 # 任务清单（需手动创建或让 Claude 生成）
    │   ├── api_contract.yaml          # API 契约
    │   ├── error_history.json         # 错误历史
    │   └── decisions.log              # 决策日志
    └── phases/                        # 阶段详细计划
```

### 第二步：在 Claude Code 中开始工作

现在打开 Claude Code 并开始一个复杂任务：

```
你: "帮我实现一个完整的用户认证系统，包括注册、登录、密码重置、
    JWT token 管理、权限控制"
```

Claude 会：

1. **设计架构**（通过 project-architect-supervisor agent）
   - 生成 `ROADMAP.md` - 包含 15-20 个详细任务
   - 生成 `api_contract.yaml` - 定义所有函数签名
   - 生成阶段计划和任务规格

2. **开始执行**（通过 code-executor agent）
   - TASK-001: 实现用户模型
   - TASK-002: 实现注册 API
   - TASK-003: 添加密码加密
   - ... (自动继续)

3. **自动审查**（通过 codex-reviewer agent）
   - 每次 git commit 前自动触发
   - 检查代码质量、测试覆盖、API 契约一致性

4. **持续执行直到完成**
   - loop_driver 检查 ROADMAP
   - 还有 pending 任务 → 继续
   - 所有完成 → 停止并报告

### 第三步：查看进度

随时查看当前状态：

```bash
claude-autonomous status
```

输出：

```
╔══════════════════════════════════════════════════════════════════╗
║          Claude Autonomous Engineering Status                     ║
╚══════════════════════════════════════════════════════════════════╝

📁 Project Root: /home/user/my-project

🧠 Current State:
   Project: My Awesome Project
   Task: TASK-005
   Status: in_progress
   Retries: 0/5

📋 Progress:
   ✓ Completed: 4
   ▶ In Progress: 1
   ○ Pending: 10
   ! Blocked: 0
   Total: 15 (26.7%)

📍 Current Phase: Phase 1 - Core Authentication
```

### 第四步（可选）：生成 Repository Map（代码骨架）

Repository Map 会用 Tree-sitter 提取代码结构骨架（函数/结构体/impl 等），在上下文注入时显著减少 token 消耗，并降低“接口幻觉”风险。

```bash
# 默认输出（推荐）：.claude/repo_map/structure.toon
claude-autonomous map

# 输出 Markdown（更适合人读，但更长）
claude-autonomous map --format markdown

# 指定输出路径
claude-autonomous map --output .claude/repo_map/structure.md --format markdown
```

说明：
- `inject_state` 会优先读取 `.claude/repo_map/structure.toon`，不存在时再读取 `.claude/repo_map/structure.md`。
- `.claude/repo_map/` 默认已加入 `.gitignore`（建议不要提交生成物）。

### 第五步（可选）：Git 状态机（state）

状态机用于把“长周期开发阶段”显式化（planning/coding/testing/reviewing/completed/blocked），并提供历史查询与回滚。

```bash
# 查看当前状态
claude-autonomous state current

# 手动创建一次状态转换（会写入 .claude/status/state.json，并创建一条 git commit + tag）
claude-autonomous state transition planning --task-id TASK-001

# 列出/可视化状态历史
claude-autonomous state list
claude-autonomous state graph --task-id TASK-001

# 回滚到某个历史 tag（仅回滚 .claude/status/state.json）
claude-autonomous state rollback state-20251231-120000-planning-TASK-001
```

注意：
- 状态机是“显式启用”：只有当 `.claude/status/state.json` 存在时，`inject_state` 才会注入状态机上下文，`loop_driver` 才会尝试自动状态转换。
- 为避免污染用户提交，状态转换会在 index 存在 staged changes 时拒绝执行（请先 commit/unstage）。

## 📚 实际使用场景

### 场景 1: 从零开始构建新功能

```bash
# 在 Claude Code 中
你: "我想添加一个完整的博客系统，包括文章 CRUD、评论、
    标签、分类、搜索功能"

Claude: "我会先规划架构..."
[自动生成 ROADMAP.md - 30 个任务]

Claude: "开始 TASK-001: 设计数据库模型..."
[TDD 方式实现]
[自动审查]
[提交]

Claude: "TASK-001 完成，继续 TASK-002..."
[持续执行...]
```

### 场景 2: 重构现有代码

```bash
你: "重构现有的认证系统，改为使用 JWT 并添加刷新 token 机制"

Claude: "分析现有代码..."
[读取当前实现]
[生成重构计划]

Claude: "TASK-001: 添加 JWT 依赖和配置..."
Claude: "TASK-002: 实现 token 生成器..."
Claude: "TASK-003: 添加刷新 token 端点..."
[逐步完成所有任务]
```

### 场景 3: 修复 Bug 和添加测试

```bash
你: "用户报告登录时密码错误没有正确提示，帮我修复并添加完整测试"

Claude: "复现问题..."
[分析代码]

Claude: "TASK-001: 修复密码验证错误处理..."
Claude: "TASK-002: 添加错误场景测试..."
Claude: "TASK-003: 添加集成测试..."
[完成所有相关工作]
```

## 🔧 命令参考

### 初始化命令

| 命令 | 说明 | 示例 |
|------|------|------|
| `init` | 初始化项目 | `claude-autonomous init` |
| `init --name <name>` | 指定项目名称 | `claude-autonomous init --name "My API"` |
| `init --force` | 强制覆盖已有配置 | `claude-autonomous init --force` |

### 查看命令

| 命令 | 说明 | 输出 |
|------|------|------|
| `status` | 显示项目状态和进度 | 当前任务、完成度、阻塞项 |
| `agents` | 列出所有嵌入的 agents | 5 个 agent 名称列表 |
| `root` | 显示项目根目录路径 | `/path/to/project` |
| `doctor` | 诊断环境和配置 | 检查文件完整性、配置正确性 |

### Hook 命令（通常由 Claude Code 自动调用）

| 命令 | 触发时机 | 作用 |
|------|----------|------|
| `hook inject_state` | UserPromptSubmit | 注入上下文到 Claude |
| `hook progress_sync` | PostToolUse (Write/Edit) | 同步 Markdown 进度到 memory.json |
| `hook codex_review_gate` | PreToolUse (Bash - git commit) | Git commit 前审查代码 |
| `hook error_tracker` | PostToolUse (Bash) | 记录失败命令到 error_history.json，并递增 retry_count |
| `hook loop_driver` | Stop | 检查是否还有任务，决定是否继续 |

## 🏗️ 系统架构

### Hook 集成流程

```
┌─────────────────────────────────────────────────────────────────┐
│                       Claude Code Session                        │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  User Prompt → [inject_state] → Claude (with full context)      │
│                     ↓                                            │
│                  读取并注入:                                       │
│                  • memory.json                                   │
│                  • ROADMAP.md (pending 任务)                      │
│                  • TASK-xxx.md (当前任务规格)                      │
│                  • api_contract.yaml                             │
│                  • error_history.json                            │
│                                                                  │
│  Claude 输出 → [progress_sync] → 自动更新 memory.json            │
│                     ↓                                            │
│                  监听文件修改:                                     │
│                  • ROADMAP.md → 同步进度                          │
│                  • TASK-xxx.md → 同步当前任务                     │
│                                                                  │
│  git commit → [codex_review_gate] → 审查 → PASS/FAIL            │
│                     ↓                                            │
│                  自动审查:                                         │
│                  • API 契约一致性                                  │
│                  • 测试覆盖                                        │
│                  • 代码质量                                        │
│                                                                  │
│  Stop → [loop_driver] → 检查 ROADMAP → CONTINUE/DONE            │
│              ↓                                                   │
│           检查是否还有:                                            │
│           • [ ] pending 任务 → 阻止停止                           │
│           • 全部 [x] → 允许停止                                    │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### 核心组件

| 组件 | 语言 | 大小 | 作用 |
|------|------|------|------|
| **CLI 主程序** | Rust | 2MB | 命令行入口、Hook 执行 |
| **State Manager** | Rust | - | 解析 ROADMAP、memory.json |
| **Context Manager** | Rust | - | 智能上下文管理（token 预算控制） |
| **Project Finder** | Rust | - | Git-like 根目录查找（支持 submodule） |
| **Templates** | Embedded | - | 5 个 agents + CLAUDE.md 模板 |

### 为什么选择 Rust？

| 特性 | 说明 |
|------|------|
| **极速启动** | < 50ms - 几乎零感知延迟 |
| **超小体积** | 2MB 单文件 - 所有功能全包含 |
| **零依赖** | 无需任何运行时或库 |
| **低内存** | < 20MB - 轻量高效 |
| **一键部署** | 单一二进制，复制即用 |
| **高性能** | Hook 执行 < 30ms - 极致优化 |

## 🔍 智能根目录检测

CLI 支持复杂的项目结构，包括 git submodule：

```
my-project/                    ← 主项目
├── .claude/                   ← 配置在这里
├── backend/
│   └── api/
└── submodules/
    └── shared-lib/            ← git submodule
        └── deep/path/
```

无论你在哪里执行命令，都能找到正确的 `.claude` 目录：

```bash
# 在主项目
cd my-project/
claude-autonomous root
# → /home/user/my-project

# 在 submodule 深层目录
cd my-project/submodules/shared-lib/deep/path/
claude-autonomous root
# → /home/user/my-project (正确找到父项目！)
```

查找顺序：
1. Git superproject（优先 - 处理 submodule）
2. Git 仓库根目录
3. 当前目录
4. 向上遍历父目录

## 📋 settings.json 配置

初始化后生成的 `settings.json` 非常简洁：

```json
{
  "hooks": {
    "UserPromptSubmit": [{
      "matcher": "*",
      "hooks": [{
        "type": "command",
        "command": "claude-autonomous hook inject_state",
        "timeout": 5
      }]
    }],
    "PostToolUse": [
      {
        "matcher": "Write|Edit|Create",
        "hooks": [{
          "type": "command",
          "command": "claude-autonomous hook progress_sync",
          "timeout": 5
        }]
      },
      {
        "matcher": "Bash",
        "hooks": [{
          "type": "command",
          "command": "claude-autonomous hook error_tracker",
          "timeout": 5
        }]
      }
    ],
    "PreToolUse": [{
      "matcher": "Bash",
      "hooks": [{
        "type": "command",
        "command": "claude-autonomous hook codex_review_gate",
        "timeout": 180
      }]
    }],
    "Stop": [{
      "matcher": "*",
      "hooks": [{
        "type": "command",
        "command": "claude-autonomous hook loop_driver",
        "timeout": 5
      }]
    }]
  }
}
```

对比之前复杂的 bash 脚本（100+ 字符），现在只需要简单的命令调用。

## 🎓 内置 Agents

### 1. project-architect-supervisor
**职责**: 架构设计和任务规划
**输出**:
- `ROADMAP.md` - 完整任务列表
- `api_contract.yaml` - API 契约
- `PHASE_PLAN.md` - 阶段计划
- `TASK-xxx.md` - 任务规格

**触发词**: "设计架构"、"规划项目"、"生成任务列表"

### 2. code-executor
**职责**: TDD 方式实现代码
**工作流**:
1. 读取 `TASK-xxx.md` 需求
2. 读取 `api_contract.yaml` 签名
3. 写测试 → 验证失败 → 实现代码 → 验证通过
4. Lint 检查
5. Git commit（触发自动审查）

**触发词**: "实现"、"写代码"、"开发功能"

### 3. codex-reviewer
**职责**: 代码审查（Git commit 前自动触发）
**检查项**:
- API 契约一致性
- 测试覆盖率
- 代码质量（Lint、格式）
- 安全问题

**输出**: PASS（允许提交）或 FAIL（阻止 + 反馈问题）

### 4. prd-generator
**职责**: 从需求生成 PRD 文档
**触发词**: "写 PRD"、"需求文档"

### 5. visual-designer
**职责**: UI/UX 设计建议
**触发词**: "设计界面"、"UI 设计"

## ❓ 常见问题

### Q: 如何让 Claude 停止自主循环？

**A**: loop_driver hook 会自动检查 ROADMAP。如果想手动停止：

```markdown
# 在 ROADMAP.md 中标记所有任务为完成
- [x] TASK-001: ...
- [x] TASK-002: ...
```

或者删除/注释掉 Stop hook：

```json
// 临时禁用 loop_driver
{
  "hooks": {
    "Stop": []  // 空数组 = 不执行
  }
}
```

### Q: 任务卡住了怎么办？

**A**: 系统会自动检测重试次数：

```json
// memory.json
{
  "current_task": {
    "id": "TASK-005",
    "retry_count": 3,
    "max_retries": 5  // 超过 5 次会标记为 BLOCKED
  }
}
```

手动干预：

```markdown
# ROADMAP.md
- [!] TASK-005: 实现 OAuth (BLOCKED: 需要外部 API key)   # 阻塞：会阻止整体完成
- [-] TASK-007: 集成第三方支付 (SKIPPED: 暂不做)           # 跳过：不阻止整体完成
- [ ] TASK-006: 实现本地认证                               # 继续下一个
```

### Q: 如何自定义 agents？

**A**: 编辑 `.claude/agents/*.md` 文件：

```bash
# 修改 code-executor 的提示词
vim .claude/agents/code-executor.md
```

Agent 定义使用 Frontmatter + Markdown：

```markdown
---
name: my-custom-agent
description: "Custom agent for special tasks"
model: sonnet
color: purple
---

# My Custom Agent

[你的提示词...]
```

### Q: 能否在多个项目中共享配置？

**A**: 可以。创建一个模板项目：

```bash
# 创建模板
mkdir ~/claude-templates/
cd ~/claude-templates/
claude-autonomous init --name "Template"

# 自定义 agents 和 settings.json

# 在新项目中复制
cp -r ~/claude-templates/.claude ~/new-project/
cp ~/claude-templates/CLAUDE.md ~/new-project/
```

### Q: 支持哪些编程语言？

**A**: 语言无关！系统只管理状态和流程，agents 可以处理任何语言：

- ✅ Rust, Go, Python, TypeScript, Java, C++...
- ✅ Web (React, Vue, Next.js...)
- ✅ Mobile (Swift, Kotlin...)
- ✅ 任何有 TDD 支持的语言

### Q: 如何与现有 Git 工作流集成？

**A**: 完全兼容标准 Git 流程：

```bash
# 正常的 Git 操作
git checkout -b feature/new-auth
git add .
git commit -m "..."  # codex_review_gate 会自动触发
git push
gh pr create

# 系统只是在 commit 前添加了审查
```

要禁用审查（CI/CD 环境）：

```bash
# 环境变量禁用
SKIP_REVIEW=1 git commit -m "..."
```

### Q: 性能如何？会不会拖慢 Claude Code？

**A**: 极快！

| Hook | 执行时间 |
|------|----------|
| inject_state | < 50ms |
| progress_sync | < 20ms |
| codex_review_gate | < 30ms (不审查时) |
| loop_driver | < 10ms |

**总开销**: 每次交互约 50-100ms，几乎感觉不到。

## 🔧 高级用法

### 自定义上下文预算

编辑 `.claude/settings.json` 添加：

```json
{
  "context_budget": {
    "max_roadmap_tasks": 20,
    "max_error_history": 10,
    "max_active_files": 5
  }
}
```

### 错误历史管理

手动添加错误记录（避免重复失败）：

```json
// .claude/status/error_history.json
[
  {
    "timestamp": "2024-01-01T10:00:00Z",
    "task_id": "TASK-005",
    "error": "OAuth provider not configured",
    "attempted_solution": "Tried to use env vars",
    "resolution": "BLOCKED - needs manual config"
  }
]
```

### 决策日志

记录重要的架构决策：

```
// .claude/status/decisions.log
2024-01-01 10:00 [TASK-003] Chose JWT over sessions (stateless, better scaling)
2024-01-01 11:30 [TASK-007] Use bcrypt for passwords (industry standard)
```

## 🛠️ 开发和贡献

### 本地开发

```bash
git clone https://github.com/hewenyu/claude-autonomous-engineer.git
cd claude-autonomous-engineer

# 开发模式运行
cargo run -- init
cargo run -- status
cargo run -- hook inject_state < test_input.json

# 运行测试
cargo test --all

# 发布构建
cargo build --release
```

### 项目结构

```
src/
├── main.rs                    # CLI 入口
├── lib.rs                     # 库导出
├── cli/                       # 命令行处理
├── hooks/                     # 4 个 hook 实现
│   ├── inject_state.rs
│   ├── progress_sync.rs
│   ├── codex_review_gate.rs
│   └── loop_driver.rs
├── state/                     # 状态管理
│   ├── models.rs              # Memory, Task 数据结构
│   ├── parser.rs              # Markdown/YAML 解析
│   └── sync.rs                # 进度同步逻辑
├── context/                   # 上下文管理
│   ├── manager.rs             # 上下文组装
│   └── truncate.rs            # Token 预算控制
├── project/                   # 项目管理
│   ├── initializer.rs         # init 命令
│   └── root_finder.rs         # 根目录查找
├── templates/                 # 资源嵌入
│   ├── agents.rs              # Agent 模板
│   └── files.rs               # 配置模板
└── utils/                     # 工具函数
    ├── git.rs
    ├── fs.rs
    └── json.rs

embedded/                      # 嵌入资源
├── agents/                    # 5 个 agent 定义
└── templates/                 # 模板文件
```

### 打包发布

```bash
# 构建 DEB 包
cargo install cargo-deb
cargo deb

# 构建 RPM 包
cargo install cargo-rpm
cargo rpm build

# 发布到 crates.io
cargo publish
```

## 📄 License

MIT License - 详见 [LICENSE](LICENSE)

## 🙏 致谢

- [Claude Code](https://claude.com/claude-code) - Anthropic 的 AI 编程助手
- Rust 社区 - 优秀的工具和生态
- 所有贡献者

## 🔗 相关链接

- [GitHub 仓库](https://github.com/hewenyu/claude-autonomous-engineer)
- [Issues](https://github.com/hewenyu/claude-autonomous-engineer/issues)
- [Releases](https://github.com/hewenyu/claude-autonomous-engineer/releases)
- [Crates.io](https://crates.io/crates/claude-autonomous)

---

**开始自主工程之旅！** 🚀

```bash
cargo install claude-autonomous
cd your-project
claude-autonomous init
# 然后在 Claude Code 中说: "帮我实现完整的用户系统"
```
