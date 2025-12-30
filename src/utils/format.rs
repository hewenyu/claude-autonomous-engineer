// Text Formatting Utilities

/// 保留头尾,截断中间
pub fn truncate_middle(text: &str, max_len: usize) -> String {
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

/// 截断文本 (只保留前面部分)
pub fn truncate(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        text.to_string()
    } else {
        format!("{}...", &text[..max_len])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_middle() {
        let text = "a".repeat(1000);
        let truncated = truncate_middle(&text, 100);
        assert!(truncated.len() < text.len());
        assert!(truncated.contains("TRUNCATED"));
    }

    #[test]
    fn test_truncate() {
        let text = "hello world this is a long text";
        let truncated = truncate(text, 10);
        assert_eq!(truncated, "hello worl...");
    }
}
