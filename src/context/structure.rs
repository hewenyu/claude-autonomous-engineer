// Project Structure Scanner
// 项目结构扫描

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

const IGNORE_DIRS: &[&str] = &[
    ".git",
    "__pycache__",
    "node_modules",
    "venv",
    ".venv",
    "dist",
    "build",
    "target",
    ".claude",
];

const CODE_EXTENSIONS: &[&str] = &["py", "js", "ts", "tsx", "jsx", "go", "rs", "java", "c", "cpp"];

pub struct ProjectStructure {
    pub files: Vec<PathBuf>,
    pub total_files: usize,
}

impl ProjectStructure {
    pub fn scan(project_root: &Path, max_depth: usize) -> Self {
        let ignore: HashSet<_> = IGNORE_DIRS.iter().cloned().collect();
        let mut files = Vec::new();
        let mut total_files = 0;

        for entry in WalkDir::new(project_root)
            .max_depth(max_depth)
            .into_iter()
            .filter_entry(|e| {
                e.file_name()
                    .to_str()
                    .map(|s| !ignore.contains(s) && !s.starts_with('.'))
                    .unwrap_or(false)
            })
        {
            if let Ok(entry) = entry {
                if entry.file_type().is_file() {
                    total_files += 1;

                    if let Some(ext) = entry.path().extension() {
                        if CODE_EXTENSIONS.contains(&ext.to_str().unwrap_or("")) {
                            if let Ok(relative) = entry.path().strip_prefix(project_root) {
                                files.push(relative.to_path_buf());
                            }
                        }
                    }
                }
            }
        }

        Self { files, total_files }
    }

    pub fn format_context(&self, max_files: usize) -> String {
        let mut ctx = String::from("\n## 🏗️ PROJECT STRUCTURE\n");
        ctx.push_str(&format!("Total files scanned: {}\n", self.total_files));
        ctx.push_str(&format!("Code files: {}\n\n", self.files.len()));

        ctx.push_str("```\n");
        for (i, file) in self.files.iter().enumerate() {
            if i >= max_files {
                ctx.push_str(&format!("... and {} more code files\n", self.files.len() - max_files));
                break;
            }

            // 添加简单的树形结构
            let depth = file.components().count();
            let indent = "  ".repeat(depth.saturating_sub(1));

            if let Some(name) = file.file_name() {
                ctx.push_str(&format!("{}📄 {}\n", indent, name.to_string_lossy()));
            }
        }
        ctx.push_str("```\n");

        ctx
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_structure_scan() {
        let current_dir = env::current_dir().unwrap();
        let structure = ProjectStructure::scan(&current_dir, 3);

        // 应该能扫描到至少一些文件
        assert!(structure.total_files > 0);
    }
}
