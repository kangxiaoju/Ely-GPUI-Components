//! The virtual table keeps both scrollbars in its viewport.
use super::settle;
use crate::{
    tables::{Cell, Column, DataTable, Row},
    theme::Theme,
};
use gpui::{
    Context, IntoElement, Modifiers, ParentElement, Render, ScrollDelta, ScrollSource,
    ScrollWheelEvent, Styled, TestAppContext, TouchPhase, Window, div, point, px,
};
use std::rc::Rc;

struct Results(Rc<Vec<Row>>);
impl Render for Results {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().w(px(480.)).h(px(300.)).child(
            DataTable::new(
                "scrolling-results",
                (0..10).map(|i| {
                    Column::new(i.to_string(), format!("Column {i}")).width(gpui::rems(12.))
                }),
            )
            .virtualized()
            .size_full()
            .rows(self.0.clone()),
        )
    }
}
fn wheel(x: f32, y: f32, cx: &mut gpui::VisualTestContext) {
    cx.simulate_event(ScrollWheelEvent {
        position: point(px(100.), px(100.)),
        delta: ScrollDelta::Pixels(point(px(x), px(y))),
        modifiers: Modifiers::none(),
        touch_phase: TouchPhase::Moved,
        source: ScrollSource::Pointer,
    });
    settle(cx);
}
#[gpui::test]
fn virtual_rows_have_viewport_anchored_scrollbars_on_both_axes(cx: &mut TestAppContext) {
    cx.update(Theme::init);
    let rows = Rc::new(
        (0..500)
            .map(|i| {
                Row::new(
                    i.to_string(),
                    (0..10).map(|j| Cell::Text(format!("Row {i}, column {j}").into())),
                )
            })
            .collect(),
    );
    let (_, cx) = cx.add_window_view(|_, _| Results(rows));
    settle(cx);
    let vertical = cx
        .debug_bounds("scrollbar-track-Vertical")
        .expect("vertical scrollbar");
    let horizontal = cx
        .debug_bounds("scrollbar-track-Horizontal")
        .expect("horizontal scrollbar");
    assert!(vertical.right() <= px(480.) && vertical.bottom() <= horizontal.top());
    assert!(horizontal.right() <= vertical.left() && horizontal.bottom() <= px(300.));
    wheel(0., -1500., cx);
    assert!(
        cx.debug_bounds("table-row 0").is_none(),
        "offscreen rows stay virtual"
    );
    wheel(0., 1500., cx);
    let before = cx.debug_bounds("table-row 0").expect("first row returns");
    wheel(-500., 0., cx);
    assert!(cx.debug_bounds("table-row 0").expect("first row").left() < before.left());
    assert_eq!(
        cx.debug_bounds("scrollbar-track-Vertical").unwrap(),
        vertical
    );
    assert_eq!(
        cx.debug_bounds("scrollbar-track-Horizontal").unwrap(),
        horizontal
    );
}
