#!/usr/bin/env python3
"""
Hook Runner - 根目录感知的 Hook 执行器

解决问题：当在 submodule 或子目录中执行命令时，
相对路径 .claude/hooks/xxx.py 会失效。

这个脚本会：
1. 自动查找包含 .claude 目录的项目根目录
2. 切换到根目录执行指定的 hook
3. 如果找不到，gracefully 跳过

使用方式（在 settings.json 中）:
  "command": "python3 .claude/hooks/run_hook.py inject_state"
  "command": "python3 .claude/hooks/run_hook.py codex_review_gate"
  "command": "python3 .claude/hooks/run_hook.py progress_sync"
  "command": "python3 .claude/hooks/run_hook.py loop_driver"
"""

import sys
import os
import subprocess
import json

def find_project_root():
    """查找包含 .claude 目录的项目根目录"""
    
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
    
    # 方法3: 向上遍历父目录
    current = os.path.abspath(".")
    for _ in range(10):
        if os.path.isdir(os.path.join(current, ".claude")):
            return current
        parent = os.path.dirname(current)
        if parent == current:
            break
        current = parent
    
    # 方法4: git superproject (submodule 的父项目)
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

def graceful_exit(hook_type):
    """当找不到 .claude 目录时，gracefully 退出"""
    
    # 根据 hook 类型返回适当的响应
    if hook_type in ["inject_state"]:
        # UserPromptSubmit hook - 返回空上下文
        print(json.dumps({
            "hookSpecificOutput": {
                "additionalContext": ""
            }
        }))
    elif hook_type in ["codex_review_gate", "pre_write_check"]:
        # PreToolUse hook - 允许操作
        print(json.dumps({
            "decision": "allow"
        }))
    elif hook_type in ["progress_sync", "post_write_update"]:
        # PostToolUse hook - 静默成功
        print(json.dumps({
            "status": "ok",
            "skipped": True
        }))
    elif hook_type in ["loop_driver"]:
        # Stop hook - 允许停止
        print(json.dumps({
            "decision": "allow",
            "reason": "[Hook] .claude directory not found"
        }))
    else:
        # 未知类型 - 返回空 JSON
        print(json.dumps({}))

def main():
    if len(sys.argv) < 2:
        print("Usage: python3 run_hook.py <hook_name>", file=sys.stderr)
        print("Available hooks: inject_state, codex_review_gate, progress_sync, loop_driver", file=sys.stderr)
        graceful_exit("unknown")
        return
    
    hook_name = sys.argv[1]
    
    # 查找项目根目录
    project_root = find_project_root()
    
    if not project_root:
        # 找不到 .claude 目录，gracefully 跳过
        graceful_exit(hook_name)
        return
    
    # 构建 hook 脚本路径
    hook_path = os.path.join(project_root, ".claude", "hooks", f"{hook_name}.py")
    
    if not os.path.exists(hook_path):
        # Hook 脚本不存在
        graceful_exit(hook_name)
        return
    
    # 读取 stdin（hook 输入）
    stdin_data = sys.stdin.read()
    
    # 在项目根目录执行 hook
    try:
        result = subprocess.run(
            [sys.executable, hook_path],
            input=stdin_data,
            capture_output=True,
            text=True,
            timeout=180,  # 3分钟超时（Codex 审查可能较慢）
            cwd=project_root,
            env={**os.environ, "PROJECT_ROOT": project_root}
        )
        
        # 输出 hook 的结果
        print(result.stdout, end='')
        
        if result.stderr:
            print(result.stderr, file=sys.stderr)
        
    except subprocess.TimeoutExpired:
        print(json.dumps({
            "decision": "allow",
            "reason": f"[Hook] {hook_name} timed out"
        }))
    except Exception as e:
        print(json.dumps({
            "decision": "allow",
            "reason": f"[Hook] Error running {hook_name}: {e}"
        }))

if __name__ == "__main__":
    main()
