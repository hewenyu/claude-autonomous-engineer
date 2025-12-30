#!/usr/bin/env python3
"""
Inject State Hook v3.0
使用统一上下文管理器注入完整上下文

放入 .claude/hooks/inject_state.py
"""

import sys
import json
import os

# 添加 lib 目录到路径
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '..', 'lib'))

from context_manager import ContextManager

def main():
    # 必须消费 stdin
    input_data = sys.stdin.read()
    
    # 获取完整上下文
    ctx = ContextManager()
    full_context = ctx.get_full_context()
    
    # 输出 JSON
    print(json.dumps({
        "hookSpecificOutput": {
            "additionalContext": full_context
        }
    }))

if __name__ == "__main__":
    main()
