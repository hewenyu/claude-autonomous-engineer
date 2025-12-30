// API Contract Handler
// api_contract.yaml 处理

use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

const CONTRACT_FILE: &str = ".claude/status/api_contract.yaml";

pub struct ApiContract {
    pub raw_content: String,
}

impl ApiContract {
    pub fn load(project_root: &Path) -> Result<Self> {
        let path = project_root.join(CONTRACT_FILE);

        if !path.exists() {
            anyhow::bail!("api_contract.yaml not found at {}", path.display());
        }

        let raw_content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;

        Ok(Self { raw_content })
    }

    pub fn try_load(project_root: &Path) -> Option<Self> {
        Self::load(project_root).ok()
    }

    pub fn format_context(&self, max_chars: usize) -> String {
        let content = truncate_middle(&self.raw_content, max_chars);
        format!("\n## 📜 API CONTRACT\n```yaml\n{}\n```\n", content)
    }
}

fn truncate_middle(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        return text.to_string();
    }

    let half = max_len / 2 - 20;
    format!(
        "{}\n\n... [TRUNCATED] ...\n\n{}",
        &text[..half],
        &text[text.len() - half..]
    )
}
