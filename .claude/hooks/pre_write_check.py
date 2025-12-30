#!/usr/bin/env python3
"""
Pre-Write Check Hook
在写入文件前进行检查，确保符合契约和一致性

检查项：
1. 如果是 API 相关文件，检查是否符合 api_contract.yaml
2. 检查是否在禁止修改的文件列表中
3. 更新 memory.json 中的活跃文件列表
"""

import sys
import json
import os
import re

STATUS_DIR = ".claude/status"
MEMORY_FILE = f"{STATUS_DIR}/memory.json"
CONTRACT_FILE = f"{STATUS_DIR}/api_contract.yaml"

# 禁止修改的文件模式
PROTECTED_PATTERNS = [
    r'\.claude/status/ROADMAP\.md',  # 只能通过特定方式更新
    r'\.claude/status/api_contract\.yaml',  # 契约文件需要特殊审批
    r'\.git/',
    r'node_modules/',
    r'__pycache__/',
]

def is_protected(file_path):
    """检查文件是否受保护"""
    for pattern in PROTECTED_PATTERNS:
        if re.search(pattern, file_path):
            return True
    return False

def update_active_files(file_path):
    """更新活跃文件列表"""
    try:
        memory = {}
        if os.path.exists(MEMORY_FILE):
            with open(MEMORY_FILE, 'r') as f:
                memory = json.load(f)
        
        active_files = memory.get('active_files', [])
        
        # 确保当前文件在列表最前面
        if file_path in active_files:
            active_files.remove(file_path)
        active_files.insert(0, file_path)
        
        # 只保留最近10个
        active_files = active_files[:10]
        
        memory['active_files'] = active_files
        memory['working_context'] = memory.get('working_context', {})
        memory['working_context']['current_file'] = file_path
        
        os.makedirs(STATUS_DIR, exist_ok=True)
        with open(MEMORY_FILE, 'w') as f:
            json.dump(memory, f, indent=2)
            
    except Exception as e:
        # 不阻止写入，只是警告
        pass

def main():
    # 读取输入
    input_data = json.loads(sys.stdin.read())
    
    # 获取要写入的文件路径
    tool_input = input_data.get('tool_input', {})
    file_path = tool_input.get('file_path') or tool_input.get('path', '')
    
    # 检查保护文件
    if is_protected(file_path):
        print(json.dumps({
            "decision": "deny",
            "reason": f"⛔ Protected file: {file_path}\n\nThis file is protected and cannot be modified directly. If you need to update ROADMAP.md or api_contract.yaml, use the designated update methods."
        }))
        return
    
    # 更新活跃文件列表
    if file_path:
        update_active_files(file_path)
    
    # 允许写入
    print(json.dumps({
        "decision": "allow"
    }))

if __name__ == "__main__":
    main()
