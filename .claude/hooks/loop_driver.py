import sys
import json
import os

input_data = json.loads(sys.stdin.read())
roadmap_file = ".claude/status/ROADMAP.md"

# 如果 ROADMAP 不存在，说明项目还没开始，允许停止（或提示初始化）
if not os.path.exists(roadmap_file):
    print(json.dumps({
        "decision": "stop", 
        "reason": "Roadmap not found. Please initialize project."
    }))
    sys.exit(0)

# 检查是否还有未完成的任务
with open(roadmap_file, 'r') as f:
    content = f.read()

if "- [ ]" in content:
    # 还有任务，阻止停止，强制继续
    print(json.dumps({
        "decision": "block",
        "reason": "⚠️ AUTONOMOUS MODE ACTIVE: Tasks remain in ROADMAP.md. You must proceed to the next unchecked task. Do not stop. Check.claude/status/memory.json for context."
    }))
else:
    # 所有任务完成，允许停止
    print(json.dumps({
        "decision": "stop",
        "reason": "All tasks completed successfully."
    }))