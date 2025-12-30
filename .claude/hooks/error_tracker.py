#!/usr/bin/env python3
"""
Error History Tracker
记录并管理错误历史，帮助 Claude 避免重复同样的错误

使用方式：
  python3 error_tracker.py add "任务名" "错误描述" "尝试的修复方案"
  python3 error_tracker.py resolve "任务名" "解决方案"
  python3 error_tracker.py list
  python3 error_tracker.py clear
"""

import sys
import json
import os
from datetime import datetime

STATUS_DIR = ".claude/status"
ERROR_FILE = f"{STATUS_DIR}/error_history.json"

def ensure_dir():
    os.makedirs(STATUS_DIR, exist_ok=True)

def load_errors():
    if os.path.exists(ERROR_FILE):
        with open(ERROR_FILE, 'r') as f:
            return json.load(f)
    return []

def save_errors(errors):
    ensure_dir()
    with open(ERROR_FILE, 'w') as f:
        json.dump(errors, f, indent=2, ensure_ascii=False)

def add_error(task, error, attempted_fix=None):
    errors = load_errors()
    errors.append({
        "timestamp": datetime.now().isoformat(),
        "task": task,
        "error": error,
        "attempted_fix": attempted_fix,
        "resolution": None,
        "resolved_at": None
    })
    # 只保留最近50条
    if len(errors) > 50:
        errors = errors[-50:]
    save_errors(errors)
    print(f"✅ Error recorded for task: {task}")

def resolve_error(task, resolution):
    errors = load_errors()
    for err in reversed(errors):
        if err["task"] == task and err["resolution"] is None:
            err["resolution"] = resolution
            err["resolved_at"] = datetime.now().isoformat()
            save_errors(errors)
            print(f"✅ Error resolved for task: {task}")
            return
    print(f"⚠️ No unresolved error found for task: {task}")

def list_errors():
    errors = load_errors()
    if not errors:
        print("No errors recorded.")
        return
    
    print(f"\n📋 Error History ({len(errors)} entries)\n" + "="*50)
    for i, err in enumerate(errors[-10:], 1):
        status = "✅ RESOLVED" if err.get("resolution") else "❌ UNRESOLVED"
        print(f"""
{i}. [{status}] {err['timestamp'][:19]}
   Task: {err['task']}
   Error: {err['error'][:100]}...
   Fix Attempted: {err.get('attempted_fix', 'N/A')[:50]}...
   Resolution: {err.get('resolution', 'Pending')}
""")

def clear_errors():
    save_errors([])
    print("✅ Error history cleared.")

def main():
    if len(sys.argv) < 2:
        print(__doc__)
        return
    
    cmd = sys.argv[1]
    
    if cmd == "add" and len(sys.argv) >= 4:
        add_error(sys.argv[2], sys.argv[3], sys.argv[4] if len(sys.argv) > 4 else None)
    elif cmd == "resolve" and len(sys.argv) >= 4:
        resolve_error(sys.argv[2], sys.argv[3])
    elif cmd == "list":
        list_errors()
    elif cmd == "clear":
        clear_errors()
    else:
        print(__doc__)

if __name__ == "__main__":
    main()
