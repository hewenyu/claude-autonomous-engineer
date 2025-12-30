import sys
import json
import subprocess
import shutil

# 读取工具输入
input_data = json.loads(sys.stdin.read())
command = input_data.get('tool_input', {}).get('command', '')

# 只有在尝试 git commit 时才触发审查
if 'git commit' not in command:
    print(json.dumps({"permissionDecision": "allow"}))
    sys.exit(0)

# 执行 Codex 审查（要求本机已安装 `codex` CLI）
try:
    if shutil.which("codex") is None:
        print(json.dumps({
            "permissionDecision": "deny",
            "permissionDecisionReason": "Commit guardrail is enabled but `codex` CLI was not found on PATH. Install `codex` or disable this PreToolUse hook."
        }))
        sys.exit(0)

    codex_path = shutil.which("codex")
    if codex_path is not None and not shutil.os.access(codex_path, shutil.os.X_OK):
        print(json.dumps({
            "permissionDecision": "deny",
            "permissionDecisionReason": f"Commit guardrail is enabled but `codex` at `{codex_path}` is not executable. Fix permissions or disable this PreToolUse hook."
        }))
        sys.exit(0)

    result = subprocess.run(
        ["codex", "review"],
        capture_output=True,
        text=True,
    )

    if result.returncode != 0:
        failure_reason = (
            "Codex review FAILED. Fix issues and re-run `codex review`.\n\n"
            f"STDOUT:\n{result.stdout}\n\nSTDERR:\n{result.stderr}"
        )
        print(json.dumps({
            "permissionDecision": "deny",
            "permissionDecisionReason": failure_reason
        }))
        sys.exit(0)

    print(json.dumps({"permissionDecision": "allow"}))

except Exception as e:
    # 故障安全：如果审查工具挂了，暂时阻止提交以防万一
    print(json.dumps({
        "permissionDecision": "deny", 
        "permissionDecisionReason": f"Review tool error: {str(e)}"
    }))
