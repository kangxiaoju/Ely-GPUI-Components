use super::{EmailDocument, EmailSelection, HtmlEmailEditor};
use gpui::{AppContext as _, Entity, IntoElement, ParentElement, Render, Styled, TestAppContext, point, px};

struct ShortViewport(Entity<HtmlEmailEditor>);
impl Render for ShortViewport {
    fn render(&mut self, _: &mut gpui::Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        gpui::div().w(px(240.)).h(px(90.)).child(self.0.clone())
    }
}

fn long_document() -> EmailDocument {
    EmailDocument::from_plain(&vec!["word ".repeat(24); 12].join("\n")).unwrap()
}

#[gpui::test]
fn down_traverses_cached_lines_beyond_short_viewport(cx: &mut TestAppContext) {
    cx.update(crate::theme::Theme::init);
    let (stage, cx) = cx.add_window_view(|window, cx| ShortViewport(cx.new(|cx| HtmlEmailEditor::new(long_document(), window, cx))));
    let editor = stage.read_with(cx, |stage, _| stage.0.clone());
    cx.run_until_parked();
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    let (first_block, shapes) = editor.read_with(cx, |editor, _| (editor.selection.head.block, editor.layout.shape_count));
    for _ in 0..32 {
        let previous = editor.read_with(cx, |editor, _| editor.layout.caret_with_affinity(editor.selection.head, editor.affinity).unwrap().origin.y - editor.layout.origin.y);
        editor.update(cx, |editor, cx| editor.move_vertical(true, false, cx));
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
        editor.read_with(cx, |editor, _| {
            let caret = editor.layout.caret_with_affinity(editor.selection.head, editor.affinity).unwrap();
            assert!(caret.origin.y - editor.layout.origin.y > previous, "Down must advance even past the last visible block");
            let viewport = editor.scroll.bounds();
            assert!(caret.top() >= viewport.top() && caret.bottom() <= viewport.bottom(), "Down must reveal the complete caret: caret={caret:?} viewport={viewport:?} offset={:?} origin={:?} affinity={:?}", editor.scroll.offset(), editor.layout.origin, editor.affinity);
            assert_eq!(editor.layout.shape_count, shapes, "arrows must reuse cached shaping");
        });
    }
    editor.read_with(cx, |editor, _| assert_ne!(editor.selection.head.block, first_block));
}

#[gpui::test]
fn typed_caret_is_revealed_after_reflow(cx: &mut TestAppContext) {
    cx.update(crate::theme::Theme::init);
    let (stage, cx) = cx.add_window_view(|window, cx| ShortViewport(cx.new(|cx| HtmlEmailEditor::new(long_document(), window, cx))));
    let editor = stage.read_with(cx, |stage, _| stage.0.clone());
    cx.run_until_parked();
    editor.update(cx, |editor, cx| {
        editor.set_selection(EmailSelection::caret(editor.document.end_point()), cx);
        editor.type_text(&"typed ".repeat(50), cx).unwrap();
    });
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    editor.read_with(cx, |editor, _| {
        let caret = editor.layout.caret_with_affinity(editor.selection.head, editor.affinity).unwrap();
        let viewport = editor.scroll.bounds();
        assert!(editor.scroll.offset().y < px(0.));
        assert!(caret.top() >= viewport.top() && caret.bottom() <= viewport.bottom(), "typing must reveal the caret using the newly arranged layout: caret={caret:?} viewport={viewport:?} offset={:?} origin={:?} affinity={:?}", editor.scroll.offset(), editor.layout.origin, editor.affinity);
    });
}

#[gpui::test]
fn manual_scroll_survives_frames_without_selection_change(cx: &mut TestAppContext) {
    cx.update(crate::theme::Theme::init);
    let (stage, cx) = cx.add_window_view(|window, cx| ShortViewport(cx.new(|cx| HtmlEmailEditor::new(long_document(), window, cx))));
    let editor = stage.read_with(cx, |stage, _| stage.0.clone());
    cx.run_until_parked();
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    editor.update(cx, |editor, cx| { editor.scroll.set_offset(point(px(0.), -px(240.))); cx.notify(); });
    for _ in 0..3 {
        cx.update(|window, _| window.refresh());
        cx.run_until_parked();
    }
    editor.read_with(cx, |editor, _| {
        assert_eq!(editor.scroll.offset().y, -px(240.), "ordinary frames must not pull manual scrolling back to the caret");
        assert_eq!(editor.selection, EmailSelection::caret(editor.document.first_point()));
    });
}
