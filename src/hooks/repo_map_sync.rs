//! Repository Map Sync Hook
//!
//! 在文件写入后自动更新 `.claude/repo_map/structure.toon`，以便 `inject_state`
//! 每轮都能注入最新的 Repository Map（代码索引/骨架）。

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

use crate::repo_map::{OutputFormat, RepoMapper};
use crate::utils::{read_json, write_json};

/// 频繁触发会浪费 CPU；做一个轻量节流（秒）
///
/// 可通过环境变量 `REPO_MAP_MIN_INTERVAL_SECS` 覆盖（设为 0 可关闭节流）。
const MIN_INTERVAL_SECS_DEFAULT: i64 = 10;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct RepoMapSyncState {
    #[serde(default)]
    last_generated_at: Option<String>,

    #[serde(default)]
    last_trigger_file: Option<String>,
}

fn noop_posttooluse_output() -> Value {
    json!({
        "hookSpecificOutput": {
            "hookEventName": "PostToolUse"
        }
    })
}

/// 运行 repo_map_sync hook（PostToolUse）
pub fn run_repo_map_sync_hook(project_root: &Path, input: &Value) -> Result<Value> {
    // Allow users to opt-out (e.g. during large refactors).
    if std::env::var("SKIP_REPO_MAP")
        .ok()
        .is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"))
    {
        return Ok(noop_posttooluse_output());
    }

    let Some(file_path) = extract_file_path(input) else {
        return Ok(noop_posttooluse_output());
    };

    if should_skip_for_file(&file_path) {
        return Ok(noop_posttooluse_output());
    }

    if is_throttled(project_root)? {
        return Ok(noop_posttooluse_output());
    }

    // Best-effort: failures should not break the main tool flow.
    if let Err(e) = generate_and_write_repo_map(project_root) {
        eprintln!("⚠️  repo_map_sync: failed to generate map: {:#}", e);
        return Ok(noop_posttooluse_output());
    }

    let _ = update_state(project_root, &file_path);
    Ok(noop_posttooluse_output())
}

fn state_file(project_root: &Path) -> PathBuf {
    project_root
        .join(".claude/status")
        .join("repo_map_state.json")
}

fn is_throttled(project_root: &Path) -> Result<bool> {
    let state_path = state_file(project_root);
    let state: RepoMapSyncState = read_json(&state_path).unwrap_or_default();

    let min_interval_secs = std::env::var("REPO_MAP_MIN_INTERVAL_SECS")
        .ok()
        .and_then(|v| v.trim().parse::<i64>().ok())
        .unwrap_or(MIN_INTERVAL_SECS_DEFAULT);

    if min_interval_secs <= 0 {
        return Ok(false);
    }

    let Some(ts) = state.last_generated_at else {
        return Ok(false);
    };

    let Ok(dt) = DateTime::parse_from_rfc3339(&ts) else {
        return Ok(false);
    };

    let dt_utc = dt.with_timezone(&Utc);
    Ok(Utc::now().signed_duration_since(dt_utc) < Duration::seconds(min_interval_secs))
}

fn update_state(project_root: &Path, trigger_file: &Path) -> Result<()> {
    let state_path = state_file(project_root);
    if let Some(parent) = state_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut state: RepoMapSyncState = read_json(&state_path).unwrap_or_default();
    state.last_generated_at = Some(Utc::now().to_rfc3339());
    state.last_trigger_file = Some(trigger_file.to_string_lossy().to_string());
    write_json(&state_path, &state)?;
    Ok(())
}

fn generate_and_write_repo_map(project_root: &Path) -> Result<()> {
    let mut mapper = RepoMapper::new(project_root)?;
    let content = mapper.generate_map_with_format(OutputFormat::Toon)?;

    let output_path = project_root.join(".claude/repo_map/structure.toon");
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(output_path, content)?;
    Ok(())
}

fn should_skip_for_file(file_path: &Path) -> bool {
    // Avoid self-triggering loops if someone edits repo_map files.
    let p = file_path.to_string_lossy();
    if p.contains("/.claude/repo_map/") || p.contains("\\.claude\\repo_map\\") {
        return true;
    }

    // Only refresh map on code-ish changes (same list as RepoMapper supports).
    let Some(ext) = file_path.extension().and_then(|e| e.to_str()) else {
        return true;
    };

    !matches!(ext, "rs" | "py" | "go" | "ts" | "tsx" | "js" | "jsx")
}

/// 从输入中提取文件路径
fn extract_file_path(input: &Value) -> Option<PathBuf> {
    if let Some(tool_input) = input.get("tool_input") {
        if let Some(path) = tool_input.get("file_path").and_then(|p| p.as_str()) {
            return Some(PathBuf::from(path));
        }
        if let Some(path) = tool_input.get("path").and_then(|p| p.as_str()) {
            return Some(PathBuf::from(path));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn noop_when_no_file() {
        let temp = TempDir::new().unwrap();
        let out = run_repo_map_sync_hook(temp.path(), &json!({})).unwrap();
        assert_eq!(out["hookSpecificOutput"]["hookEventName"], "PostToolUse");
        assert!(!temp.path().join(".claude/repo_map/structure.toon").exists());
    }

    #[test]
    fn noop_when_unsupported_file() {
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join("a.txt"), "hi\n").unwrap();
        let out = run_repo_map_sync_hook(
            temp.path(),
            &json!({ "tool_input": { "file_path": "a.txt" } }),
        )
        .unwrap();
        assert_eq!(out["hookSpecificOutput"]["hookEventName"], "PostToolUse");
        assert!(!temp.path().join(".claude/repo_map/structure.toon").exists());
    }

    #[test]
    fn generates_map_for_supported_file() {
        let temp = TempDir::new().unwrap();
        std::fs::create_dir_all(temp.path().join("src")).unwrap();
        std::fs::write(
            temp.path().join("src/lib.rs"),
            "pub fn hello() -> i32 { 1 }\n",
        )
        .unwrap();

        let out = run_repo_map_sync_hook(
            temp.path(),
            &json!({ "tool_input": { "file_path": "src/lib.rs" } }),
        )
        .unwrap();
        assert_eq!(out["hookSpecificOutput"]["hookEventName"], "PostToolUse");

        let map_path = temp.path().join(".claude/repo_map/structure.toon");
        assert!(map_path.exists());
        let content = std::fs::read_to_string(map_path).unwrap();
        assert!(content.contains("src/lib.rs"));
    }

    #[test]
    fn throttles_when_recently_generated() {
        let temp = TempDir::new().unwrap();
        std::fs::create_dir_all(temp.path().join(".claude/status")).unwrap();
        std::fs::create_dir_all(temp.path().join(".claude/repo_map")).unwrap();

        // Set a recent state and a sentinel map file.
        std::fs::write(
            temp.path().join(".claude/status/repo_map_state.json"),
            serde_json::to_string(&RepoMapSyncState {
                last_generated_at: Some(Utc::now().to_rfc3339()),
                last_trigger_file: Some("x.rs".to_string()),
            })
            .unwrap(),
        )
        .unwrap();
        std::fs::write(
            temp.path().join(".claude/repo_map/structure.toon"),
            "SENTINEL\n",
        )
        .unwrap();

        std::fs::create_dir_all(temp.path().join("src")).unwrap();
        std::fs::write(temp.path().join("src/lib.rs"), "pub fn a() {}\n").unwrap();

        let _ = run_repo_map_sync_hook(
            temp.path(),
            &json!({ "tool_input": { "file_path": "src/lib.rs" } }),
        )
        .unwrap();

        let content =
            std::fs::read_to_string(temp.path().join(".claude/repo_map/structure.toon")).unwrap();
        assert_eq!(content, "SENTINEL\n");
    }
}
