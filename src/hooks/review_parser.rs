//! 审查结果解析器
//!
//! 解析 codex review 的输出

use crate::hooks::review_context::ReviewMode;
use anyhow::Result;
use lazy_static::lazy_static;
use regex::Regex;

/// 审查判定结果
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Pass,
    Warn,
    Fail,
}

/// 问题严重性
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Critical,
    Error,
    Warn,
}

/// 审查问题
#[derive(Debug, Clone)]
pub struct Issue {
    pub severity: Severity,
    pub description: String,
}

/// 审查结果
#[derive(Debug, Clone)]
pub struct ReviewResult {
    pub verdict: Verdict,
    pub state_transition_valid: bool, // 仅在深度审查时有效
    pub issues: Vec<Issue>,
}

impl ReviewResult {
    /// 格式化错误消息（用于 hook 返回）
    pub fn format_error_message(&self) -> String {
        let mut msg = String::new();

        msg.push_str("\n❌ Code Review Failed:\n\n");

        for issue in &self.issues {
            let icon = match issue.severity {
                Severity::Critical => "🔴",
                Severity::Error => "⚠️ ",
                Severity::Warn => "💡",
            };

            msg.push_str(&format!(
                "   {} [{:?}] {}\n",
                icon, issue.severity, issue.description
            ));
        }

        if !self.state_transition_valid {
            msg.push_str("\n⛔ State transition is invalid. Please fix issues before changing task status.\n");
        }

        msg.push_str("\n💡 Fix the issues above and try again.\n");

        msg
    }
}

lazy_static! {
    static ref VERDICT_REGEX: Regex = Regex::new(r"(?i)VERDICT:\s*(PASS|FAIL|WARN)").unwrap();
    static ref STATE_TRANSITION_REGEX: Regex =
        Regex::new(r"(?i)STATE_TRANSITION_VALID:\s*(YES|NO)").unwrap();
    static ref ISSUE_REGEX: Regex =
        Regex::new(r"(?i)-\s*\[Severity:\s*(CRITICAL|ERROR|WARN)\]\s*(.+)").unwrap();
}

/// 解析 codex review 输出
pub fn parse_review_output(output: &str, mode: ReviewMode) -> Result<ReviewResult> {
    let mut verdict = Verdict::Fail;
    // 深度审查时如果缺少字段，默认视为 YES，避免误阻塞长周期自动化。
    let mut state_transition_valid = true;
    let mut issues = Vec::new();

    // 解析 VERDICT
    if let Some(captures) = VERDICT_REGEX.captures(output) {
        verdict = match captures[1].to_uppercase().as_str() {
            "PASS" => Verdict::Pass,
            "WARN" => Verdict::Warn,
            "FAIL" => Verdict::Fail,
            _ => Verdict::Fail,
        };
    } else {
        // 如果没有找到 VERDICT，默认为 FAIL
        eprintln!("⚠️  Warning: No VERDICT found in codex output, defaulting to FAIL");
    }

    // 深度审查模式下解析 STATE_TRANSITION_VALID
    if mode == ReviewMode::Deep {
        if let Some(captures) = STATE_TRANSITION_REGEX.captures(output) {
            state_transition_valid = captures[1].to_uppercase() == "YES";
        } else {
            // 兼容 Codex/模型输出缺少该字段：不阻塞提交，但给出提示
            eprintln!(
                "⚠️  Warning: No STATE_TRANSITION_VALID found in deep review output (assuming YES)"
            );
            issues.push(Issue {
                severity: Severity::Warn,
                description: "Missing STATE_TRANSITION_VALID in deep review output; assumed YES"
                    .to_string(),
            });
        }
    }

    // 解析 ISSUES
    for captures in ISSUE_REGEX.captures_iter(output) {
        let severity = match captures[1].to_uppercase().as_str() {
            "CRITICAL" => Severity::Critical,
            "ERROR" => Severity::Error,
            "WARN" => Severity::Warn,
            _ => Severity::Warn,
        };

        let description = captures[2].trim().to_string();

        issues.push(Issue {
            severity,
            description,
        });
    }

    // 如果审查失败但没有提取到具体问题，返回原始输出作为问题
    if verdict == Verdict::Fail && issues.is_empty() {
        eprintln!("⚠️  Warning: Review FAILED but no specific issues were extracted");
        eprintln!("Raw output:\n{}", output);

        // 将原始输出作为一个 Critical issue
        issues.push(Issue {
            severity: Severity::Critical,
            description: format!(
                "Review failed but no specific issues were parsed. Raw codex output:\n\n{}",
                output
            ),
        });
    }

    Ok(ReviewResult {
        verdict,
        state_transition_valid,
        issues,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_pass_verdict() {
        let output = r#"
VERDICT: PASS
ISSUES:
"#;

        let result = parse_review_output(output, ReviewMode::Regular).unwrap();
        assert_eq!(result.verdict, Verdict::Pass);
        assert_eq!(result.issues.len(), 0);
    }

    #[test]
    fn test_parse_fail_with_issues() {
        let output = r#"
VERDICT: FAIL
ISSUES:
- [Severity: ERROR] Missing error handling
- [Severity: WARN] Consider adding documentation
"#;

        let result = parse_review_output(output, ReviewMode::Regular).unwrap();
        assert_eq!(result.verdict, Verdict::Fail);
        assert_eq!(result.issues.len(), 2);
        assert_eq!(result.issues[0].severity, Severity::Error);
        assert!(result.issues[0].description.contains("error handling"));
    }

    #[test]
    fn test_parse_deep_review() {
        let output = r#"
VERDICT: PASS
STATE_TRANSITION_VALID: YES
ISSUES:
- [Severity: WARN] Minor style issue
"#;

        let result = parse_review_output(output, ReviewMode::Deep).unwrap();
        assert_eq!(result.verdict, Verdict::Pass);
        assert!(result.state_transition_valid);
        assert_eq!(result.issues.len(), 1);
    }

    #[test]
    fn test_parse_no_verdict() {
        let output = "Some random output without verdict";

        let result = parse_review_output(output, ReviewMode::Regular).unwrap();
        assert_eq!(result.verdict, Verdict::Fail); // 默认 FAIL
    }
}
