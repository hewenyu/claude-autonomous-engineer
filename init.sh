#!/bin/bash
#
# Autonomous Engineering System - Initialization Script
# 初始化自主工程系统
#
# Usage: ./init.sh [project_name]
#

set -e

PROJECT_NAME="${1:-my-project}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "╔══════════════════════════════════════════════════════════════════╗"
echo "║       Autonomous Engineering System - Initialization             ║"
echo "╚══════════════════════════════════════════════════════════════════╝"
echo ""

# 创建目录结构
echo "📁 Creating directory structure..."
mkdir -p .claude/status
mkdir -p .claude/phases
mkdir -p .claude/lib
mkdir -p .claude/hooks
mkdir -p .claude/agents

# 复制核心文件（如果不存在）
echo "📋 Setting up core files..."

# 检查 Python
if ! command -v python3 &> /dev/null; then
    echo "❌ Python 3 is required but not installed."
    exit 1
fi

# 初始化 memory.json
if [ ! -f .claude/status/memory.json ]; then
    cat > .claude/status/memory.json << 'EOF'
{
  "_schema_version": "3.0",
  "session": {"started_at": null, "loop_count": 0},
  "current_task": {"id": null, "status": "NOT_STARTED", "retry_count": 0, "max_retries": 5},
  "working_context": {"current_file": null, "pending_tests": [], "pending_implementations": []},
  "active_files": [],
  "progress": {"tasks_completed": 0, "tasks_total": 0, "current_phase": null},
  "error_state": {"last_error": null, "error_count": 0, "blocked": false},
  "next_action": {"action": "INITIALIZE", "target": "Run project-architect-supervisor"}
}
EOF
    echo "  ✓ Created memory.json"
fi

# 初始化 error_history.json
if [ ! -f .claude/status/error_history.json ]; then
    echo "[]" > .claude/status/error_history.json
    echo "  ✓ Created error_history.json"
fi

# 创建空的 decisions.log
touch .claude/status/decisions.log
echo "  ✓ Created decisions.log"

# 初始化 ROADMAP.md 模板
if [ ! -f .claude/status/ROADMAP.md ]; then
    cat > .claude/status/ROADMAP.md << EOF
# Project Roadmap: ${PROJECT_NAME}

## Overview
[Project description - to be filled by project-architect-supervisor]

## Architecture Tree
[To be generated]

## Progress
- Total Tasks: 0
- Completed: 0
- Current Phase: Not Started

## Phases

| Phase | Name | Status | Tasks |
|-------|------|--------|-------|
| - | - | - | - |

## Task List

<!-- 
Task Status Legend:
- [ ] = Pending
- [>] = In Progress  
- [x] = Completed
- [!] = Blocked
-->

[Tasks will be added by project-architect-supervisor]
EOF
    echo "  ✓ Created ROADMAP.md template"
fi

# 验证核心文件存在
echo ""
echo "📊 Verification..."

REQUIRED_FILES=(
    ".claude/CLAUDE.md"
    ".claude/settings.json"
    ".claude/lib/context_manager.py"
    ".claude/hooks/inject_state.py"
    ".claude/hooks/progress_sync.py"
    ".claude/hooks/codex_review_gate.py"
    ".claude/hooks/loop_driver.py"
    ".claude/agents/project-architect-supervisor.md"
    ".claude/agents/code-executor.md"
    ".claude/agents/codex-reviewer.md"
)

MISSING=0
for file in "${REQUIRED_FILES[@]}"; do
    if [ -f "$file" ]; then
        echo "  ✓ $file"
    else
        echo "  ✗ $file (MISSING)"
        MISSING=$((MISSING + 1))
    fi
done

echo ""
if [ $MISSING -eq 0 ]; then
    echo "✅ Initialization complete!"
    echo ""
    echo "Next steps:"
    echo "  1. Start Claude Code in this directory"
    echo "  2. Say: 'Plan the project: ${PROJECT_NAME}'"
    echo "  3. The architect will generate:"
    echo "     - ROADMAP.md with all tasks"
    echo "     - api_contract.yaml with all signatures"
    echo "     - TASK-xxx.md files for each task"
    echo "  4. Confirm to start the autonomous loop"
    echo ""
    echo "The system will then:"
    echo "  - Inject context automatically"
    echo "  - Track progress automatically"
    echo "  - Review code before commits"
    echo "  - Continue until all tasks are done"
else
    echo "⚠️  $MISSING files missing. Please ensure all core files are in place."
    echo ""
    echo "Copy the following from the improved-system-v2 directory:"
    for file in "${REQUIRED_FILES[@]}"; do
        if [ ! -f "$file" ]; then
            echo "  - $file"
        fi
    done
fi

echo ""
echo "═══════════════════════════════════════════════════════════════════"
