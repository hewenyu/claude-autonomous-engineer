//! 智能截断工具

/// 截断中间部分，保留头尾
pub fn truncate_middle(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        return text.to_string();
    }

    let half = (max_len / 2).saturating_sub(20);
    if half == 0 {
        return text[..max_len.min(text.len())].to_string();
    }

    format!(
        "{}\n\n... [TRUNCATED] ...\n\n{}",
        &text[..half.min(text.len())],
        &text[text.len().saturating_sub(half)..]
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_middle() {
        let text = "a".repeat(1000);
        let truncated = truncate_middle(&text, 100);
        assert!(truncated.len() <= 150); // 大约 100 + 省略标记
        assert!(truncated.contains("[TRUNCATED]"));
    }

    #[test]
    fn test_no_truncate_if_short() {
        let text = "short";
        let result = truncate_middle(text, 100);
        assert_eq!(result, text);
    }
}
