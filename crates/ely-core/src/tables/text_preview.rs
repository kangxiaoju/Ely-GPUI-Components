//! Bounded paint-only text. Filtering, sorting and editing keep the original Cell.
use std::borrow::Cow;

/// More than fits a normal table column, bounded independently of the value size.
const MAX_PREVIEW_BYTES: usize = 1024;

pub(super) fn preview(text: &str) -> Cow<'_, str> {
    let mut end = text.len().min(MAX_PREVIEW_BYTES);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let visible = &text[..end];
    let controls = visible.chars().any(char::is_control);
    if end == text.len() && !controls {
        return Cow::Borrowed(text);
    }
    let mut output = String::with_capacity(end + 3);
    output.extend(
        visible
            .chars()
            .map(|c| if c.is_control() { ' ' } else { c }),
    );
    if end < text.len() {
        output.push('…');
    }
    Cow::Owned(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn long_values_are_bounded_without_mutating_original() {
        let text = "中文🙂".repeat(100_000);
        let shown = preview(&text);
        assert!(shown.len() <= MAX_PREVIEW_BYTES + '…'.len_utf8());
        assert!(shown.ends_with('…'));
        assert_eq!(text.len(), 1_000_000);
    }
    #[test]
    fn short_values_borrow_and_newlines_never_reach_single_line_layout() {
        assert!(matches!(preview("hello"), Cow::Borrowed("hello")));
        assert_eq!(preview("a\r\nb\tc"), "a  b c");
        assert_eq!(preview(""), "");
    }
    #[test]
    fn preview_does_not_walk_the_hidden_tail() {
        let text = format!(
            "{}\n{}",
            "a".repeat(MAX_PREVIEW_BYTES),
            "z".repeat(10_000_000)
        );
        assert_eq!(
            preview(&text),
            format!("{}…", "a".repeat(MAX_PREVIEW_BYTES))
        );
    }
}

#[cfg(test)]
mod timing {
    #[test]
    fn bounded_preview_workload() {
        use std::{hint::black_box, time::Instant};
        let text = "中".repeat(1_000_000);
        let begin = Instant::now();
        for _ in 0..10_000 {
            black_box(super::preview(black_box(&text)));
        }
        println!(
            "preview 10000x3MB: {:?}; layout bytes <= 1027 vs {}",
            begin.elapsed(),
            text.len()
        );
    }
}
