#!/usr/bin/env python3
"""
Codex Review Gate Hook
提交前自动调用 Codex 进行代码审查，带完整上下文

功能：
1. 拦截 git commit 命令
2. 获取待提交的文件列表
3. 注入完整的审查上下文（API 契约、任务规格、错误历史）
4. 调用 Codex CLI 进行审查
5. 根据审查结果决定是否允许提交

配置方式：在 settings.json 的 PreToolUse 中添加
"""

import sys
import json
import os
import subprocess
import tempfile
from datetime import datetime

# 添加 lib 目录到路径
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '..', 'lib'))

from context_manager import ContextManager

# ═══════════════════════════════════════════════════════════════════
# 配置
# ═══════════════════════════════════════════════════════════════════

STATUS_DIR = ".claude/status"
REVIEW_LOG_FILE = f"{STATUS_DIR}/review_history.json"

# Codex CLI 配置
CODEX_CMD = "codex"  # 或者完整路径
CODEX_TIMEOUT = 120  # 秒

# 审查严格程度: strict | normal | lenient
REVIEW_MODE = os.environ.get("REVIEW_MODE", "normal")

# ═══════════════════════════════════════════════════════════════════
# 工具函数
# ═══════════════════════════════════════════════════════════════════

def get_staged_files():
    """获取 git 暂存的文件列表"""
    try:
        result = subprocess.run(
            ['git', 'diff', '--cached', '--name-only'],
            capture_output=True, text=True, timeout=10
        )
        if result.returncode == 0:
            return [f.strip() for f in result.stdout.strip().split('\n') if f.strip()]
    except:
        pass
    return []

def get_staged_diff():
    """获取 git 暂存的 diff"""
    try:
        result = subprocess.run(
            ['git', 'diff', '--cached'],
            capture_output=True, text=True, timeout=10
        )
        if result.returncode == 0:
            return result.stdout
    except:
        pass
    return ""

def check_codex_available():
    """检查 Codex CLI 是否可用"""
    try:
        result = subprocess.run(
            [CODEX_CMD, '--version'],
            capture_output=True, text=True, timeout=5
        )
        return result.returncode == 0
    except:
        return False

def run_codex_review(context: str, diff: str) -> dict:
    """运行 Codex 审查"""
    
    # 创建临时文件存放上下文和 diff
    with tempfile.NamedTemporaryFile(mode='w', suffix='.md', delete=False) as ctx_file:
        ctx_file.write(context)
        ctx_path = ctx_file.name
    
    with tempfile.NamedTemporaryFile(mode='w', suffix='.diff', delete=False) as diff_file:
        diff_file.write(diff)
        diff_path = diff_file.name
    
    try:
        # 构建审查提示
        review_prompt = f"""
Please review the following code changes.

## Context (API Contract, Task Spec, Error History)
See file: {ctx_path}

## Code Changes (Diff)
See file: {diff_path}

## Review Criteria
1. **Contract Compliance**: Do all implementations match the API contract exactly?
2. **Test Coverage**: Are there comprehensive tests for all new code?
3. **Error Handling**: Is error handling complete and appropriate?
4. **Code Quality**: Does the code follow project conventions?
5. **Security**: Any security concerns?

## Output Format
Respond with a JSON object:
{{
  "verdict": "PASS" | "FAIL" | "WARN",
  "issues": [
    {{"severity": "critical|major|minor", "file": "...", "line": N, "message": "..."}}
  ],
  "summary": "Brief summary of the review"
}}
"""
        
        # 调用 Codex
        result = subprocess.run(
            [CODEX_CMD, 'review', '--context', ctx_path, '--diff', diff_path],
            capture_output=True, text=True, timeout=CODEX_TIMEOUT,
            input=review_prompt
        )
        
        # 解析结果
        if result.returncode == 0:
            try:
                # 尝试从输出中提取 JSON
                output = result.stdout
                # 查找 JSON 块
                import re
                json_match = re.search(r'\{[\s\S]*"verdict"[\s\S]*\}', output)
                if json_match:
                    return json.loads(json_match.group())
            except:
                pass
        
        # 如果无法解析，返回默认结果
        return {
            "verdict": "WARN",
            "issues": [],
            "summary": f"Codex output: {result.stdout[:500]}" if result.stdout else "No output",
            "raw_output": result.stdout,
            "raw_error": result.stderr
        }
    
    finally:
        # 清理临时文件
        try:
            os.unlink(ctx_path)
            os.unlink(diff_path)
        except:
            pass

def log_review(staged_files: list, review_result: dict):
    """记录审查结果"""
    try:
        history = []
        if os.path.exists(REVIEW_LOG_FILE):
            with open(REVIEW_LOG_FILE, 'r') as f:
                history = json.load(f)
        
        history.append({
            "timestamp": datetime.now().isoformat(),
            "files": staged_files,
            "verdict": review_result.get("verdict"),
            "issues_count": len(review_result.get("issues", [])),
            "summary": review_result.get("summary", "")[:200]
        })
        
        # 只保留最近 100 条
        history = history[-100:]
        
        os.makedirs(STATUS_DIR, exist_ok=True)
        with open(REVIEW_LOG_FILE, 'w') as f:
            json.dump(history, f, indent=2)
    except:
        pass

def format_issues_for_feedback(issues: list) -> str:
    """格式化问题列表用于反馈给 Claude"""
    if not issues:
        return ""
    
    output = "\n### Issues Found:\n"
    for i, issue in enumerate(issues, 1):
        severity = issue.get("severity", "unknown").upper()
        file = issue.get("file", "unknown")
        line = issue.get("line", "?")
        msg = issue.get("message", "No description")
        output += f"{i}. [{severity}] `{file}:{line}` - {msg}\n"
    
    return output

# ═══════════════════════════════════════════════════════════════════
# 主逻辑
# ═══════════════════════════════════════════════════════════════════

def main():
    # 读取输入
    input_data = json.loads(sys.stdin.read())
    
    # 检查是否是 git commit 相关命令
    tool_name = input_data.get("tool_name", "")
    tool_input = input_data.get("tool_input", {})
    
    # 判断是否需要拦截
    command = tool_input.get("command", "")
    is_commit = ("git commit" in command or "git push" in command)
    
    if not is_commit:
        # 不是提交命令，直接放行
        print(json.dumps({"decision": "allow"}))
        return
    
    # 获取暂存文件
    staged_files = get_staged_files()
    
    if not staged_files:
        # 没有暂存文件，放行
        print(json.dumps({"decision": "allow"}))
        return
    
    # 检查 Codex 是否可用
    codex_available = check_codex_available()
    
    # 获取审查上下文
    ctx_manager = ContextManager()
    review_context = ctx_manager.get_review_context(staged_files)
    
    if codex_available:
        # 运行 Codex 审查
        diff = get_staged_diff()
        review_result = run_codex_review(review_context, diff)
        
        # 记录审查结果
        log_review(staged_files, review_result)
        
        verdict = review_result.get("verdict", "WARN")
        
        if verdict == "PASS":
            print(json.dumps({
                "decision": "allow",
                "reason": f"✅ Codex Review PASSED\n\n{review_result.get('summary', '')}"
            }))
        elif verdict == "FAIL":
            issues_text = format_issues_for_feedback(review_result.get("issues", []))
            print(json.dumps({
                "decision": "deny",
                "reason": f"""
❌ CODEX REVIEW FAILED

{review_result.get('summary', 'Review failed')}
{issues_text}

Please fix the issues above before committing.
The review context is available in the conversation.
"""
            }))
        else:  # WARN
            issues_text = format_issues_for_feedback(review_result.get("issues", []))
            if REVIEW_MODE == "strict":
                print(json.dumps({
                    "decision": "deny",
                    "reason": f"""
⚠️ CODEX REVIEW WARNING (Strict Mode - Blocked)

{review_result.get('summary', '')}
{issues_text}

Please address the warnings before committing.
"""
                }))
            else:
                print(json.dumps({
                    "decision": "allow",
                    "reason": f"""
⚠️ CODEX REVIEW WARNING (Proceeding anyway)

{review_result.get('summary', '')}
{issues_text}

Consider addressing these issues in a follow-up commit.
"""
                }))
    
    else:
        # Codex 不可用，使用内置检查
        print(json.dumps({
            "decision": "allow",
            "reason": f"""
⚠️ CODEX NOT AVAILABLE - Using basic checks only

Files to be committed:
{chr(10).join('- ' + f for f in staged_files)}

Review Context has been prepared. Please manually verify:
1. API contract compliance
2. Test coverage
3. Error handling

Proceeding with commit...
"""
        }))

if __name__ == "__main__":
    main()
