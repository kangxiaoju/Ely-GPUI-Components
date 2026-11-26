use super::*;
use gpui::{EntityInputHandler, TestAppContext};

#[gpui::test]
fn composition_chinese_emoji_is_one_undo(cx: &mut TestAppContext) {
    cx.update(crate::theme::Theme::init);
    let (editor, cx) = cx.add_window_view(|window, cx| HtmlEmailEditor::new(EmailDocument::default(), window, cx));
    editor.update_in(cx, |editor, window, cx| {
        editor.replace_and_mark_text_in_range(None, "中😀", Some(3..3), window, cx);
        assert_eq!(editor.document().to_plain(), "中😀");
        assert_eq!(editor.snapshot().to_plain(), "", "saving during composition excludes uncommitted preview");
        assert_eq!(editor.marked_text_range(window, cx), Some(0..3));
        editor.replace_text_in_range(None, "中文😀", window, cx);
        assert_eq!(editor.document().to_plain(), "中文😀");
        editor.undo(cx);
        assert_eq!(editor.document().to_plain(), "");
        editor.redo(cx);
        assert_eq!(editor.document().to_plain(), "中文😀");
    });
}

#[gpui::test]
fn composition_cancel_restores_selection_and_does_not_add_history(cx: &mut TestAppContext) {
    cx.update(crate::theme::Theme::init);
    let (editor, cx) = cx.add_window_view(|window, cx| HtmlEmailEditor::new(EmailDocument::from_plain("before").unwrap(), window, cx));
    editor.update_in(cx, |editor, window, cx| {
        let initial = editor.selection();
        editor.replace_and_mark_text_in_range(None, "预览", None, window, cx);
        editor.cancel_composition(cx);
        assert_eq!(editor.document().to_plain(), "before");
        assert_eq!(editor.selection(), initial);
        assert!(!editor.can_undo());
    });
}

#[gpui::test]
fn cross_paragraph_replace_format_and_undo_restore_model(cx: &mut TestAppContext) {
    let document = EmailDocument::from_plain("first\nsecond").unwrap();
    cx.update(crate::theme::Theme::init);
    let (editor, cx) = cx.add_window_view(|window, cx| HtmlEmailEditor::new(document, window, cx));
    editor.update(cx, |editor, cx| {
        let initial = editor.snapshot();
        editor.set_selection(EmailSelection { anchor: editor.document().first_point(), head: editor.document().end_point() }, cx);
        editor.apply_format(EmailFormat::Size(Some(32)), cx).unwrap();
        editor.insert_text("替换😀", cx).unwrap();
        assert_eq!(editor.document().to_plain(), "替换😀");
        editor.undo(cx);
        editor.undo(cx);
        assert_eq!(editor.document().blocks, initial.blocks);
    });
}

#[test]
fn utf16_mapping_preserves_tables_atomic_blocks_and_surrogate_boundaries() {
    let doc = EmailDocument::from_plain("中😀\nnext").unwrap();
    let map = super::input::InputMap::new(&doc);
    assert_eq!(map.text(0..8), "中😀\nnext");
    assert_eq!(map.point(3).offset, "中😀".len());
    assert_eq!(map.point(2).offset, "中".len());
    assert_eq!(map.utf16(doc.end_point()), 8);
}

#[gpui::test]
fn preview_and_cancel_never_emit_saved_document_changes(cx: &mut TestAppContext) {
    use std::{cell::Cell, rc::Rc};
    cx.update(crate::theme::Theme::init);
    let (editor, cx) = cx.add_window_view(|window, cx| HtmlEmailEditor::new(EmailDocument::from_plain("A\nB\nC").unwrap(), window, cx));
    let count = Rc::new(Cell::new(0));
    let observed = count.clone();
    cx.update(|_, cx| { cx.subscribe(&editor, move |_, event, _| { if matches!(event, EmailEditorEvent::Changed) { observed.set(observed.get() + 1); } }).detach(); });
    editor.update_in(cx, |editor, window, cx| {
        editor.replace_and_mark_text_in_range(Some(2..3), "ni", None, window, cx);
        editor.replace_and_mark_text_in_range(Some(0..4), "你", None, window, cx);
        assert_eq!(editor.snapshot().to_plain(), "A\nB\nC");
        editor.cancel_composition(cx);
        assert_eq!(editor.document().to_plain(), "A\nB\nC");
    });
    cx.run_until_parked();
    assert_eq!(count.get(), 0);
    editor.update_in(cx, |editor, window, cx| {
        editor.replace_and_mark_text_in_range(None, "ni", None, window, cx);
        editor.replace_text_in_range(None, "你", window, cx);
    });
    cx.run_until_parked();
    assert_eq!(count.get(), 1);
}

#[gpui::test]
fn asynchronous_paste_rejects_stale_selection_and_readonly_blocks_ime(cx: &mut TestAppContext) {
    cx.update(crate::theme::Theme::init);
    let (editor, cx) = cx.add_window_view(|window, cx| HtmlEmailEditor::new(EmailDocument::from_plain("hello").unwrap(), window, cx));
    editor.update_in(cx, |editor, window, cx| {
        let request = editor.request_paste(cx);
        editor.set_selection(EmailSelection::caret(editor.document().end_point()), cx);
        assert!(editor.complete_paste(request, EmailDocument::from_plain("late").unwrap(), cx).is_err());
        assert_eq!(editor.document().to_plain(), "hello");
        editor.set_read_only(true, cx);
        editor.replace_and_mark_text_in_range(None, "输入", None, window, cx);
        editor.replace_text_in_range(None, "输入", window, cx);
        assert_eq!(editor.document().to_plain(), "hello");
        assert_eq!(editor.request_image(cx), 0);
        assert!(!editor.can_undo());
    });
}

#[gpui::test]
fn list_continuation_hardbreak_and_emoji_delete_share_semantic_history(cx: &mut TestAppContext) {
    cx.update(crate::theme::Theme::init);
    let (editor, cx) = cx.add_window_view(|window, cx| HtmlEmailEditor::new(EmailDocument::default(), window, cx));
    editor.update(cx, |editor, cx| {
        editor.paragraph_format(EmailParagraphKind::Numbered(3), EmailAlign::Left, 0, cx).unwrap();
        editor.apply_format(EmailFormat::Bold(true), cx).unwrap();
        editor.insert_text("中😀", cx).unwrap();
        assert!(editor.current_style().bold);
        editor.newline(false, cx);
        assert_eq!(editor.current_paragraph().unwrap().kind, EmailParagraphKind::Numbered(4));
        editor.insert_text("👩‍👩‍👧‍👦", cx).unwrap();
        editor.delete_direction(false, cx);
        assert!(editor.current_paragraph().unwrap().content.text.is_empty());
        editor.undo(cx);
        assert_eq!(editor.current_paragraph().unwrap().content.text, "👩‍👩‍👧‍👦");
        editor.newline(true, cx);
        assert!(editor.current_paragraph().unwrap().content.text.ends_with('\u{2028}'));
    });
}

#[gpui::test]
fn platform_selection_services_use_model_utf16_and_visual_rectangles(cx: &mut TestAppContext) {
    cx.update(crate::theme::Theme::init);
    let (editor, cx) = cx.add_window_view(|window, cx| HtmlEmailEditor::new(EmailDocument::from_plain("中😀\nsecond").unwrap(), window, cx));
    editor.update_in(cx, |editor, window, cx| {
        editor.layout.arrange(&editor.document, gpui::px(300.), window);
        assert_eq!(editor.text_length_utf16(window, cx), Some(10));
        editor.set_selected_text_range(1..6, window, cx);
        assert_eq!(editor.selected_text_range(false, window, cx).unwrap().range, 1..6);
        let rectangles = editor.selection_bounds_for_range(1..6, gpui::Bounds::default(), window, cx);
        assert!(rectangles.len() >= 2);
        let bounds = editor.bounds_for_range(1..6, gpui::Bounds::default(), window, cx).unwrap();
        assert!(rectangles.iter().all(|rectangle| bounds.contains(&rectangle.origin)));
        editor.set_read_only(true, cx);
        assert!(!editor.accepts_text_input(window, cx));
    });
}

#[gpui::test]
fn outdated_paste_completion_preserves_newer_pending_request(cx: &mut TestAppContext) {
    cx.update(crate::theme::Theme::init);
    let (editor, cx) = cx.add_window_view(|window, cx| HtmlEmailEditor::new(EmailDocument::default(), window, cx));
    editor.update(cx, |editor, cx| {
        let first = editor.request_paste(cx);
        let second = editor.request_paste(cx);
        assert_ne!(first, second);
        assert!(editor.complete_paste(first, EmailDocument::from_plain("old").unwrap(), cx).is_err());
        assert_eq!(editor.document().to_plain(), "");
        editor.complete_paste(second, EmailDocument::from_plain("new").unwrap(), cx).unwrap();
        assert_eq!(editor.document().to_plain(), "new");
        assert!(editor.complete_paste(second, EmailDocument::from_plain("duplicate").unwrap(), cx).is_err());
        assert_eq!(editor.document().to_plain(), "new");
    });
}

#[gpui::test]
fn deletion_undo_restores_caret_and_arrows_collapse_without_extra_step(cx: &mut TestAppContext) {
    cx.update(crate::theme::Theme::init);
    let (editor,cx)=cx.add_window_view(|window,cx| HtmlEmailEditor::new(EmailDocument::from_plain("abc").unwrap(),window,cx));
    editor.update(cx,|editor,cx| {
        let end=editor.document.end_point(); editor.set_selection(EmailSelection::caret(end),cx);
        editor.delete_direction(false,cx); editor.undo(cx); assert_eq!(editor.selection(),EmailSelection::caret(end));
        let start=editor.document.first_point(); editor.set_selection(EmailSelection{anchor:start,head:end},cx);
        editor.move_horizontal(false,false,cx); assert_eq!(editor.selection(),EmailSelection::caret(start));
        editor.set_selection(EmailSelection{anchor:start,head:end},cx); editor.move_horizontal(true,false,cx); assert_eq!(editor.selection(),EmailSelection::caret(end));
    });
}

#[gpui::test]
fn toolbar_format_cancels_preview_and_redo_never_leaves_composition(cx: &mut TestAppContext) {
    cx.update(crate::theme::Theme::init);
    let (editor,cx)=cx.add_window_view(|window,cx| HtmlEmailEditor::new(EmailDocument::default(),window,cx));
    editor.update_in(cx,|editor,window,cx| {
        editor.insert_text("x",cx).unwrap(); editor.undo(cx);
        editor.replace_and_mark_text_in_range(None,"ni",None,window,cx);
        editor.redo(cx); assert_eq!(editor.document.to_plain(),"x"); assert!(editor.marked_text_range(window,cx).is_none());
        editor.replace_and_mark_text_in_range(None,"pinyin",None,window,cx);
        editor.apply_format(EmailFormat::Bold(true),cx).unwrap(); assert_eq!(editor.document.to_plain(),"x"); assert!(editor.marked_text_range(window,cx).is_none());
    });
}

#[gpui::test]
fn table_edits_normalize_offsets_and_tab_skips_covered_cells(cx: &mut TestAppContext) {
    cx.update(crate::theme::Theme::init);
    let document=EmailDocument::from_blocks(vec![EmailBlock{id:1,kind:EmailBlockKind::Table(EmailTable{columns:3,cells:vec![EmailText::plain("long"),EmailText::plain(""),EmailText::plain("z")],spans:vec![2,0,1]})}]).unwrap();
    let (editor,cx)=cx.add_window_view(|window,cx| HtmlEmailEditor::new(document,window,cx));
    editor.update(cx,|editor,cx| {
        editor.move_cell(true,cx); assert_eq!(editor.selection.head.cell,2);
        editor.move_cell(false,cx); assert_eq!(editor.selection.head.cell,0);
        editor.set_selection(EmailSelection::caret(EmailPoint{block:1,cell:0,offset:4}),cx);
        editor.edit_table(EmailTableEdit::DeleteColumn(0),cx).unwrap();
        editor.document.ordered_selection(&editor.selection).unwrap();
        editor.undo(cx); assert_eq!(editor.document.to_plain(),"long\t\tz");
    });
}

#[gpui::test]
fn transaction_validation_failure_restores_content_and_selection(cx: &mut TestAppContext) {
    cx.update(crate::theme::Theme::init);
    let (editor,cx)=cx.add_window_view(|window,cx| HtmlEmailEditor::new(EmailDocument::from_plain("original").unwrap(),window,cx));
    editor.update(cx,|editor,cx| {
        let before=editor.snapshot(); let selection=editor.selection();
        let result=editor.transaction(cx,|doc,selection| { doc.replace(selection,"changed",&EmailStyle::default())?; Ok(EmailSelection::caret(EmailPoint{block:9999,cell:0,offset:0})) });
        assert!(result.is_err()); assert_eq!(editor.document.blocks,before.blocks); assert_eq!(editor.selection(),selection); assert!(!editor.can_undo());
    });
}

#[gpui::test]
fn explicit_composition_target_inherits_target_style_and_marks_dirty(cx: &mut TestAppContext) {
    cx.update(crate::theme::Theme::init);
    let mut document=EmailDocument::from_plain("ab").unwrap(); let block=document.first_point().block;
    document.format(&EmailSelection{anchor:EmailPoint{block,cell:0,offset:0},head:EmailPoint{block,cell:0,offset:1}},&EmailFormat::Bold(true)).unwrap();
    let (editor,cx)=cx.add_window_view(|window,cx|HtmlEmailEditor::new(document,window,cx));
    editor.update_in(cx,|editor,window,cx| {
        editor.set_selection(EmailSelection::caret(editor.document.end_point()),cx);
        editor.text_input_dirty=false; editor.selection_dirty=false;
        let revision=editor.committed_revision();
        editor.replace_and_mark_text_in_range(Some(0..1),"中",None,window,cx);
        assert!(editor.document.text_at(editor.document.first_point()).unwrap().style_at(0).bold);
        assert!(editor.text_input_dirty && editor.selection_dirty); assert_eq!(editor.committed_revision(),revision);
        editor.cancel_composition(cx); assert_eq!(editor.document.to_plain(),"ab");
    });
}

#[gpui::test]
fn pending_bold_survives_caret_ime_and_backspace_cancels_before_targeting(cx: &mut TestAppContext) {
    cx.update(crate::theme::Theme::init);
    let (editor,cx)=cx.add_window_view(|window,cx|HtmlEmailEditor::new(EmailDocument::default(),window,cx));
    editor.update_in(cx,|editor,window,cx| {
        editor.replace_and_mark_text_in_range(None,"ni",None,window,cx); editor.delete_direction(false,cx);
        assert_eq!(editor.document.to_plain(),""); assert!(editor.problem.is_none());
        editor.apply_format(EmailFormat::Bold(true),cx).unwrap();
        editor.replace_and_mark_text_in_range(None,"zhong",None,window,cx); editor.replace_text_in_range(None,"中",window,cx);
        assert!(editor.document.text_at(editor.document.first_point()).unwrap().style_at(0).bold);
        editor.undo(cx); assert_eq!(editor.document.to_plain(),"");
    });
}

#[gpui::test]
fn inline_object_ime_replacement_and_undo_preserve_surrounding_text(cx: &mut TestAppContext) {
    cx.update(crate::theme::Theme::init);
    let (editor,cx)=cx.add_window_view(|window,cx|HtmlEmailEditor::new(EmailDocument::from_plain("left right").unwrap(),window,cx));
    editor.update_in(cx,|editor,window,cx| {
        let insertion=EmailPoint{offset:5,..editor.document.first_point()};editor.set_selection(EmailSelection::caret(insertion),cx);
        editor.insert_image(EmailImage{source:"cid:first".into(),alt:"first".into(),width:Some(40),height:Some(30),align:EmailAlign::Left},cx).unwrap();
        assert_eq!(editor.document.blocks.len(),1);assert_eq!(editor.current_image().unwrap().source,"cid:first");
        let before=editor.snapshot();editor.replace_and_mark_text_in_range(None,"ni",None,window,cx);editor.cancel_composition(cx);assert_eq!(editor.document.blocks,before.blocks);
        let request=editor.request_image(cx);editor.complete_image(request,EmailImage{source:"cid:second".into(),alt:"second".into(),width:Some(60),height:Some(30),align:EmailAlign::Left},cx).unwrap();
        assert_eq!(editor.current_image().unwrap().source,"cid:second");editor.undo(cx);assert_eq!(editor.current_image().unwrap().source,"cid:first");
        editor.delete_image(cx).unwrap();assert_eq!(editor.document.to_plain(),"left right");editor.undo(cx);assert_eq!(editor.current_image().unwrap().source,"cid:first");
        let map=editor.input_map();assert_eq!(map.utf16(editor.document.end_point()),11);
    });
}

#[gpui::test]
fn native_key_bindings_register_without_platform_parse_errors(cx: &mut TestAppContext) { cx.update(crate::init_for_tests); }

#[gpui::test]
fn clicking_native_body_routes_platform_input_and_keyboard_selection(cx:&mut TestAppContext){
    cx.update(crate::init_for_tests);
    let(editor,cx)=cx.add_window_view(|window,cx|HtmlEmailEditor::new(EmailDocument::from_plain("hello").unwrap(),window,cx));
    cx.run_until_parked();
    let body=cx.debug_bounds("html-email-editor").expect("native body root must draw");
    cx.simulate_click(body.origin+gpui::point(gpui::px(10.),gpui::px(16.)),gpui::Modifiers::none());
    cx.run_until_parked();
    cx.update(|window,cx|assert!(editor.read(cx).focus.is_focused(window),"body pointer event focuses its one native input client"));
    cx.simulate_input("中😀");cx.run_until_parked();
    assert_eq!(editor.read_with(cx,|editor,_|editor.document.to_plain()),"中😀hello");
    cx.simulate_keystrokes("shift-right");cx.simulate_input("X");cx.run_until_parked();
    assert_eq!(editor.read_with(cx,|editor,_|editor.document.to_plain()),"中😀Xello");
    cx.simulate_keystrokes("secondary-z");cx.run_until_parked();
    assert_eq!(editor.read_with(cx,|editor,_|editor.document.to_plain()),"中😀hello");
}

#[gpui::test]
fn keyboard_replace_all_across_paragraphs_restores_exact_selection_on_undo_redo(cx:&mut TestAppContext){
    cx.update(crate::init_for_tests);
    let(editor,cx)=cx.add_window_view(|window,cx|HtmlEmailEditor::new(EmailDocument::from_plain("first 中😀\nsecond paragraph").unwrap(),window,cx));
    cx.run_until_parked();
    let body=cx.debug_bounds("html-email-editor").expect("native body draws");
    cx.simulate_click(body.origin+gpui::point(gpui::px(10.),gpui::px(16.)),gpui::Modifiers::none());cx.run_until_parked();
    cx.simulate_keystrokes("secondary-a");cx.run_until_parked();
    let original=editor.read_with(cx,|editor,_|editor.snapshot());
    let selected=editor.read_with(cx,|editor,_|editor.selection());
    assert_eq!(selected,EmailSelection{anchor:original.first_point(),head:original.end_point()});
    cx.simulate_input("replacement 😀");cx.run_until_parked();
    let replaced=editor.read_with(cx,|editor,_|editor.snapshot());
    let caret=editor.read_with(cx,|editor,_|editor.selection());
    assert_eq!(replaced.to_plain(),"replacement 😀");assert_eq!(caret,EmailSelection::caret(replaced.end_point()));
    cx.simulate_keystrokes("secondary-z");cx.run_until_parked();
    editor.read_with(cx,|editor,_|{assert_eq!(editor.document.blocks,original.blocks);assert_eq!(editor.selection(),selected);});
    cx.simulate_keystrokes("secondary-shift-z");cx.run_until_parked();
    editor.read_with(cx,|editor,_|{assert_eq!(editor.document.blocks,replaced.blocks);assert_eq!(editor.selection(),caret);});
    cx.simulate_keystrokes("secondary-z");cx.run_until_parked();
    editor.read_with(cx,|editor,_|{assert_eq!(editor.document.blocks,original.blocks);assert_eq!(editor.selection(),selected);});
}

#[gpui::test]
fn typing_groups_split_after_timeout_style_paste_and_ime(cx:&mut TestAppContext){
    cx.update(crate::init_for_tests);let(editor,cx)=cx.add_window_view(|window,cx|HtmlEmailEditor::new(EmailDocument::default(),window,cx));
    editor.update_in(cx,|editor,window,cx|{
        editor.replace_text_in_range(None,"a",window,cx);editor.replace_text_in_range(None,"b",window,cx);
        editor.typing_group.as_mut().unwrap().at-=std::time::Duration::from_millis(901);
        editor.replace_text_in_range(None,"c",window,cx);editor.undo(cx);assert_eq!(editor.document.to_plain(),"ab");editor.undo(cx);assert_eq!(editor.document.to_plain(),"");
        editor.replace_text_in_range(None,"a",window,cx);editor.apply_format(EmailFormat::Bold(true),cx).unwrap();editor.replace_text_in_range(None,"b",window,cx);editor.undo(cx);assert_eq!(editor.document.to_plain(),"a");
        EntityInputHandler::paste(editor,gpui::ClipboardItem::new_string("paste".into()),window,cx);editor.replace_text_in_range(None,"x",window,cx);editor.undo(cx);assert_eq!(editor.document.to_plain(),"apaste");editor.undo(cx);assert_eq!(editor.document.to_plain(),"a");
        editor.replace_and_mark_text_in_range(None,"ni",None,window,cx);editor.replace_text_in_range(None,"你",window,cx);editor.replace_text_in_range(None,"z",window,cx);editor.undo(cx);assert_eq!(editor.document.to_plain(),"a你");editor.undo(cx);assert_eq!(editor.document.to_plain(),"a");
    });
}
