//! 智能截断工具

fn clamp_to_char_boundary(s: &str, mut idx: usize) -> usize {
    idx = idx.min(s.len());
    while idx > 0 && !s.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}

/// 截断中间部分，保留头尾
pub fn truncate_middle(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        return text.to_string();
    }

    let half = (max_len / 2).saturating_sub(20);
    if half == 0 {
        let cut = clamp_to_char_boundary(text, max_len);
        return text[..cut].to_string();
    }

    let head_end = clamp_to_char_boundary(text, half);
    let tail_start = clamp_to_char_boundary(text, text.len().saturating_sub(half));

    format!(
        "{}\n\n... [TRUNCATED] ...\n\n{}",
        &text[..head_end],
        &text[tail_start..]
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

    #[test]
    fn test_truncate_middle_utf8_safe() {
        let text = "中文🙂".repeat(200);
        let truncated = truncate_middle(&text, 100);
        assert!(truncated.contains("[TRUNCATED]"));
    }
}
