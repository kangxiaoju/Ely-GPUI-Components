//! Selection-based local, atomic editing operations.
use super::model::*;

impl EmailDocument {
    fn commit_splice(&mut self, start: usize, end: usize, blocks: Vec<EmailBlock>) -> Result<(), EmailError> { self.splice_blocks(start, end, blocks) }
    /// Extract a rich selected fragment, preserving paragraph and table formatting.
    pub fn fragment(&self, selection: &EmailSelection) -> Result<Self, EmailError> {
        let (first, last) = self.ordered_selection(selection)?;
        let start = self.index(first)?; let end = self.index(last)?;
        if first == last { return Ok(Self::default()); }
        let mut blocks = self.blocks[start..=end].to_vec();
        for (index, block) in blocks.iter_mut().enumerate() {
            let original_index = start + index;
            match &mut block.kind {
                EmailBlockKind::Paragraph(p) => { let from = if original_index == start { first.offset } else { 0 }; let to = if original_index == end { last.offset } else { p.content.text.len() }; p.content = p.content.slice(from..to)?; }
                EmailBlockKind::Table(t) => { for (cell, text) in t.cells.iter_mut().enumerate() { if (original_index == start && cell < first.cell) || (original_index == end && cell > last.cell) { *text = EmailText::default(); continue; } let from = if original_index == start && cell == first.cell { first.offset } else { 0 }; let to = if original_index == end && cell == last.cell { last.offset } else { text.text.len() }; *text = text.slice(from..to)?; } }
                _ => {}
            }
        }
        if matches!(blocks.first().map(|b| &b.kind), Some(EmailBlockKind::Image(_) | EmailBlockKind::Rule)) && first.offset == 1 { blocks.remove(0); }
        if !blocks.is_empty() && matches!(blocks.last().map(|b| &b.kind), Some(EmailBlockKind::Image(_) | EmailBlockKind::Rule)) && last.offset == 0 { blocks.pop(); }
        if blocks.is_empty() { return Ok(Self::default()); }
        Self::from_blocks(blocks)
    }
    /// Export exactly the selected content to plain text.
    pub fn selected_plain(&self, selection: &EmailSelection) -> Result<String, EmailError> { if selection.anchor == selection.head { self.ordered_selection(selection)?; return Ok(String::new()); } Ok(self.fragment(selection)?.to_plain()) }
    /// Replace a selection with literal text; LF splits paragraphs, U+2028 is a hard break.
    pub fn replace(&mut self, selection: &EmailSelection, text: &str, style: &EmailStyle) -> Result<EmailSelection, EmailError> {
        style.validate()?;
        let mut fragment = Self::from_plain(text)?;
        let metadata = style.font.as_ref().map_or(0, String::len) + style.link.as_ref().map_or(0, String::len);
        let run_count = fragment.blocks.iter().map(|b| Self::block_budget(b).1).sum::<usize>();
        if run_count.checked_mul(metadata).and_then(|n| n.checked_add(text.len())).is_none_or(|n| n > MAX_BYTES) { return Err(EmailError::Limit("expanded document bytes")); }
        let (first, _) = self.ordered_selection(selection)?;
        if let EmailBlockKind::Paragraph(original) = &self.blocks[self.index(first)?].kind {
            if selection.anchor == selection.head && text == "\n" && original.content.text.is_empty() && matches!(original.kind, EmailParagraphKind::Bullet | EmailParagraphKind::Numbered(_) | EmailParagraphKind::Task(_)) {
                let mut block = self.blocks[self.index(first)?].clone();
                if let EmailBlockKind::Paragraph(p) = &mut block.kind { p.kind = EmailParagraphKind::Normal; p.indent = 0; }
                let index = self.index(first)?; self.commit_splice(index, index + 1, vec![block])?; return Ok(EmailSelection::caret(first));
            }
            for (index, block) in fragment.blocks.iter_mut().enumerate() { if let EmailBlockKind::Paragraph(p) = &mut block.kind { p.kind = match original.kind { EmailParagraphKind::Numbered(n) => EmailParagraphKind::Numbered(n.saturating_add(index as u32)), EmailParagraphKind::Task(_) if index > 0 => EmailParagraphKind::Task(false), _ => original.kind.clone() }; p.align = original.align; p.indent = original.indent; } }
        }
        for block in &mut fragment.blocks { if let EmailBlockKind::Paragraph(p) = &mut block.kind { for run in &mut p.content.runs { run.style = style.clone(); } } }
        self.replace_fragment(selection, &fragment)
    }
    /// Replace a selection with a validated rich fragment, allocating fresh block IDs.
    pub fn replace_fragment(&mut self, selection: &EmailSelection, fragment: &Self) -> Result<EmailSelection, EmailError> {
        fragment.validate()?;
        let (first, last) = self.ordered_selection(selection)?; let start = self.index(first)?; let end = self.index(last)?;
        for block in &self.blocks[start..=end] { match &block.kind { EmailBlockKind::Table(t) => t.validate()?, EmailBlockKind::Paragraph(p) => p.content.validate()?, _ => {} } }
        // A plain rich-text fragment edits a table cell in place, not the table structure.
        if start == end && matches!(self.blocks[start].kind, EmailBlockKind::Table(_)) && fragment.blocks.iter().all(|b| matches!(b.kind, EmailBlockKind::Paragraph(_))) {
            let mut block = self.blocks[start].clone();
            let mut inserted = EmailText::default();
            for (index, b) in fragment.blocks.iter().enumerate() { if index > 0 { inserted.append(&EmailText::plain("\u{2028}")); } if let EmailBlockKind::Paragraph(p) = &b.kind { inserted.append(&p.content); } }
            if let EmailBlockKind::Table(table) = &mut block.kind {
                let suffix = table.cells[last.cell].slice(last.offset..table.cells[last.cell].text.len())?;
                let mut result = table.cells[first.cell].slice(0..first.offset)?; result.append(&inserted); result.append(&suffix);
                table.cells[first.cell] = result;
                for cell in first.cell + 1..=last.cell { table.cells[cell] = EmailText::default(); }
            }
            let point = EmailPoint { offset: first.offset + inserted.text.len(), ..first }; self.commit_splice(start, end + 1, vec![block])?; return Ok(EmailSelection::caret(point));
        }
        let mut replacement = fragment.blocks.clone(); let mut next = self.next_id()?;
        for block in &mut replacement { block.id = next; next = next.checked_add(1).ok_or_else(|| invalid("Block identity exhausted"))?; }
        let inserted_last_id = replacement.last().ok_or_else(|| invalid("Empty fragment"))?.id;
        let mut prefix = None; let mut suffix = None;
        if let EmailBlockKind::Paragraph(p) = &self.blocks[start].kind { let mut p = p.clone(); p.content = p.content.slice(0..first.offset)?; prefix = Some(p); }
        else if first.offset > 0 || first.cell > 0 { let mut block = self.blocks[start].clone(); if let EmailBlockKind::Table(t) = &mut block.kind { for cell in first.cell..t.cells.len() { let from = if cell == first.cell { first.offset } else { 0 }; let len = t.cells[cell].text.len(); t.cells[cell].replace(from..len, "", &EmailStyle::default())?; } } replacement.insert(0, block); }
        if let EmailBlockKind::Paragraph(p) = &self.blocks[end].kind { let mut p = p.clone(); p.content = p.content.slice(last.offset..p.content.text.len())?; suffix = Some(p); }
        else if last != Self::point(&self.blocks[end], true) { let mut block = self.blocks[end].clone(); if let EmailBlockKind::Table(t) = &mut block.kind { for cell in 0..=last.cell { let to = if cell == last.cell { last.offset } else { t.cells[cell].text.len() }; t.cells[cell].replace(0..to, "", &EmailStyle::default())?; } } if start == end && replacement.first().is_some_and(|b| b.id == block.id) { block.id = next; next = next.checked_add(1).ok_or_else(|| invalid("Block identity exhausted"))?; } replacement.push(block); }
        let mut prefix_len = 0;
        if let Some(mut p) = prefix { prefix_len = p.content.text.len(); if let Some(EmailBlock { kind: EmailBlockKind::Paragraph(inserted), .. }) = replacement.first_mut() { let kind = inserted.kind.clone(); let align = inserted.align; let indent = inserted.indent; p.content.append(&inserted.content); *inserted = p; inserted.kind = kind; inserted.align = align; inserted.indent = indent; replacement[0].id = self.blocks[start].id; } else { replacement.insert(0, EmailBlock { id: self.blocks[start].id, kind: EmailBlockKind::Paragraph(p) }); } }
        let last_index = replacement.iter().position(|b| b.id == inserted_last_id).unwrap_or(0);
        let mut caret = Self::point(&replacement[last_index], true);
        if replacement.len() == 1 && matches!(replacement[0].kind, EmailBlockKind::Paragraph(_)) { caret.offset = caret.offset.max(prefix_len); }
        if let Some(p) = suffix { if let EmailBlockKind::Paragraph(inserted) = &mut replacement[last_index].kind { inserted.content.append(&p.content); } else { let id = next; replacement.push(EmailBlock { id, kind: EmailBlockKind::Paragraph(p) }); caret = EmailPoint { block: id, cell: 0, offset: 0 }; } }
        self.commit_splice(start, end + 1, replacement)?; Ok(EmailSelection::caret(caret))
    }
    /// Apply an inline format to every selected text interval atomically.
    pub fn format(&mut self, selection: &EmailSelection, format: &EmailFormat) -> Result<(), EmailError> {
        let mut probe = EmailStyle::default(); probe.apply(format)?;
        self.edit_texts(selection, |text, range| text.format(range, format))
    }
    fn edit_texts(&mut self, selection: &EmailSelection, mut operation: impl FnMut(&mut EmailText, std::ops::Range<usize>) -> Result<(), EmailError>) -> Result<(), EmailError> {
        let (first, last) = self.ordered_selection(selection)?; if first == last { return Ok(()); }
        let start = self.index(first)?; let end = self.index(last)?; let mut blocks = self.blocks[start..=end].to_vec();
        let mut budget = blocks.iter().map(|b| Self::block_budget(b).0).sum::<usize>();
        for (index, block) in blocks.iter_mut().enumerate() { let index = start + index; let mut apply = |cell, text: &mut EmailText| { if (index == start && cell < first.cell) || (index == end && cell > last.cell) { return Ok(()); } let from = if index == start && cell == first.cell { first.offset } else { 0 }; let to = if index == end && cell == last.cell { last.offset } else { text.text.len() }; let old_bytes = text.byte_len(); operation(text, from..to)?; budget = budget - old_bytes + text.byte_len(); if budget > MAX_BYTES { return Err(EmailError::Limit("expanded document bytes")); } Ok(()) }; match &mut block.kind { EmailBlockKind::Paragraph(p) => apply(0, &mut p.content)?, EmailBlockKind::Table(t) => for (cell, text) in t.cells.iter_mut().enumerate() { if t.spans[cell] > 0 { apply(cell, text)?; } }, _ => {} } }
        self.commit_splice(start, end + 1, blocks)
    }
    /// Set semantic paragraph properties on all touched paragraphs.
    pub fn paragraph(&mut self, selection: &EmailSelection, kind: EmailParagraphKind, align: EmailAlign, indent: u8) -> Result<(), EmailError> {
        let (first, last) = self.ordered_selection(selection)?; let start = self.index(first)?; let end = self.index(last)?; let mut blocks = self.blocks[start..=end].to_vec();
        for block in &mut blocks { if let EmailBlockKind::Paragraph(p) = &mut block.kind { p.kind = kind.clone(); p.align = align; p.indent = indent; } } self.commit_splice(start, end + 1, blocks)
    }
    fn edit_paragraphs(&mut self, selection: &EmailSelection, mut update: impl FnMut(&mut EmailParagraph)) -> Result<(), EmailError> {
        let (first, last) = self.ordered_selection(selection)?;
        let start = self.index(first)?; let end = self.index(last)?;
        let mut blocks = self.blocks[start..=end].to_vec();
        for block in &mut blocks { if let EmailBlockKind::Paragraph(paragraph) = &mut block.kind { update(paragraph); } }
        self.commit_splice(start, end + 1, blocks)
    }
    /// Change alignment of touched paragraphs without changing their kinds or indentation.
    pub fn set_alignment(&mut self, selection: &EmailSelection, align: EmailAlign) -> Result<(), EmailError> {
        self.edit_paragraphs(selection, |paragraph| paragraph.align = align)
    }
    /// Increase or decrease each touched paragraph's own indentation, bounded to 0–32.
    /// Semantic kinds and alignment are preserved independently for each paragraph.
    pub fn change_indent(&mut self, selection: &EmailSelection, increase: bool) -> Result<(), EmailError> {
        self.edit_paragraphs(selection, |paragraph| paragraph.indent = if increase { paragraph.indent.saturating_add(1).min(32) } else { paragraph.indent.saturating_sub(1) })
    }
    /// Change semantic kind without changing any touched paragraph's alignment or indentation.
    pub fn set_paragraph_kind(&mut self, selection: &EmailSelection, kind: EmailParagraphKind) -> Result<(), EmailError> {
        self.edit_paragraphs(selection, |paragraph| paragraph.kind = kind.clone())
    }
    /// Atomically replace the image addressed by a document point, retaining its stable ID.
    /// The new source and dimensions are validated before any document mutation.
    pub fn update_image(&mut self, point: EmailPoint, image: EmailImage) -> Result<(), EmailError> {
        image.validate()?; let index = self.index(point)?;
        let mut block = self.blocks[index].clone();
        match &mut block.kind {
            EmailBlockKind::Image(current) => *current = image,
            EmailBlockKind::Paragraph(p) => { let at = Self::inline_image_offset(&p.content, point.offset).ok_or_else(|| invalid("Point does not address an image"))?; p.content.images.insert(at, image); }
            EmailBlockKind::Table(t) => { let text = t.cells.get_mut(point.cell).ok_or_else(|| invalid("Unknown table cell"))?; let at = Self::inline_image_offset(text, point.offset).ok_or_else(|| invalid("Point does not address an image"))?; text.images.insert(at, image); }
            EmailBlockKind::Rule => return Err(invalid("Point does not address an image")),
        }
        self.commit_splice(index, index + 1, vec![block])
    }
    fn insert_block(&mut self, selection: &EmailSelection, kind: EmailBlockKind) -> Result<EmailSelection, EmailError> { let id = self.next_id()?; let fragment = Self::from_blocks(vec![EmailBlock { id: 1, kind }])?; self.replace_fragment(selection, &fragment)?; let inserted = self.blocks.iter().find(|b| b.id == id).ok_or_else(|| invalid("Inserted block missing"))?; Ok(EmailSelection::caret(Self::point(inserted, false))) }
    /// Insert an atomic inline image in the current paragraph or table cell's text flow.
    pub fn insert_image(&mut self, selection: &EmailSelection, image: EmailImage) -> Result<EmailSelection, EmailError> {
        image.validate()?; let mut content = EmailText::plain("\u{fffc}"); content.images.insert(0, image);
        let (first, _) = self.ordered_selection(selection)?; for run in &mut content.runs { run.style = self.text_at(first).map(|t| t.style_at(first.offset)).unwrap_or_default(); }
        let paragraph = EmailParagraph { content, ..Default::default() };
        let fragment = Self::from_blocks(vec![EmailBlock { id: 1, kind: EmailBlockKind::Paragraph(paragraph) }])?;
        self.replace_fragment(selection, &fragment)
    }
    /// Delete only the resolved inline image token or standalone image block.
    pub fn delete_image(&mut self, point: EmailPoint) -> Result<EmailSelection, EmailError> {
        let index = self.index(point)?;
        if matches!(self.blocks[index].kind, EmailBlockKind::Image(_)) {
            if self.blocks.len() == 1 { self.commit_splice(index, index + 1, vec![EmailBlock { id: point.block, kind: EmailBlockKind::Paragraph(EmailParagraph::default()) }])?; return Ok(EmailSelection::caret(self.first_point())); }
            let caret = if index + 1 < self.blocks.len() { Self::point(&self.blocks[index + 1], false) } else { Self::point(&self.blocks[index - 1], true) };
            self.splice_blocks(index, index + 1, Vec::new())?; return Ok(EmailSelection::caret(caret));
        }
        let text = self.text_at(point).ok_or_else(|| invalid("Point does not address an image"))?;
        let offset = Self::inline_image_offset(text, point.offset).ok_or_else(|| invalid("Point does not address an image"))?;
        let target = EmailSelection { anchor: EmailPoint { offset, ..point }, head: EmailPoint { offset: offset + 3, ..point } };
        self.replace(&target, "", &EmailStyle::default())
    }
    /// Insert a bounded rectangular table and select its first cell.
    pub fn insert_table(&mut self, selection: &EmailSelection, rows: usize, columns: usize) -> Result<EmailSelection, EmailError> { if rows == 0 || columns == 0 || rows > 64 || columns > 64 { return Err(EmailError::Limit("table dimensions")); } self.insert_block(selection, EmailBlockKind::Table(EmailTable { columns, cells: vec![EmailText::default(); rows * columns], spans: vec![1; rows * columns] })) }
    /// Insert an atomic horizontal rule.
    pub fn insert_rule(&mut self, selection: &EmailSelection) -> Result<EmailSelection, EmailError> { self.insert_block(selection, EmailBlockKind::Rule) }
    /// Apply a structural table operation atomically.
    pub fn table_edit(&mut self, id: u64, edit: &EmailTableEdit) -> Result<(), EmailError> {
        let index = self.blocks.iter().position(|b| b.id == id).ok_or_else(|| invalid("Unknown table"))?; let mut block = self.blocks[index].clone();
        let EmailBlockKind::Table(t) = &mut block.kind else { return Err(invalid("Block is not a table")); }; t.validate()?;
        let rows = t.cells.len() / t.columns;
        match *edit {
            EmailTableEdit::Merge { row, column, columns } => { if row >= rows || columns < 2 || column >= t.columns || columns > t.columns - column { return Err(invalid("Invalid merge range")); } let start = row * t.columns + column; let end = start + columns; let mut at = start; while at < end { let span = t.spans[at]; if span == 0 || at + span > end { return Err(invalid("Merge intersects a span")); } at += span; } let mut content = t.cells[start].clone(); for cell in start + 1..end { if !t.cells[cell].text.is_empty() { if !content.text.is_empty() { content.append(&EmailText::plain("\u{2028}")); } content.append(&t.cells[cell]); } t.cells[cell] = EmailText::default(); t.spans[cell] = 0; } t.cells[start] = content; t.spans[start] = columns; }
            EmailTableEdit::Unmerge { row, column } => { if row >= rows || column >= t.columns { return Err(invalid("Invalid unmerge cell")); } let start = row * t.columns + column; let span = t.spans[start]; if span == 0 { return Err(invalid("Cell is covered")); } t.spans[start..start + span].fill(1); }
            EmailTableEdit::InsertRow(row) => { if row > rows || rows == 64 { return Err(invalid("Invalid inserted row")); } let at = row * t.columns; t.cells.splice(at..at, vec![EmailText::default(); t.columns]); t.spans.splice(at..at, vec![1; t.columns]); }
            EmailTableEdit::DeleteRow(row) => { if row >= rows || rows == 1 { return Err(invalid("Cannot delete table row")); } let at = row * t.columns; t.cells.drain(at..at + t.columns); t.spans.drain(at..at + t.columns); }
            EmailTableEdit::InsertColumn(column) => {
                if column > t.columns || t.columns == 64 { return Err(invalid("Invalid inserted column")); }
                let old = t.columns;
                for row in (0..rows).rev() { let base = row * old; let owner = (0..column).rev().find(|c| t.spans[base + c] > 0 && c + t.spans[base + c] > column); let span = if let Some(owner) = owner { t.spans[base + owner] += 1; 0 } else { 1 }; t.cells.insert(base + column, EmailText::default()); t.spans.insert(base + column, span); } t.columns += 1;
            }
            EmailTableEdit::DeleteColumn(column) => {
                if column >= t.columns || t.columns == 1 { return Err(invalid("Cannot delete table column")); }
                let old = t.columns;
                for row in (0..rows).rev() { let base = row * old; let at = base + column; if t.spans[at] == 0 { if let Some(owner) = (0..column).rev().find(|c| t.spans[base + c] > 0) { t.spans[base + owner] -= 1; } } else if t.spans[at] > 1 { t.spans[at + 1] = t.spans[at] - 1; t.cells.swap(at, at + 1); } t.cells.remove(at); t.spans.remove(at); } t.columns -= 1;
            }
        }
        self.commit_splice(index, index + 1, vec![block])
    }
}

#[cfg(test)]
mod paragraph_tests {
    use super::*;
    #[test]
    fn image_property_update_preserves_id_and_rejects_unsafe_source() {
        let mut document = EmailDocument::default();
        let image = EmailImage { source: "cid:original".into(), alt: "original".into(), width: Some(100), height: None, align: EmailAlign::Left };
        let selection = document.insert_image(&EmailSelection::caret(document.first_point()), image.clone()).unwrap();
        let mut changed = image; changed.alt = "updated".into(); changed.width = Some(240); changed.align = EmailAlign::Center;
        document.update_image(selection.head, changed.clone()).unwrap();
        assert_eq!(document.image_at(selection.head), Some(&changed));
        let before = document.clone(); changed.source = "javascript:bad".into();
        assert!(document.update_image(selection.head, changed).is_err()); assert_eq!(document, before);
    }
    #[test]
    fn reverse_selection_formats_each_property_independently() {
        let mut document = EmailDocument::from_blocks(vec![
            EmailBlock { id: 1, kind: EmailBlockKind::Paragraph(EmailParagraph { kind: EmailParagraphKind::Quote, align: EmailAlign::Right, indent: 2, content: EmailText::plain("one") }) },
            EmailBlock { id: 2, kind: EmailBlockKind::Paragraph(EmailParagraph { kind: EmailParagraphKind::Task(true), align: EmailAlign::Left, indent: 5, content: EmailText::plain("two") }) },
        ]).unwrap();
        let selection = EmailSelection { anchor: document.end_point(), head: document.first_point() };
        document.set_alignment(&selection, EmailAlign::Center).unwrap();
        for (index, block) in document.blocks().iter().enumerate() { let EmailBlockKind::Paragraph(p) = &block.kind else { panic!("paragraph"); }; assert_eq!(p.align, EmailAlign::Center); assert_eq!(p.indent, [2, 5][index]); assert_eq!(p.kind, [EmailParagraphKind::Quote, EmailParagraphKind::Task(true)][index]); }
        document.change_indent(&selection, true).unwrap();
        for (index, block) in document.blocks().iter().enumerate() { let EmailBlockKind::Paragraph(p) = &block.kind else { panic!("paragraph"); }; assert_eq!(p.indent, [3, 6][index]); assert_eq!(p.align, EmailAlign::Center); }
        document.set_paragraph_kind(&selection, EmailParagraphKind::Bullet).unwrap();
        for (index, block) in document.blocks().iter().enumerate() { let EmailBlockKind::Paragraph(p) = &block.kind else { panic!("paragraph"); }; assert_eq!(p.kind, EmailParagraphKind::Bullet); assert_eq!(p.indent, [3, 6][index]); assert_eq!(p.align, EmailAlign::Center); }
        document.change_indent(&selection, false).unwrap();
        for (index, block) in document.blocks().iter().enumerate() { let EmailBlockKind::Paragraph(p) = &block.kind else { panic!("paragraph"); }; assert_eq!(p.indent, [2, 5][index]); }
    }
    #[test]
    fn indent_changes_saturate_and_invalid_kind_is_atomic() {
        let mut document = EmailDocument::from_plain("one\ntwo").unwrap();
        let selection = EmailSelection { anchor: document.end_point(), head: document.first_point() };
        document.change_indent(&selection, false).unwrap();
        document.paragraph(&selection, EmailParagraphKind::Normal, EmailAlign::Left, 32).unwrap();
        document.change_indent(&selection, true).unwrap();
        for block in document.blocks() { let EmailBlockKind::Paragraph(p) = &block.kind else { panic!("paragraph"); }; assert_eq!(p.indent, 32); }
        let before = document.clone();
        assert!(document.set_paragraph_kind(&selection, EmailParagraphKind::Heading(0)).is_err());
        assert_eq!(document, before);
    }
}
