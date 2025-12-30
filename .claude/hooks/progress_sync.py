#!/usr/bin/env python3
"""
Progress Sync Hook
自动从 ROADMAP.md 和任务文件同步进度到 memory.json

功能：
1. 检测 ROADMAP.md 修改 → 更新 memory.json 的 progress
2. 检测 TASK-xxx.md 修改 → 更新 current_task
3. 检测 PHASE_PLAN.md 修改 → 更新 current_phase
4. 保持所有状态文件同步

触发时机：PostToolUse（文件写入后）
"""

import sys
import json
import os
import re
from datetime import datetime
from pathlib import Path

# ═══════════════════════════════════════════════════════════════════
# 配置
# ═══════════════════════════════════════════════════════════════════

STATUS_DIR = ".claude/status"
PHASES_DIR = ".claude/phases"

FILES = {
    "memory": f"{STATUS_DIR}/memory.json",
    "roadmap": f"{STATUS_DIR}/ROADMAP.md",
}

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
    return {}

def write_json(path, data):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, 'w', encoding='utf-8') as f:
        json.dump(data, f, indent=2, ensure_ascii=False)

def log_decision(message):
    """记录决策日志"""
    log_file = f"{STATUS_DIR}/decisions.log"
    try:
        os.makedirs(STATUS_DIR, exist_ok=True)
        with open(log_file, 'a', encoding='utf-8') as f:
            timestamp = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
            f.write(f"[{timestamp}] {message}\n")
    except:
        pass

# ═══════════════════════════════════════════════════════════════════
# 解析函数
# ═══════════════════════════════════════════════════════════════════

def parse_roadmap(content):
    """解析 ROADMAP.md，提取任务统计"""
    if not content:
        return None
    
    pending = []
    in_progress = []
    completed = []
    
    # 解析任务行
    for line in content.split('\n'):
        stripped = line.strip()
        
        # 提取任务 ID（如果有）
        task_match = re.search(r'(TASK-\d+|#\d+)', line)
        task_id = task_match.group(1) if task_match else None
        
        if stripped.startswith("- [ ]"):
            pending.append({"line": line, "id": task_id})
        elif stripped.startswith("- [>]") or stripped.startswith("- [~]"):
            in_progress.append({"line": line, "id": task_id})
        elif stripped.startswith("- [x]") or stripped.startswith("- [X]"):
            completed.append({"line": line, "id": task_id})
    
    # 解析当前阶段
    phase_match = re.search(r'##\s*Current[:\s]+(?:Phase\s+)?(\d+|[A-Za-z]+)', content, re.IGNORECASE)
    current_phase = phase_match.group(1) if phase_match else None
    
    return {
        "pending": pending,
        "in_progress": in_progress,
        "completed": completed,
        "current_phase": current_phase,
        "total": len(pending) + len(in_progress) + len(completed)
    }

def parse_task_file(content, task_id):
    """解析任务文件，提取任务详情"""
    if not content:
        return None
    
    # 提取状态
    status_match = re.search(r'##\s*Status[:\s]+(\w+)', content, re.IGNORECASE)
    status = status_match.group(1) if status_match else "Unknown"
    
    # 提取名称
    name_match = re.search(r'^#\s*(?:TASK-\d+[:\s]+)?(.+)$', content, re.MULTILINE)
    name = name_match.group(1).strip() if name_match else task_id
    
    # 提取验收标准
    acceptance_criteria = []
    in_criteria_section = False
    for line in content.split('\n'):
        if '## Acceptance' in line or '## 验收' in line:
            in_criteria_section = True
            continue
        if in_criteria_section:
            if line.startswith('##'):
                break
            if line.strip().startswith('- ['):
                acceptance_criteria.append(line.strip())
    
    return {
        "id": task_id,
        "name": name,
        "status": status,
        "acceptance_criteria": acceptance_criteria
    }

def find_current_task(roadmap_data):
    """从 ROADMAP 数据中确定当前任务"""
    # 优先返回进行中的任务
    if roadmap_data["in_progress"]:
        return roadmap_data["in_progress"][0]
    # 否则返回第一个待处理的任务
    if roadmap_data["pending"]:
        return roadmap_data["pending"][0]
    return None

# ═══════════════════════════════════════════════════════════════════
# 同步函数
# ═══════════════════════════════════════════════════════════════════

def sync_from_roadmap(file_path):
    """从 ROADMAP.md 同步进度到 memory.json"""
    content = read_file(file_path)
    roadmap_data = parse_roadmap(content)
    
    if not roadmap_data:
        return False
    
    memory = read_json(FILES["memory"])
    
    # 更新进度
    memory["progress"] = {
        "tasks_completed": len(roadmap_data["completed"]),
        "tasks_total": roadmap_data["total"],
        "tasks_pending": len(roadmap_data["pending"]),
        "tasks_in_progress": len(roadmap_data["in_progress"]),
        "current_phase": roadmap_data["current_phase"],
        "last_synced": datetime.now().isoformat()
    }
    
    # 确定当前任务
    current = find_current_task(roadmap_data)
    if current:
        task_id = current.get("id")
        if task_id and task_id != memory.get("current_task", {}).get("id"):
            # 任务变更，更新 current_task
            memory["current_task"] = {
                "id": task_id,
                "name": current.get("line", "").replace("- [ ]", "").replace("- [>]", "").strip()[:100],
                "status": "IN_PROGRESS" if current in roadmap_data["in_progress"] else "PENDING",
                "started_at": datetime.now().isoformat() if current in roadmap_data["in_progress"] else None,
                "retry_count": 0,
                "max_retries": 5
            }
            log_decision(f"SYNC: Current task updated to {task_id}")
    
    # 如果所有任务完成
    if len(roadmap_data["pending"]) == 0 and len(roadmap_data["in_progress"]) == 0:
        memory["current_task"] = {
            "id": None,
            "status": "ALL_COMPLETED",
            "completed_at": datetime.now().isoformat()
        }
        memory["next_action"] = {
            "action": "FINALIZE",
            "target": "Generate completion report",
            "reason": "All tasks in ROADMAP completed"
        }
        log_decision("SYNC: All tasks completed!")
    
    write_json(FILES["memory"], memory)
    return True

def sync_from_task_file(file_path):
    """从任务文件同步状态"""
    # 从文件名提取任务 ID
    filename = os.path.basename(file_path)
    task_match = re.search(r'(TASK-\d+)', filename)
    if not task_match:
        return False
    
    task_id = task_match.group(1)
    content = read_file(file_path)
    task_data = parse_task_file(content, task_id)
    
    if not task_data:
        return False
    
    memory = read_json(FILES["memory"])
    
    # 检查是否是当前任务
    if memory.get("current_task", {}).get("id") == task_id:
        # 更新当前任务状态
        memory["current_task"].update({
            "name": task_data["name"],
            "status": task_data["status"],
            "last_updated": datetime.now().isoformat()
        })
        
        # 检查验收标准完成情况
        criteria = task_data.get("acceptance_criteria", [])
        completed = sum(1 for c in criteria if "[x]" in c.lower())
        total = len(criteria)
        
        if total > 0:
            memory["current_task"]["acceptance_progress"] = f"{completed}/{total}"
            
            if completed == total:
                log_decision(f"SYNC: Task {task_id} all acceptance criteria met!")
        
        write_json(FILES["memory"], memory)
        log_decision(f"SYNC: Updated task {task_id} from task file")
    
    return True

def sync_from_phase_plan(file_path):
    """从阶段计划文件同步状态"""
    content = read_file(file_path)
    if not content:
        return False
    
    # 提取阶段信息
    phase_match = re.search(r'#\s*Phase\s*(\d+)[:\s]+(.+)', content)
    if not phase_match:
        return False
    
    phase_num = phase_match.group(1)
    phase_name = phase_match.group(2).strip()
    
    # 提取阶段状态
    status_match = re.search(r'##\s*Status[:\s]+(\w+)', content, re.IGNORECASE)
    status = status_match.group(1) if status_match else "Unknown"
    
    memory = read_json(FILES["memory"])
    memory["progress"] = memory.get("progress", {})
    memory["progress"]["current_phase"] = f"Phase {phase_num}"
    memory["progress"]["current_phase_name"] = phase_name
    memory["progress"]["current_phase_status"] = status
    
    write_json(FILES["memory"], memory)
    log_decision(f"SYNC: Updated current phase to Phase {phase_num}: {phase_name}")
    
    return True

# ═══════════════════════════════════════════════════════════════════
# 主逻辑
# ═══════════════════════════════════════════════════════════════════

def main():
    # 读取输入
    input_data = json.loads(sys.stdin.read())
    
    # 获取修改的文件路径
    tool_input = input_data.get("tool_input", {})
    file_path = tool_input.get("file_path") or tool_input.get("path", "")
    
    if not file_path:
        print(json.dumps({"status": "ok", "action": "none"}))
        return
    
    # 规范化路径
    file_path = os.path.normpath(file_path)
    filename = os.path.basename(file_path)
    
    synced = False
    sync_type = None
    
    # 检测文件类型并同步
    if filename == "ROADMAP.md" or file_path.endswith("ROADMAP.md"):
        synced = sync_from_roadmap(file_path)
        sync_type = "roadmap"
    
    elif "TASK-" in filename and filename.endswith(".md"):
        synced = sync_from_task_file(file_path)
        sync_type = "task"
    
    elif filename == "PHASE_PLAN.md" or "PHASE_PLAN" in filename:
        synced = sync_from_phase_plan(file_path)
        sync_type = "phase"
    
    # 输出结果
    if synced:
        print(json.dumps({
            "status": "ok",
            "action": "synced",
            "sync_type": sync_type,
            "file": file_path,
            "message": f"Progress synced from {sync_type} file"
        }))
    else:
        print(json.dumps({
            "status": "ok", 
            "action": "none",
            "file": file_path
        }))

if __name__ == "__main__":
    main()
