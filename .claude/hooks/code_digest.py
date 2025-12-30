#!/usr/bin/env python3
"""
Code Digest Generator
生成代码库的结构化摘要，帮助 Claude 在上下文压缩后恢复对代码库的理解

功能：
1. 扫描代码库结构
2. 提取函数/类签名
3. 识别依赖关系
4. 生成可读的摘要文件

使用方式：
  python3 code_digest.py generate [root_path]
  python3 code_digest.py update <file_path>  # 更新单个文件的摘要
"""

import sys
import os
import json
import re
import hashlib
from pathlib import Path
from datetime import datetime

STATUS_DIR = ".claude/status"
DIGEST_FILE = f"{STATUS_DIR}/code_digest.json"

# 忽略的目录和文件
IGNORE_DIRS = {
    '.git', '__pycache__', 'node_modules', 'venv', '.venv', 
    'env', '.env', 'dist', 'build', '.claude', 'coverage',
    '.pytest_cache', '.mypy_cache', 'eggs', '*.egg-info'
}

IGNORE_FILES = {
    '.DS_Store', 'Thumbs.db', '*.pyc', '*.pyo', '*.class',
    '*.so', '*.dll', '*.exe', 'package-lock.json', 'yarn.lock'
}

# 支持的代码文件类型
CODE_EXTENSIONS = {
    '.py': 'python',
    '.js': 'javascript',
    '.ts': 'typescript',
    '.jsx': 'javascript',
    '.tsx': 'typescript',
    '.go': 'go',
    '.rs': 'rust',
    '.java': 'java',
    '.cpp': 'cpp',
    '.c': 'c',
    '.h': 'c',
    '.hpp': 'cpp',
    '.rb': 'ruby',
    '.php': 'php',
}

def should_ignore(path):
    """检查是否应该忽略这个路径"""
    name = os.path.basename(path)
    
    for pattern in IGNORE_DIRS | IGNORE_FILES:
        if '*' in pattern:
            if name.endswith(pattern.replace('*', '')):
                return True
        elif name == pattern:
            return True
    
    return False

def get_file_hash(content):
    """获取内容的 hash"""
    return hashlib.md5(content.encode()).hexdigest()[:8]

def extract_python_signatures(content):
    """提取 Python 函数和类签名"""
    signatures = []
    
    # 类定义
    for match in re.finditer(r'^class\s+(\w+)(?:\(([^)]*)\))?:', content, re.MULTILINE):
        class_name = match.group(1)
        bases = match.group(2) or ''
        signatures.append({
            'type': 'class',
            'name': class_name,
            'signature': f"class {class_name}({bases})" if bases else f"class {class_name}",
            'line': content[:match.start()].count('\n') + 1
        })
    
    # 函数定义
    for match in re.finditer(
        r'^(\s*)(async\s+)?def\s+(\w+)\s*\(([^)]*)\)(?:\s*->\s*([^:]+))?:',
        content, re.MULTILINE
    ):
        indent = len(match.group(1))
        is_async = bool(match.group(2))
        func_name = match.group(3)
        params = match.group(4).strip()
        return_type = match.group(5).strip() if match.group(5) else None
        
        sig = f"{'async ' if is_async else ''}def {func_name}({params})"
        if return_type:
            sig += f" -> {return_type}"
        
        signatures.append({
            'type': 'method' if indent > 0 else 'function',
            'name': func_name,
            'signature': sig,
            'line': content[:match.start()].count('\n') + 1,
            'is_async': is_async
        })
    
    return signatures

def extract_js_ts_signatures(content):
    """提取 JavaScript/TypeScript 函数和类签名"""
    signatures = []
    
    # 类定义
    for match in re.finditer(r'(?:export\s+)?class\s+(\w+)(?:\s+extends\s+(\w+))?', content):
        class_name = match.group(1)
        extends = match.group(2)
        sig = f"class {class_name}"
        if extends:
            sig += f" extends {extends}"
        signatures.append({
            'type': 'class',
            'name': class_name,
            'signature': sig,
            'line': content[:match.start()].count('\n') + 1
        })
    
    # 函数定义
    for match in re.finditer(
        r'(?:export\s+)?(?:async\s+)?function\s+(\w+)\s*\(([^)]*)\)',
        content
    ):
        func_name = match.group(1)
        params = match.group(2).strip()
        signatures.append({
            'type': 'function',
            'name': func_name,
            'signature': f"function {func_name}({params})",
            'line': content[:match.start()].count('\n') + 1
        })
    
    # 箭头函数
    for match in re.finditer(
        r'(?:export\s+)?(?:const|let)\s+(\w+)\s*=\s*(?:async\s+)?\([^)]*\)\s*=>',
        content
    ):
        func_name = match.group(1)
        signatures.append({
            'type': 'arrow_function',
            'name': func_name,
            'signature': f"const {func_name} = (...) =>",
            'line': content[:match.start()].count('\n') + 1
        })
    
    return signatures

def extract_signatures(file_path, content, lang):
    """根据语言提取签名"""
    if lang == 'python':
        return extract_python_signatures(content)
    elif lang in ('javascript', 'typescript'):
        return extract_js_ts_signatures(content)
    else:
        # 对于其他语言，尝试通用模式
        return []

def extract_imports(content, lang):
    """提取导入语句"""
    imports = []
    
    if lang == 'python':
        for match in re.finditer(r'^(?:from\s+(\S+)\s+)?import\s+(.+)$', content, re.MULTILINE):
            from_module = match.group(1)
            imports_str = match.group(2)
            if from_module:
                imports.append(f"from {from_module} import ...")
            else:
                imports.append(f"import {imports_str.split(',')[0].strip()}")
    
    elif lang in ('javascript', 'typescript'):
        for match in re.finditer(r'^import\s+.+\s+from\s+[\'"]([^\'"]+)[\'"]', content, re.MULTILINE):
            imports.append(match.group(1))
    
    return imports[:10]  # 限制数量

def analyze_file(file_path):
    """分析单个文件"""
    ext = os.path.splitext(file_path)[1].lower()
    lang = CODE_EXTENSIONS.get(ext)
    
    if not lang:
        return None
    
    try:
        with open(file_path, 'r', encoding='utf-8', errors='ignore') as f:
            content = f.read()
    except:
        return None
    
    if len(content) > 100000:  # 跳过超大文件
        return {
            'path': file_path,
            'language': lang,
            'status': 'skipped',
            'reason': 'file too large'
        }
    
    return {
        'path': file_path,
        'language': lang,
        'hash': get_file_hash(content),
        'lines': content.count('\n') + 1,
        'signatures': extract_signatures(file_path, content, lang),
        'imports': extract_imports(content, lang),
        'last_analyzed': datetime.now().isoformat()
    }

def scan_directory(root_path='.'):
    """扫描目录生成摘要"""
    files = []
    
    for dirpath, dirnames, filenames in os.walk(root_path):
        # 过滤忽略的目录
        dirnames[:] = [d for d in dirnames if not should_ignore(d)]
        
        for filename in filenames:
            if should_ignore(filename):
                continue
            
            file_path = os.path.join(dirpath, filename)
            ext = os.path.splitext(filename)[1].lower()
            
            if ext in CODE_EXTENSIONS:
                analysis = analyze_file(file_path)
                if analysis:
                    files.append(analysis)
    
    return files

def generate_digest(root_path='.'):
    """生成完整的代码摘要"""
    print(f"Scanning {root_path}...")
    
    files = scan_directory(root_path)
    
    # 按目录组织
    structure = {}
    for f in files:
        path = f['path']
        parts = path.split(os.sep)
        current = structure
        for part in parts[:-1]:
            if part not in current:
                current[part] = {}
            current = current[part]
        current[parts[-1]] = {
            'lang': f['language'],
            'lines': f.get('lines', 0),
            'signatures': len(f.get('signatures', []))
        }
    
    digest = {
        'generated_at': datetime.now().isoformat(),
        'root_path': os.path.abspath(root_path),
        'stats': {
            'total_files': len(files),
            'total_lines': sum(f.get('lines', 0) for f in files),
            'by_language': {}
        },
        'files': files,
        'structure': structure
    }
    
    # 统计语言分布
    for f in files:
        lang = f['language']
        if lang not in digest['stats']['by_language']:
            digest['stats']['by_language'][lang] = {'files': 0, 'lines': 0}
        digest['stats']['by_language'][lang]['files'] += 1
        digest['stats']['by_language'][lang]['lines'] += f.get('lines', 0)
    
    # 保存
    os.makedirs(STATUS_DIR, exist_ok=True)
    with open(DIGEST_FILE, 'w') as f:
        json.dump(digest, f, indent=2)
    
    print(f"✅ Digest saved to {DIGEST_FILE}")
    print(f"   Files analyzed: {len(files)}")
    print(f"   Total lines: {digest['stats']['total_lines']}")
    
    return digest

def print_summary():
    """打印摘要"""
    if not os.path.exists(DIGEST_FILE):
        print("No digest found. Run 'generate' first.")
        return
    
    with open(DIGEST_FILE, 'r') as f:
        digest = json.load(f)
    
    print("\n📊 Code Digest Summary")
    print("=" * 50)
    print(f"Generated: {digest['generated_at']}")
    print(f"Root: {digest['root_path']}")
    print(f"\nStats:")
    print(f"  Total files: {digest['stats']['total_files']}")
    print(f"  Total lines: {digest['stats']['total_lines']}")
    print(f"\nBy language:")
    for lang, stats in digest['stats']['by_language'].items():
        print(f"  {lang}: {stats['files']} files, {stats['lines']} lines")
    
    print("\nTop files by signatures:")
    files_with_sigs = [(f['path'], len(f.get('signatures', []))) 
                       for f in digest['files'] 
                       if f.get('signatures')]
    files_with_sigs.sort(key=lambda x: x[1], reverse=True)
    for path, count in files_with_sigs[:10]:
        print(f"  {path}: {count} signatures")

def main():
    if len(sys.argv) < 2:
        print(__doc__)
        return
    
    cmd = sys.argv[1]
    
    if cmd == 'generate':
        root = sys.argv[2] if len(sys.argv) > 2 else '.'
        generate_digest(root)
    elif cmd == 'summary':
        print_summary()
    else:
        print(__doc__)

if __name__ == "__main__":
    main()
