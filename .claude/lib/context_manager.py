#!/usr/bin/env python3
"""
Unified Context Manager v1.0
统一上下文管理器 - 所有 Agent 和 Hook 的上下文来源

设计原则：
1. 单一数据源 - 所有上下文从这里获取
2. 分层组装 - 根据调用者需求组装不同层级的上下文
3. 智能缓存 - 避免重复读取和解析
4. 格式统一 - 输出格式可被 LLM 和人类理解

使用方式：
  from context_manager import ContextManager
  ctx = ContextManager()
  full_context = ctx.get_full_context()
  review_context = ctx.get_review_context(changed_files)
"""

import os
import json
import re
import subprocess
import hashlib
from pathlib import Path
from datetime import datetime
from typing import Dict, List, Optional, Any

# ═══════════════════════════════════════════════════════════════════
# 配置
# ═══════════════════════════════════════════════════════════════════

STATUS_DIR = ".claude/status"
PHASES_DIR = ".claude/phases"
AGENTS_DIR = ".claude/agents"

# 核心状态文件
FILES = {
    "memory": f"{STATUS_DIR}/memory.json",
    "roadmap": f"{STATUS_DIR}/ROADMAP.md",
    "contract": f"{STATUS_DIR}/api_contract.yaml",
    "errors": f"{STATUS_DIR}/error_history.json",
    "digest": f"{STATUS_DIR}/code_digest.json",
    "changes": f"{STATUS_DIR}/recent_changes.json",
    "decisions": f"{STATUS_DIR}/decisions.log",
}

# 上下文预算（字符数）
BUDGETS = {
    "full": 80000,      # 完整上下文
    "review": 40000,    # 代码审查上下文
    "task": 30000,      # 单任务上下文
    "minimal": 10000,   # 最小上下文
}

# ═══════════════════════════════════════════════════════════════════
# 工具函数
# ═══════════════════════════════════════════════════════════════════

def read_file(path: str) -> Optional[str]:
    """安全读取文件"""
    try:
        if os.path.exists(path):
            with open(path, 'r', encoding='utf-8') as f:
                return f.read()
    except Exception as e:
        return f"[Error reading {path}: {e}]"
    return None

def read_json(path: str) -> Optional[Dict]:
    """安全读取 JSON"""
    content = read_file(path)
    if content:
        try:
            return json.loads(content)
        except:
            pass
    return None

def get_file_hash(content: str) -> str:
    """获取内容 hash"""
    return hashlib.md5(content.encode()).hexdigest()[:8]

def truncate_middle(text: str, max_len: int) -> str:
    """保留头尾，截断中间"""
    if len(text) <= max_len:
        return text
    half = max_len // 2 - 20
    return text[:half] + "\n\n... [TRUNCATED] ...\n\n" + text[-half:]

# ═══════════════════════════════════════════════════════════════════
# 上下文生成器
# ═══════════════════════════════════════════════════════════════════

class ContextManager:
    """统一上下文管理器"""
    
    def __init__(self, project_root: str = "."):
        self.root = project_root
        self._cache = {}
    
    # ─────────────────────────────────────────────────────────────────
    # Layer 0: 系统指令
    # ─────────────────────────────────────────────────────────────────
    
    def get_system_header(self, mode: str = "autonomous") -> str:
        """生成系统头部"""
        headers = {
            "autonomous": """
╔══════════════════════════════════════════════════════════════════════════════╗
║                    🤖 AUTONOMOUS MODE - CONTEXT INJECTION                     ║
╠══════════════════════════════════════════════════════════════════════════════╣
║  ⚠️ WARNING: Your conversation history may be compressed/truncated            ║
║  ⚠️ TRUST ONLY the state files below, NOT your "memory"                       ║
║  ⚠️ CONTINUE the loop - do NOT stop until ROADMAP is complete                 ║
╚══════════════════════════════════════════════════════════════════════════════╝
""",
            "review": """
╔══════════════════════════════════════════════════════════════════════════════╗
║                    🔍 CODE REVIEW MODE - CONTEXT INJECTION                    ║
╠══════════════════════════════════════════════════════════════════════════════╣
║  Review the code changes against the API contract and project standards       ║
║  Check for: contract compliance, test coverage, error handling, consistency   ║
╚══════════════════════════════════════════════════════════════════════════════╝
""",
            "task": """
╔══════════════════════════════════════════════════════════════════════════════╗
║                    📋 TASK EXECUTION MODE - CONTEXT INJECTION                 ║
╠══════════════════════════════════════════════════════════════════════════════╣
║  Focus on the current task specification below                                ║
║  Follow TDD: write failing test first, then implement, then verify            ║
╚══════════════════════════════════════════════════════════════════════════════╝
"""
        }
        return headers.get(mode, headers["autonomous"])
    
    # ─────────────────────────────────────────────────────────────────
    # Layer 1: 当前状态 (memory.json)
    # ─────────────────────────────────────────────────────────────────
    
    def get_memory_context(self) -> str:
        """获取当前状态上下文"""
        memory = read_json(FILES["memory"])
        if not memory:
            return """
## 🧠 CURRENT STATE
```json
{"status": "NOT_INITIALIZED", "message": "Run initialization first"}
```
"""
        
        # 格式化输出
        ctx = "\n## 🧠 CURRENT STATE\n"
        
        # 当前任务
        task = memory.get("current_task", {})
        if task.get("id"):
            ctx += f"""
### Current Task
- **ID**: {task.get('id')}
- **Name**: {task.get('name', 'Unknown')}
- **Status**: {task.get('status', 'Unknown')}
- **Retry Count**: {task.get('retry_count', 0)}/{task.get('max_retries', 5)}
"""
        
        # 工作上下文
        wctx = memory.get("working_context", {})
        if wctx.get("current_file"):
            ctx += f"""
### Working Context
- **Current File**: `{wctx.get('current_file')}`
- **Current Function**: `{wctx.get('current_function', 'N/A')}`
"""
            if wctx.get("pending_tests"):
                ctx += f"- **Pending Tests**: {', '.join(wctx['pending_tests'][:5])}\n"
            if wctx.get("pending_implementations"):
                ctx += f"- **Pending Impl**: {', '.join(wctx['pending_implementations'][:5])}\n"
        
        # 下一步行动
        next_action = memory.get("next_action", {})
        if next_action.get("action"):
            ctx += f"""
### Next Action
- **Action**: {next_action.get('action')}
- **Target**: {next_action.get('target', 'N/A')}
- **Reason**: {next_action.get('reason', 'N/A')}
"""
        
        # 进度
        progress = memory.get("progress", {})
        if progress.get("tasks_total"):
            completed = progress.get("tasks_completed", 0)
            total = progress.get("tasks_total", 0)
            pct = (completed / total * 100) if total > 0 else 0
            ctx += f"""
### Progress
- **Tasks**: {completed}/{total} ({pct:.1f}%)
- **Current Phase**: {progress.get('current_phase', 'N/A')}
"""
        
        return ctx
    
    # ─────────────────────────────────────────────────────────────────
    # Layer 2: 任务列表 (ROADMAP.md + Phase Plans)
    # ─────────────────────────────────────────────────────────────────
    
    def get_roadmap_context(self, include_completed: bool = False) -> str:
        """获取任务列表上下文"""
        roadmap = read_file(FILES["roadmap"])
        if not roadmap:
            return "\n## ❌ ROADMAP NOT FOUND\nInitialize `.claude/status/ROADMAP.md` first!\n"
        
        ctx = "\n## 📋 ROADMAP\n"
        
        # 解析任务
        pending = []
        in_progress = []
        completed = []
        
        for line in roadmap.split('\n'):
            stripped = line.strip()
            if stripped.startswith("- [ ]"):
                pending.append(line)
            elif stripped.startswith("- [>]") or stripped.startswith("- [~]"):
                in_progress.append(line)
            elif stripped.startswith("- [x]") or stripped.startswith("- [X]"):
                completed.append(line)
        
        total = len(pending) + len(in_progress) + len(completed)
        ctx += f"\n**Progress**: {len(completed)}/{total} tasks completed\n"
        
        if in_progress:
            ctx += "\n### 🔄 IN PROGRESS\n"
            for task in in_progress:
                ctx += f"{task}\n"
        
        ctx += "\n### ⏳ PENDING\n"
        for task in pending[:20]:
            ctx += f"{task}\n"
        if len(pending) > 20:
            ctx += f"... and {len(pending) - 20} more\n"
        
        if include_completed and completed:
            ctx += "\n### ✅ COMPLETED (Recent)\n"
            for task in completed[-5:]:
                ctx += f"{task}\n"
        
        return ctx
    
    def get_current_phase_context(self) -> str:
        """获取当前阶段详情"""
        memory = read_json(FILES["memory"])
        if not memory:
            return ""
        
        current_phase = memory.get("progress", {}).get("current_phase")
        if not current_phase:
            return ""
        
        # 查找阶段目录
        phase_dir = None
        if os.path.exists(PHASES_DIR):
            for d in os.listdir(PHASES_DIR):
                if current_phase.lower() in d.lower():
                    phase_dir = os.path.join(PHASES_DIR, d)
                    break
        
        if not phase_dir:
            return ""
        
        ctx = f"\n## 📁 CURRENT PHASE: {current_phase}\n"
        
        # 读取 PHASE_PLAN.md
        plan_file = os.path.join(phase_dir, "PHASE_PLAN.md")
        if os.path.exists(plan_file):
            plan = read_file(plan_file)
            ctx += f"\n### Phase Plan\n```markdown\n{truncate_middle(plan, 3000)}\n```\n"
        
        return ctx
    
    def get_current_task_spec(self) -> str:
        """获取当前任务规格"""
        memory = read_json(FILES["memory"])
        if not memory:
            return ""
        
        task_id = memory.get("current_task", {}).get("id")
        if not task_id:
            return ""
        
        # 在 phases 目录中查找任务文件
        if os.path.exists(PHASES_DIR):
            for phase_dir in os.listdir(PHASES_DIR):
                phase_path = os.path.join(PHASES_DIR, phase_dir)
                if os.path.isdir(phase_path):
                    for f in os.listdir(phase_path):
                        if task_id in f and f.endswith('.md'):
                            task_file = os.path.join(phase_path, f)
                            content = read_file(task_file)
                            if content:
                                return f"\n## 📝 CURRENT TASK SPEC: {task_id}\n```markdown\n{content}\n```\n"
        
        return ""
    
    # ─────────────────────────────────────────────────────────────────
    # Layer 3: 错误历史
    # ─────────────────────────────────────────────────────────────────
    
    def get_error_context(self, task_filter: Optional[str] = None) -> str:
        """获取错误历史上下文"""
        errors = read_json(FILES["errors"])
        if not errors:
            return ""
        
        # 过滤相关错误
        if task_filter:
            relevant = [e for e in errors if e.get("task") == task_filter]
        else:
            relevant = errors[-15:]  # 最近15条
        
        if not relevant:
            return ""
        
        ctx = "\n## ⚠️ ERROR HISTORY (MUST AVOID REPEATING)\n"
        
        unresolved = [e for e in relevant if not e.get("resolution")]
        resolved = [e for e in relevant if e.get("resolution")]
        
        if unresolved:
            ctx += "\n### ❌ Unresolved Errors\n"
            for err in unresolved[-5:]:
                ctx += f"""
**Task**: {err.get('task', 'unknown')}
**Error**: {err.get('error', 'unknown')[:200]}
**Attempted**: {err.get('attempted_fix', 'N/A')[:100]}
---
"""
        
        if resolved:
            ctx += "\n### ✅ Resolved (Learn from these)\n"
            for err in resolved[-3:]:
                ctx += f"""
**Task**: {err.get('task', 'unknown')}
**Error**: {err.get('error', 'unknown')[:100]}
**Solution**: {err.get('resolution', 'N/A')[:150]}
---
"""
        
        return ctx
    
    # ─────────────────────────────────────────────────────────────────
    # Layer 4: API 契约
    # ─────────────────────────────────────────────────────────────────
    
    def get_contract_context(self, relevant_modules: Optional[List[str]] = None) -> str:
        """获取 API 契约上下文"""
        contract = read_file(FILES["contract"])
        if not contract:
            return ""
        
        ctx = "\n## 📜 API CONTRACT\n"
        
        if relevant_modules:
            # 只提取相关模块的契约
            ctx += f"(Filtered for: {', '.join(relevant_modules)})\n"
            # TODO: 实现契约过滤逻辑
        
        ctx += f"```yaml\n{truncate_middle(contract, 8000)}\n```\n"
        return ctx
    
    # ─────────────────────────────────────────────────────────────────
    # Layer 5: 活跃文件内容
    # ─────────────────────────────────────────────────────────────────
    
    def get_active_files_context(self, max_files: int = 5, max_chars_per_file: int = 4000) -> str:
        """获取活跃文件上下文"""
        memory = read_json(FILES["memory"])
        if not memory:
            return ""
        
        active_files = memory.get("active_files", [])
        current_file = memory.get("working_context", {}).get("current_file")
        
        if current_file and current_file not in active_files:
            active_files.insert(0, current_file)
        
        if not active_files:
            return ""
        
        ctx = "\n## 📂 ACTIVE FILES\n"
        
        for fp in active_files[:max_files]:
            content = read_file(fp)
            if content:
                ctx += f"\n### `{fp}`\n"
                ctx += f"```\n{truncate_middle(content, max_chars_per_file)}\n```\n"
        
        return ctx
    
    def get_changed_files_context(self, changed_files: List[str], include_diff: bool = True) -> str:
        """获取变更文件上下文（用于代码审查）"""
        if not changed_files:
            return ""
        
        ctx = "\n## 📝 CHANGED FILES FOR REVIEW\n"
        
        for fp in changed_files[:10]:
            content = read_file(fp)
            if content:
                ctx += f"\n### `{fp}`\n"
                ctx += f"```\n{truncate_middle(content, 5000)}\n```\n"
        
        # 获取 git diff
        if include_diff:
            try:
                result = subprocess.run(
                    ['git', 'diff', '--cached'] + changed_files,
                    capture_output=True, text=True, timeout=5
                )
                if result.stdout:
                    ctx += f"\n### Git Diff\n```diff\n{truncate_middle(result.stdout, 5000)}\n```\n"
            except:
                pass
        
        return ctx
    
    # ─────────────────────────────────────────────────────────────────
    # Layer 6: 项目结构
    # ─────────────────────────────────────────────────────────────────
    
    def get_structure_context(self, max_depth: int = 3) -> str:
        """获取项目结构上下文"""
        digest = read_json(FILES["digest"])
        
        if digest:
            # 使用预生成的摘要
            ctx = "\n## 🏗️ PROJECT STRUCTURE (from digest)\n"
            ctx += f"Files: {digest.get('stats', {}).get('total_files', 'N/A')}\n"
            ctx += f"Lines: {digest.get('stats', {}).get('total_lines', 'N/A')}\n"
            
            # 按语言统计
            by_lang = digest.get('stats', {}).get('by_language', {})
            if by_lang:
                ctx += "\nBy Language:\n"
                for lang, stats in by_lang.items():
                    ctx += f"  - {lang}: {stats['files']} files, {stats['lines']} lines\n"
            
            # 关键签名
            ctx += "\n### Key Signatures\n"
            files = digest.get('files', [])
            for f in files[:20]:
                sigs = f.get('signatures', [])
                if sigs:
                    ctx += f"\n**{f['path']}**\n"
                    for sig in sigs[:5]:
                        ctx += f"  - `{sig.get('signature', sig.get('name', 'unknown'))}`\n"
            
            return ctx
        
        # 如果没有摘要，动态生成简单结构
        return self._generate_simple_structure(max_depth)
    
    def _generate_simple_structure(self, max_depth: int) -> str:
        """生成简单的项目结构"""
        ctx = "\n## 🏗️ PROJECT STRUCTURE\n```\n"
        
        ignore = {'.git', '__pycache__', 'node_modules', 'venv', '.venv', 'dist', 'build', '.claude'}
        
        def scan(path, depth=0):
            if depth > max_depth:
                return ""
            
            result = ""
            try:
                items = sorted(os.listdir(path))
            except:
                return ""
            
            for item in items:
                if item.startswith('.') or item in ignore:
                    continue
                
                full_path = os.path.join(path, item)
                indent = "  " * depth
                
                if os.path.isdir(full_path):
                    result += f"{indent}📁 {item}/\n"
                    result += scan(full_path, depth + 1)
                elif item.endswith(('.py', '.js', '.ts', '.go', '.rs', '.java')):
                    result += f"{indent}📄 {item}\n"
            
            return result
        
        ctx += scan(self.root)
        ctx += "```\n"
        return ctx[:5000]  # 限制长度
    
    # ─────────────────────────────────────────────────────────────────
    # Layer 7: Git 历史
    # ─────────────────────────────────────────────────────────────────
    
    def get_git_context(self, limit: int = 10) -> str:
        """获取 Git 历史上下文"""
        try:
            result = subprocess.run(
                ['git', 'log', f'-{limit}', '--oneline', '--name-status'],
                capture_output=True, text=True, timeout=5
            )
            if result.returncode == 0 and result.stdout:
                return f"\n## 📜 RECENT GIT HISTORY\n```\n{result.stdout[:2000]}\n```\n"
        except:
            pass
        return ""
    
    # ─────────────────────────────────────────────────────────────────
    # Layer 8: 决策日志
    # ─────────────────────────────────────────────────────────────────
    
    def get_decisions_context(self, limit: int = 20) -> str:
        """获取决策日志上下文"""
        content = read_file(FILES["decisions"])
        if not content:
            return ""
        
        lines = content.strip().split('\n')
        recent = lines[-limit:]
        return f"\n## 📝 RECENT DECISIONS\n```\n" + '\n'.join(recent) + "\n```\n"
    
    # ═══════════════════════════════════════════════════════════════════
    # 组装方法
    # ═══════════════════════════════════════════════════════════════════
    
    def get_full_context(self) -> str:
        """获取完整上下文（用于 UserPromptSubmit）"""
        parts = [
            self.get_system_header("autonomous"),
            self.get_memory_context(),
            self.get_roadmap_context(),
            self.get_current_task_spec(),
            self.get_error_context(),
            self.get_contract_context(),
            self.get_active_files_context(),
            self.get_structure_context(),
            self.get_git_context(),
            self.get_decisions_context(),
        ]
        
        ctx = ''.join(parts)
        
        # 添加行动指令
        ctx += """
═══════════════════════════════════════════════════════════════════════════════
📌 MANDATORY ACTIONS:
1. Read the CURRENT STATE above carefully
2. Check ERROR HISTORY to avoid repeating mistakes  
3. Follow the NEXT ACTION from memory.json
4. Execute following TDD (test first, then implement)
5. Update memory.json IMMEDIATELY after any progress
6. Continue loop - DO NOT STOP until all tasks are [x] marked
═══════════════════════════════════════════════════════════════════════════════
"""
        
        return truncate_middle(ctx, BUDGETS["full"])
    
    def get_review_context(self, changed_files: List[str]) -> str:
        """获取代码审查上下文（用于 Codex Review）"""
        parts = [
            self.get_system_header("review"),
            self.get_memory_context(),
            self.get_current_task_spec(),
            self.get_contract_context(),
            self.get_changed_files_context(changed_files),
            self.get_error_context(),
        ]
        
        ctx = ''.join(parts)
        
        ctx += """
═══════════════════════════════════════════════════════════════════════════════
📌 REVIEW CHECKLIST:
1. Does the code match the API CONTRACT exactly? (signatures, types, returns)
2. Are there comprehensive tests? (happy path + edge cases + error cases)
3. Is error handling complete?
4. Does it follow project conventions?
5. Any security concerns?
═══════════════════════════════════════════════════════════════════════════════
"""
        
        return truncate_middle(ctx, BUDGETS["review"])
    
    def get_task_context(self, task_id: str) -> str:
        """获取特定任务的上下文"""
        parts = [
            self.get_system_header("task"),
            self.get_memory_context(),
            self.get_current_task_spec(),
            self.get_contract_context(),
            self.get_error_context(task_filter=task_id),
            self.get_active_files_context(max_files=3),
        ]
        
        return truncate_middle(''.join(parts), BUDGETS["task"])


# ═══════════════════════════════════════════════════════════════════
# CLI 接口
# ═══════════════════════════════════════════════════════════════════

if __name__ == "__main__":
    import sys
    
    ctx = ContextManager()
    
    if len(sys.argv) < 2:
        print("Usage: python context_manager.py [full|review|task|structure]")
        sys.exit(1)
    
    mode = sys.argv[1]
    
    if mode == "full":
        print(ctx.get_full_context())
    elif mode == "review":
        files = sys.argv[2:] if len(sys.argv) > 2 else []
        print(ctx.get_review_context(files))
    elif mode == "task":
        task_id = sys.argv[2] if len(sys.argv) > 2 else None
        print(ctx.get_task_context(task_id))
    elif mode == "structure":
        print(ctx.get_structure_context())
    else:
        print(f"Unknown mode: {mode}")
