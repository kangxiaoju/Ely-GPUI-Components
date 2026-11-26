use gpui::{Context, IntoElement, Modifiers, MouseButton, ParentElement, Render, Styled, TestAppContext, Window, div, point, px};

use crate::chat::StreamingMarkdown;

struct Page;

impl Render for Page {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .p_4()
            .child(StreamingMarkdown::new(
                "message",
                "selectable markdown words",
                false,
            ))
    }
}

struct MathPage;

impl Render for MathPage {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .p_4()
            .child(StreamingMarkdown::new(
                "math-message",
                "selectable $x$ words",
                false,
            ))
    }
}

#[gpui::test]
fn finished_streaming_markdown_drag_copies_only_the_selection(cx: &mut TestAppContext) {
    cx.update(crate::init_for_tests);
    let (_, cx) = cx.add_window_view(|_, _| Page);
    cx.run_until_parked();

    let from = point(px(18.0), px(22.0));
    let to = point(px(94.0), px(22.0));
    cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(to, Some(MouseButton::Left), Modifiers::none());
    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
    cx.simulate_keystrokes("secondary-c");

    let copied = cx
        .read_from_clipboard()
        .and_then(|item| item.text())
        .expect("dragged text copied");
    assert_eq!(copied, "selectable");
}

#[gpui::test]
fn finished_streaming_markdown_select_all_copies_the_plain_message(cx: &mut TestAppContext) {
    cx.update(crate::init_for_tests);
    let (_, cx) = cx.add_window_view(|_, _| Page);
    cx.run_until_parked();

    let at = point(px(24.0), px(22.0));
    cx.simulate_mouse_down(at, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_up(at, MouseButton::Left, Modifiers::none());
    cx.simulate_keystrokes("secondary-a secondary-c");

    let copied = cx
        .read_from_clipboard()
        .and_then(|item| item.text())
        .expect("message copied");
    assert_eq!(copied, "selectable markdown words");
}

#[gpui::test]
fn finished_streaming_markdown_with_inline_math_keeps_drag_copy(cx: &mut TestAppContext) {
    cx.update(crate::init_for_tests);
    let (_, cx) = cx.add_window_view(|_, _| MathPage);
    cx.run_until_parked();

    let from = point(px(18.0), px(22.0));
    let to = point(px(94.0), px(22.0));
    cx.simulate_mouse_down(from, MouseButton::Left, Modifiers::none());
    cx.simulate_mouse_move(to, Some(MouseButton::Left), Modifiers::none());
    cx.simulate_mouse_up(to, MouseButton::Left, Modifiers::none());
    cx.simulate_keystrokes("secondary-c");

    let copied = cx
        .read_from_clipboard()
        .and_then(|item| item.text())
        .expect("math paragraph selection copied");
    assert_eq!(copied, "selectable");
}
