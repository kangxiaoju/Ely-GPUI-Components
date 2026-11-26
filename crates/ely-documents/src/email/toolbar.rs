use gpui::{App, ElementId, Entity, Focusable, InteractiveElement, IntoElement, ParentElement, RenderOnce, Styled, Window, px};
use crate::{buttons::{IconButton, ToggleButton, ToggleItem}, i18n, menus::{DropdownMenu, Menu, MenuItem, OverflowMenu}, primitives::{IconName, Measure}, shell::{Toolbar, ToolbarGroup, ToolbarSeparator}, theme::ControlSize};
use super::{dialogs::{DialogKind, EmailDialogs}, editor::HtmlEmailEditor, model::*};

#[derive(Clone)]
pub(crate) enum Command {
    Format(EmailFormat), Undo, Redo, Kind(EmailParagraphKind), Align(EmailAlign), Indent(bool),
    Image, DeleteImage, Table, TableEdit(EmailTableEdit), Rule,
}

pub(crate) fn execute(editor: &Entity<HtmlEmailEditor>, command: Command, window: &mut Window, cx: &mut App) {
    editor.update(cx, |editor, cx| {
        if editor.read_only() || editor.disabled() { return; }
        let result = match command {
            Command::Format(format) => editor.apply_format(format, cx),
            Command::Undo => { editor.undo(cx); Ok(()) },
            Command::Redo => { editor.redo(cx); Ok(()) },
            Command::Image => { editor.request_image(cx); Ok(()) },
            Command::DeleteImage => editor.delete_image(cx),
            Command::Table => editor.insert_table(3, 3, cx),
            Command::TableEdit(edit) => editor.edit_table(edit, cx),
            Command::Rule => editor.insert_rule(cx),
            Command::Kind(kind) => {
                let current = uniform_paragraph(editor.document(), &editor.selection(), |paragraph| std::mem::discriminant(&paragraph.kind));
                let kind = if current == Some(std::mem::discriminant(&kind)) { EmailParagraphKind::Normal } else { kind };
                editor.set_paragraph_kind(kind, cx)
            },
            Command::Align(align) => editor.set_alignment(align, cx),
            Command::Indent(increase) => editor.change_indent(increase, cx),
        };
        if let Err(error) = result { editor.set_problem(Some(error.to_string()), cx); }
    });
    window.focus(&editor.focus_handle(cx), cx);
}

pub(crate) fn uniform_paragraph<T: PartialEq>(document: &EmailDocument, selection: &EmailSelection, value: impl Fn(&EmailParagraph) -> T) -> Option<T> {
    let (start, end) = document.ordered_selection(selection).ok()?;
    let mut inside = false;
    let mut uniform = None;
    for block in &document.blocks {
        if block.id == start.block { inside = true; }
        if !inside { continue; }
        if block.id == end.block && end.offset == 0 && start.block != end.block { break; }
        if let EmailBlockKind::Paragraph(paragraph) = &block.kind {
            let next = value(paragraph);
            if uniform.as_ref().is_some_and(|first| first != &next) { return None; }
            uniform = Some(next);
        }
        if block.id == end.block { break; }
    }
    uniform
}

/// Returns a uniform selection value, or `None` for a genuinely mixed selection.
/// A caret reads pending typing style rather than only the underlying run.
pub(crate) fn uniform_style<T: PartialEq>(document: &EmailDocument, selection: &EmailSelection, pending: &EmailStyle, value: impl Fn(&EmailStyle) -> T) -> Option<T> {
    if selection.anchor == selection.head { return Some(value(pending)); }
    let (start, end) = document.ordered_selection(selection).ok()?;
    let mut uniform = None;
    let mut inside = false;
    for block in &document.blocks {
        if block.id == start.block { inside = true; }
        if !inside { continue; }
        let texts: &[EmailText] = match &block.kind {
            EmailBlockKind::Paragraph(paragraph) => std::slice::from_ref(&paragraph.content),
            EmailBlockKind::Table(table) => &table.cells,
            _ => &[],
        };
        for (cell, text) in texts.iter().enumerate() {
            if block.id == start.block && cell < start.cell || block.id == end.block && cell > end.cell { continue; }
            let from = if block.id == start.block && cell == start.cell { start.offset } else { 0 };
            let to = if block.id == end.block && cell == end.cell { end.offset } else { text.text.len() };
            for run in &text.runs {
                if run.range.start < to && run.range.end > from {
                    let next = value(&run.style);
                    if uniform.as_ref().is_some_and(|first| first != &next) { return None; }
                    uniform = Some(next);
                }
            }
        }
        if block.id == end.block { break; }
    }
    uniform
}

/// Compact Ely controls for a native HTML email editor, with a measured overflow menu.
#[derive(IntoElement)]
pub struct EmailToolbar { id: ElementId, editor: Entity<HtmlEmailEditor> }
impl EmailToolbar {
    /// Creates tools bound directly to `editor`; commands retain its native selection.
    pub fn new(id: impl Into<ElementId>, editor: &Entity<HtmlEmailEditor>) -> Self { Self { id: id.into(), editor: editor.clone() } }
}
impl RenderOnce for EmailToolbar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let dialogs = window.use_keyed_state((self.id.clone(), "dialogs"), cx, |_, _| EmailDialogs::default());
        let width = window.use_keyed_state((self.id.clone(), "width"), cx, |_, _| px(0.0));
        let narrow = *width.read(cx) < px(840.0);
        let measured = width.clone();
        let editor = self.editor.read(cx);
        let disabled = editor.read_only() || editor.disabled();
        let style = editor.current_style();
        let selection = editor.selection();
        let mut inline = ToolbarGroup::new();
        let mut overflow = Menu::new();
        for (index, (key, icon, get, format)) in [
            ("bold", IconName::Bold, (|s: &EmailStyle| s.bold) as fn(&EmailStyle)->bool, EmailFormat::Bold as fn(bool)->EmailFormat),
            ("italic", IconName::Italic, |s: &EmailStyle| s.italic, EmailFormat::Italic),
            ("underline", IconName::Underline, |s: &EmailStyle| s.underline, EmailFormat::Underline),
            ("strike", IconName::Strikethrough, |s: &EmailStyle| s.strike, EmailFormat::Strike),
            ("code", IconName::Code, |s: &EmailStyle| s.code, EmailFormat::Code),
        ].into_iter().enumerate() {
            let active = uniform_style(editor.document(), &selection, &style, get);
            let mut label = i18n::text(cx, &format!("email.{key}"), &[]).to_string();
            if active.is_none() { label.push_str(&format!(" ({})", i18n::text(cx, "email.mixed", &[]))); }
            let command = Command::Format(format(active != Some(true)));
            let target = self.editor.clone();
            if !narrow || index < 3 {
                inline = inline.child(ToggleButton::new((self.id.clone(), key), ToggleItem::new(key).icon(icon).tooltip(label), active == Some(true)).size(ControlSize::Sm).disabled(disabled).on_toggle(move |_, window, cx| execute(&target, command.clone(), window, cx)));
            } else {
                overflow = overflow.item(MenuItem::check(label, active == Some(true)).icon(icon).disabled(disabled).on_click(move |window, cx| execute(&target, command.clone(), window, cx)));
            }
        }
        let item = |key: &'static str, command: Command, enabled: bool| {
            let target = self.editor.clone();
            MenuItem::new(i18n::text(cx, &format!("email.{key}"), &[])).disabled(disabled || !enabled).on_click(move |window, cx| execute(&target, command.clone(), window, cx))
        };
        let mut history = ToolbarGroup::new();
        for (key, icon, command, enabled) in [("undo", IconName::Undo2, Command::Undo, editor.can_undo()), ("redo", IconName::Redo2, Command::Redo, editor.can_redo())] {
            let target = self.editor.clone();
            history = history.child(IconButton::new((self.id.clone(), key), icon).size(ControlSize::Sm).tooltip(i18n::text(cx, &format!("email.{key}"), &[])).disabled(disabled || !enabled).on_click(move |_, window, cx| execute(&target, command.clone(), window, cx)));
        }
        let mut paragraph = Menu::new();
        for (key, kind) in [("normal", EmailParagraphKind::Normal), ("bullet", EmailParagraphKind::Bullet), ("numbered", EmailParagraphKind::Numbered(1)), ("task", EmailParagraphKind::Task(false)), ("quote", EmailParagraphKind::Quote)] {
            let uniform = uniform_paragraph(editor.document(), &selection, |paragraph| std::mem::discriminant(&paragraph.kind));
            let on = uniform == Some(std::mem::discriminant(&kind));
            let target = self.editor.clone();
            let label = i18n::text(cx, &format!("email.{key}"), &[]);
            let label = if uniform.is_none() { format!("{label} ({})", i18n::text(cx, "email.mixed", &[])) } else { label.to_string() };
            paragraph = paragraph.item(MenuItem::radio(label, on).disabled(disabled).on_click(move |window, cx| execute(&target, Command::Kind(kind.clone()), window, cx)));
        }
        for (key, align) in [("left", EmailAlign::Left), ("center", EmailAlign::Center), ("right", EmailAlign::Right), ("justify", EmailAlign::Justify)] {
            let uniform = uniform_paragraph(editor.document(), &selection, |paragraph| paragraph.align);
            let on = uniform == Some(align);
            let target = self.editor.clone();
            let label = i18n::text(cx, &format!("email.{key}"), &[]);
            let label = if uniform.is_none() { format!("{label} ({})", i18n::text(cx, "email.mixed", &[])) } else { label.to_string() };
            paragraph = paragraph.item(MenuItem::radio(label, on).disabled(disabled).on_click(move |window, cx| execute(&target, Command::Align(align), window, cx)));
        }
        paragraph = paragraph.separator().item(item("indent", Command::Indent(true), true)).item(item("outdent", Command::Indent(false), true));
        let mut table_menu = Menu::new().item(item("table", Command::Table, true));
        let table = editor.document().blocks.iter().find(|block| block.id == selection.head.block).and_then(|block| match &block.kind { EmailBlockKind::Table(table) => Some(table), _ => None });
        let (row, column) = table.map_or((0, 0), |table| (selection.head.cell / table.columns, selection.head.cell % table.columns));
        let colspan = table.and_then(|table| table.spans.get(selection.head.cell)).copied().unwrap_or(1);
        for (key, edit, enabled) in [
            ("row_add", EmailTableEdit::InsertRow(row + 1), true), ("row_delete", EmailTableEdit::DeleteRow(row), true),
            ("column_add", EmailTableEdit::InsertColumn(column + 1), true), ("column_delete", EmailTableEdit::DeleteColumn(column), true),
            ("merge", EmailTableEdit::Merge { row, column, columns: colspan + 1 }, table.is_some_and(|table| column + colspan < table.columns)),
            ("unmerge", EmailTableEdit::Unmerge { row, column }, colspan > 1),
        ] { table_menu = table_menu.item(item(key, Command::TableEdit(edit), table.is_some() && enabled)); }
        let mut insert = Menu::new();
        if editor.current_image().is_some() {
            let (state, target) = (dialogs.clone(), self.editor.clone());
            let image = Menu::new().item(MenuItem::new(i18n::text(cx, "email.image.properties", &[])).disabled(disabled).on_click(move |window, cx| EmailDialogs::open(&state, DialogKind::Image, &target, window, cx)))
                .item(item("image.replace", Command::Image, true)).item(item("image.delete", Command::DeleteImage, true));
            insert = insert.item(MenuItem::submenu(i18n::text(cx, "email.image", &[]), image));
        } else { insert = insert.item(item("image", Command::Image, true)); }
        insert = insert.item(MenuItem::submenu(i18n::text(cx, "email.table", &[]), table_menu)).item(item("rule", Command::Rule, true));
        for (key, kind) in [("font", DialogKind::Font), ("size", DialogKind::Size), ("color", DialogKind::Color), ("link", DialogKind::Link)] {
            let (state, target) = (dialogs.clone(), self.editor.clone());
            let label = i18n::text(cx, &format!("email.{key}"), &[]);
            let value = match kind {
                DialogKind::Font => uniform_style(editor.document(), &selection, &style, |style| style.font.clone()).map(|value| value.unwrap_or_default()),
                DialogKind::Size => uniform_style(editor.document(), &selection, &style, |style| style.size).map(|value| value.map(|size| size.to_string()).unwrap_or_default()),
                DialogKind::Color => uniform_style(editor.document(), &selection, &style, |style| style.color).map(|value| value.map(|color| format!("#{:02X}{:02X}{:02X}", color.red, color.green, color.blue)).unwrap_or_default()),
                DialogKind::Link => uniform_style(editor.document(), &selection, &style, |style| style.link.clone()).map(|value| value.unwrap_or_default()),
                DialogKind::Image => None,
            };
            let label = match value { None => format!("{label} ({})", i18n::text(cx, "email.mixed", &[])), Some(value) if !value.is_empty() => format!("{label}: {value}"), _ => label.to_string() };
            insert = insert.item(MenuItem::new(label).disabled(disabled).on_click(move |window, cx| EmailDialogs::open(&state, kind, &target, window, cx)));
        }
        insert = insert.item(item("clear", Command::Format(EmailFormat::Clear), true));
        overflow = overflow.item(MenuItem::submenu(i18n::text(cx, "email.paragraph", &[]), paragraph.clone())).item(MenuItem::submenu(i18n::text(cx, "email.insert", &[]), insert.clone()));
        let mut bar = Toolbar::new().min_w_0().child(history).child(ToolbarSeparator).child(inline);
        if !narrow {
            bar = bar.child(ToolbarSeparator).child(DropdownMenu::new((self.id.clone(), "paragraph"), i18n::text(cx, "email.paragraph", &[]), paragraph)).child(DropdownMenu::new((self.id.clone(), "insert"), i18n::text(cx, "email.insert", &[]), insert));
        }
        let bar = bar.child(gpui::div().debug_selector(|| "email-toolbar-more".into()).child(OverflowMenu::new((self.id.clone(), "more"), overflow).tooltip(i18n::text(cx, "email.more", &[]))));
        Measure::new((self.id.clone(), "measure"), move |bounds, _, cx| {
            if *measured.read(cx) != bounds.size.width { measured.update(cx, |width, cx| { *width = bounds.size.width; cx.notify(); }); }
        }).w_full().min_w_0().child(bar)
            .children(EmailDialogs::render(&dialogs, &self.id, &self.editor, window, cx))
    }
}

#[cfg(all(test, feature = "test-support"))]
#[path = "ui_tests.rs"]
mod ui_tests;
