use gpui::{actions, App, ClipboardItem, Context, KeyBinding, Window};
use super::{editor::{EmailEditorEvent, HtmlEmailEditor}, model::*};
actions!(html_email, [Left, Right, Up, Down, SelectLeft, SelectRight, SelectUp, SelectDown, Home, End, SelectHome, SelectEnd, Backspace, Delete, Enter, HardBreak, SelectAll, Copy, Cut, Paste, Undo, Redo, Cancel, NextCell, PreviousCell]);
/// Registers keyboard bindings scoped to the native email body.
pub(crate) fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("tab", NextCell, Some("HtmlEmailEditor")), KeyBinding::new("shift-tab", PreviousCell, Some("HtmlEmailEditor")),
        KeyBinding::new("left", Left, Some("HtmlEmailEditor")), KeyBinding::new("right", Right, Some("HtmlEmailEditor")),
        KeyBinding::new("up", Up, Some("HtmlEmailEditor")), KeyBinding::new("down", Down, Some("HtmlEmailEditor")),
        KeyBinding::new("shift-left", SelectLeft, Some("HtmlEmailEditor")), KeyBinding::new("shift-right", SelectRight, Some("HtmlEmailEditor")),
        KeyBinding::new("shift-up", SelectUp, Some("HtmlEmailEditor")), KeyBinding::new("shift-down", SelectDown, Some("HtmlEmailEditor")),
        KeyBinding::new("home", Home, Some("HtmlEmailEditor")), KeyBinding::new("end", End, Some("HtmlEmailEditor")),
        KeyBinding::new("shift-home", SelectHome, Some("HtmlEmailEditor")), KeyBinding::new("shift-end", SelectEnd, Some("HtmlEmailEditor")),
        KeyBinding::new("backspace", Backspace, Some("HtmlEmailEditor")), KeyBinding::new("delete", Delete, Some("HtmlEmailEditor")),
        KeyBinding::new("enter", Enter, Some("HtmlEmailEditor")), KeyBinding::new("shift-enter", HardBreak, Some("HtmlEmailEditor")),
        KeyBinding::new(ely_core::secondary!("a"), SelectAll, Some("HtmlEmailEditor")), KeyBinding::new(ely_core::secondary!("c"), Copy, Some("HtmlEmailEditor")),
        KeyBinding::new(ely_core::secondary!("x"), Cut, Some("HtmlEmailEditor")), KeyBinding::new(ely_core::secondary!("v"), Paste, Some("HtmlEmailEditor")),
        KeyBinding::new(ely_core::secondary!("z"), Undo, Some("HtmlEmailEditor")), KeyBinding::new(ely_core::secondary!("shift-z"), Redo, Some("HtmlEmailEditor")), KeyBinding::new("escape", Cancel, Some("HtmlEmailEditor")),
    ]);
}
impl HtmlEmailEditor {
    pub(crate) fn move_cell(&mut self, forward: bool, cx: &mut Context<Self>) { if self.disabled { return; } self.cancel_composition(cx); let point = self.selection.head; if let Some(EmailBlock { kind: EmailBlockKind::Table(table), .. }) = self.document.blocks.iter().find(|block| block.id == point.block) { let cell = if forward { (point.cell + 1..table.cells.len()).find(|cell| table.spans.get(*cell) != Some(&0)) } else { (0..point.cell).rev().find(|cell| table.spans.get(*cell) != Some(&0)) }; if let Some(cell) = cell { self.set_selection(EmailSelection::caret(EmailPoint { block:point.block,cell,offset:0 }),cx); } } }
    pub(crate) fn move_horizontal(&mut self, forward: bool, extend: bool, cx: &mut Context<Self>) {
        if self.disabled { return; }
        if !extend && self.selection.anchor != self.selection.head { if let Ok((start,end)) = self.document.ordered_selection(&self.selection) { self.set_selection(EmailSelection::caret(if forward { end } else { start }), cx); } return; }
        // Prefer visual caret order, including mixed-direction fragments.
        let point = self.selection.head;
        let visual = self.layout.lines.iter().enumerate().filter(|(index,_)| self.affinity.is_none_or(|affinity| affinity == *index)).find_map(|(line_index, line)| if line.block == point.block && line.cell == point.cell { line.stops.iter().position(|(offset, _)| *offset == point.offset).map(|index| (line_index, index)) } else { None });
        let next = visual.and_then(|(line_index, index)| {
            let line = &self.layout.lines[line_index]; let mut stops = line.stops.as_ref().clone(); stops.sort_by(|a,b| f32::from(a.1).total_cmp(&f32::from(b.1))); stops.dedup_by(|a,b| a.0 == b.0 && a.1 == b.1); let current = stops.iter().position(|stop| stop.0 == line.stops[index].0)?;
            if forward { stops[current + 1..].iter().find(|stop| stop.0 != point.offset).map(|stop| EmailPoint { offset: stop.0, ..point }) } else { stops[..current].iter().rev().find(|stop| stop.0 != point.offset).map(|stop| EmailPoint { offset: stop.0, ..point }) }
        }).unwrap_or_else(|| self.logical_neighbor(forward));
        self.set_selection(EmailSelection { anchor: if extend { self.selection.anchor } else { next }, head: next }, cx);
        if let Some((index,_)) = visual { if self.layout.lines[index].stops.iter().any(|(offset,_)| *offset == next.offset) && self.layout.lines[index].block == next.block { self.affinity = Some(index); } }
    }
    fn logical_neighbor(&self, forward: bool) -> EmailPoint { self.input_map().neighbor(self.selection.head,forward) }
    pub(crate) fn move_vertical(&mut self, down: bool, extend: bool, cx: &mut Context<Self>) { if self.disabled { return; } if let Some((head, affinity)) = self.layout.vertical_neighbor(self.selection.head, self.affinity, down) { self.set_selection(EmailSelection { anchor: if extend { self.selection.anchor } else { head }, head }, cx); self.affinity = affinity; } }
    pub(crate) fn move_edge(&mut self, end: bool, extend: bool, cx: &mut Context<Self>) { if self.disabled { return; } if let Some((index,line)) = self.layout.lines.iter().enumerate().filter(|(index,_)| self.affinity.is_none_or(|affinity| affinity == *index)).find(|(_,line)| line.block == self.selection.head.block && line.cell == self.selection.head.cell && line.stops.iter().any(|(offset,_)| *offset == self.selection.head.offset)) { let edge = if end { line.stops.iter().max_by(|a,b| f32::from(a.1).total_cmp(&f32::from(b.1))) } else { line.stops.iter().min_by(|a,b| f32::from(a.1).total_cmp(&f32::from(b.1))) }; if let Some((offset,_)) = edge { let head = EmailPoint { offset: *offset, ..self.selection.head }; self.set_selection(EmailSelection { anchor: if extend { self.selection.anchor } else { head }, head }, cx); self.affinity = Some(index); } } }
    pub(crate) fn delete_direction(&mut self, forward: bool, cx: &mut Context<Self>) {
        use super::history::EmailHistory;
        if !self.editable() { return; }
        self.typing_group = None;
        self.cancel_composition(cx);
        let mut target = self.selection.clone();
        if target.anchor == target.head { target.head = self.logical_neighbor(forward); }
        let style = self.current_style();
        let result = (|| {
            let mut checkpoint = EmailHistory::begin(&self.document, &self.selection)?;
            EmailHistory::expand(&self.document, &mut checkpoint, &target)?;
            let edit = self.document.replace(&target, "", &style).and_then(|selection| {
                self.history.commit(&self.document, checkpoint.clone(), selection.clone())?;
                Ok(selection)
            });
            match edit {
                Ok(selection) => { self.selection = selection; self.affinity = None; self.changed(cx); Ok(()) }
                Err(error) => {
                    self.selection = EmailHistory::rollback(&mut self.document, &checkpoint)?;
                    self.input_cache.borrow_mut().take();
                    cx.notify();
                    Err(error)
                }
            }
        })();
        if let Err(error) = result { self.report(error, cx); }
    }
    pub(crate) fn newline(&mut self, hard: bool, cx: &mut Context<Self>) {
        if !self.editable() { return; }
        if !hard { if let Some(paragraph) = self.current_paragraph() { if paragraph.content.text.is_empty() && matches!(paragraph.kind, EmailParagraphKind::Bullet | EmailParagraphKind::Numbered(_) | EmailParagraphKind::Task(_)) { if let Err(error) = self.paragraph_format(EmailParagraphKind::Normal, paragraph.align, paragraph.indent, cx) { self.report(error, cx); } return; } } }
        if let Err(error) = self.insert_text(if hard { "\u{2028}" } else { "\n" }, cx) { self.report(error, cx); }
    }
    pub(crate) fn copy_selection(&mut self, cut: bool, cx: &mut Context<Self>) {
        if self.disabled || (cut && !self.editable()) { return; }
        match self.document.fragment(&self.selection) {
            Ok(fragment) => { let html = fragment.to_html(); let text = fragment.to_plain(); if self.host_clipboard { cx.emit(EmailEditorEvent::CopyRequested { html, text, cut }); } else { cx.write_to_clipboard(ClipboardItem::new_string_with_metadata(text, format!("ely-html-email:{html}"))); if cut { if let Err(error) = self.insert_text("", cx) { self.report(error, cx); } } } }, Err(error) => self.report(error, cx)
        }
    }
    pub(crate) fn paste_clipboard(&mut self, window: &mut Window, cx: &mut Context<Self>) { if !self.editable() { return; } if self.host_clipboard { self.request_paste(cx); } else if let Some(item) = cx.read_from_clipboard() { gpui::EntityInputHandler::paste(self, item, window, cx); } }
}
