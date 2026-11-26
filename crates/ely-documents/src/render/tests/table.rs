use std::rc::Rc;

use gpui::{Context, IntoElement, Render, TestAppContext, Window, div, prelude::*, px};

use crate::{
    MarkdownRenderer,
    theme::Theme,
    tree::{Node, parse},
};

struct Page(Rc<[Node]>);

impl Render for Page {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .w(px(720.0))
            .flex()
            .flex_col()
            .items_start()
            .child(MarkdownRenderer::blocks("table-page", self.0.clone()))
    }
}

#[gpui::test]
fn markdown_columns_fill_a_wide_container(cx: &mut TestAppContext) {
    cx.update(Theme::init);
    let (_, cx) = cx.add_window_view(|_, _| {
        Page(parse("| A | B | C |\n| --- | --- | --- |\n| one | two | three |\n").into())
    });
    cx.run_until_parked();
    let table = cx.debug_bounds("markdown-table-0").expect("table drawn");
    let first = cx.debug_bounds("markdown-cell-0-0-0").expect("first cell");
    let last = cx.debug_bounds("markdown-cell-0-0-2").expect("last cell");
    assert!((table.size.width - px(720.0)).abs() < px(1.0), "{table:?}");
    assert!((first.left() - table.left()).abs() < px(2.0), "{first:?}");
    assert!((last.right() - table.right()).abs() < px(2.0), "{last:?}");
    assert!((first.size.width - last.size.width).abs() < px(1.0));
}

fn merged(spans: Vec<Vec<usize>>) -> Page {
    let mut nodes = parse(
        "| A | B | C |\n| --- | --- | --- |\n| A long explanation that needs the available table width rather than one narrow column. | covered | tail |\n",
    );
    let Node::Table { spans: held, .. } = &mut nodes[0] else {
        panic!("table");
    };
    *held = spans;
    Page(nodes.into())
}

#[gpui::test]
fn a_full_span_explanation_uses_the_table_width(cx: &mut TestAppContext) {
    cx.update(Theme::init);
    let (_, cx) = cx.add_window_view(|_, _| merged(vec![vec![1, 1, 1], vec![3, 0, 0]]));
    cx.run_until_parked();
    let table = cx.debug_bounds("markdown-table-0").expect("table drawn");
    let cell = cx
        .debug_bounds("markdown-cell-0-1-0")
        .expect("explanation cell");
    let content = cx
        .debug_bounds("markdown-cell-content-0-1-0")
        .expect("explanation content");
    assert!(
        (cell.size.width - table.size.width).abs() < px(3.0),
        "{table:?} {cell:?}"
    );
    assert!(content.size.width > table.size.width * 0.9, "{content:?}");
    assert!(content.left() >= cell.left() && content.right() <= cell.right());
    assert!(cx.debug_bounds("markdown-cell-0-1-1").is_none());
    assert!(cx.debug_bounds("markdown-cell-0-1-2").is_none());
}

#[gpui::test]
fn a_partial_span_keeps_the_remaining_column_aligned(cx: &mut TestAppContext) {
    cx.update(Theme::init);
    let (_, cx) = cx.add_window_view(|_, _| merged(vec![vec![1, 1, 1], vec![2, 0, 1]]));
    cx.run_until_parked();
    let head = cx
        .debug_bounds("markdown-cell-0-0-2")
        .expect("last heading");
    let wide = cx.debug_bounds("markdown-cell-0-1-0").expect("merged cell");
    let tail = cx
        .debug_bounds("markdown-cell-0-1-2")
        .expect("remaining cell");
    assert!(
        (head.left() - tail.left()).abs() < px(1.0),
        "{head:?} {tail:?}"
    );
    assert!(
        (wide.size.width - tail.size.width * 2.0).abs() < px(1.0),
        "{wide:?} {tail:?}"
    );
    assert!(cx.debug_bounds("markdown-cell-0-1-1").is_none());
}
