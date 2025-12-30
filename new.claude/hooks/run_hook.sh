#!/bin/bash
#
# Hook Runner - 根目录感知的 Hook 执行器
#
# 这个脚本解决了在 submodule 或子目录中执行时，
# 相对路径 .claude/hooks/xxx.py 失效的问题。
#
# 使用方式（在 settings.json 中）:
#   "command": "bash .claude/hooks/run_hook.sh inject_state"
#

HOOK_NAME="$1"

# 函数：查找项目根目录（包含 .claude 的目录）
find_project_root() {
    local current="$(pwd)"
    
    # 方法1: 当前目录
    if [ -d ".claude" ]; then
        echo "$(pwd)"
        return 0
    fi
    
    # 方法2: git 仓库根目录
    local git_root
    git_root=$(git rev-parse --show-toplevel 2>/dev/null)
    if [ -n "$git_root" ] && [ -d "$git_root/.claude" ]; then
        echo "$git_root"
        return 0
    fi
    
    # 方法3: 向上遍历
    while [ "$current" != "/" ]; do
        if [ -d "$current/.claude" ]; then
            echo "$current"
            return 0
        fi
        current=$(dirname "$current")
    done
    
    # 方法4: git superproject (submodule 的父项目)
    local super_root
    super_root=$(git rev-parse --show-superproject-working-tree 2>/dev/null)
    if [ -n "$super_root" ] && [ -d "$super_root/.claude" ]; then
        echo "$super_root"
        return 0
    fi
    
    return 1
}

# 函数：graceful 退出（根据 hook 类型返回适当的响应）
graceful_exit() {
    local hook_type="$1"
    
    case "$hook_type" in
        inject_state)
            echo '{"hookSpecificOutput":{"additionalContext":""}}'
            ;;
        codex_review_gate|pre_write_check)
            echo '{"decision":"allow"}'
            ;;
        progress_sync|post_write_update)
            echo '{"status":"ok","skipped":true}'
            ;;
        loop_driver)
            echo '{"decision":"allow","reason":"[Hook] .claude directory not found"}'
            ;;
        *)
            echo '{}'
            ;;
    esac
}

# 主逻辑
if [ -z "$HOOK_NAME" ]; then
    echo "Usage: run_hook.sh <hook_name>" >&2
    graceful_exit "unknown"
    exit 0
fi

# 查找项目根目录
PROJECT_ROOT=$(find_project_root)

if [ -z "$PROJECT_ROOT" ]; then
    # 找不到 .claude 目录，gracefully 跳过
    graceful_exit "$HOOK_NAME"
    exit 0
fi

# 构建 hook 脚本路径
HOOK_PATH="$PROJECT_ROOT/.claude/hooks/${HOOK_NAME}.py"

if [ ! -f "$HOOK_PATH" ]; then
    # Hook 脚本不存在
    graceful_exit "$HOOK_NAME"
    exit 0
fi

# 在项目根目录执行 hook
cd "$PROJECT_ROOT"
export PROJECT_ROOT
python3 "$HOOK_PATH"
