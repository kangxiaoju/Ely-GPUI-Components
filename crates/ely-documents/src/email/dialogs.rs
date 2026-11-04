use gpui::{AnyElement, App, AppContext as _, ElementId, Entity, Focusable, InteractiveElement, IntoElement, ParentElement, Window};
use crate::{buttons::Button, forms::{Choice, ColorPicker, FontPicker, FormField, Input, Select, TextInput}, i18n, overlays::Dialog};
use super::{editor::HtmlEmailEditor, model::*};

#[derive(Clone, Copy)]
pub(crate) enum DialogKind { Font, Size, Color, Link, Image }
impl DialogKind {
    fn key(self) -> &'static str { match self { Self::Font => "email.font", Self::Size => "email.size", Self::Color => "email.color", Self::Link => "email.link", Self::Image => "email.image.properties" } }
}
struct Editing {
    kind: DialogKind,
    revision: u64,
    generation: u64,
    selection: EmailSelection,
    field: Entity<TextInput>,
    color: EmailColor,
    image: Option<ImageEditing>,
}
struct ImageEditing { original: EmailImage, width: Entity<TextInput>, height: Entity<TextInput>, align: EmailAlign }
#[derive(Default)]
pub(crate) struct EmailDialogs { editing: Option<Editing> }
impl EmailDialogs {
    pub(crate) fn open(state: &Entity<Self>, kind: DialogKind, editor: &Entity<HtmlEmailEditor>, window: &mut Window, cx: &mut App) {
        let native = editor.read(cx);
        if native.disabled() || native.read_only() { return; }
        let style = native.current_style();
        let image = native.current_image();
        if matches!(kind, DialogKind::Image) && image.is_none() { return; }
        let text = match kind { DialogKind::Font => style.font.unwrap_or_default(), DialogKind::Size => style.size.map(|size| size.to_string()).unwrap_or_default(), DialogKind::Color => String::new(), DialogKind::Link => style.link.unwrap_or_default(), DialogKind::Image => image.as_ref().map(|image| image.alt.clone()).unwrap_or_default() };
        let revision = native.document().revision;
        let generation = native.generation();
        let selection = native.selection();
        let color = style.color.unwrap_or(EmailColor { red: 0, green: 0, blue: 0 });
        let label = kind.key();
        let field = cx.new(|cx| { let mut field = TextInput::new(window, cx).label_key(label); field.set_text(text, cx); field });
        let image = image.filter(|_| matches!(kind, DialogKind::Image)).map(|image| {
            let width = cx.new(|cx| { let mut field = TextInput::new(window, cx).label_key("email.image.width"); field.set_text(image.width.map(|value| value.to_string()).unwrap_or_default(), cx); field });
            let height = cx.new(|cx| { let mut field = TextInput::new(window, cx).label_key("email.image.height"); field.set_text(image.height.map(|value| value.to_string()).unwrap_or_default(), cx); field });
            ImageEditing { align: image.align, original: image, width, height }
        });
        state.update(cx, |state, cx| { state.editing = Some(Editing { kind, revision, generation, selection, field, color, image }); cx.notify(); });
    }

    fn apply(state: &Entity<Self>, editor: &Entity<HtmlEmailEditor>, remove: bool, window: &mut Window, cx: &mut App) -> bool {
        let Some(editing) = &state.read(cx).editing else { return false; };
        let text = editing.field.read(cx).text().trim();
        if editor.read(cx).disabled() || editor.read(cx).read_only() {
            let message = i18n::text(cx, "email.readonly", &[]).to_string();
            editor.update(cx, |editor, cx| editor.set_problem(Some(message), cx));
            return false;
        }
        if matches!(editing.kind, DialogKind::Image) { return Self::apply_image(state, editor, remove, window, cx); }
        let format = match editing.kind {
            DialogKind::Font => EmailFormat::Font((!remove && !text.is_empty()).then(|| text.to_owned())),
            DialogKind::Size => {
                let size = if remove || text.is_empty() { None } else {
                    match text.parse::<u16>() { Ok(size) if (8..=72).contains(&size) => Some(size), _ => { let message = i18n::text(cx, "email.invalid_size", &[]).to_string(); editor.update(cx, |editor, cx| editor.set_problem(Some(message), cx)); return false; } }
                };
                EmailFormat::Size(size)
            },
            DialogKind::Color => EmailFormat::Color((!remove).then(|| editing.color.clone())),
            DialogKind::Link => EmailFormat::Link((!remove && !text.is_empty()).then(|| text.to_owned())),
            DialogKind::Image => return Self::apply_image(state, editor, remove, window, cx),
        };
        let revision = editing.revision;
        let selection = editing.selection.clone();
        if editor.read(cx).document().revision != revision || editor.read(cx).generation() != editing.generation {
            let message = i18n::text(cx, "email.stale", &[]).to_string();
            editor.update(cx, |editor, cx| editor.set_problem(Some(message), cx));
            return false;
        }
        let result = editor.update(cx, |editor, cx| {
            if editor.disabled() || editor.read_only() { return false; }
            if editor.selection() != selection { editor.set_selection(selection, cx); }
            match editor.apply_format(format, cx) {
                Ok(()) => true,
                Err(error) => { editor.set_problem(Some(error.to_string()), cx); false }
            }
        });
        if result { window.focus(&editor.focus_handle(cx), cx); }
        result
    }

    fn apply_image(state: &Entity<Self>, editor: &Entity<HtmlEmailEditor>, reset: bool, window: &mut Window, cx: &mut App) -> bool {
        let Some(editing) = &state.read(cx).editing else { return false; };
        let Some(image) = &editing.image else { return false; };
        if editor.read(cx).generation() != editing.generation || editor.read(cx).document().revision != editing.revision {
            let message = i18n::text(cx, "email.stale", &[]).to_string();
            editor.update(cx, |editor, cx| editor.set_problem(Some(message), cx));
            return false;
        }
        let dimension = |field: &Entity<TextInput>| -> Result<Option<u32>, ()> {
            let text = field.read(cx).text().trim();
            if text.is_empty() || reset { Ok(None) } else { text.parse::<u32>().ok().filter(|value| *value > 0).map(Some).ok_or(()) }
        };
        let (Ok(width), Ok(height)) = (dimension(&image.width), dimension(&image.height)) else {
            let message = i18n::text(cx, "email.image.invalid_size", &[]).to_string();
            editor.update(cx, |editor, cx| editor.set_problem(Some(message), cx));
            return false;
        };
        let next = EmailImage { source: image.original.source.clone(), alt: editing.field.read(cx).text().to_owned(), width, height, align: if reset { EmailAlign::Left } else { image.align } };
        let selection = editing.selection.clone();
        let result = editor.update(cx, |editor, cx| {
            if editor.disabled() || editor.read_only() { return false; }
            if editor.selection() != selection { editor.set_selection(selection, cx); }
            match editor.update_image(next, cx) { Ok(()) => true, Err(error) => { editor.set_problem(Some(error.to_string()), cx); false } }
        });
        if result { window.focus(&editor.focus_handle(cx), cx); }
        result
    }

    pub(crate) fn render(state: &Entity<Self>, id: &ElementId, editor: &Entity<HtmlEmailEditor>, _: &mut Window, cx: &mut App) -> Option<AnyElement> {
        let editing = state.read(cx).editing.as_ref()?;
        let field = editing.field.clone();
        let kind = editing.kind;
        let color = editing.color.clone();
        let disabled = editor.read(cx).read_only() || editor.read(cx).disabled();
        let image_fields = editing.image.as_ref().map(|image| (image.width.clone(), image.height.clone(), image.align));
        for input in std::iter::once(field.clone()).chain(image_fields.as_ref().into_iter().flat_map(|(width, height, _)| [width.clone(), height.clone()])) {
            if input.read(cx).is_disabled() != disabled { input.update(cx, |input, cx| input.set_disabled(disabled, cx)); }
        }
        let close_state = state.clone();
        let close_editor = editor.clone();
        let mut dialog = Dialog::new((id.clone(), "dialog"), i18n::text(cx, kind.key(), &[]), move |window, cx| {
            close_state.update(cx, |state, cx| { state.editing = None; cx.notify(); });
            window.focus(&close_editor.focus_handle(cx), cx);
        });
        if disabled {
            dialog = dialog.child(i18n::text(cx, "email.readonly", &[]));
        } else if matches!(kind, DialogKind::Color) {
            let target = state.clone();
            let rgb = ((color.red as u32) << 16) | ((color.green as u32) << 8) | color.blue as u32;
            dialog = dialog.held().child(ColorPicker::new((id.clone(), "color-picker"), gpui::rgb(rgb)).opaque().on_change(move |color, _, cx| {
                let rgba = color.to_rgb();
                target.update(cx, |state, cx| { if let Some(editing) = &mut state.editing { editing.color = EmailColor { red: (rgba.r * 255.0).round() as u8, green: (rgba.g * 255.0).round() as u8, blue: (rgba.b * 255.0).round() as u8 }; cx.notify(); } });
            }));
        } else if matches!(kind, DialogKind::Font) {
            let selected = field.read(cx).text().to_owned();
            let target = field.clone();
            dialog = dialog.held().focus_first(field.focus_handle(cx)).child(FontPicker::new((id.clone(), "font-picker"), &field).selected(selected).on_change(move |value, _, cx| target.update(cx, |field, cx| field.set_text(value.to_string(), cx))));
        } else if matches!(kind, DialogKind::Size) {
            let target = field.clone();
            let selected = field.read(cx).text().to_owned();
            dialog = dialog.held().child(Select::new((id.clone(), "size-picker"), (8..=72).map(|size| Choice::new(size.to_string(), format!("{size} pt")))).label(i18n::text(cx, "email.size", &[])).selected(selected).on_change(move |value, _, cx| target.update(cx, |field, cx| field.set_text(value.to_string(), cx))));
        } else if let Some((width, height, align)) = image_fields {
            let target = state.clone();
            let selected = match align { EmailAlign::Left => "left", EmailAlign::Center => "center", EmailAlign::Right => "right", EmailAlign::Justify => "justify" };
            dialog = dialog.held().focus_first(field.focus_handle(cx))
                .child(FormField::new((id.clone(), "alt"), i18n::text(cx, "email.image.alt", &[])).child(Input::new(&field)))
                .child(FormField::new((id.clone(), "width"), i18n::text(cx, "email.image.width", &[])).child(Input::new(&width)))
                .child(FormField::new((id.clone(), "height"), i18n::text(cx, "email.image.height", &[])).child(Input::new(&height)))
                .child(FormField::new((id.clone(), "align"), i18n::text(cx, "email.image.align", &[])).child(Select::new((id.clone(), "image-align"), ["left", "center", "right", "justify"].map(|key| Choice::new(key, i18n::text(cx, &format!("email.{key}"), &[])))).selected(selected).label(i18n::text(cx, "email.image.align", &[])).on_change(move |value, _, cx| target.update(cx, |state, cx| {
                    if let Some(image) = state.editing.as_mut().and_then(|editing| editing.image.as_mut()) { image.align = match value.as_ref() { "center" => EmailAlign::Center, "right" => EmailAlign::Right, "justify" => EmailAlign::Justify, _ => EmailAlign::Left }; cx.notify(); }
                }))));
        } else {
            dialog = dialog.focus_first(field.focus_handle(cx)).child(Input::new(&field));
        }
        let cancel_label = i18n::text(cx, "email.cancel", &[]);
        let remove_label = i18n::text(cx, "email.reset", &[]);
        let apply_label = i18n::text(cx, "email.apply", &[]);
        let cancel_id = (id.clone(), "cancel");
        let remove_id = (id.clone(), "reset");
        let apply_id = (id.clone(), "apply");
        let (remove_state, apply_state) = (state.clone(), state.clone());
        let (remove_editor, apply_editor) = (editor.clone(), editor.clone());
        Some(dialog.action(move |close| Button::new(cancel_id, cancel_label).on_click(move |_, window, cx| close(window, cx)))
            .action(move |close| Button::new(remove_id, remove_label).disabled(disabled).on_click(move |_, window, cx| { if Self::apply(&remove_state, &remove_editor, true, window, cx) { close(window, cx); } }))
            .action(move |close| gpui::div().debug_selector(|| "email-dialog-apply".into()).child(Button::new(apply_id, apply_label).primary().disabled(disabled).on_click(move |_, window, cx| { if Self::apply(&apply_state, &apply_editor, false, window, cx) { close(window, cx); } })))
            .into_any_element())
    }
}

#[cfg(all(test, feature = "test-support"))]
mod tests {
    use super::*;
    struct DialogStage { editor: Entity<HtmlEmailEditor>, dialogs: Entity<EmailDialogs> }
    impl gpui::Render for DialogStage {
        fn render(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
            use gpui::Styled;
            gpui::div().size_full().child(self.editor.clone()).children(EmailDialogs::render(&self.dialogs, &"test-dialog".into(), &self.editor, window, cx))
        }
    }

    #[gpui::test]
    fn actual_dialog_apply_click_preserves_pending_style(cx: &mut TestAppContext) {
        cx.update(Theme::init);
        let (stage, cx) = cx.add_window_view(|window, cx| {
            let editor = cx.new(|cx| HtmlEmailEditor::new(EmailDocument::default(), window, cx));
            let dialogs = cx.new(|_| EmailDialogs::default());
            editor.update(cx, |editor, cx| editor.apply_format(EmailFormat::Bold(true), cx).unwrap());
            EmailDialogs::open(&dialogs, DialogKind::Size, &editor, window, cx);
            let field = dialogs.read(cx).editing.as_ref().unwrap().field.clone();
            field.update(cx, |field, cx| field.set_text("18", cx));
            DialogStage { editor, dialogs }
        });
        cx.run_until_parked();
        cx.update(|window, _| { window.activate_window(); window.refresh(); });
        cx.run_until_parked();
        let button = cx.debug_bounds("email-dialog-apply").expect("real Ely dialog apply action");
        cx.simulate_click(button.center(), gpui::Modifiers::none());
        cx.run_until_parked();
        stage.read_with(cx, |stage, cx| {
            assert!(stage.dialogs.read(cx).editing.is_none());
            assert!(stage.editor.read(cx).current_style().bold);
            assert_eq!(stage.editor.read(cx).current_style().size, Some(18));
        });
    }
    #[gpui::test]
    fn open_dialog_disables_apply_when_host_changes_readonly(cx: &mut TestAppContext) {
        cx.update(Theme::init);
        let (stage, cx) = cx.add_window_view(|window, cx| {
            let editor = cx.new(|cx| HtmlEmailEditor::new(EmailDocument::default(), window, cx));
            let dialogs = cx.new(|_| EmailDialogs::default());
            EmailDialogs::open(&dialogs, DialogKind::Size, &editor, window, cx);
            let field = dialogs.read(cx).editing.as_ref().unwrap().field.clone();
            field.update(cx, |field, cx| field.set_text("18", cx));
            editor.update(cx, |editor, cx| editor.set_read_only(true, cx));
            DialogStage { editor, dialogs }
        });
        cx.run_until_parked();
        let button = cx.debug_bounds("email-dialog-apply").expect("disabled action remains visible");
        cx.simulate_click(button.center(), gpui::Modifiers::none());
        stage.read_with(cx, |stage, cx| {
            assert!(stage.dialogs.read(cx).editing.is_some());
            assert!(stage.dialogs.read(cx).editing.as_ref().unwrap().field.read(cx).is_disabled());
            assert_eq!(stage.editor.read(cx).current_style().size, None);
        });
    }
    use gpui::TestAppContext;
    use crate::theme::Theme;

    #[gpui::test]
    fn link_dialog_restores_saved_selection_and_never_inserts_markdown(cx: &mut TestAppContext) {
        cx.update(Theme::init);
        let (editor, cx) = cx.add_window_view(|window, cx| HtmlEmailEditor::new(EmailDocument::from_plain("link label").unwrap(), window, cx));
        cx.update(|window, cx| {
            let state = cx.new(|_| EmailDialogs::default());
            let selection = EmailSelection { anchor: editor.read(cx).document().first_point(), head: editor.read(cx).document().end_point() };
            editor.update(cx, |editor, cx| editor.set_selection(selection.clone(), cx));
            EmailDialogs::open(&state, DialogKind::Link, &editor, window, cx);
            let field = state.read(cx).editing.as_ref().unwrap().field.clone();
            field.update(cx, |field, cx| field.set_text("https://example.com", cx));
            assert!(EmailDialogs::apply(&state, &editor, false, window, cx));
            let editor = editor.read(cx);
            assert_eq!(editor.document().to_plain(), "link label");
            assert_eq!(editor.selection(), selection);
            assert_eq!(editor.document().text_at(selection.anchor).unwrap().style_at(0).link.as_deref(), Some("https://example.com"));
            assert!(editor.focus_handle(cx).is_focused(window));
        });
    }

    #[gpui::test]
    fn dialog_rejects_edits_after_document_revision_changes(cx: &mut TestAppContext) {
        cx.update(Theme::init);
        let (editor, cx) = cx.add_window_view(|window, cx| HtmlEmailEditor::new(EmailDocument::from_plain("original").unwrap(), window, cx));
        cx.update(|window, cx| {
            let state = cx.new(|_| EmailDialogs::default());
            EmailDialogs::open(&state, DialogKind::Font, &editor, window, cx);
            editor.update(cx, |editor, cx| editor.insert_text("new ", cx).unwrap());
            let before = editor.read(cx).snapshot();
            assert!(!EmailDialogs::apply(&state, &editor, false, window, cx));
            assert_eq!(editor.read(cx).snapshot(), before);
        });
    }
    #[gpui::test]
    fn dialog_preserves_pending_style_and_rejects_same_revision_replacement(cx: &mut TestAppContext) {
        cx.update(Theme::init);
        let (editor, cx) = cx.add_window_view(|window, cx| HtmlEmailEditor::new(EmailDocument::default(), window, cx));
        cx.update(|window, cx| {
            let state = cx.new(|_| EmailDialogs::default());
            editor.update(cx, |editor, cx| editor.apply_format(EmailFormat::Bold(true), cx).unwrap());
            EmailDialogs::open(&state, DialogKind::Size, &editor, window, cx);
            let field = state.read(cx).editing.as_ref().unwrap().field.clone();
            field.update(cx, |field, cx| field.set_text("18", cx));
            assert!(EmailDialogs::apply(&state, &editor, false, window, cx));
            assert!(editor.read(cx).current_style().bold);
            assert_eq!(editor.read(cx).current_style().size, Some(18));
            EmailDialogs::open(&state, DialogKind::Link, &editor, window, cx);
            editor.update(cx, |editor, cx| editor.set_document(EmailDocument::from_plain("replacement").unwrap(), cx).unwrap());
            assert!(!EmailDialogs::apply(&state, &editor, false, window, cx));
        });
    }

    #[gpui::test]
    fn image_properties_change_dimensions_without_replacing_source(cx: &mut TestAppContext) {
        cx.update(Theme::init);
        let document = EmailDocument::from_html("<img src=\"cid:photo\" alt=\"old\">").unwrap();
        let (editor, cx) = cx.add_window_view(|window, cx| HtmlEmailEditor::new(document, window, cx));
        cx.update(|window, cx| {
            let state = cx.new(|_| EmailDialogs::default());
            EmailDialogs::open(&state, DialogKind::Image, &editor, window, cx);
            let editing = state.read(cx).editing.as_ref().unwrap();
            let (alt, width, height) = (editing.field.clone(), editing.image.as_ref().unwrap().width.clone(), editing.image.as_ref().unwrap().height.clone());
            alt.update(cx, |field, cx| field.set_text("description", cx));
            width.update(cx, |field, cx| field.set_text("320", cx));
            height.update(cx, |field, cx| field.set_text("180", cx));
            assert!(EmailDialogs::apply(&state, &editor, false, window, cx));
            let image = editor.read(cx).current_image().unwrap();
            assert_eq!(image.source, "cid:photo");
            assert_eq!(image.alt, "description");
            assert_eq!((image.width, image.height), (Some(320), Some(180)));
        });
    }

    #[gpui::test]
    fn inline_image_dialog_preserves_surrounding_text_flow(cx: &mut TestAppContext) {
        cx.update(Theme::init);
        let document = EmailDocument::from_html("<p>left <img src=\"cid:inline\" alt=\"picture\"> right</p>").unwrap();
        let (editor, cx) = cx.add_window_view(|window, cx| HtmlEmailEditor::new(document, window, cx));
        cx.update(|window, cx| {
            let start = editor.read(cx).document().first_point();
            let point = EmailPoint { offset: 5, ..start };
            editor.update(cx, |editor, cx| editor.set_selection(EmailSelection { anchor: point, head: point }, cx));
            let state = cx.new(|_| EmailDialogs::default());
            EmailDialogs::open(&state, DialogKind::Image, &editor, window, cx);
            let width = state.read(cx).editing.as_ref().unwrap().image.as_ref().unwrap().width.clone();
            width.update(cx, |field, cx| field.set_text("240", cx));
            assert!(EmailDialogs::apply(&state, &editor, false, window, cx));
            assert_eq!(editor.read(cx).current_image().unwrap().width, Some(240));
            assert_eq!(editor.read(cx).document().to_plain(), "left picture right");
            assert_eq!(editor.read(cx).document().blocks.len(), 1);
            let html = editor.read(cx).document().to_html();
            assert!(html.find("left ").unwrap() < html.find("cid:inline").unwrap());
            assert!(html.find("cid:inline").unwrap() < html.find(" right").unwrap());
        });
    }
}
