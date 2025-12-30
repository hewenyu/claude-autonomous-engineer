#!/usr/bin/env python3
"""
Enhanced Context Injection System v2.0
多层上下文注入系统 - 解决长时间自动化执行的上下文丢失问题

设计原则：
1. 分层注入 - 按优先级注入不同类型的上下文
2. 智能摘要 - 对大文件生成结构化摘要而非全文
3. 增量感知 - 重点注入最近变更的内容
4. 错误记忆 - 特别强调历史错误和解决方案
"""

import sys
import json
import os
import hashlib
from pathlib import Path
from datetime import datetime
import subprocess

# 配置
STATUS_DIR = ".claude/status"
MEMORY_FILE = f"{STATUS_DIR}/memory.json"
ROADMAP_FILE = f"{STATUS_DIR}/ROADMAP.md"
CONTRACT_FILE = f"{STATUS_DIR}/api_contract.yaml"
ERROR_LOG_FILE = f"{STATUS_DIR}/error_history.json"
CODE_DIGEST_FILE = f"{STATUS_DIR}/code_digest.json"
CONTEXT_BUDGET = 50000  # 字符预算（可调整）

def read_file_safe(path):
    """安全读取文件"""
    try:
        if os.path.exists(path):
            with open(path, 'r', encoding='utf-8') as f:
                return f.read()
    except Exception as e:
        return f"[Error reading {path}: {e}]"
    return None

def get_git_recent_changes(limit=10):
    """获取最近的 Git 变更摘要"""
    try:
        result = subprocess.run(
            ['git', 'log', f'-{limit}', '--oneline', '--name-status'],
            capture_output=True, text=True, timeout=5
        )
        if result.returncode == 0:
            return result.stdout[:2000]  # 限制长度
    except:
        pass
    return None

def get_project_structure(root_path='.', max_depth=3):
    """生成项目结构树（带函数签名摘要）"""
    structure = []
    
    def scan_dir(path, depth=0):
        if depth > max_depth:
            return
        
        try:
            items = sorted(os.listdir(path))
        except PermissionError:
            return
            
        for item in items:
            # 跳过隐藏文件和常见忽略目录
            if item.startswith('.') or item in ['node_modules', '__pycache__', 'venv', '.git', 'dist', 'build']:
                continue
                
            full_path = os.path.join(path, item)
            indent = "  " * depth
            
            if os.path.isdir(full_path):
                structure.append(f"{indent}📁 {item}/")
                scan_dir(full_path, depth + 1)
            elif item.endswith(('.py', '.js', '.ts', '.jsx', '.tsx', '.go', '.rs')):
                # 对代码文件提取签名摘要
                signatures = extract_signatures(full_path)
                structure.append(f"{indent}📄 {item}")
                for sig in signatures[:5]:  # 每个文件最多5个签名
                    structure.append(f"{indent}   └─ {sig}")
    
    scan_dir(root_path)
    return "\n".join(structure[:100])  # 限制行数

def extract_signatures(file_path):
    """提取文件中的函数/类签名"""
    signatures = []
    try:
        with open(file_path, 'r', encoding='utf-8') as f:
            content = f.read()
            
        # Python
        if file_path.endswith('.py'):
            import re
            # 匹配函数定义
            for match in re.finditer(r'^(async\s+)?def\s+(\w+)\s*\([^)]*\)(?:\s*->\s*[^:]+)?:', content, re.MULTILINE):
                signatures.append(f"def {match.group(2)}(...)")
            # 匹配类定义
            for match in re.finditer(r'^class\s+(\w+)(?:\([^)]*\))?:', content, re.MULTILINE):
                signatures.append(f"class {match.group(1)}")
                
        # JavaScript/TypeScript
        elif file_path.endswith(('.js', '.ts', '.jsx', '.tsx')):
            import re
            for match in re.finditer(r'(?:export\s+)?(?:async\s+)?function\s+(\w+)\s*\(', content):
                signatures.append(f"function {match.group(1)}()")
            for match in re.finditer(r'(?:export\s+)?class\s+(\w+)', content):
                signatures.append(f"class {match.group(1)}")
            for match in re.finditer(r'(?:const|let)\s+(\w+)\s*=\s*(?:async\s+)?\([^)]*\)\s*=>', content):
                signatures.append(f"const {match.group(1)} = () =>")
                
    except Exception:
        pass
    return signatures

def generate_error_context():
    """生成错误历史上下文"""
    error_data = read_file_safe(ERROR_LOG_FILE)
    if not error_data:
        return ""
    
    try:
        errors = json.loads(error_data)
        if not errors:
            return ""
        
        # 只保留最近10个错误
        recent_errors = errors[-10:]
        
        context = "\n## ⚠️ ERROR HISTORY (MUST AVOID REPEATING)\n"
        for err in recent_errors:
            context += f"""
### Error at {err.get('timestamp', 'unknown')}
- Task: {err.get('task', 'unknown')}
- Error: {err.get('error', 'unknown')}
- Attempted Fix: {err.get('attempted_fix', 'N/A')}
- Resolution: {err.get('resolution', 'UNRESOLVED')}
"""
        return context
    except:
        return ""

def generate_contract_summary():
    """生成 API 契约摘要"""
    contract = read_file_safe(CONTRACT_FILE)
    if not contract:
        return ""
    
    # 如果契约太长，只保留关键部分
    if len(contract) > 5000:
        lines = contract.split('\n')
        # 保留前100行和最后50行
        summary = '\n'.join(lines[:100]) + "\n\n... [TRUNCATED] ...\n\n" + '\n'.join(lines[-50:])
        return f"\n## 📜 API CONTRACT (Summary)\n```yaml\n{summary}\n```\n"
    
    return f"\n## 📜 API CONTRACT (Full)\n```yaml\n{contract}\n```\n"

def generate_active_files_context():
    """生成当前活跃文件的详细上下文"""
    memory = read_file_safe(MEMORY_FILE)
    if not memory:
        return ""
    
    try:
        state = json.loads(memory)
        active_files = state.get('active_files', [])
        current_file = state.get('current_file')
        
        if current_file and current_file not in active_files:
            active_files.insert(0, current_file)
        
        if not active_files:
            return ""
        
        context = "\n## 📂 ACTIVE FILES CONTENT\n"
        total_chars = 0
        max_chars = 15000  # 活跃文件的字符预算
        
        for file_path in active_files[:5]:  # 最多5个文件
            content = read_file_safe(file_path)
            if content:
                # 如果文件太大，只保留头尾
                if len(content) > 3000:
                    lines = content.split('\n')
                    content = '\n'.join(lines[:50]) + "\n\n... [TRUNCATED] ...\n\n" + '\n'.join(lines[-30:])
                
                if total_chars + len(content) > max_chars:
                    break
                    
                context += f"\n### {file_path}\n```\n{content}\n```\n"
                total_chars += len(content)
        
        return context
    except:
        return ""

def generate_pending_tasks():
    """生成待处理任务列表（增强版）"""
    roadmap = read_file_safe(ROADMAP_FILE)
    if not roadmap:
        return "\n## ❌ ROADMAP NOT FOUND - Initialize .claude/status/ROADMAP.md first!\n"
    
    context = "\n## 📋 PENDING TASKS\n"
    
    lines = roadmap.split('\n')
    pending = []
    in_progress = []
    completed_count = 0
    
    for line in lines:
        stripped = line.lstrip()
        if stripped.startswith('- [ ]'):
            pending.append(line)
        elif stripped.startswith('- [x]') or stripped.startswith('- [X]'):
            completed_count += 1
        # 检测正在进行的任务（自定义标记）
        elif stripped.startswith('- [>]') or stripped.startswith('- [~]'):
            in_progress.append(line)
    
    total = len(pending) + completed_count + len(in_progress)
    
    context += f"\nProgress: {completed_count}/{total} completed\n"
    
    if in_progress:
        context += "\n### 🔄 IN PROGRESS:\n"
        for task in in_progress:
            context += f"{task}\n"
    
    context += "\n### ⏳ PENDING:\n"
    for task in pending[:15]:  # 只显示前15个待处理任务
        context += f"{task}\n"
    
    if len(pending) > 15:
        context += f"\n... and {len(pending) - 15} more tasks\n"
    
    return context

def generate_memory_state():
    """生成当前状态摘要"""
    memory = read_file_safe(MEMORY_FILE)
    if not memory:
        return """
## 🧠 CURRENT STATE
```json
{
  "status": "NOT_STARTED",
  "message": "Initialize .claude/status/memory.json to begin"
}
```
"""
    
    try:
        state = json.loads(memory)
        # 美化输出
        formatted = json.dumps(state, indent=2, ensure_ascii=False)
        return f"\n## 🧠 CURRENT STATE\n```json\n{formatted}\n```\n"
    except:
        return f"\n## 🧠 CURRENT STATE (RAW)\n```\n{memory}\n```\n"

def generate_recent_decisions():
    """生成最近的决策日志（如果存在）"""
    decisions_file = f"{STATUS_DIR}/decisions.log"
    content = read_file_safe(decisions_file)
    if not content:
        return ""
    
    # 只保留最近20行
    lines = content.strip().split('\n')
    recent = lines[-20:]
    return f"\n## 📝 RECENT DECISIONS\n```\n" + '\n'.join(recent) + "\n```\n"

def main():
    # 读取标准输入（必须消费）
    input_data = sys.stdin.read()
    
    # 构建分层上下文
    context_parts = []
    
    # Layer 0: 系统指令（最高优先级）
    context_parts.append("""
╔══════════════════════════════════════════════════════════════════╗
║           🤖 AUTONOMOUS MODE CONTEXT INJECTION                   ║
╠══════════════════════════════════════════════════════════════════╣
║  WARNING: Your conversation history may be compressed/truncated  ║
║  TRUST ONLY the state files below, NOT your "memory"             ║
║  CONTINUE the loop - do NOT stop until ROADMAP is complete       ║
╚══════════════════════════════════════════════════════════════════╝
""")
    
    # Layer 1: 当前状态（关键）
    context_parts.append(generate_memory_state())
    
    # Layer 2: 待处理任务
    context_parts.append(generate_pending_tasks())
    
    # Layer 3: 错误历史（防止重复错误）
    context_parts.append(generate_error_context())
    
    # Layer 4: API 契约
    context_parts.append(generate_contract_summary())
    
    # Layer 5: 活跃文件内容
    context_parts.append(generate_active_files_context())
    
    # Layer 6: 项目结构（如果还有预算）
    current_length = sum(len(p) for p in context_parts)
    if current_length < CONTEXT_BUDGET - 5000:
        context_parts.append(f"\n## 🏗️ PROJECT STRUCTURE\n```\n{get_project_structure()}\n```\n")
    
    # Layer 7: 最近的 Git 变更
    git_changes = get_git_recent_changes()
    if git_changes and current_length < CONTEXT_BUDGET - 2000:
        context_parts.append(f"\n## 📜 RECENT GIT CHANGES\n```\n{git_changes}\n```\n")
    
    # Layer 8: 最近决策
    context_parts.append(generate_recent_decisions())
    
    # 合并上下文
    full_context = ''.join(context_parts)
    
    # 添加行动指令
    full_context += """

═══════════════════════════════════════════════════════════════════
📌 MANDATORY ACTIONS:
1. Read the CURRENT STATE above carefully
2. Check ERROR HISTORY to avoid repeating mistakes
3. Pick the NEXT pending task from ROADMAP
4. Execute following TDD (test first, then implement)
5. Update memory.json IMMEDIATELY after any progress
6. Continue loop - DO NOT STOP until all tasks are [x] marked
═══════════════════════════════════════════════════════════════════
"""
    
    # 输出 JSON
    print(json.dumps({
        "hookSpecificOutput": {
            "additionalContext": full_context
        }
    }))

if __name__ == "__main__":
    main()
