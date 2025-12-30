# 🚀 Claude Autonomous Engineering System v3.0

## 完整改进方案

基于你的原始方案，我做了以下关键改进：

### 核心改进对比

| 问题 | 原始方案 | 改进方案 |
|------|---------|---------|
| 上下文注入量 | ~500 字符 | ~14,000 token（分层注入）|
| Codex 审查 | 无上下文 | 自动注入 API 契约 + 任务规格 |
| 进度同步 | 手动更新 | 自动同步（Hook 检测 MD 文件修改）|
| Agent 信息复用 | 各自独立 | 统一上下文管理器 |
| 错误记忆 | 无 | 完整错误历史 + 自动避免重复 |

---

## 📁 系统架构

```
.claude/
├── CLAUDE.md                     # 宪法级协议
├── settings.json                 # Hook 配置
│
├── lib/
│   └── context_manager.py        # 🆕 统一上下文管理器
│                                  #    所有 Agent 和 Hook 的上下文来源
│
├── hooks/
│   ├── inject_state.py           # 每次交互注入完整上下文
│   ├── progress_sync.py          # 🆕 MD 文件修改后自动同步进度
│   ├── codex_review_gate.py      # 🆕 提交前自动审查（带完整上下文）
│   └── loop_driver.py            # 控制自主循环
│
├── agents/
│   ├── project-architect-supervisor.md  # 🆕 输出格式与系统对接
│   ├── code-executor.md                 # 🆕 读取注入的上下文
│   └── codex-reviewer.md                # 🆕 审查上下文说明
│
├── status/                       # 状态文件
│   ├── ROADMAP.md                # 任务列表（Hook 自动解析）
│   ├── api_contract.yaml         # API 契约（审查时自动注入）
│   ├── memory.json               # 当前状态（自动同步）
│   ├── error_history.json        # 🆕 错误历史
│   └── decisions.log             # 🆕 决策日志
│
└── phases/                       # 阶段详情
    └── phase-N_xxx/
        ├── PHASE_PLAN.md
        └── TASK-NNN_xxx.md       # 任务规格（自动注入给 executor）
```

---

## 🔗 关键改进详解

### 1. 统一上下文管理器 (`context_manager.py`)

**问题**：原始方案中各个 Hook 和 Agent 各自读取文件，没有统一管理。

**改进**：创建统一的上下文管理器，提供不同场景的上下文组装：

```python
from context_manager import ContextManager
ctx = ContextManager()

# 完整上下文（用于 UserPromptSubmit）
full_context = ctx.get_full_context()

# 审查上下文（用于 Codex Review）
review_context = ctx.get_review_context(changed_files)

# 任务上下文（用于 code-executor）
task_context = ctx.get_task_context(task_id)
```

### 2. 自动进度同步 (`progress_sync.py`)

**问题**：原始方案需要 Claude "记得"更新 memory.json。

**改进**：PostToolUse Hook 自动检测 MD 文件修改并同步：

```
修改 ROADMAP.md → 自动解析任务状态 → 更新 memory.json.progress
修改 TASK-xxx.md → 自动提取任务状态 → 更新 memory.json.current_task
修改 PHASE_PLAN.md → 自动提取阶段信息 → 更新 memory.json.current_phase
```

### 3. Codex 审查上下文 (`codex_review_gate.py`)

**问题**：Codex 审查时没有 API 契约和任务规格，无法有效验证。

**改进**：提交前自动注入完整上下文：

```
git commit 触发 Hook
        ↓
获取 staged files
        ↓
调用 context_manager.get_review_context()
        ↓
注入：
├── API 契约（验证签名）
├── 任务规格（验证需求）
├── 变更文件内容
├── Git diff
└── 错误历史（检查模式）
        ↓
Codex CLI 审查
        ↓
PASS → 允许提交
FAIL → 阻止提交，反馈给 Claude
```

### 4. Agent 信息流

**问题**：architect 生成的文件没有被后续流程自动消费。

**改进**：建立清晰的信息流：

```
project-architect-supervisor
        │
        ├─── ROADMAP.md ─────────┐
        ├─── api_contract.yaml ──┼─► progress_sync.py ─► memory.json
        └─── TASK-xxx.md ────────┘         │
                                           │
        ┌──────────────────────────────────┘
        ▼
code-executor 自动收到：
├── memory.json (当前任务)
├── TASK-xxx.md (任务规格)
├── api_contract.yaml (签名)
└── error_history.json (避免重复)
        │
        ├─── 写代码
        └─── git commit
                │
                ▼
codex_review_gate.py 自动注入：
├── api_contract.yaml (验证签名)
├── TASK-xxx.md (验证需求)
├── changed files (审查内容)
└── error_history (检查模式)
```

---

## 🚀 使用指南

### 快速开始

```bash
# 1. 复制 .claude 目录到你的项目
cp -r improved-system-v2/.claude /path/to/your/project/

# 2. 初始化
cd /path/to/your/project
chmod +x .claude/init.sh
./.claude/init.sh "my-project-name"

# 3. 启动 Claude Code
# 说: "Plan the project: [描述你的项目]"
```

### 初始化后的流程

1. **Claude 调用 project-architect-supervisor**
   - 生成 ROADMAP.md（任务列表）
   - 生成 api_contract.yaml（API 契约）
   - 生成 TASK-xxx.md（任务规格）

2. **确认后进入自主循环**
   - inject_state.py 自动注入上下文
   - code-executor 执行任务
   - progress_sync.py 自动同步进度
   - codex_review_gate.py 自动审查
   - loop_driver.py 控制循环

3. **循环直到完成**
   - 每次交互都有完整上下文
   - 每次文件修改都自动同步
   - 每次提交都自动审查
   - 任务完成自动继续下一个

---

## 📊 上下文注入详情

### 注入内容（~14,000 token）

```
╔══════════════════════════════════════════════════════════════════╗
║  Layer 0: 系统头部 (~200 token)                                   ║
║  - 警告信息：不要相信记忆，信任文件                               ║
║  - 模式指示：AUTONOMOUS / REVIEW / TASK                          ║
╠══════════════════════════════════════════════════════════════════╣
║  Layer 1: 当前状态 (~500 token)                                   ║
║  - current_task (ID, 名称, 状态, 重试次数)                        ║
║  - working_context (当前文件, 待处理测试)                         ║
║  - next_action (下一步行动)                                       ║
╠══════════════════════════════════════════════════════════════════╣
║  Layer 2: 任务列表 (~1,000 token)                                 ║
║  - 总体进度 (完成/总数)                                           ║
║  - 待处理任务列表                                                 ║
║  - 当前阶段                                                       ║
╠══════════════════════════════════════════════════════════════════╣
║  Layer 3: 当前任务规格 (~1,000 token)                             ║
║  - TASK-xxx.md 完整内容                                           ║
║  - 需求、文件列表、验收标准                                       ║
╠══════════════════════════════════════════════════════════════════╣
║  Layer 4: API 契约 (~3,000 token)                                 ║
║  - 函数签名、参数类型、返回类型                                    ║
║  - 异常规范                                                       ║
╠══════════════════════════════════════════════════════════════════╣
║  Layer 5: 错误历史 (~1,000 token)                                 ║
║  - 未解决的错误（避免重复！）                                      ║
║  - 已解决的错误（学习经验）                                        ║
╠══════════════════════════════════════════════════════════════════╣
║  Layer 6: 活跃文件 (~5,000 token)                                 ║
║  - 当前正在编辑的文件内容                                          ║
║  - 最近修改的文件                                                  ║
╠══════════════════════════════════════════════════════════════════╣
║  Layer 7: 项目结构 (~2,000 token)                                 ║
║  - 目录树                                                         ║
║  - 关键函数签名                                                    ║
╚══════════════════════════════════════════════════════════════════╝
```

### 不同场景的上下文

| 场景 | 上下文类型 | 大小 |
|------|-----------|------|
| 每次交互 | `get_full_context()` | ~14K token |
| 代码审查 | `get_review_context()` | ~8K token |
| 单任务执行 | `get_task_context()` | ~6K token |

---

## ⚙️ 配置说明

### settings.json

```json
{
  "hooks": {
    "UserPromptSubmit": [
      {
        "matcher": "*",
        "hooks": [{
          "type": "command",
          "command": "python3 .claude/hooks/inject_state.py"
        }]
      }
    ],
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [{
          "type": "command",
          "command": "python3 .claude/hooks/codex_review_gate.py"
        }]
      }
    ],
    "PostToolUse": [
      {
        "matcher": "Write|Edit|Create",
        "hooks": [{
          "type": "command",
          "command": "python3 .claude/hooks/progress_sync.py"
        }]
      }
    ],
    "Stop": [
      {
        "matcher": "*",
        "hooks": [{
          "type": "command",
          "command": "python3 .claude/hooks/loop_driver.py"
        }]
      }
    ]
  }
}
```

### 环境变量

```bash
# 审查严格程度
export REVIEW_MODE=strict|normal|lenient

# Codex CLI 路径（如果不在 PATH 中）
export CODEX_CMD=/path/to/codex
```

---

## 🔧 故障排除

### Hook 不工作

```bash
# 检查 Python
python3 --version

# 测试 inject_state
echo '{}' | python3 .claude/hooks/inject_state.py

# 检查 settings.json 语法
python3 -c "import json; json.load(open('.claude/settings.json'))"
```

### 上下文没有注入

1. 检查 `.claude/lib/context_manager.py` 是否存在
2. 检查 `.claude/status/` 目录下的文件是否存在
3. 查看 Claude Code 日志

### Codex 审查不触发

1. 确认是 `git commit` 命令
2. 确认有 staged files (`git status`)
3. 检查 Codex CLI 是否可用 (`codex --version`)

---

## 📈 效果预期

| 指标 | 原始方案 | 改进后 |
|------|---------|-------|
| 上下文丢失 | 频繁 | 极少（自动注入）|
| 重复错误 | 常见 | 自动避免 |
| 进度追踪 | 手动 | 自动同步 |
| 代码质量 | 依赖记忆 | 自动审查 |
| 任务连续性 | 容易中断 | 自动恢复 |

---

## License

MIT
