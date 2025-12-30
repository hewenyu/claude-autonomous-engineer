# 构建弹性自主软件工程系统（Claude Code 长周期自动化 · 状态管理 · 自我纠正）

本仓库是一份面向“无人值守、长周期（10h+）自动化软件工程”的架构蓝图与最小实现骨架：用 **Claude Code 的 Agent + Hooks** 搭建确定性的“控制平面”，用 **文件系统状态** 解决上下文压缩带来的记忆丢失，并用 **Codex CLI**（可选）作为强制质量门禁，形成“实现→验证→审查→纠错”的闭环。

---

## 目录

- [1. 绪论：从辅助编程到自主工程](#1-绪论从辅助编程到自主工程)
- [2. 长周期任务的系统性挑战](#2-长周期任务的系统性挑战)
- [3. 总体架构：双层控制模型](#3-总体架构双层控制模型)
- [4. 治理层：`.claude/CLAUDE.md` 作为“宪法”](#4-治理层claudeclaudemd-作为宪法)
- [5. 状态持久化：用文件系统做“外部海马体”](#5-状态持久化用文件系统做外部海马体)
- [6. Hook 控制平面：事前约束 + 事后审计 + 防止提前停止](#6-hook-控制平面事前约束--事后审计--防止提前停止)
- [7. 专用智能体网络（Agent Swarm）](#7-专用智能体网络agent-swarm)
- [8. 自动化反馈闭环与自愈策略](#8-自动化反馈闭环与自愈策略)
- [9. 实施路径（从 0 到 10h+ Loop）](#9-实施路径从-0-到-10h-loop)
- [10. 交付物清单](#10-交付物清单)

---

## 1. 绪论：从辅助编程到自主工程

LLM 赋能的软件开发正在从“交互式辅助（Interactive Copilot）”迈向“自主智能体（Autonomous Agent）”。在复杂工程任务上，用户常见痛点（执行遗漏、功能不完整、API 实现不一致、长周期运行的上下文丢失）并非偶发瑕疵，而是 **长时序、多步骤任务中的系统性熵增**：模型是概率系统，越长的链路越容易漂移；上下文越长越可能被压缩；无人值守时偏差会累积成灾难。

本仓库的核心思想是：**不信任模型的短期记忆，把“可信”转移到文件系统状态与 Hook 脚本（确定性控制）上**。

---

## 2. 长周期任务的系统性挑战

### 2.1 上下文压缩导致“记忆重置”

Claude Code（及同类工具）在上下文接近上限时会进行自动压缩（Auto-Compact）。摘要对人类“回忆”够用，但对自主系统是有损转换：关键约束（禁改文件、必须遵循契约、验收标准）可能被压掉，导致系统退化为“无宪法状态”。

### 2.2 概率性执行偏差的累积

长周期里最常见的失败模式之一是 **接口/实现不一致**：早期定义了 `getUserData(id)`，后期调用变成 `fetchUserInfo(userId)`。无人值守时，这种漂移会沿依赖传播，直到构建失败或埋下隐患。

### 2.3 “任务完成错觉”与执行遗漏

在缺乏强验收门禁时，智能体容易提前“宣布完成”（Termination Bias），用占位实现、只覆盖快乐路径、遗漏边界与错误处理；任务列表变长后也容易丢全局进度。

---

## 3. 总体架构：双层控制模型

**双层控制模型（Two-Layer Control Model）**：

- **认知层（Cognitive Layer）**：Claude/Agent 负责理解需求、分解任务、生成/修改代码。
- **确定性控制层（Deterministic Control Layer）**：Hooks + 脚本负责注入状态、拦截危险动作、强制质量门禁、阻止提前停止。

核心组件（概念层）：

| 组件 | 职责 | 解决的问题 |
|---|---|---|
| Orchestrator（治理提示） | 定义身份、流程、最高指令 | 行为边界、抗漂移 |
| Hooks（控制平面） | 注入状态 / 拦截写入 / 审查门禁 / Stop 逻辑 | 上下文丢失、API 不一致、无人值守错误累积 |
| State Persistence（外部记忆） | 用文件保存 roadmap/契约/运行时状态 | 跨压缩周期记忆持久化 |
| Agent Swarm（专用分工） | 架构/实现/审查隔离 | 降低单体上下文负载，提升一致性 |
| Codex Gatekeeper（可选） | 外部 CLI 强制质量审查 | 防“看起来对但其实错” |

---

## 仓库结构（当前实现骨架）

本仓库已提供 `.claude/` 目录的关键文件（实际路径以本节为准）：

```
.claude/
├── CLAUDE.md                 # 宪法级治理层（Orchestrator 协议）
├── settings.json             # Hook 配置入口
├── hooks/
│   ├── inject_state.py       # UserPromptSubmit：注入外部状态到上下文
│   ├── loop_driver.py        # Stop：如果 ROADMAP 未完成则阻止停止
│   └── guardrail_commit.py   # （可选）PreToolUse：提交前门禁示例
├── agents/                   # 专用智能体定义
│   ├── project-architect-supervisor.md
│   ├── code-executor.md
│   ├── codex-reviewer.md
│   ├── prd-generator.md
│   └── visual-designer.md
└── skills/
    └── codex-collaboration/  # Codex CLI 交互模式（Skill）
```

运行时状态目录（需要你初始化）：

```
.claude/status/
├── ROADMAP.md                # 全局进度表（Hook 用它判断是否继续 loop）
├── memory.json               # 微观状态快照（跨压缩周期“复活”上下文）
└── api_contract.yaml         # API 契约（Executor/Reviewer 的“法律”）
```

---

## 4. 治理层：`.claude/CLAUDE.md` 作为“宪法”

`.claude/CLAUDE.md` 的定位不是 README，而是“宪法级协议”，用来覆盖模型的默认偏好（求快、少做、提前结束），并规定：

- **Memory First**：任何行动前先读取外部状态（而不是依赖对话历史）。
- **No Human Interaction（无人值守）**：遇到问题先自修复，只有多次失败才允许停止。
- **Quality Gate**：代码变更需要通过审查门禁（如 Codex Reviewer）。
- **State Persistence**：完成子任务后必须更新状态文件。
- **Agent Swarm Protocol**：架构/实现/审查分工，Orchestrator 不直接写业务代码。

参见：`.claude/CLAUDE.md`

---

## 5. 状态持久化：用文件系统做“外部海马体”

目标：即使发生上下文压缩/会话重启，系统也能“读回现场”，继续执行而不漂移。

推荐最小状态：

- `ROADMAP.md`：宏观任务队列（Todo / In Progress / Done）。
- `memory.json`：微观状态机（当前任务、活跃文件、上次失败原因、重试次数等）。
- `api_contract.yaml`：接口契约（函数签名、参数、返回值、错误约束等）。

本仓库已实现“状态注入”Hook：`.claude/hooks/inject_state.py` 会在每次提示提交时把 `memory.json` 与 `ROADMAP.md` 未完成任务注入到上下文，抵抗 Auto-Compact。

---

## 6. Hook 控制平面：事前约束 + 事后审计 + 防止提前停止

### 6.1 UserPromptSubmit：状态注入（已实现）

- 配置：`.claude/settings.json`
- 实现：`.claude/hooks/inject_state.py`
- 效果：每次交互强制注入 `memory.json` 与 `ROADMAP.md` 的未完成项，提醒“以文件为准继续 loop”。

### 6.2 Stop Hook：防止提前停止（已实现）

- 配置：`.claude/settings.json`
- 实现：`.claude/hooks/loop_driver.py`
- 逻辑：若 `ROADMAP.md` 中仍存在 `- [ ]`，则阻止停止并要求继续下一个任务；否则允许 stop。

### 6.3 PreToolUse / PostToolUse：确定性门禁（示例，需按需启用）

典型用法：

- **PreToolUse（写入前防御）**：在写入 API 相关文件前检查签名/依赖/契约一致性，不通过则 `deny` 并给出明确原因。
- **PreToolUse（提交前门禁）**：拦截 `git commit`，先运行 `codex review`，FAIL 则拒绝提交并把审查意见回灌给智能体。

仓库中提供了提交门禁示例脚本：`.claude/hooks/guardrail_commit.py`（当前未在 `.claude/settings.json` 启用，你可以按需要接入）。

---

## 7. 专用智能体网络（Agent Swarm）

该模式通过“角色隔离”降低漂移：

- `project-architect-supervisor`：输出架构树、分阶段计划、可执行任务清单（只做规划，不写实现）。
- `code-executor`：严格 TDD，实现必须与 `api_contract.yaml` 100%一致，并写测试/跑测试/跑 linter。
- `codex-reviewer`：只负责运行 Codex CLI 并报告 PASS/FAIL（不手工审查、不伪造结果）。
- `prd-generator`：把模糊需求变成可执行 PRD。
- `visual-designer`：输出 ASCII wireframe / Mermaid 图。

参见：`.claude/agents/*`

---

## 8. 自动化反馈闭环与自愈策略

推荐的无人值守闭环：

1. **任务获取**：从 `ROADMAP.md` 取下一条未完成任务。
2. **状态注入**：Hook 注入 `memory.json`/Roadmap，抵抗上下文丢失。
3. **实现（TDD）**：先写失败测试→实现→测试通过→重构（必要时）→linter 通过。
4. **外部审查（可选但强烈建议）**：Codex CLI 作为质量门禁，FAIL 则回到第 3 步修复。
5. **写回状态**：更新 `memory.json` 和 `ROADMAP.md`。
6. **Stop 检查**：若仍有未完成任务，Stop Hook 阻止停止并继续循环。

自愈建议（概念层）：

- **指数退避**：对外部依赖/网络抖动的重试采用 backoff。
- **死循环熔断**：同一任务失败超过阈值（如 5 次）触发“换方案/降级/回滚/请求人工介入”。
- **错误记账**：把失败原因与修复尝试写入 `memory.json`，避免重复踩坑。

---

## 9. 实施路径（从 0 到 10h+ Loop）

1. **初始化状态目录**
   - 创建 `.claude/status/ROADMAP.md`、`.claude/status/memory.json`、（可选）`.claude/status/api_contract.yaml`。
2. **确认 Hooks 已启用**
   - 参见 `.claude/settings.json`，确保 `inject_state.py` 与 `loop_driver.py` 生效。
3. **先规划后执行**
   - 用 `project-architect-supervisor` 生成架构树与分阶段任务；把任务写进 `ROADMAP.md`。
4. **进入执行循环**
   - 用 `code-executor` 按任务逐个实现（强制 TDD），每完成一步都更新 `memory.json`/`ROADMAP.md`。
5. **引入质量门禁（可选）**
   - 用 `codex-reviewer` 在提交前跑 Codex CLI；或把门禁挂到 PreToolUse（参考 `guardrail_commit.py`）。

---

## 10. 交付物清单

面向“弹性自主工程系统”的关键交付物（仓库已包含/建议补齐）：

- 治理层：`.claude/CLAUDE.md`
- Hook 控制平面：`.claude/settings.json` + `.claude/hooks/*`
- 外部记忆：`.claude/status/ROADMAP.md`、`.claude/status/memory.json`、`.claude/status/api_contract.yaml`
- 专用智能体：`.claude/agents/*`
- Codex 协作模式（可选）：`.claude/skills/codex-collaboration/SKILL.md`

---

## License

See `LICENSE`.

