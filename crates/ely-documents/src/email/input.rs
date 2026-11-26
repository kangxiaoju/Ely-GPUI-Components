use std::{collections::HashMap, ops::Range, sync::Arc};
use gpui::{Bounds, ClipboardItem, Context, EntityInputHandler, Pixels, Point, UTF16Selection, Window};
use super::{editor::{Composition, HtmlEmailEditor}, history::EmailHistory, model::*};

struct BlockProjection { id: u64, epoch:u64, text: String, points: Vec<EmailPoint>, bytes: Vec<usize>, reverse: HashMap<(usize,usize),usize> }
/// Segmented platform projection: editing one block never copies unrelated text.
#[derive(Clone)]
pub(crate) struct InputMap { blocks: Vec<Arc<BlockProjection>>, lengths: super::prefix::PrefixIndex, indices: HashMap<u64,usize>, len: usize, revision:u64 }
impl BlockProjection {
    fn new(block: &EmailBlock,epoch:u64) -> Self {
        let mut projection = Self { id: block.id, epoch, text: String::new(), points: vec![EmailPoint { block: block.id, cell: 0, offset: 0 }], bytes: vec![0], reverse: HashMap::new() };
        match &block.kind {
            EmailBlockKind::Paragraph(paragraph) => projection.push_text(0, &paragraph.content.text),
            EmailBlockKind::Table(table) => { let mut first = true; for (cell,text) in table.cells.iter().enumerate() { if table.spans.get(cell) == Some(&0) { continue; } if !first { projection.text.push(if cell % table.columns == 0 { '\n' } else { '\t' }); projection.points.push(EmailPoint { block: block.id, cell, offset: 0 }); projection.bytes.push(projection.text.len()); } first = false; projection.push_text(cell, &text.text); } },
            _ => { projection.text.push('\u{fffc}'); projection.points.push(EmailPoint { block: block.id, cell: 0, offset: 1 }); projection.bytes.push(projection.text.len()); }
        }
        for (index,point) in projection.points.iter().enumerate() { projection.reverse.entry((point.cell,point.offset)).or_insert(index); }
        projection
    }
    fn push_text(&mut self, cell: usize, text: &str) {
        let block = self.id; let base = self.text.len();
        *self.points.last_mut().unwrap() = EmailPoint { block,cell,offset:0 };
        for (offset,c) in text.char_indices() { self.text.push(c); for _ in 1..c.len_utf16() { self.points.push(EmailPoint {block,cell,offset}); self.bytes.push(base + offset); } self.points.push(EmailPoint {block,cell,offset:offset+c.len_utf8()}); self.bytes.push(base+offset+c.len_utf8()); }
    }
}
impl InputMap {
    pub(crate) fn new(document: &EmailDocument) -> Self { let mut map=Self{blocks:Vec::new(),lengths:Default::default(),indices:HashMap::new(),len:0,revision:document.revision};map.rebuild(document);map }
    fn rebuild(&mut self,document:&EmailDocument){let previous:HashMap<_,_>=std::mem::take(&mut self.blocks).into_iter().map(|block|(block.id,block)).collect();self.blocks=document.blocks.iter().map(|block|{let epoch=document.block_revision(block.id).unwrap_or(0);previous.get(&block.id).filter(|cached|cached.epoch==epoch).cloned().unwrap_or_else(||Arc::new(BlockProjection::new(block,epoch)))}).collect();self.indices=self.blocks.iter().enumerate().map(|(index,block)|(block.id,index)).collect();self.lengths.rebuild(self.blocks.iter().map(|block|block.points.len() as f64));self.len=self.lengths.total() as usize-1;self.revision=document.revision;}
    fn update(&mut self,document:&EmailDocument){
        let Some(changes)=document.changes_since(self.revision) else {self.rebuild(document);return;};
        if changes.iter().any(|change|change.removed!=change.inserted){self.rebuild(document);return;}
        let mut touched=std::collections::HashSet::new();for change in changes{touched.extend(change.start..change.start+change.inserted);}
        for index in touched{if let Some(block)=document.block_at(index){self.indices.remove(&self.blocks[index].id);let next=Arc::new(BlockProjection::new(block,document.block_revision(block.id).unwrap_or(0)));self.lengths.set(index,next.points.len() as f64);self.indices.insert(block.id,index);self.blocks[index]=next;}}
        self.len=self.lengths.total() as usize-1;self.revision=document.revision;
    }
    pub(crate) fn point(&self, utf16: usize) -> EmailPoint { let offset=utf16.min(self.len);let index=self.lengths.locate(offset as f64);let block=&self.blocks[index];let start=self.lengths.sum(index) as usize;block.points[(offset-start).min(block.points.len()-1)] }
    pub(crate) fn utf16(&self,point:EmailPoint)->usize{self.indices.get(&point.block).and_then(|index|self.blocks[*index].reverse.get(&(point.cell,point.offset)).map(|offset|self.lengths.sum(*index) as usize+offset)).unwrap_or(0)}
    pub(crate) fn selection(&self, range: Range<usize>) -> EmailSelection { EmailSelection { anchor:self.point(range.start),head:self.point(range.end) } }
    pub(crate) fn range(&self, selection: &EmailSelection) -> Range<usize> { let a=self.utf16(selection.anchor); let b=self.utf16(selection.head); a.min(b)..a.max(b) }
    pub(crate) fn text(&self,range:Range<usize>)->String{let mut result=String::new();let first=self.lengths.locate(range.start as f64);let last=self.lengths.locate(range.end.min(self.len) as f64);for index in first..=last{let block=&self.blocks[index];let start=self.lengths.sum(index) as usize;let end=start+block.points.len()-1;if index>0&&start>range.start&&start<=range.end{result.push('\n');}if end>range.start&&start<range.end{let left=range.start.saturating_sub(start).min(block.bytes.len()-1);let right=range.end.saturating_sub(start).min(block.bytes.len()-1);result.push_str(&block.text[block.bytes[left]..block.bytes[right]]);}}result}
    pub(crate) fn neighbor(&self, point: EmailPoint, forward: bool) -> EmailPoint { use unicode_segmentation::UnicodeSegmentation; let Some(index)=self.indices.get(&point.block).copied() else { return point; }; let block=&self.blocks[index]; let local=block.reverse.get(&(point.cell,point.offset)).copied().unwrap_or(0); if !forward && local==0 { return if index>0 { *self.blocks[index-1].points.last().unwrap() } else { point }; } let mut units=0; let mut previous=0; for grapheme in block.text.graphemes(true) { units+=grapheme.encode_utf16().count(); if forward && units>local { return block.points[units]; } if !forward && units>=local { return block.points[previous]; } previous=units; } if forward && index+1<self.blocks.len() { self.blocks[index+1].points[0] } else { point } }
}
impl HtmlEmailEditor {
    pub(crate) fn input_map(&self)->Arc<InputMap>{let mut cache=self.input_cache.borrow_mut();if let Some((revision,map))=cache.as_mut(){if *revision!=self.document.revision{Arc::make_mut(map).update(&self.document);*revision=self.document.revision;}}else{*cache=Some((self.document.revision,Arc::new(InputMap::new(&self.document))));}cache.as_ref().unwrap().1.clone()}
    pub(crate) fn finish_composition(&mut self, cx: &mut Context<Self>) -> Result<(), EmailError> {
        if let Some(composition) = self.composition.take() { self.history.commit(&self.document, composition.checkpoint, self.selection.clone())?; self.changed(cx); } Ok(())
    }
    /// Cancels the active IME preview without adding an undo transaction.
    pub fn cancel_composition(&mut self, cx: &mut Context<Self>) {
        if let Some(composition) = self.composition.take() { match EmailHistory::rollback(&mut self.document, &composition.checkpoint) { Ok(selection) => { self.selection = selection; self.affinity = None; self.text_input_dirty = true; self.selection_dirty = true; cx.notify(); }, Err(error) => self.report(error, cx) } }
    }
}
impl EntityInputHandler for HtmlEmailEditor {
    fn paste(&mut self, item: ClipboardItem, _: &mut Window, cx: &mut Context<Self>) {
        if self.host_clipboard { self.request_paste(cx); return; }
        let result = if let Some(html) = item.metadata().and_then(|metadata| metadata.strip_prefix("ely-html-email:")) { self.insert_html(html, cx) } else if let Some(text) = item.text() { self.insert_text(&text, cx) } else { Err(EmailError::Invalid("The platform clipboard contains no supported HTML or text".into())) };
        if let Err(error) = result { self.report(error, cx); }
    }
    fn text_for_range(&mut self, range: Range<usize>, actual: &mut Option<Range<usize>>, _: &mut Window, _: &mut Context<Self>) -> Option<String> { let map=self.input_map(); let range=map.range(&map.selection(range)); *actual=Some(range.clone()); Some(map.text(range)) }
    fn selected_text_range(&mut self, _: bool, _: &mut Window, _: &mut Context<Self>) -> Option<UTF16Selection> { if self.disabled { return None; } let map = self.input_map(); Some(UTF16Selection { range: map.range(&self.selection), reversed: map.utf16(self.selection.head) < map.utf16(self.selection.anchor) }) }
    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> { self.composition.as_ref().map(|c| self.input_map().range(&c.marked)) }
    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) { if let Err(error) = self.finish_composition(cx) { self.report(error, cx); } }
    fn replace_text_in_range(&mut self, range: Option<Range<usize>>, text: &str, _: &mut Window, cx: &mut Context<Self>) {
        if !self.editable() { return; }
        let implicit_target = range.is_none();
        let selection = range.map(|r| self.input_map().selection(r)).or_else(|| self.composition.as_ref().map(|c| c.marked.clone())).unwrap_or_else(|| self.selection.clone());
        if self.composition.is_some() {
            if let Some(composition) = &mut self.composition { if let Err(error) = EmailHistory::expand(&self.document, &mut composition.checkpoint, &selection) { self.report(error, cx); return; } }
            let style = if implicit_target { self.current_style() } else { self.document.text_at(selection.anchor).map(|text| text.style_at(selection.anchor.offset)).unwrap_or_default() }; match self.document.replace(&selection, text, &style) { Ok(selection) => { self.selection = selection; if let Err(error) = self.finish_composition(cx) { self.report(error, cx); } }, Err(error) => self.report(error, cx) }
        } else { self.selection = selection; let result=if implicit_target{self.type_text(text,cx)}else{self.insert_text(text,cx)};if let Err(error)=result{self.report(error,cx);} }
    }
    fn replace_and_mark_text_in_range(&mut self, range: Option<Range<usize>>, text: &str, selected: Option<Range<usize>>, _: &mut Window, cx: &mut Context<Self>) {
        if !self.editable() { return; }
        self.typing_group=None;
        let implicit_target = range.is_none();
        let map = self.input_map();
        let target = range.map(|r| map.selection(r)).or_else(|| self.composition.as_ref().map(|c| c.marked.clone())).unwrap_or_else(|| self.selection.clone());
        if self.composition.is_none() { match EmailHistory::begin(&self.document, &target) { Ok(checkpoint) => self.composition = Some(Composition { checkpoint, marked: target.clone() }), Err(error) => { self.report(error, cx); return; } } }
        if let Some(composition) = &mut self.composition { if let Err(error) = EmailHistory::expand(&self.document, &mut composition.checkpoint, &target) { self.report(error, cx); return; } }
        let start = self.document.ordered_selection(&target).map(|(start, _)| start).unwrap_or(target.anchor);
        let style = if implicit_target { self.current_style() } else { self.document.text_at(start).map(|text| text.style_at(start.offset)).unwrap_or_default() };
        match self.document.replace(&target, text, &style) {
            Ok(selection) => {
                let marked = EmailSelection { anchor: start, head: selection.head };
                self.selection = selection;
                if let Some(composition) = &mut self.composition { composition.marked = marked.clone(); }
                if let Some(inner) = selected { let map = self.input_map(); let base = map.utf16(start); self.selection = map.selection(base + inner.start..base + inner.end); }
                self.text_input_dirty = true; self.selection_dirty = true; self.affinity = None;
                if text.is_empty() { self.cancel_composition(cx); } else { cx.notify(); }
            }, Err(error) => self.report(error, cx)
        }
    }
    fn bounds_for_range(&mut self, range: Range<usize>, element: Bounds<Pixels>, window: &mut Window, cx: &mut Context<Self>) -> Option<Bounds<Pixels>> { let rectangles = self.selection_bounds_for_range(range, element, window, cx); let mut iter = rectangles.into_iter(); let mut bounds = iter.next()?; for rectangle in iter { let left = bounds.origin.x.min(rectangle.origin.x); let top = bounds.origin.y.min(rectangle.origin.y); let right = bounds.right().max(rectangle.right()); let bottom = bounds.bottom().max(rectangle.bottom()); bounds = Bounds::new(gpui::point(left, top), gpui::size(right - left, bottom - top)); } Some(bounds) }
    fn character_index_for_point(&mut self, position: Point<Pixels>, _: &mut Window, _: &mut Context<Self>) -> Option<usize> { self.layout.hit(position).map(|p| self.input_map().utf16(p)) }
    fn accepts_text_input(&self, _: &mut Window, _: &mut Context<Self>) -> bool { self.editable() }
    fn text_length_utf16(&mut self, _: &mut Window, _: &mut Context<Self>) -> Option<usize> { Some(self.input_map().len) }
    fn set_selected_text_range(&mut self, range: Range<usize>, window: &mut Window, cx: &mut Context<Self>) { if !self.disabled { self.set_selection(self.input_map().selection(range), cx); window.notify_text_selection_changed(&self.focus); } }
    fn selection_bounds_for_range(&mut self, range: Range<usize>, _: Bounds<Pixels>, _: &mut Window, _: &mut Context<Self>) -> Vec<Bounds<Pixels>> {
        let map = self.input_map(); let mut bounds = Vec::new();
        for line in &self.layout.lines { for cluster in line.clusters.iter() { let start = map.utf16(EmailPoint { block: line.block, cell: line.cell, offset: cluster.range.start }); let end = map.utf16(EmailPoint { block: line.block, cell: line.cell, offset: cluster.range.end }); if start < range.end && end > range.start { bounds.push(Bounds::new(self.layout.origin + line.origin + gpui::point(cluster.x, gpui::px(0.)), gpui::size(cluster.width, line.height))); } } }
        for image in &self.layout.images { let start = map.utf16(EmailPoint { block: image.block, cell: image.cell, offset: image.offset }); let end = map.utf16(EmailPoint { block: image.block, cell: image.cell, offset: image.end }); if start < range.end && end > range.start { bounds.push(Bounds::new(self.layout.origin + image.bounds.origin, image.bounds.size)); } }
        if bounds.is_empty() { if let Some(caret) = self.layout.caret_with_affinity(map.point(range.start), self.affinity) { bounds.push(caret); } } bounds
    }
}

#[cfg(test)]
mod incremental_tests {
    use super::*;
    #[test]
    fn local_edit_reuses_unrelated_projection_and_updates_prefix_delta(){let mut document=EmailDocument::from_plain("first\nsecond\nthird").unwrap();let mut map=InputMap::new(&document);let untouched=map.blocks[2].clone();document.replace(&EmailSelection::caret(document.first_point()),"中😀",&EmailStyle::default()).unwrap();map.update(&document);assert!(Arc::ptr_eq(&untouched,&map.blocks[2]));assert_eq!(map.utf16(document.end_point()),21);assert_eq!(map.point(map.utf16(document.end_point())),document.end_point());}
}
