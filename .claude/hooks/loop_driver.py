import sys
import json
import os

input_data = json.loads(sys.stdin.read())
roadmap_file = ".claude/status/ROADMAP.md"

# 如果 ROADMAP 不存在，阻止停止并要求先初始化（否则无人值守 loop 无从继续）
if not os.path.exists(roadmap_file):
    print(json.dumps({
        "decision": "block",
        "reason": "Roadmap not found. Please initialize `.claude/status/ROADMAP.md` (and `.claude/status/memory.json`) before stopping."
    }))
    sys.exit(0)

# 检查是否还有未完成的任务
with open(roadmap_file, 'r') as f:
    lines = f.readlines()

has_unchecked = any(line.lstrip().startswith("- [ ]") for line in lines)

if has_unchecked:
    # 还有任务，阻止停止，强制继续
    print(json.dumps({
        "decision": "block",
        "reason": "⚠️ AUTONOMOUS MODE ACTIVE: Tasks remain in ROADMAP.md. Proceed to the next unchecked task. Do not stop. Check `.claude/status/memory.json` for context."
    }))
else:
    # 所有任务完成，允许停止
    print(json.dumps({
        "decision": "stop",
        "reason": "All tasks completed successfully."
    }))
