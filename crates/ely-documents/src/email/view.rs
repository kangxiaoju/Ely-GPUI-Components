use gpui::{App, ElementId, Entity, InteractiveElement, IntoElement, ParentElement, RenderOnce, Styled, Window, div};
use super::{editor::HtmlEmailEditor, toolbar::EmailToolbar};

/// A native HTML email editor with its compact, selection-aware Ely formatting toolbar.
/// The supplied editor remains the single owner of document state, selection, and input focus.
#[derive(IntoElement)]
pub struct EmailEditorView {
    id: ElementId,
    editor: Entity<HtmlEmailEditor>,
}
impl EmailEditorView {
    /// Wraps `editor` without copying its document or introducing another text input.
    pub fn new(id: impl Into<ElementId>, editor: &Entity<HtmlEmailEditor>) -> Self {
        Self { id: id.into(), editor: editor.clone() }
    }
}
impl RenderOnce for EmailEditorView {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let editor = self.editor.clone();
        let _observation = window.use_keyed_state((self.id.clone(), "observe"), cx, |_, cx| {
            cx.observe(&editor, |_, _, cx| { cx.notify(); })
        });
        let _ = _observation.read(cx);
        div().id(self.id.clone()).flex().flex_col().size_full().min_w_0().min_h_0()
            .child(EmailToolbar::new((self.id, "toolbar"), &self.editor))
            .child(div().flex_1().min_h_0().min_w_0().child(self.editor))
    }
}
