use std::ops::Range;

use gpui::{
    actions, App, Context, FocusHandle, HighlightStyle, KeyBinding, Modifiers, SharedString,
};
use unicode_segmentation::UnicodeSegmentation;

use crate::{editor::Selection, theme::ActiveTheme};

const CONTEXT: &str = "ElySelectableMarkdown";

actions!(selectable_markdown, [Copy, SelectAll]);

pub(crate) fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new(ely_core::secondary!("c"), Copy, Some(CONTEXT)),
        KeyBinding::new(ely_core::secondary!("a"), SelectAll, Some(CONTEXT)),
    ]);
}

/// Selection state shared by one selectable `MarkdownRenderer` across frames.
pub(crate) struct MarkdownSelection {
    text: SharedString,
    selection: Selection,
    selecting: bool,
    pub(crate) focus: FocusHandle,
}

impl MarkdownSelection {
    pub(crate) fn new(cx: &mut Context<Self>) -> Self {
        Self {
            text: "".into(),
            selection: Selection::caret(0),
            selecting: false,
            focus: cx.focus_handle(),
        }
    }

    pub(crate) fn sync_text(&mut self, text: SharedString) {
        if self.text == text {
            return;
        }
        self.text = text;
        self.selection = Selection::caret(0);
        self.selecting = false;
    }

    pub(crate) fn range(&self) -> Range<usize> {
        self.selection.range()
    }

    pub(crate) fn selected_text(&self) -> Option<String> {
        let range = self.range();
        (!range.is_empty()).then(|| self.text[range].to_string())
    }

    pub(crate) fn press(&mut self, at: usize, modifiers: Modifiers, click_count: usize) {
        let at = self.boundary(at);
        match click_count {
            2 => {
                self.selection = self.word(at);
                self.selecting = false;
            }
            3.. => {
                self.selection = self.line(at);
                self.selecting = false;
            }
            _ if modifiers.shift => {
                self.selection.head = at;
                self.selecting = false;
            }
            _ => {
                self.selection = Selection::caret(at);
                self.selecting = true;
            }
        }
    }

    pub(crate) fn drag_to(&mut self, at: usize) {
        if self.selecting {
            self.selection.head = self.boundary(at);
        }
    }

    pub(crate) fn release(&mut self) {
        self.selecting = false;
    }

    pub(crate) fn select_all(&mut self) {
        self.selection = Selection {
            anchor: 0,
            head: self.text.len(),
            goal: None,
        };
    }

    fn boundary(&self, at: usize) -> usize {
        let mut at = at.min(self.text.len());
        while at > 0 && !self.text.is_char_boundary(at) {
            at -= 1;
        }
        at
    }

    fn word(&self, at: usize) -> Selection {
        self.text
            .split_word_bound_indices()
            .find(|(start, word)| *start <= at && at < start + word.len())
            .map_or_else(|| Selection::caret(at), |(start, word)| Selection {
                anchor: start,
                head: start + word.len(),
                goal: None,
            })
    }

    fn line(&self, at: usize) -> Selection {
        let start = self.text[..at].rfind('\n').map_or(0, |ix| ix + 1);
        let end = self.text[at..]
            .find('\n')
            .map_or(self.text.len(), |ix| at + ix);
        Selection {
            anchor: start,
            head: end,
            goal: None,
        }
    }
}

pub(crate) fn context() -> &'static str {
    CONTEXT
}

pub(crate) fn highlight(
    range: Range<usize>,
    selected: Range<usize>,
    cx: &App,
) -> Option<(Range<usize>, HighlightStyle)> {
    let from = selected.start.max(range.start);
    let to = selected.end.min(range.end);
    (from < to).then(|| {
        (
            from - range.start..to - range.start,
            HighlightStyle {
                background_color: Some(cx.theme().colors.selection),
                ..HighlightStyle::default()
            },
        )
    })
}
