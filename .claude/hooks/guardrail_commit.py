import sys
import json
import subprocess

# 读取工具输入
input_data = json.loads(sys.stdin.read())
command = input_data.get('tool_input', {}).get('command', '')

# 只有在尝试 git commit 时才触发审查
if 'git commit' not in command:
    print(json.dumps({"permissionDecision": "allow"}))
    sys.exit(0)

# 执行 Codex 审查 (假设 codex CLI 已安装)
# 你可以替换为你实际的 codex 命令，例如: codex review --diff
try:
    # 模拟运行 codex，实际使用时请去掉 echo 并换成真实命令
    # result = subprocess.run(['codex', 'review', '--diff'], capture_output=True, text=True)
    # 这里为了演示，假设我们有一个 check_code.sh 脚本
    print(json.dumps({"permissionDecision": "allow"}), file=sys.stderr) # 调试日志
    
    # 真实逻辑：
    # if result.returncode!= 0:
    #     failure_reason = f"Codex Review FAILED:\n{result.stderr}\n\nFIX THE CODE BEFORE COMMITTING."
    #     print(json.dumps({
    #         "permissionDecision": "deny",
    #         "permissionDecisionReason": failure_reason
    #     }))
    # else:
    #     print(json.dumps({"permissionDecision": "allow"}))

    # 暂时全部放行，请根据你的环境取消上面的注释
    print(json.dumps({"permissionDecision": "allow"}))

except Exception as e:
    # 故障安全：如果审查工具挂了，暂时阻止提交以防万一
    print(json.dumps({
        "permissionDecision": "deny", 
        "permissionDecisionReason": f"Review tool error: {str(e)}"
    }))