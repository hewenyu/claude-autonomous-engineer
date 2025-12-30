#!/usr/bin/env python3
"""
Post-Write Update Hook
在文件写入后更新状态

功能：
1. 更新 code_digest.json 中该文件的摘要
2. 记录变更历史
3. 触发增量分析
"""

import sys
import json
import os
import hashlib
import re
from datetime import datetime

STATUS_DIR = ".claude/status"
MEMORY_FILE = f"{STATUS_DIR}/memory.json"
CHANGES_FILE = f"{STATUS_DIR}/recent_changes.json"

def get_file_hash(content):
    """获取内容的 hash"""
    return hashlib.md5(content.encode()).hexdigest()[:8]

def extract_quick_summary(file_path):
    """快速提取文件摘要"""
    try:
        with open(file_path, 'r', encoding='utf-8', errors='ignore') as f:
            content = f.read()
        
        # 提取函数名
        functions = []
        if file_path.endswith('.py'):
            for match in re.finditer(r'^(?:async\s+)?def\s+(\w+)', content, re.MULTILINE):
                functions.append(match.group(1))
        elif file_path.endswith(('.js', '.ts', '.jsx', '.tsx')):
            for match in re.finditer(r'(?:function\s+|const\s+)(\w+)\s*[=(]', content):
                functions.append(match.group(1))
        
        return {
            'lines': content.count('\n') + 1,
            'hash': get_file_hash(content),
            'functions': functions[:10]
        }
    except:
        return None

def record_change(file_path, operation='write'):
    """记录变更"""
    changes = []
    if os.path.exists(CHANGES_FILE):
        try:
            with open(CHANGES_FILE, 'r') as f:
                changes = json.load(f)
        except:
            pass
    
    summary = extract_quick_summary(file_path)
    
    changes.append({
        'timestamp': datetime.now().isoformat(),
        'file': file_path,
        'operation': operation,
        'summary': summary
    })
    
    # 只保留最近50条
    changes = changes[-50:]
    
    os.makedirs(STATUS_DIR, exist_ok=True)
    with open(CHANGES_FILE, 'w') as f:
        json.dump(changes, f, indent=2)

def update_memory_checkpoint(file_path):
    """在 memory.json 中添加检查点"""
    try:
        memory = {}
        if os.path.exists(MEMORY_FILE):
            with open(MEMORY_FILE, 'r') as f:
                memory = json.load(f)
        
        checkpoints = memory.get('checkpoints', [])
        checkpoints.append({
            'timestamp': datetime.now().isoformat(),
            'action': 'file_written',
            'file': file_path
        })
        
        # 只保留最近20个检查点
        memory['checkpoints'] = checkpoints[-20:]
        
        with open(MEMORY_FILE, 'w') as f:
            json.dump(memory, f, indent=2)
    except:
        pass

def main():
    # 读取输入
    input_data = json.loads(sys.stdin.read())
    
    # 获取文件路径
    tool_input = input_data.get('tool_input', {})
    file_path = tool_input.get('file_path') or tool_input.get('path', '')
    
    if file_path and os.path.exists(file_path):
        # 记录变更
        record_change(file_path)
        
        # 更新检查点
        update_memory_checkpoint(file_path)
    
    # Post hooks 不需要返回 decision
    print(json.dumps({"status": "ok"}))

if __name__ == "__main__":
    main()
