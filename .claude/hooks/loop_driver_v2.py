#!/usr/bin/env python3
"""
Enhanced Loop Driver v2.0
增强版循环驱动器 - 更智能的停止控制和自愈能力

功能：
1. 检查 ROADMAP 是否完成
2. 检查是否陷入死循环（同一任务失败太多次）
3. 提供恢复建议
4. 支持紧急熔断
"""

import sys
import json
import os
from datetime import datetime, timedelta

STATUS_DIR = ".claude/status"
ROADMAP_FILE = f"{STATUS_DIR}/ROADMAP.md"
MEMORY_FILE = f"{STATUS_DIR}/memory.json"
ERROR_FILE = f"{STATUS_DIR}/error_history.json"

# 配置
MAX_RETRIES_PER_TASK = 5
MAX_CONSECUTIVE_ERRORS = 10
STUCK_DETECTION_THRESHOLD = 3  # 同一任务失败次数超过这个值视为卡住

def load_json_safe(path):
    try:
        if os.path.exists(path):
            with open(path, 'r') as f:
                return json.load(f)
    except:
        pass
    return None

def check_roadmap_completion():
    """检查 ROADMAP 是否完成"""
    if not os.path.exists(ROADMAP_FILE):
        return {
            "complete": False,
            "reason": "ROADMAP not found",
            "pending_count": -1,
            "completed_count": 0
        }
    
    with open(ROADMAP_FILE, 'r') as f:
        content = f.read()
    
    lines = content.split('\n')
    pending = [l for l in lines if l.lstrip().startswith("- [ ]")]
    completed = [l for l in lines if l.lstrip().startswith("- [x]") or l.lstrip().startswith("- [X]")]
    in_progress = [l for l in lines if l.lstrip().startswith("- [>]") or l.lstrip().startswith("- [~]")]
    
    return {
        "complete": len(pending) == 0 and len(in_progress) == 0,
        "reason": None if len(pending) == 0 else f"{len(pending)} tasks remaining",
        "pending_count": len(pending),
        "completed_count": len(completed),
        "in_progress_count": len(in_progress),
        "next_pending": pending[0] if pending else None
    }

def check_stuck_state():
    """检查是否陷入死循环"""
    memory = load_json_safe(MEMORY_FILE)
    errors = load_json_safe(ERROR_FILE) or []
    
    if not memory:
        return {"stuck": False, "reason": None}
    
    current_task = memory.get("current_task", {})
    task_id = current_task.get("id")
    retry_count = current_task.get("retry_count", 0)
    
    # 检查重试次数
    if retry_count >= MAX_RETRIES_PER_TASK:
        return {
            "stuck": True,
            "reason": f"Task {task_id} has failed {retry_count} times",
            "suggestion": "Consider skipping this task or trying a completely different approach"
        }
    
    # 检查错误历史中同一任务的失败次数
    if task_id and errors:
        task_errors = [e for e in errors if e.get("task") == task_id and not e.get("resolution")]
        if len(task_errors) >= STUCK_DETECTION_THRESHOLD:
            return {
                "stuck": True,
                "reason": f"Task {task_id} has {len(task_errors)} unresolved errors",
                "suggestion": "Review error patterns, try alternative implementation, or mark as blocked"
            }
    
    # 检查总体错误数
    recent_errors = [e for e in errors[-MAX_CONSECUTIVE_ERRORS:] if not e.get("resolution")]
    if len(recent_errors) >= MAX_CONSECUTIVE_ERRORS:
        return {
            "stuck": True,
            "reason": f"{len(recent_errors)} consecutive unresolved errors",
            "suggestion": "System may need human intervention or environment reset"
        }
    
    return {"stuck": False, "reason": None}

def generate_recovery_instructions():
    """生成恢复指令"""
    memory = load_json_safe(MEMORY_FILE)
    
    instructions = []
    
    if memory:
        next_action = memory.get("next_action", {})
        if next_action:
            instructions.append(f"Resume from: {next_action.get('action', 'UNKNOWN')}")
            if next_action.get("target"):
                instructions.append(f"Target: {next_action['target']}")
        
        working_context = memory.get("working_context", {})
        if working_context.get("current_file"):
            instructions.append(f"Continue working on: {working_context['current_file']}")
        
        if working_context.get("pending_tests"):
            instructions.append(f"Pending tests: {', '.join(working_context['pending_tests'][:3])}")
    
    return instructions

def main():
    # 读取输入
    input_data = json.loads(sys.stdin.read())
    
    # 检查 ROADMAP 完成状态
    roadmap_status = check_roadmap_completion()
    
    # 检查是否卡住
    stuck_status = check_stuck_state()
    
    # 决策逻辑
    if roadmap_status["complete"]:
        # 所有任务完成，允许停止
        print(json.dumps({
            "decision": "allow",
            "reason": f"""
🎉 ALL TASKS COMPLETED!

Summary:
- Completed tasks: {roadmap_status['completed_count']}
- Session finished successfully

You may now stop the autonomous loop.
"""
        }))
        return
    
    if stuck_status["stuck"]:
        # 系统卡住，需要特殊处理
        recovery = generate_recovery_instructions()
        print(json.dumps({
            "decision": "block",
            "reason": f"""
⚠️ STUCK STATE DETECTED

Reason: {stuck_status['reason']}
Suggestion: {stuck_status.get('suggestion', 'Review and retry')}

Options:
1. Try a completely different approach for the current task
2. Skip the current task and mark it as BLOCKED in ROADMAP
3. If truly blocked, update memory.json with detailed block reason

DO NOT simply retry the same approach that failed.

Recovery hints:
{chr(10).join('- ' + i for i in recovery) if recovery else '- Read memory.json for context'}
"""
        }))
        return
    
    if roadmap_status["pending_count"] == -1:
        # ROADMAP 不存在
        print(json.dumps({
            "decision": "block",
            "reason": """
❌ ROADMAP NOT FOUND

Cannot continue autonomous loop without a roadmap.

Please initialize:
1. .claude/status/ROADMAP.md - Task list
2. .claude/status/memory.json - Current state

Use the project-architect-supervisor agent to generate these files.
"""
        }))
        return
    
    # 正常情况：还有任务待完成
    recovery = generate_recovery_instructions()
    next_task = roadmap_status.get("next_pending", "Unknown")
    
    print(json.dumps({
        "decision": "block",
        "reason": f"""
🔄 AUTONOMOUS MODE ACTIVE

Progress: {roadmap_status['completed_count']}/{roadmap_status['completed_count'] + roadmap_status['pending_count']} tasks completed
Pending: {roadmap_status['pending_count']} tasks
In Progress: {roadmap_status.get('in_progress_count', 0)} tasks

Next task: {next_task.strip() if next_task else 'Check ROADMAP'}

{chr(10).join('📌 ' + i for i in recovery) if recovery else ''}

Continue the loop. DO NOT STOP.
"""
    }))

if __name__ == "__main__":
    main()
