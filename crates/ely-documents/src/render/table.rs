use std::cell::RefCell;

use crate::markdown::mdast::AlignKind;
use gpui::{
    AnyElement, App, FontWeight, IntoElement, MouseButton, ParentElement, Styled, div, prelude::*,
};

use super::Draw;
use crate::{
    theme::{ActiveTheme, Radius},
    tree::Node,
};

/// A cell: table in drawing order, row, column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct CellAt {
    pub table: usize,
    pub row: usize,
    pub column: usize,
}

/// The cell open for typing and its field, drawn once.
pub(crate) struct OpenCell {
    pub at: CellAt,
    pub field: RefCell<Option<AnyElement>>,
}

impl Draw<'_> {
    pub(super) fn table(
        &self,
        aligns: &[AlignKind],
        head: &[Vec<Node>],
        rows: &[Vec<Vec<Node>>],
        spans: &[Vec<usize>],
        cx: &App,
    ) -> AnyElement {
        let theme = cx.theme();
        let colors = theme.colors.clone();
        let table = self.tables.get();
        self.tables.set(table + 1);
        let cell = |blocks: &[Node], row: usize, column: usize, cx: &App| {
            let span = spans
                .get(row)
                .and_then(|spans| spans.get(column))
                .copied()
                .unwrap_or(1);
            if span == 0 {
                return None;
            }
            let at = CellAt { table, row, column };
            let open = (self.hooks.cell)
                .filter(|open| open.at == at)
                .and_then(|open| open.field.borrow_mut().take());
            let mut drawn = div()
                .debug_selector(move || format!("markdown-cell-{table}-{row}-{column}"))
                .flex_1()
                .min_w_0()
                .flex()
                .when(aligns.get(column) == Some(&AlignKind::Right), |cell| {
                    cell.text_right()
                })
                .when(aligns.get(column) == Some(&AlignKind::Center), |cell| {
                    cell.text_center()
                })
                .when(row == 0, |cell| {
                    cell.font_weight(FontWeight::MEDIUM)
                        .text_color(colors.fg_muted)
                });
            drawn.style().flex_grow = Some(span as f32);
            let inside = match open {
                // Only the field stops presses; padding still points at the cell's end.
                Some(field) => div()
                    .w_full()
                    .min_w_0()
                    .flex_shrink_0()
                    .px_3()
                    .py_1p5()
                    .child(
                        div()
                            .w_full()
                            .min_w_0()
                            .child(field)
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation()),
                    ),
                None => div()
                    .w_full()
                    .min_w_0()
                    .flex_shrink_0()
                    .px_3()
                    .py_1p5()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .children(self.nodes(blocks, cx)),
            };
            let inside = inside
                .debug_selector(move || format!("markdown-cell-content-{table}-{row}-{column}"));
            Some(self.pressed_cell(drawn.child(inside), at))
        };
        div()
            .debug_selector(move || format!("markdown-table-{table}"))
            .w_full()
            .min_w_0()
            .flex()
            .flex_col()
            .rounded(theme.radius(Radius::Md))
            .border_1()
            .border_color(colors.border)
            .overflow_hidden()
            .child(
                div().w_full().min_w_0().flex().bg(colors.hover).children(
                    head.iter()
                        .enumerate()
                        .filter_map(|(column, blocks)| cell(blocks, 0, column, cx)),
                ),
            )
            .children(rows.iter().enumerate().map(|(row, cells)| {
                div()
                    .w_full()
                    .min_w_0()
                    .flex()
                    .border_t_1()
                    .border_color(colors.border)
                    .children(
                        cells
                            .iter()
                            .enumerate()
                            .filter_map(|(column, blocks)| cell(blocks, row + 1, column, cx)),
                    )
            }))
            .into_any_element()
    }
}
