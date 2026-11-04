use std::{collections::HashMap, sync::Arc};
use gpui::{AnyElement, App, Context, EventEmitter, FocusHandle, Focusable, Render, Window};
use super::{model::*, history::{EmailCheckpoint, EmailHistory}};
use super::layout::EmailLayout;

/// Notifications emitted by the native HTML email body editor.
#[derive(Clone, Debug)]
pub enum EmailEditorEvent {
    /// The semantic document changed.
    Changed,
    /// The host should read its rich clipboard and complete this request.
    PasteRequested { /// Identifier passed back to the completion method.
        request: u64 },
    /// The host should write both MIME representations to its clipboard.
    CopyRequested { /// Sanitized HTML representation.
        html: String, /// Literal plain-text representation.
        text: String, /// Whether deletion should follow successful host copying.
        cut: bool },
    /// The host should pick and resolve an image.
    InsertImageRequested { /// Identifier passed back to image completion.
        request: u64 },
    /// An actionable editor error.
    Problem(String),
}

pub(crate) struct PendingRequest { pub id: u64, pub revision: u64, pub selection: EmailSelection }
pub(crate) struct Composition { pub checkpoint: EmailCheckpoint, pub marked: EmailSelection }
pub(crate) struct TypingGroup { pub(crate) at:web_time::Instant, pub(crate) head:EmailPoint, pub(crate) style:EmailStyle, pub(crate) revision:u64 }

/// Native, model-backed HTML editor with one platform input client.
pub struct HtmlEmailEditor {
    pub(crate) document: EmailDocument,
    pub(crate) selection: EmailSelection,
    pub(crate) focus: FocusHandle,
    pub(crate) history: EmailHistory,
    pub(crate) composition: Option<Composition>,
    pub(crate) typing_style: Option<EmailStyle>,
    pub(crate) typing_group: Option<TypingGroup>,
    pub(crate) layout: EmailLayout,
    pub(crate) images: HashMap<String, Arc<gpui::Image>>,
    pub(crate) scroll: gpui::ScrollHandle,
    pub(crate) input_cache: std::cell::RefCell<Option<(u64, Arc<super::input::InputMap>)>>,
    pub(crate) affinity: Option<usize>,
    generation: u64,
    pub(crate) text_input_dirty: bool,
    pub(crate) selection_dirty: bool,
    pub(crate) reveal_selection: bool,
    committed_revision: u64,
    committed_empty: bool,
    committed_bytes: usize,
    committed_blank: bool,
    pub(crate) dragging: bool,
    pub(crate) readonly: bool,
    pub(crate) disabled: bool,
    pub(crate) problem: Option<String>,
    pub(crate) host_clipboard: bool,
    next_request: u64,
    paste_request: Option<PendingRequest>,
    image_request: Option<PendingRequest>,
}
impl HtmlEmailEditor {
    /// Creates a body editor; the separate view adds the formatting toolbar.
    pub fn new(document: EmailDocument, _: &mut Window, cx: &mut Context<Self>) -> Self {
        let selection = EmailSelection { anchor: document.first_point(), head: document.first_point() };
        let committed_empty = document.is_empty();
        let committed_bytes = document.estimated_bytes();
        let committed_blank = document.is_blank();
        Self { document, selection, focus: cx.focus_handle(), history: EmailHistory::default(), composition: None, typing_style: None, typing_group:None, layout: EmailLayout::default(), images: HashMap::new(), scroll: gpui::ScrollHandle::new(), input_cache: Default::default(), affinity: None, generation: 0, text_input_dirty: true, selection_dirty: true, reveal_selection: true, committed_revision: 0, committed_empty, committed_bytes, committed_blank, dragging: false, readonly: false, disabled: false, problem: None, host_clipboard: false, next_request: 0, paste_request: None, image_request: None }
    }
    /// Monotonic committed-content identity, unchanged by composition previews.
    pub fn committed_revision(&self) -> u64 { self.committed_revision }
    /// Cached semantic emptiness of the committed document, excluding IME previews.
    pub fn committed_is_empty(&self) -> bool { self.committed_empty }
    /// Cached semantic byte estimate; excludes MIME markup and attachments.
    pub fn committed_estimated_bytes(&self) -> usize { self.committed_bytes }
    /// Whether committed paragraphs contain only whitespace and no atomic content.
    pub fn committed_is_blank(&self) -> bool { self.committed_blank }
    /// Supplies already-resolved local image assets keyed by their full source URI.
    /// Remote sources are never fetched implicitly by the editor.
    pub fn set_images(&mut self, images: HashMap<String, Arc<gpui::Image>>, cx: &mut Context<Self>) { self.images = images; cx.notify(); }
    /// Borrows the semantic source of truth.
    pub fn document(&self) -> &EmailDocument { &self.document }
    /// Takes an owned document snapshot for saving/export.
    pub fn snapshot(&self) -> EmailDocument { let mut document = self.document.clone(); if let Some(composition) = &self.composition { let _ = EmailHistory::rollback(&mut document, &composition.checkpoint); } document }
    /// Returns the directed model selection.
    pub fn selection(&self) -> EmailSelection { self.selection.clone() }
    /// Changes the selection if both model points are valid.
    pub fn set_selection(&mut self, selection: EmailSelection, cx: &mut Context<Self>) {
        self.typing_group=None;
        if self.document.ordered_selection(&selection).is_ok() { self.reveal_selection |= self.selection != selection; self.selection = selection; self.affinity = None; self.typing_style = None; self.selection_dirty = true; cx.notify(); }
    }
    /// Replaces the whole document and clears history and pending composition.
    pub fn set_document(&mut self, document: EmailDocument, cx: &mut Context<Self>) -> Result<(), EmailError> {
        self.generation = self.generation.wrapping_add(1);
        document.validate()?; self.document = document; self.selection = EmailSelection { anchor: self.document.first_point(), head: self.document.first_point() }; self.composition = None; self.history.clear(); self.layout = EmailLayout::default(); self.input_cache.borrow_mut().take(); self.paste_request = None; self.image_request = None; self.changed(cx); Ok(())
    }
    /// Identifies whole-document replacements independently of model revisions.
    pub fn generation(&self) -> u64 { self.generation }
    /// Changes only alignment, preserving each paragraph's kind and indent.
    pub fn set_alignment(&mut self, align: EmailAlign, cx: &mut Context<Self>) -> Result<(), EmailError> { self.transaction(cx, |doc, selection| { doc.set_alignment(selection, align)?; Ok(selection.clone()) }) }
    /// Changes each selected paragraph's own indentation by one level.
    pub fn change_indent(&mut self, increase: bool, cx: &mut Context<Self>) -> Result<(), EmailError> { self.transaction(cx, |doc, selection| { doc.change_indent(selection, increase)?; Ok(selection.clone()) }) }
    /// Changes only semantic paragraph kind.
    pub fn set_paragraph_kind(&mut self, kind: EmailParagraphKind, cx: &mut Context<Self>) -> Result<(), EmailError> { self.transaction(cx, |doc, selection| { doc.set_paragraph_kind(selection, kind)?; Ok(selection.clone()) }) }
    /// Returns the active atomic image, if any.
    pub fn current_image(&self) -> Option<EmailImage> { self.document.image_at(self.selection.head).cloned() }
    /// Updates the active image's source or properties as one undo transaction.
    pub fn update_image(&mut self, image: EmailImage, cx: &mut Context<Self>) -> Result<(), EmailError> { self.transaction(cx, |doc, selection| { doc.update_image(selection.head, image)?; Ok(selection.clone()) }) }
    /// Deletes the active image as one atomic operation.
    pub fn delete_image(&mut self, cx: &mut Context<Self>) -> Result<(), EmailError> { self.transaction(cx, |doc,selection| doc.delete_image(selection.head)) }
    pub(crate) fn editable(&self) -> bool { !self.readonly && !self.disabled }
    pub(crate) fn changed(&mut self, cx: &mut Context<Self>) { self.committed_revision = self.committed_revision.wrapping_add(1); self.committed_empty = self.document.is_empty(); self.committed_bytes = self.document.estimated_bytes(); self.committed_blank = self.document.is_blank(); self.text_input_dirty = true; self.selection_dirty = true; self.reveal_selection = true; cx.emit(EmailEditorEvent::Changed); cx.notify(); }
    pub(crate) fn report(&mut self, error: EmailError, cx: &mut Context<Self>) { let message = error.to_string(); self.problem = Some(message.clone()); cx.emit(EmailEditorEvent::Problem(message)); cx.notify(); }
    pub(crate) fn transaction(&mut self, cx: &mut Context<Self>, edit: impl FnOnce(&mut EmailDocument, &EmailSelection) -> Result<EmailSelection, EmailError>) -> Result<(), EmailError> {
        if !self.editable() { return Err(EmailError::Invalid("Editor is read-only or disabled".into())); }
        self.typing_group=None;
        self.cancel_composition(cx);
        let checkpoint = EmailHistory::begin(&self.document, &self.selection)?;
        match edit(&mut self.document, &self.selection) {
            Ok(selection) => { if let Err(error) = self.history.commit(&self.document, checkpoint.clone(), selection.clone()) { self.selection = EmailHistory::rollback(&mut self.document, &checkpoint)?; self.input_cache.borrow_mut().take(); cx.notify(); return Err(error); } self.selection = selection; self.affinity = None; self.changed(cx); Ok(()) }
            Err(error) => { self.selection = EmailHistory::rollback(&mut self.document, &checkpoint)?; Err(error) }
        }
    }
    /// Inserts literal text, splitting paragraphs only on newlines.
    pub fn insert_text(&mut self, text: &str, cx: &mut Context<Self>) -> Result<(), EmailError> { let style = self.current_style(); self.transaction(cx, |doc, selection| doc.replace(selection, text, &style)) }
    pub(crate) fn type_text(&mut self,text:&str,cx:&mut Context<Self>)->Result<(),EmailError>{
        if !self.editable(){return Err(EmailError::Invalid("Editor is read-only or disabled".into()));}
        self.cancel_composition(cx);let now=web_time::Instant::now();let style=self.current_style();
        let join=!text.is_empty()&&!text.contains(['\n','\u{2028}'])&&self.selection.anchor==self.selection.head&&self.typing_group.as_ref().is_some_and(|group|group.head==self.selection.head&&group.style==style&&group.revision==self.document.revision&&now.duration_since(group.at)<std::time::Duration::from_millis(900));
        let checkpoint=EmailHistory::begin(&self.document,&self.selection)?;
        let selection=match self.document.replace(&self.selection,text,&style){Ok(selection)=>selection,Err(error)=>{self.selection=EmailHistory::rollback(&mut self.document,&checkpoint)?;return Err(error);}};
        let recorded=if join{self.history.amend_typing(&self.document,selection.clone()).and_then(|amended|if amended{Ok(())}else{self.history.commit(&self.document,checkpoint.clone(),selection.clone())})}else{self.history.commit(&self.document,checkpoint.clone(),selection.clone())};
        if let Err(error)=recorded{self.selection=EmailHistory::rollback(&mut self.document,&checkpoint)?;self.typing_group=None;return Err(error);}
        self.selection=selection;self.affinity=None;self.changed(cx);self.typing_group=(!text.is_empty()&&!text.contains(['\n','\u{2028}'])).then_some(TypingGroup{at:now,head:self.selection.head,style,revision:self.document.revision});Ok(())
    }
    /// Imports sanitized HTML and inserts its semantic fragment.
    pub fn insert_html(&mut self, html: &str, cx: &mut Context<Self>) -> Result<(), EmailError> { self.insert_document(EmailDocument::from_html(html)?, cx) }
    /// Replaces the selection with an imported semantic fragment.
    pub fn insert_document(&mut self, document: EmailDocument, cx: &mut Context<Self>) -> Result<(), EmailError> { document.validate()?; self.transaction(cx, |doc, selection| doc.replace_fragment(selection, &document)) }
    /// Applies inline formatting or changes the insertion style at a caret.
    pub fn apply_format(&mut self, format: EmailFormat, cx: &mut Context<Self>) -> Result<(), EmailError> {
        self.typing_group=None;
        self.cancel_composition(cx);
        if self.selection.anchor == self.selection.head {
            if !self.editable() { return Err(EmailError::Invalid("Editor is read-only or disabled".into())); }
            let mut text = EmailText::plain("x"); text.runs[0].style = self.current_style(); text.format(0..1, &format)?; self.typing_style = Some(text.style_at(0)); cx.notify(); Ok(())
        } else { self.transaction(cx, |doc, selection| { doc.format(selection, &format)?; Ok(selection.clone()) }) }
    }
    /// Changes the selected paragraphs' block formatting.
    pub fn paragraph_format(&mut self, kind: EmailParagraphKind, align: EmailAlign, indent: u8, cx: &mut Context<Self>) -> Result<(), EmailError> { self.transaction(cx, |doc, selection| { doc.paragraph(selection, kind, align, indent)?; Ok(selection.clone()) }) }
    /// Returns formatting at the active caret.
    pub fn current_style(&self) -> EmailStyle { self.typing_style.clone().unwrap_or_else(|| self.document.text_at(self.selection.head).map(|text| { let offset = text.text.char_indices().take_while(|(offset, _)| *offset < self.selection.head.offset).last().map(|(offset, _)| offset).unwrap_or(0); text.style_at(offset) }).unwrap_or_default()) }
    /// Returns the active paragraph, if the caret is in a paragraph.
    pub fn current_paragraph(&self) -> Option<EmailParagraph> { self.document.blocks.iter().find(|b| b.id == self.selection.head.block).and_then(|b| match &b.kind { EmailBlockKind::Paragraph(p) => Some(p.clone()), _ => None }) }
    /// Inserts an image as a semantic atomic block.
    pub fn insert_image(&mut self, image: EmailImage, cx: &mut Context<Self>) -> Result<(), EmailError> { self.transaction(cx, |doc, selection| doc.insert_image(selection, image)) }
    /// Inserts a rectangular table.
    pub fn insert_table(&mut self, rows: usize, columns: usize, cx: &mut Context<Self>) -> Result<(), EmailError> { self.transaction(cx, |doc, selection| doc.insert_table(selection, rows, columns)) }
    /// Edits the active table and records it as one undo transaction.
    pub fn edit_table(&mut self, edit: EmailTableEdit, cx: &mut Context<Self>) -> Result<(), EmailError> {
        let id = self.selection.head.block;
        self.transaction(cx, |doc, selection| { doc.table_edit(id, &edit)?; let normalize = |mut point: EmailPoint| { if let Some(EmailBlock { kind: EmailBlockKind::Table(table), .. }) = doc.blocks.iter().find(|block| block.id == point.block) { point.cell = point.cell.min(table.cells.len().saturating_sub(1)); while point.cell > 0 && table.spans.get(point.cell) == Some(&0) { point.cell -= 1; } } if let Some(text) = doc.text_at(point) { point.offset = point.offset.min(text.text.len()); while !text.text.is_char_boundary(point.offset) { point.offset -= 1; } point } else { doc.first_point() } }; Ok(EmailSelection { anchor: normalize(selection.anchor), head: normalize(selection.head) }) })
    }
    /// Inserts a horizontal rule.
    pub fn insert_rule(&mut self, cx: &mut Context<Self>) -> Result<(), EmailError> { self.transaction(cx, |doc, selection| doc.insert_rule(selection)) }
    /// Restores the preceding semantic transaction and directed selection.
    pub fn undo(&mut self, cx: &mut Context<Self>) { if !self.editable() { return; } self.typing_group=None; self.cancel_composition(cx); if let Some(selection) = self.history.undo(&mut self.document) { self.selection = selection; self.changed(cx); } }
    /// Reapplies the next semantic transaction.
    pub fn redo(&mut self, cx: &mut Context<Self>) { if !self.editable() { return; } self.typing_group=None; self.cancel_composition(cx); if let Some(selection) = self.history.redo(&mut self.document) { self.selection = selection; self.affinity = None; self.changed(cx); } }
    /// Whether an undo transaction is available.
    pub fn can_undo(&self) -> bool { self.history.can_undo() }
    /// Whether a redo transaction is available.
    pub fn can_redo(&self) -> bool { self.history.can_redo() }
    /// Whether mutation is prohibited while selection remains available.
    pub fn read_only(&self) -> bool { self.readonly }
    /// Whether all interaction is disabled.
    pub fn disabled(&self) -> bool { self.disabled }
    /// Changes read-only mode.
    pub fn set_read_only(&mut self, value: bool, cx: &mut Context<Self>) { self.cancel_composition(cx); self.readonly = value; cx.notify(); }
    /// Changes disabled mode.
    pub fn set_disabled(&mut self, value: bool, cx: &mut Context<Self>) { self.cancel_composition(cx); self.disabled = value; cx.notify(); }
    /// Sets the visible validation/platform error.
    pub fn set_problem(&mut self, problem: Option<String>, cx: &mut Context<Self>) { self.problem = problem; cx.notify(); }
    /// Delegates rich clipboard operations to the embedding application.
    pub fn set_host_clipboard(&mut self, enabled: bool) { self.host_clipboard = enabled; }
    fn request(&mut self) -> PendingRequest { self.next_request = self.next_request.wrapping_add(1); PendingRequest { id: self.next_request, revision: self.document.revision, selection: self.selection.clone() } }
    /// Requests a host-provided image without losing the insertion selection.
    pub fn request_image(&mut self, cx: &mut Context<Self>) -> u64 { if !self.editable() { return 0; } let request = self.request(); let id = request.id; self.image_request = Some(request); cx.emit(EmailEditorEvent::InsertImageRequested { request: id }); id }
    /// Requests a rich clipboard read from the embedding application.
    pub fn request_paste(&mut self, cx: &mut Context<Self>) -> u64 { if !self.editable() { return 0; } let request = self.request(); let id = request.id; self.paste_request = Some(request); cx.emit(EmailEditorEvent::PasteRequested { request: id }); id }
    fn accept(&mut self, pending: Option<PendingRequest>, request: u64) -> Result<(), EmailError> { let pending = pending.ok_or_else(|| EmailError::Invalid("Clipboard/image request has expired".into()))?; if pending.id != request || pending.revision != self.document.revision || pending.selection != self.selection || !self.editable() { return Err(EmailError::Invalid("Document or selection changed while the request was pending".into())); } self.selection = pending.selection; Ok(()) }
    /// Completes a paste only when its revision and request still match.
    pub fn complete_paste(&mut self, request: u64, document: EmailDocument, cx: &mut Context<Self>) -> Result<(), EmailError> { let pending = if self.paste_request.as_ref().is_some_and(|pending| pending.id == request) { self.paste_request.take() } else { None }; self.accept(pending, request)?; self.insert_document(document, cx) }
    /// Completes an image request only against its original revision.
    pub fn complete_image(&mut self, request: u64, image: EmailImage, cx: &mut Context<Self>) -> Result<(), EmailError> { let replacement = self.image_request.as_ref().is_some_and(|pending| pending.id == request); let pending = if replacement { self.image_request.take() } else if self.paste_request.as_ref().is_some_and(|pending| pending.id == request) { self.paste_request.take() } else { None }; self.accept(pending, request)?; if replacement && self.current_image().is_some() { self.update_image(image, cx) } else { self.insert_image(image, cx) } }
    pub(crate) fn body(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement { super::element::body(self, window, cx) }
}
impl EventEmitter<EmailEditorEvent> for HtmlEmailEditor {}
impl Focusable for HtmlEmailEditor { fn focus_handle(&self, _: &App) -> FocusHandle { self.focus.clone() } }
impl Render for HtmlEmailEditor { fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl gpui::IntoElement { self.body(window, cx) } }
