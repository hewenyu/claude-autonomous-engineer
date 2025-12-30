import sys
import json
import os

# 读取标准输入（必须消费 stdin 否则 hook 会卡住）
input_data = sys.stdin.read()

status_file = ".claude/status/memory.json"
roadmap_file = ".claude/status/ROADMAP.md"

context = "\n\n=== SYSTEM CONTEXT INJECTION ===\n"

# 1. 注入当前状态
if os.path.exists(status_file):
    with open(status_file, 'r') as f:
        context += f"Current State: {f.read()}\n"
else:
    context += "Current State: NOT_STARTED\n"

# 2. 注入 Roadmap 摘要 (只取未完成的任务)
if os.path.exists(roadmap_file):
    context += "Pending Tasks from ROADMAP:\n"
    with open(roadmap_file, 'r') as f:
        lines = f.readlines()
        for line in lines:
            if "[ ]" in line: # 只显示未勾选的任务
                context += line

context += "================================\n"
context += "INSTRUCTION: Continue the loop based on the state above. Do not ask for user input."

# 输出 JSON 给 Claude
print(json.dumps({
    "hookSpecificOutput": {
        "additionalContext": context
    }
}))