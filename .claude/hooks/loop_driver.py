#!/usr/bin/env python3
"""
Loop Driver v3.0
智能循环驱动器 - 控制自主循环的继续/停止

功能：
1. 检查 ROADMAP 完成状态
2. 检测死循环（同一任务连续失败）
3. 提供恢复指令
4. 支持紧急熔断
"""

import sys
import json
import os
from datetime import datetime

# 添加 lib 目录到路径
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '..', 'lib'))

try:
    from context_manager import ContextManager
except ImportError:
    ContextManager = None

# ═══════════════════════════════════════════════════════════════════
# 配置
# ═══════════════════════════════════════════════════════════════════

STATUS_DIR = ".claude/status"
FILES = {
    "memory": f"{STATUS_DIR}/memory.json",
    "roadmap": f"{STATUS_DIR}/ROADMAP.md",
    "errors": f"{STATUS_DIR}/error_history.json",
}

MAX_RETRIES = 5
MAX_CONSECUTIVE_ERRORS = 10

# ═══════════════════════════════════════════════════════════════════
# 工具函数
# ═══════════════════════════════════════════════════════════════════

def read_file(path):
    try:
        if os.path.exists(path):
            with open(path, 'r', encoding='utf-8') as f:
                return f.read()
    except:
        pass
    return None

def read_json(path):
    content = read_file(path)
    if content:
        try:
            return json.loads(content)
        except:
            pass
    return None

# ═══════════════════════════════════════════════════════════════════
# 检查函数
# ═══════════════════════════════════════════════════════════════════

def check_roadmap():
    """检查 ROADMAP 完成状态"""
    content = read_file(FILES["roadmap"])
    if not content:
        return {
            "exists": False,
            "complete": False,
            "pending": 0,
            "in_progress": 0,
            "completed": 0
        }
    
    pending = []
    in_progress = []
    completed = []
    
    for line in content.split('\n'):
        stripped = line.strip()
        if stripped.startswith("- [ ]"):
            pending.append(line)
        elif stripped.startswith("- [>]") or stripped.startswith("- [~]"):
            in_progress.append(line)
        elif stripped.startswith("- [x]") or stripped.startswith("- [X]"):
            completed.append(line)
    
    return {
        "exists": True,
        "complete": len(pending) == 0 and len(in_progress) == 0,
        "pending": len(pending),
        "in_progress": len(in_progress),
        "completed": len(completed),
        "total": len(pending) + len(in_progress) + len(completed),
        "next_task": pending[0] if pending else (in_progress[0] if in_progress else None)
    }

def check_stuck():
    """检查是否卡住"""
    memory = read_json(FILES["memory"])
    errors = read_json(FILES["errors"]) or []
    
    if not memory:
        return {"stuck": False}
    
    # 检查重试次数
    current_task = memory.get("current_task", {})
    task_id = current_task.get("id")
    retry_count = current_task.get("retry_count", 0)
    
    if retry_count >= MAX_RETRIES:
        return {
            "stuck": True,
            "reason": f"Task {task_id} exceeded {MAX_RETRIES} retries",
            "suggestion": "Try different approach or skip task"
        }
    
    # 检查错误历史
    if task_id and errors:
        task_errors = [e for e in errors if e.get("task") == task_id and not e.get("resolution")]
        if len(task_errors) >= 3:
            return {
                "stuck": True,
                "reason": f"Task {task_id} has {len(task_errors)} unresolved errors",
                "suggestion": "Review error patterns, try alternative"
            }
    
    # 检查连续错误
    recent_unresolved = [e for e in errors[-MAX_CONSECUTIVE_ERRORS:] if not e.get("resolution")]
    if len(recent_unresolved) >= MAX_CONSECUTIVE_ERRORS:
        return {
            "stuck": True,
            "reason": f"{len(recent_unresolved)} consecutive errors",
            "suggestion": "System may need intervention"
        }
    
    return {"stuck": False}

def get_recovery_context():
    """获取恢复上下文"""
    memory = read_json(FILES["memory"])
    if not memory:
        return []
    
    hints = []
    
    next_action = memory.get("next_action", {})
    if next_action.get("action"):
        hints.append(f"Next Action: {next_action['action']}")
        if next_action.get("target"):
            hints.append(f"Target: {next_action['target']}")
    
    working = memory.get("working_context", {})
    if working.get("current_file"):
        hints.append(f"Working on: {working['current_file']}")
    if working.get("pending_tests"):
        hints.append(f"Pending tests: {', '.join(working['pending_tests'][:3])}")
    
    return hints

# ═══════════════════════════════════════════════════════════════════
# 主逻辑
# ═══════════════════════════════════════════════════════════════════

def main():
    input_data = json.loads(sys.stdin.read())
    
    roadmap = check_roadmap()
    stuck = check_stuck()
    
    # 情况1: ROADMAP 不存在
    if not roadmap["exists"]:
        print(json.dumps({
            "decision": "block",
            "reason": """
❌ ROADMAP NOT FOUND

Cannot run autonomous loop without a roadmap.

Action Required:
1. Use project-architect-supervisor to create:
   - .claude/status/ROADMAP.md
   - .claude/status/api_contract.yaml
   - .claude/status/memory.json

2. Or create manually following the template.
"""
        }))
        return
    
    # 情况2: 所有任务完成
    if roadmap["complete"]:
        print(json.dumps({
            "decision": "allow",
            "reason": f"""
🎉 ALL TASKS COMPLETED!

Summary:
- Total tasks: {roadmap['total']}
- Completed: {roadmap['completed']}

The autonomous loop has finished successfully.
You may now stop.
"""
        }))
        return
    
    # 情况3: 系统卡住
    if stuck["stuck"]:
        hints = get_recovery_context()
        print(json.dumps({
            "decision": "block",
            "reason": f"""
⚠️ STUCK STATE DETECTED

Reason: {stuck['reason']}
Suggestion: {stuck.get('suggestion', 'Review and retry')}

Options:
1. Try a COMPLETELY DIFFERENT approach
2. Skip current task: Mark as [!] in ROADMAP
3. Request human intervention

{'Recovery Hints:' + chr(10) + chr(10).join('  - ' + h for h in hints) if hints else ''}

DO NOT simply retry the same approach.
"""
        }))
        return
    
    # 情况4: 正常继续
    hints = get_recovery_context()
    progress_pct = (roadmap['completed'] / roadmap['total'] * 100) if roadmap['total'] > 0 else 0
    
    print(json.dumps({
        "decision": "block",
        "reason": f"""
🔄 AUTONOMOUS MODE ACTIVE

Progress: {roadmap['completed']}/{roadmap['total']} ({progress_pct:.1f}%)
├── Completed: {roadmap['completed']}
├── In Progress: {roadmap['in_progress']}
└── Pending: {roadmap['pending']}

Next Task: {roadmap['next_task'][:80] if roadmap['next_task'] else 'Check ROADMAP'}

{'Recovery Context:' + chr(10) + chr(10).join('  📌 ' + h for h in hints) if hints else ''}

Continue the loop. DO NOT STOP.
"""
    }))

if __name__ == "__main__":
    main()
