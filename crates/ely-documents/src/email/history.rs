//! Bounded undo records containing only the affected block splice.
use super::model::*;
use std::collections::VecDeque;

/// A local pre-edit snapshot, reusable for IME preview rollback.
#[derive(Clone, Debug)]
pub struct EmailCheckpoint {
    start: usize,
    total: usize,
    before: Vec<EmailBlock>,
    selection: EmailSelection,
}
#[derive(Debug)]
struct Transaction { start: usize, total_after: usize, before: Vec<EmailBlock>, after: Vec<EmailBlock>, old_selection: EmailSelection, new_selection: EmailSelection, bytes: usize }
/// Undo/redo history limited to 100 complete operations and 32 MiB.
#[derive(Debug, Default)]
pub struct EmailHistory { undo: VecDeque<Transaction>, redo: Vec<Transaction>, bytes: usize }
fn block_bytes(block: &EmailBlock) -> usize {
    let text = |text: &EmailText| text.text.capacity() + text.runs.capacity() * std::mem::size_of::<EmailRun>() + text.images.values().map(|image| std::mem::size_of::<EmailImage>() + 8 * std::mem::size_of::<usize>() + image.source.capacity() + image.alt.capacity()).sum::<usize>() + text.runs.iter().map(|r| r.style.font.as_ref().map_or(0, String::capacity) + r.style.link.as_ref().map_or(0, String::capacity)).sum::<usize>();
    std::mem::size_of::<EmailBlock>() + match &block.kind { EmailBlockKind::Paragraph(p) => text(&p.content), EmailBlockKind::Image(i) => i.source.capacity() + i.alt.capacity(), EmailBlockKind::Table(t) => t.cells.capacity() * std::mem::size_of::<EmailText>() + t.cells.iter().map(text).sum::<usize>() + t.spans.capacity() * std::mem::size_of::<usize>(), EmailBlockKind::Rule => 0 }
}
impl EmailHistory {
    /// Extend a live composition checkpoint to include newly touched adjacent blocks.
    /// Existing original content and the original directed selection remain unchanged.
    pub fn expand(document: &EmailDocument, checkpoint: &mut EmailCheckpoint, selection: &EmailSelection) -> Result<(), EmailError> {
        let (first, last) = document.ordered_selection(selection)?;
        let start = document.index(first)?; let end = document.index(last)? + 1;
        let old_end = Self::after_end(document, checkpoint)?;
        if end > old_end { checkpoint.before.extend_from_slice(&document.blocks[old_end..end]); }
        if start < checkpoint.start { let mut before = document.blocks[start..checkpoint.start].to_vec(); before.append(&mut checkpoint.before); checkpoint.before = before; checkpoint.start = start; }
        Ok(())
    }
    /// Whether undo is available.
    pub fn can_undo(&self) -> bool { !self.undo.is_empty() }
    /// Whether redo is available.
    pub fn can_redo(&self) -> bool { !self.redo.is_empty() }
    /// Forget all undo and redo operations.
    pub fn clear(&mut self) { self.undo.clear(); self.redo.clear(); self.bytes = 0; }
    /// Capture only blocks touched by a validated selection.
    pub fn begin(document: &EmailDocument, selection: &EmailSelection) -> Result<EmailCheckpoint, EmailError> {
        let (first, last) = document.ordered_selection(selection)?;
        let start = document.index(first)?; let end = document.index(last)?;
        Ok(EmailCheckpoint { start, total: document.blocks.len(), before: document.blocks[start..=end].to_vec(), selection: selection.clone() })
    }
    fn after_end(document: &EmailDocument, checkpoint: &EmailCheckpoint) -> Result<usize, EmailError> {
        document.blocks.len().checked_sub(checkpoint.total - checkpoint.before.len()).and_then(|len| checkpoint.start.checked_add(len)).filter(|end| *end <= document.blocks.len()).ok_or_else(|| invalid("Checkpoint no longer matches document"))
    }
    /// Restore a local snapshot without consuming it, for composition previews.
    pub fn rollback(document: &mut EmailDocument, checkpoint: &EmailCheckpoint) -> Result<EmailSelection, EmailError> {
        let end = Self::after_end(document, checkpoint)?;
        document.splice_blocks(checkpoint.start, end, checkpoint.before.clone())?;
        Ok(checkpoint.selection.clone())
    }
    /// Commit the resulting local splice as one indivisible undo operation.
    pub fn commit(&mut self, document: &EmailDocument, checkpoint: EmailCheckpoint, selection: EmailSelection) -> Result<(), EmailError> {
        document.ordered_selection(&selection)?;
        let end = Self::after_end(document, &checkpoint)?;
        let after = document.blocks[checkpoint.start..end].to_vec();
        if after == checkpoint.before { return Ok(()); }
        self.bytes -= self.redo.iter().map(|t| t.bytes).sum::<usize>(); self.redo.clear();
        let bytes = checkpoint.before.iter().chain(&after).map(block_bytes).sum();
        self.undo.push_back(Transaction { start: checkpoint.start, total_after: document.blocks.len(), before: checkpoint.before, after, old_selection: checkpoint.selection, new_selection: selection, bytes }); self.bytes += bytes;
        while self.undo.len() > 100 || self.bytes > 32 * 1024 * 1024 { if let Some(old) = self.undo.pop_front() { self.bytes -= old.bytes; } else { break; } }
        Ok(())
    }
    /// Extend the latest undo transaction with a caller-approved contiguous typing edit.
    /// The caller owns timing, style, caret and revision guards; paste, IME and formatting must not join.
    /// Original affected blocks and selection remain intact, even when the first edit replaced multiple blocks.
    /// Returns `false` when no undo record is available or a redo branch is active.
    pub fn amend_typing(&mut self, document: &EmailDocument, selection: EmailSelection) -> Result<bool, EmailError> {
        if self.undo.is_empty() || !self.redo.is_empty() { return Ok(false); }
        document.ordered_selection(&selection)?;
        let transaction = self.undo.back_mut().ok_or_else(|| invalid("Missing typing transaction"))?;
        let untouched = transaction.total_after.checked_sub(transaction.after.len()).ok_or_else(|| invalid("Invalid typing transaction dimensions"))?;
        let affected = document.blocks.len().checked_sub(untouched).ok_or_else(|| invalid("Typing transaction no longer matches document"))?;
        let end = transaction.start.checked_add(affected).filter(|end| *end <= document.blocks.len()).ok_or_else(|| invalid("Typing transaction exceeds document"))?;
        let after = document.blocks[transaction.start..end].to_vec();
        let bytes = transaction.before.iter().chain(&after).map(block_bytes).sum();
        self.bytes = self.bytes - transaction.bytes + bytes;
        transaction.after = after; transaction.new_selection = selection; transaction.bytes = bytes; transaction.total_after = document.blocks.len();
        while self.undo.len() > 100 || self.bytes > 32 * 1024 * 1024 { if let Some(old) = self.undo.pop_front() { self.bytes -= old.bytes; } else { break; } }
        Ok(true)
    }
    /// Undo one complete operation and restore its original directed selection.
    pub fn undo(&mut self, document: &mut EmailDocument) -> Option<EmailSelection> {
        let transaction = self.undo.pop_back()?;
        if document.splice_blocks(transaction.start, transaction.start + transaction.after.len(), transaction.before.clone()).is_err() { self.undo.push_back(transaction); return None; }
        let selection = transaction.old_selection.clone(); self.redo.push(transaction); Some(selection)
    }
    /// Redo one complete operation and restore its resulting selection.
    pub fn redo(&mut self, document: &mut EmailDocument) -> Option<EmailSelection> {
        let transaction = self.redo.pop()?;
        if document.splice_blocks(transaction.start, transaction.start + transaction.before.len(), transaction.after.clone()).is_err() { self.redo.push(transaction); return None; }
        let selection = transaction.new_selection.clone(); self.undo.push_back(transaction); Some(selection)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn amended_typing_retains_first_cross_block_checkpoint_and_selection() {
        let mut document = EmailDocument::from_plain("one\ntwo").unwrap();
        let original = document.blocks.clone();
        let selection = EmailSelection { anchor: document.end_point(), head: document.first_point() };
        let checkpoint = EmailHistory::begin(&document, &selection).unwrap();
        let mut after = document.replace(&selection, "a", &EmailStyle::default()).unwrap();
        let mut history = EmailHistory::default(); history.commit(&document, checkpoint, after.clone()).unwrap();
        after = document.replace(&after, "b", &EmailStyle::default()).unwrap();
        assert!(history.amend_typing(&document, after.clone()).unwrap());
        after = document.replace(&after, "c", &EmailStyle::default()).unwrap();
        assert!(history.amend_typing(&document, after.clone()).unwrap());
        assert_eq!(history.undo(&mut document), Some(selection)); assert_eq!(document.blocks, original); assert!(!history.can_undo());
        assert_eq!(history.redo(&mut document), Some(after)); assert_eq!(document.to_plain(), "abc");
    }
    #[test]
    fn amended_typing_handles_structural_delta_and_no_available_record() {
        let mut document = EmailDocument::from_plain("left\nright").unwrap();
        let mut history = EmailHistory::default(); let at = EmailSelection::caret(document.first_point());
        assert!(!history.amend_typing(&document, at.clone()).unwrap());
        let checkpoint = EmailHistory::begin(&document, &at).unwrap();
        let after = document.replace(&at, "x", &EmailStyle::default()).unwrap(); history.commit(&document, checkpoint, after.clone()).unwrap();
        let after = document.replace(&after, "\ny", &EmailStyle::default()).unwrap(); history.amend_typing(&document, after).unwrap();
        history.undo(&mut document).unwrap(); assert_eq!(document.to_plain(), "left\nright");
    }
    use super::*;
    #[test]
    fn expanded_composition_checkpoint_restores_adjacent_blocks() {
        let mut document = EmailDocument::from_plain("A\nB\nC").unwrap();
        let selection = EmailSelection::caret(EmailPoint { block: 2, cell: 0, offset: 1 });
        let mut checkpoint = EmailHistory::begin(&document, &selection).unwrap();
        document.replace(&selection, "preview", &EmailStyle::default()).unwrap();
        let expanded = EmailSelection { anchor: document.first_point(), head: document.end_point() };
        EmailHistory::expand(&document, &mut checkpoint, &expanded).unwrap();
        document.replace(&expanded, "changed", &EmailStyle::default()).unwrap();
        assert_eq!(EmailHistory::rollback(&mut document, &checkpoint).unwrap(), selection);
        assert_eq!(document.to_plain(), "A\nB\nC");
    }
    #[test]
    fn checkpoint_captures_one_block_and_composition_rollback_is_reusable() {
        let mut document = EmailDocument::from_plain("left\n中🦀\nright").unwrap();
        let selection = EmailSelection::caret(EmailPoint { block: 2, cell: 0, offset: 3 });
        let checkpoint = EmailHistory::begin(&document, &selection).unwrap();
        assert_eq!(checkpoint.before.len(), 1);
        document.replace(&selection, "ni", &EmailStyle::default()).unwrap();
        assert_eq!(EmailHistory::rollback(&mut document, &checkpoint).unwrap(), selection);
        document.replace(&selection, "你", &EmailStyle::default()).unwrap();
        EmailHistory::rollback(&mut document, &checkpoint).unwrap();
        let after = document.replace(&selection, "好", &EmailStyle::default()).unwrap();
        let mut history = EmailHistory::default(); history.commit(&document, checkpoint, after).unwrap();
        history.undo(&mut document).unwrap();
        assert_eq!(document.to_plain(), "left\n中🦀\nright");
        assert!(!history.can_undo());
    }
    #[test]
    fn history_discards_oldest_whole_transactions() {
        let mut document = EmailDocument::default(); let mut history = EmailHistory::default();
        let mut selection = EmailSelection::caret(document.first_point());
        for _ in 0..105 { let checkpoint = EmailHistory::begin(&document, &selection).unwrap(); selection = document.replace(&selection, "x", &EmailStyle::default()).unwrap(); history.commit(&document, checkpoint, selection.clone()).unwrap(); }
        for _ in 0..100 { history.undo(&mut document).unwrap(); }
        assert!(!history.can_undo()); assert_eq!(document.to_plain(), "xxxxx");
    }
}
