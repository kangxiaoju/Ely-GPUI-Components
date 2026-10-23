use std::ops::Range;

use gpui::{App, HighlightStyle};

use super::buffer::Buffer;
use crate::forms::{Kind, Syntax, code_highlights_with, lex};

/// Each line that opens a block, and the last line the block holds, read from indentation.
pub(crate) fn folds(buffer: &Buffer) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut stack: Vec<(usize, usize, Option<usize>)> = Vec::new();
    for line in 0..buffer.lines() {
        let text = buffer.line(line);
        if text.trim().is_empty() {
            continue;
        }
        let indent = buffer.indent(line);
        while let Some(&(_, level, _)) = stack.last() {
            if level < indent {
                break;
            }
            let (header, _, descendant) = stack.pop().expect("the stack has a last entry");
            if let Some(end) = descendant {
                out.push((header, end));
            }
        }
        for (_, _, descendant) in &mut stack {
            *descendant = Some(line);
        }
        stack.push((line, indent, None));
    }
    for (header, _, descendant) in stack {
        if let Some(end) = descendant {
            out.push((header, end));
        }
    }
    out.sort_unstable();
    out
}

/// A bracket in the code, how deep it sits, and where its partner is when it has one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Bracket {
    pub offset: usize,
    pub depth: usize,
    pub partner: Option<usize>,
}

fn opener(close: char) -> char {
    match close {
        ')' => '(',
        ']' => '[',
        _ => '{',
    }
}

/// Every bracket outside strings and comments; a closer takes its opener's depth.
pub(crate) fn brackets(buffer: &Buffer) -> Vec<Bracket> {
    let mut out: Vec<Bracket> = Vec::new();
    let mut open: Vec<(usize, char)> = Vec::new();
    for line in 0..buffer.lines() {
        let start = buffer.line_range(line).start;
        let text = buffer.line(line);
        for (range, kind) in lex(text) {
            if kind != Kind::Mark {
                continue;
            }
            let Some(ch) = text[range.clone()].chars().next() else {
                continue;
            };
            let offset = start + range.start;
            match ch {
                '(' | '[' | '{' => {
                    open.push((out.len(), ch));
                    out.push(Bracket {
                        offset,
                        depth: open.len() - 1,
                        partner: None,
                    });
                }
                ')' | ']' | '}' => match open.last() {
                    Some((ix, wanted)) if *wanted == opener(ch) => {
                        let ix = *ix;
                        open.pop();
                        let partner = out[ix].offset;
                        out[ix].partner = Some(offset);
                        out.push(Bracket {
                            offset,
                            depth: open.len(),
                            partner: Some(partner),
                        });
                    }
                    _ => out.push(Bracket {
                        offset,
                        depth: open.len(),
                        partner: None,
                    }),
                },
                _ => {}
            }
        }
    }
    out
}

/// The pair beside a caret: the bracket just after it, or else the one just before.
pub(crate) fn matched(brackets: &[Bracket], caret: usize) -> Option<(usize, usize)> {
    let at = |offset: usize| {
        brackets
            .binary_search_by_key(&offset, |bracket| bracket.offset)
            .ok()
            .map(|ix| brackets[ix])
    };
    let bracket = at(caret).or_else(|| caret.checked_sub(1).and_then(at))?;
    Some((bracket.offset, bracket.partner?))
}

/// A line's syntax colors as highlight styles.
pub(crate) fn colors(text: &str, cx: &App) -> Vec<(Range<usize>, HighlightStyle)> {
    styles(crate::forms::code_highlights(text, cx))
}

/// A line's colors as `lang` reads it.
pub(crate) fn colors_with(
    text: &str,
    lang: &Syntax,
    cx: &App,
) -> Vec<(Range<usize>, HighlightStyle)> {
    styles(code_highlights_with(text, lang, cx))
}

/// Highlights as gpui styles.
fn styles(
    highlights: Vec<(Range<usize>, crate::forms::Highlight)>,
) -> Vec<(Range<usize>, HighlightStyle)> {
    highlights
        .into_iter()
        .map(|(range, highlight)| {
            (
                range,
                HighlightStyle {
                    color: Some(highlight.color),
                    background_color: highlight.background,
                    ..HighlightStyle::default()
                },
            )
        })
        .collect()
}

/// Where `needle` appears in the lines of `range`, as offsets, whole words only when it is a word.
pub(crate) fn occurrences(buffer: &Buffer, needle: &str, lines: Range<usize>) -> Vec<Range<usize>> {
    if needle.is_empty() || needle.contains('\n') {
        return Vec::new();
    }
    let word = needle.chars().all(|ch| ch.is_alphanumeric() || ch == '_');
    let boundary = |ch: Option<char>| ch.is_none_or(|ch| !(ch.is_alphanumeric() || ch == '_'));
    let mut out = Vec::new();
    for line in lines.start..lines.end.min(buffer.lines()) {
        let start = buffer.line_range(line).start;
        let text = buffer.line(line);
        for (at, _) in text.match_indices(needle) {
            let end = at + needle.len();
            let whole =
                boundary(text[..at].chars().next_back()) && boundary(text[end..].chars().next());
            if !word || whole {
                out.push(start + at..start + end);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_fold_by_indentation_and_skip_blank_lines() {
        let buffer = Buffer::new("fn a() {\n    one();\n\n    two();\n}\nfn b() {}");
        assert_eq!(folds(&buffer), [(0, 3)]);
    }

    #[test]
    fn brackets_pair_by_depth_outside_strings_and_comments() {
        let buffer = Buffer::new("f(a[0], \"(\") // )\n{ }");
        let found = brackets(&buffer);
        let offsets: Vec<usize> = found.iter().map(|bracket| bracket.offset).collect();
        assert_eq!(
            offsets,
            [1, 3, 5, 11, 18, 20],
            "the quoted and the commented ones do not count"
        );
        assert_eq!(found[1].depth, 1);
        assert_eq!(found[3].partner, Some(1));
        assert_eq!(matched(&found, 12), Some((11, 1)), "just before the caret");
        assert_eq!(matched(&found, 18), Some((18, 20)), "just after the caret");
    }

    #[test]
    fn occurrences_of_a_word_skip_longer_words() {
        let buffer = Buffer::new("let total = total_cost + total;");
        let found = occurrences(&buffer, "total", 0..1);
        assert_eq!(found, [4..9, 25..30]);
    }
    #[test]
    fn deeply_indented_lines_fold_in_one_forward_pass() {
        let text = (0..10_000)
            .map(|line| format!("{}line{line}", " ".repeat(line % 20)))
            .collect::<Vec<_>>()
            .join("\n");
        let buffer = Buffer::new(text);
        let found = folds(&buffer);
        assert!(!found.is_empty());
        assert!(found.iter().all(|(start, end)| start < end));
    }
}
