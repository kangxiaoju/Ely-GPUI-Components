use gpui::{AppContext as _, Entity, Focusable, IntoElement, ParentElement, Render, Styled, TestAppContext};
use super::*;
use crate::email::{EmailDocument, EmailFormat, EmailPoint, EmailSelection};
use crate::theme::Theme;

struct Stage(Entity<HtmlEmailEditor>);
impl Render for Stage {
    fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        gpui::div().size_full().child(super::super::view::EmailEditorView::new("email", &self.0))
    }
}

#[gpui::test]
fn toolbar_formatting_preserves_literal_text_selection_and_focus(cx: &mut TestAppContext) {
    cx.update(Theme::init);
    let (stage, cx) = cx.add_window_view(|window, cx| Stage(cx.new(|cx| HtmlEmailEditor::new(EmailDocument::from_plain("**literal** 中文").unwrap(), window, cx))));
    let editor = stage.read_with(cx, |stage, _| stage.0.clone());
    cx.update(|window, cx| {
        let block = editor.read(cx).document().first_point().block;
        let selection = EmailSelection { anchor: EmailPoint { block, cell: 0, offset: 0 }, head: EmailPoint { block, cell: 0, offset: 11 } };
        editor.update(cx, |editor, cx| editor.set_selection(selection.clone(), cx));
        execute(&editor, Command::Format(EmailFormat::Bold(true)), window, cx);
        assert_eq!(editor.read(cx).document().to_plain(), "**literal** 中文");
        assert_eq!(editor.read(cx).selection(), selection);
        assert!(editor.read(cx).focus_handle(cx).is_focused(window));
        assert!(editor.read(cx).document().text_at(selection.anchor).unwrap().style_at(0).bold);
        execute(&editor, Command::Undo, window, cx);
        assert!(!editor.read(cx).document().text_at(selection.anchor).unwrap().style_at(0).bold);
        execute(&editor, Command::Redo, window, cx);
        assert!(editor.read(cx).document().text_at(selection.anchor).unwrap().style_at(0).bold);
    });
}

#[gpui::test]
fn caret_toolbar_format_is_pending_without_inserting_markup(cx: &mut TestAppContext) {
    cx.update(Theme::init);
    let (stage, cx) = cx.add_window_view(|window, cx| Stage(cx.new(|cx| HtmlEmailEditor::new(EmailDocument::default(), window, cx))));
    let editor = stage.read_with(cx, |stage, _| stage.0.clone());
    cx.update(|window, cx| {
        execute(&editor, Command::Format(EmailFormat::Italic(true)), window, cx);
        assert!(editor.read(cx).current_style().italic);
        assert_eq!(editor.read(cx).document().to_plain(), "");
        editor.update(cx, |editor, cx| editor.insert_text("typed", cx).unwrap());
        let point = editor.read(cx).document().first_point();
        assert!(editor.read(cx).document().text_at(point).unwrap().style_at(0).italic);
    });
}

#[test]
fn mixed_style_is_not_reported_as_uniform() {
    let mut document = EmailDocument::from_plain("one two").unwrap();
    let start = document.first_point();
    let selection = EmailSelection { anchor: start, head: EmailPoint { offset: 3, ..start } };
    document.format(&selection, &EmailFormat::Bold(true)).unwrap();
    let all = EmailSelection { anchor: start, head: document.end_point() };
    assert_eq!(uniform_style(&document, &all, &Default::default(), |style| style.bold), None);
    assert_eq!(uniform_style(&document, &selection, &Default::default(), |style| style.bold), Some(true));
}

#[gpui::test]
fn clicking_bold_changes_style_not_text_and_returns_native_focus(cx: &mut TestAppContext) {
    cx.update(Theme::init);
    let (stage, cx) = cx.add_window_view(|window, cx| Stage(cx.new(|cx| HtmlEmailEditor::new(EmailDocument::from_plain("literal ** stars").unwrap(), window, cx))));
    let editor = stage.read_with(cx, |stage, _| stage.0.clone());
    let selection = cx.update(|window, cx| {
        window.activate_window();
        let start = editor.read(cx).document().first_point();
        let selection = EmailSelection { anchor: start, head: EmailPoint { offset: 7, ..start } };
        editor.update(cx, |editor, cx| editor.set_selection(selection.clone(), cx));
        window.focus(&editor.focus_handle(cx), cx);
        selection
    });
    cx.run_until_parked();
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    let bold = cx.debug_bounds("toggle bold").expect("the real Ely bold toggle draws");
    cx.simulate_click(bold.center(), gpui::Modifiers::none());
    cx.run_until_parked();
    cx.update(|window, cx| {
        let editor = editor.read(cx);
        assert_eq!(editor.document().to_plain(), "literal ** stars");
        assert_eq!(editor.selection(), selection);
        assert!(editor.focus_handle(cx).is_focused(window));
        assert!(editor.document().text_at(selection.anchor).unwrap().style_at(0).bold);
    });
}

#[gpui::test]
fn all_inline_controls_update_native_styles_without_replacing_selected_text(cx: &mut TestAppContext) {
    cx.update(Theme::init);
    let (stage, cx) = cx.add_window_view(|window, cx| Stage(cx.new(|cx| HtmlEmailEditor::new(EmailDocument::from_plain("unchanged").unwrap(), window, cx))));
    let editor = stage.read_with(cx, |stage, _| stage.0.clone());
    cx.update(|window, cx| {
        let selection = EmailSelection { anchor: editor.read(cx).document().first_point(), head: editor.read(cx).document().end_point() };
        editor.update(cx, |editor, cx| editor.set_selection(selection.clone(), cx));
        for format in [EmailFormat::Bold(true), EmailFormat::Italic(true), EmailFormat::Underline(true), EmailFormat::Strike(true), EmailFormat::Code(true), EmailFormat::Font(Some("Arial".into())), EmailFormat::Size(Some(18)), EmailFormat::Color(Some(EmailColor { red: 140, green: 20, blue: 50 })), EmailFormat::Link(Some("https://example.com".into()))] {
            execute(&editor, Command::Format(format), window, cx);
            assert_eq!(editor.read(cx).document().to_plain(), "unchanged");
            assert_eq!(editor.read(cx).selection(), selection);
            assert!(editor.read(cx).focus_handle(cx).is_focused(window));
        }
        let style = editor.read(cx).document().text_at(selection.anchor).unwrap().style_at(0);
        assert!(style.bold && style.italic && style.underline && style.strike && style.code);
        assert_eq!(style.font.as_deref(), Some("Arial"));
        assert_eq!(style.size, Some(18));
        assert_eq!(style.link.as_deref(), Some("https://example.com"));
        execute(&editor, Command::Format(EmailFormat::Clear), window, cx);
        assert_eq!(editor.read(cx).document().text_at(selection.anchor).unwrap().style_at(0), EmailStyle::default());
    });
}

#[gpui::test]
fn reverse_cross_paragraph_commands_preserve_orthogonal_properties(cx: &mut TestAppContext) {
    cx.update(Theme::init);
    let document = EmailDocument::from_html("<blockquote>first</blockquote><p>second</p>").unwrap();
    let (stage, cx) = cx.add_window_view(|window, cx| Stage(cx.new(|cx| HtmlEmailEditor::new(document, window, cx))));
    let editor = stage.read_with(cx, |stage, _| stage.0.clone());
    cx.update(|window, cx| {
        let selection = EmailSelection { anchor: editor.read(cx).document().end_point(), head: editor.read(cx).document().first_point() };
        editor.update(cx, |editor, cx| editor.set_selection(selection.clone(), cx));
        assert_eq!(uniform_paragraph(editor.read(cx).document(), &selection, |paragraph| std::mem::discriminant(&paragraph.kind)), None);
        execute(&editor, Command::Align(EmailAlign::Center), window, cx);
        execute(&editor, Command::Indent(true), window, cx);
        let paragraphs = editor.read(cx).document().blocks.iter().filter_map(|block| match &block.kind { EmailBlockKind::Paragraph(paragraph) => Some(paragraph), _ => None }).collect::<Vec<_>>();
        assert_eq!(paragraphs[0].kind, EmailParagraphKind::Quote);
        assert_eq!(paragraphs[1].kind, EmailParagraphKind::Normal);
        assert!(paragraphs.iter().all(|paragraph| paragraph.align == EmailAlign::Center && paragraph.indent == 1));
        assert_eq!(editor.read(cx).selection(), selection);
    });
}

struct Narrow(Entity<HtmlEmailEditor>);
impl Render for Narrow {
    fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
        gpui::div().w(gpui::px(280.0)).h_full().child(super::super::view::EmailEditorView::new("narrow-email", &self.0))
    }
}

#[gpui::test]
fn narrow_toolbar_keeps_visible_toggles_inside_its_container(cx: &mut TestAppContext) {
    cx.update(Theme::init);
    let (_, cx) = cx.add_window_view(|window, cx| Narrow(cx.new(|cx| HtmlEmailEditor::new(EmailDocument::default(), window, cx))));
    cx.run_until_parked();
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    for selector in ["toggle bold", "toggle italic", "toggle underline"] {
        assert!(cx.debug_bounds(selector).expect("compact native formatting control").right() <= gpui::px(280.0));
    }
    assert!(cx.debug_bounds("toggle strike").is_none());
    assert!(cx.debug_bounds("toggle code").is_none());
}

#[gpui::test]
fn narrow_overflow_applies_hidden_code_command(cx: &mut TestAppContext) {
    cx.update(Theme::init);
    let (stage, cx) = cx.add_window_view(|window, cx| Narrow(cx.new(|cx| HtmlEmailEditor::new(EmailDocument::default(), window, cx))));
    let editor = stage.read_with(cx, |stage, _| stage.0.clone());
    cx.update(|window, cx| { window.activate_window(); window.focus(&editor.focus_handle(cx), cx); });
    cx.run_until_parked();
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    let more = cx.debug_bounds("email-toolbar-more").expect("overflow trigger");
    assert!(more.right() <= gpui::px(280.0));
    cx.simulate_click(more.center(), gpui::Modifiers::none());
    cx.run_until_parked();
    let code = cx.debug_bounds("menu-row-Code").expect("code remains available in overflow");
    cx.simulate_click(code.center(), gpui::Modifiers::none());
    cx.run_until_parked();
    cx.update(|window, cx| {
        assert!(editor.read(cx).current_style().code);
        assert_eq!(editor.read(cx).document().to_plain(), "");
        assert!(editor.focus_handle(cx).is_focused(window));
    });
}
