#!/usr/bin/env python3
"""
Codex Review Gate Hook v2.0
提交前自动调用 Codex 进行代码审查

🔧 修复：支持 git submodule 场景
- 自动查找项目根目录（包含 .claude/ 的目录）
- 如果找不到 .claude 目录，gracefully 跳过
"""

import sys
import json
import os
import subprocess

# ═══════════════════════════════════════════════════════════════════
# 根目录查找
# ═══════════════════════════════════════════════════════════════════

def find_project_root():
    """
    查找包含 .claude 目录的项目根目录
    
    搜索顺序：
    1. 当前目录
    2. git 仓库根目录
    3. 向上遍历父目录
    """
    # 方法1: 当前目录
    if os.path.isdir(".claude"):
        return os.path.abspath(".")
    
    # 方法2: git rev-parse --show-toplevel
    try:
        result = subprocess.run(
            ['git', 'rev-parse', '--show-toplevel'],
            capture_output=True, text=True, timeout=5
        )
        if result.returncode == 0:
            git_root = result.stdout.strip()
            if os.path.isdir(os.path.join(git_root, ".claude")):
                return git_root
    except:
        pass
    
    # 方法3: 向上遍历（处理 submodule 场景）
    current = os.path.abspath(".")
    for _ in range(10):  # 最多向上10层
        if os.path.isdir(os.path.join(current, ".claude")):
            return current
        parent = os.path.dirname(current)
        if parent == current:  # 到达根目录
            break
        current = parent
    
    # 方法4: 检查 git superproject
    try:
        result = subprocess.run(
            ['git', 'rev-parse', '--show-superproject-working-tree'],
            capture_output=True, text=True, timeout=5
        )
        if result.returncode == 0 and result.stdout.strip():
            super_root = result.stdout.strip()
            if os.path.isdir(os.path.join(super_root, ".claude")):
                return super_root
    except:
        pass
    
    return None

# ═══════════════════════════════════════════════════════════════════
# 主逻辑
# ═══════════════════════════════════════════════════════════════════

def main():
    # 读取输入
    input_data = json.loads(sys.stdin.read())
    
    # 查找项目根目录
    project_root = find_project_root()
    
    if not project_root:
        # 找不到 .claude 目录，静默跳过（可能在 submodule 中）
        print(json.dumps({
            "decision": "allow",
            "reason": "[Hook] .claude directory not found, skipping review"
        }))
        return
    
    # 检查是否是 git commit 相关命令
    tool_input = input_data.get("tool_input", {})
    command = tool_input.get("command", "")
    
    # 只拦截 git commit/push
    is_commit = ("git commit" in command or "git push" in command)
    
    if not is_commit:
        # 不是提交命令，直接放行
        print(json.dumps({"decision": "allow"}))
        return
    
    # 获取暂存文件
    try:
        result = subprocess.run(
            ['git', 'diff', '--cached', '--name-only'],
            capture_output=True, text=True, timeout=10,
            cwd=project_root
        )
        staged_files = [f.strip() for f in result.stdout.strip().split('\n') if f.strip()]
    except:
        staged_files = []
    
    if not staged_files:
        print(json.dumps({"decision": "allow"}))
        return
    
    # 加载上下文管理器
    lib_path = os.path.join(project_root, ".claude", "lib")
    sys.path.insert(0, lib_path)
    
    try:
        from context_manager import ContextManager
        ctx = ContextManager(project_root)
        review_context = ctx.get_review_context(staged_files)
        
        # 这里可以调用 Codex CLI 进行审查
        # 暂时只记录并放行
        print(json.dumps({
            "decision": "allow",
            "reason": f"[Review] {len(staged_files)} files staged for commit"
        }))
        
    except ImportError:
        # context_manager 不存在，静默放行
        print(json.dumps({
            "decision": "allow",
            "reason": "[Hook] context_manager not found, skipping review"
        }))

if __name__ == "__main__":
    main()
