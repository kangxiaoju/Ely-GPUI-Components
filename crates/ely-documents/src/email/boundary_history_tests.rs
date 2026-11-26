use super::*;
use gpui::TestAppContext;

#[gpui::test]
fn backspace_paragraph_boundary_undo_redo_restores_exact_blocks_and_caret(cx: &mut TestAppContext) {
    cx.update(crate::theme::Theme::init);
    let document = EmailDocument::from_plain("A\nB").unwrap();
    let (editor, cx) = cx.add_window_view(|window, cx| HtmlEmailEditor::new(document, window, cx));
    editor.update(cx, |editor, cx| {
        let before = editor.document().blocks.clone();
        let original = EmailSelection::caret(EmailPoint { offset: 0, ..editor.document().end_point() });
        editor.set_selection(original.clone(), cx);
        editor.delete_direction(false, cx);
        assert_eq!(editor.document().to_plain(), "AB");
        assert_eq!(editor.document().blocks.len(), 1);
        let after = editor.document().blocks.clone();
        let merged = EmailSelection::caret(EmailPoint { offset: 1, ..editor.document().first_point() });
        assert_eq!(editor.selection(), merged);
        editor.undo(cx);
        assert_eq!(editor.document().blocks, before);
        assert_eq!(editor.selection(), original);
        assert!(!editor.can_undo());
        editor.redo(cx);
        assert_eq!(editor.document().blocks, after);
        assert_eq!(editor.selection(), merged);
        assert!(!editor.can_redo());
    });
}

#[gpui::test]
fn forward_delete_paragraph_boundary_undo_redo_restores_exact_blocks_and_caret(cx: &mut TestAppContext) {
    cx.update(crate::theme::Theme::init);
    let document = EmailDocument::from_plain("A\nB").unwrap();
    let (editor, cx) = cx.add_window_view(|window, cx| HtmlEmailEditor::new(document, window, cx));
    editor.update(cx, |editor, cx| {
        let before = editor.document().blocks.clone();
        let original = EmailSelection::caret(EmailPoint { offset: 1, ..editor.document().first_point() });
        editor.set_selection(original.clone(), cx);
        editor.delete_direction(true, cx);
        assert_eq!(editor.document().to_plain(), "AB");
        assert_eq!(editor.document().blocks.len(), 1);
        let after = editor.document().blocks.clone();
        assert_eq!(editor.selection(), original);
        editor.undo(cx);
        assert_eq!(editor.document().blocks, before);
        assert_eq!(editor.selection(), original);
        assert!(!editor.can_undo());
        editor.redo(cx);
        assert_eq!(editor.document().blocks, after);
        assert_eq!(editor.selection(), original);
        assert!(!editor.can_redo());
    });
}
