//! Pure, UTF-8 structured mail content and bounded editing primitives.
use serde::{Deserialize, Serialize};
use std::{fmt, ops::Range};

pub(crate) const MAX_BYTES: usize = 2 * 1024 * 1024;
pub(crate) const MAX_BLOCKS: usize = 4096;
pub(crate) const MAX_RUNS: usize = 65536;

/// An explicit sRGB text color.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmailColor { /// Red channel.
    pub red: u8, /// Green channel.
    pub green: u8, /// Blue channel.
    pub blue: u8 }
/// Inline formatting; absent properties inherit the editor theme.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmailStyle {
    /// Bold weight.
    pub bold: bool,
    /// Italic face.
    pub italic: bool,
    /// Underlined text.
    pub underline: bool,
    /// Struck-through text.
    pub strike: bool,
    /// Inline code face.
    pub code: bool,
    /// Safe font family.
    pub font: Option<String>,
    /// Point size, between 8 and 72.
    pub size: Option<u16>,
    /// Explicit foreground color.
    pub color: Option<EmailColor>,
    /// Safe HTTP, HTTPS, or mailto link.
    pub link: Option<String>,
}
/// A nonempty, UTF-8 aligned style interval.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmailRun { /// Byte interval.
    pub range: Range<usize>, /// Formatting over the interval.
    pub style: EmailStyle }
/// Real Unicode text with a complete normalized style cover.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmailText { /// Literal text, never markup.
    pub text: String, /// Sorted, nonoverlapping intervals.
    pub runs: Vec<EmailRun>,
    /// Atomic inline images keyed by the byte offset of their U+FFFC object token.
    #[serde(default)]
    pub images: std::collections::BTreeMap<usize, EmailImage> }
/// Horizontal block alignment.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum EmailAlign { /// Inherited left alignment.
    #[default] Left, /// Center alignment.
    Center, /// Right alignment.
    Right, /// Justified alignment.
    Justify }
/// Semantic paragraph presentation.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum EmailParagraphKind { /// Ordinary paragraph.
    #[default] Normal, /// Heading level 1 through 6.
    Heading(u8), /// Quoted text.
    Quote, /// Unordered list item.
    Bullet, /// Ordered item number.
    Numbered(u32), /// Task item completion.
    Task(bool), /// Preformatted code paragraph.
    Code }
/// A paragraph and its literal rich text.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmailParagraph { /// Semantic kind.
    pub kind: EmailParagraphKind, /// Horizontal alignment.
    pub align: EmailAlign, /// Nesting/indent level, at most 32.
    pub indent: u8, /// Rich text content.
    pub content: EmailText }
/// An inert image reference; importing never fetches its source.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmailImage { /// Safe CID, asset, HTTP(S), or raster data URL.
    pub source: String, /// Accessible alternative text.
    pub alt: String, /// Pixel width.
    pub width: Option<u32>, /// Pixel height.
    pub height: Option<u32>, /// Block alignment.
    pub align: EmailAlign }
/// Rectangular row-major table with horizontal span ownership.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmailTable { /// Column count, at most 64.
    pub columns: usize, /// Row-major text including empty covered cells.
    pub cells: Vec<EmailText>, /// Positive owner colspan, zero for covered cells.
    pub spans: Vec<usize> }
/// Supported editable mail block types.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EmailBlockKind { /// A text paragraph.
    Paragraph(EmailParagraph), /// An inert image.
    Image(EmailImage), /// A rectangular table.
    Table(EmailTable), /// Horizontal rule.
    Rule }
/// A stable-identity document block.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmailBlock { /// Unique document-local identity.
    pub id: u64, /// Block content.
    pub kind: EmailBlockKind }
/// UTF-8 position within a stable block.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmailPoint { /// Stable block identity.
    pub block: u64, /// Table cell index, otherwise zero.
    pub cell: usize, /// Byte offset, or 0/1 for atomic blocks.
    pub offset: usize }
/// Direction-preserving selection endpoints.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmailSelection { /// Fixed endpoint.
    pub anchor: EmailPoint, /// Active endpoint.
    pub head: EmailPoint }
impl EmailSelection {
    /// Construct a collapsed selection.
    pub fn caret(point: EmailPoint) -> Self { Self { anchor: point, head: point } }
}
/// A single inline formatting operation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EmailFormat { /// Set bold.
    Bold(bool), /// Set italic.
    Italic(bool), /// Set underline.
    Underline(bool), /// Set strike.
    Strike(bool), /// Set code.
    Code(bool), /// Set or inherit font.
    Font(Option<String>), /// Set or inherit point size.
    Size(Option<u16>), /// Set or inherit color.
    Color(Option<EmailColor>), /// Set or remove safe link.
    Link(Option<String>), /// Remove all inline formatting.
    Clear }
/// A bounded structural table operation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EmailTableEdit { /// Insert a row at index.
    InsertRow(usize), /// Delete a row at index.
    DeleteRow(usize), /// Insert a column at index.
    InsertColumn(usize), /// Delete a column at index.
    DeleteColumn(usize), /// Merge adjacent cells in one row.
    Merge { /// Row index.
        row: usize, /// First column.
        column: usize, /// Number of columns.
        columns: usize }, /// Remove a horizontal merge.
    Unmerge { /// Row index.
        row: usize, /// Owner column.
        column: usize } }
/// Recoverable model/import failure. Failed edits preserve the document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EmailError { /// Invalid structure, range, URL, or property.
    Invalid(String), /// A documented resource limit was exceeded.
    Limit(&'static str) }
impl fmt::Display for EmailError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { match self { Self::Invalid(s) => f.write_str(s), Self::Limit(s) => write!(f, "Email limit exceeded: {s}") } }
}
impl std::error::Error for EmailError {}
pub(crate) fn invalid(message: &str) -> EmailError { EmailError::Invalid(message.into()) }
/// A committed local block splice for incremental projection and layout caches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EmailSpliceChange {
    /// Document revision containing this change.
    pub revision: u64,
    /// First affected block index before this splice.
    pub start: usize,
    /// Number of removed blocks.
    pub removed: usize,
    /// Number of inserted blocks.
    pub inserted: usize,
}
/// The serializable source of truth for native HTML mail editing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct EmailDocument {
    pub(crate) blocks: Vec<EmailBlock>,
    pub(crate) revision: u64,
    #[serde(skip)]
    bytes: usize,
    #[serde(skip)]
    runs: usize,
    #[serde(skip)]
    next: u64,
    #[serde(skip)]
    block_revisions: std::collections::HashMap<u64, u64>,
    #[serde(skip)]
    nonempty_blocks: usize,
    #[serde(skip)]
    nonblank_blocks: usize,
    #[serde(skip)]
    indices: std::collections::HashMap<u64, usize>,
    #[serde(skip)]
    changes: std::collections::VecDeque<EmailSpliceChange>,
    #[serde(skip)]
    changes_floor: u64,
}
impl<'de> Deserialize<'de> for EmailDocument {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)] struct Stored { blocks: Vec<EmailBlock>, #[serde(default)] revision: u64 }
        let stored = Stored::deserialize(deserializer)?;
        let mut document = Self::from_blocks(stored.blocks).map_err(serde::de::Error::custom)?; document.revision = stored.revision; document.changes_floor = stored.revision; Ok(document)
    }
}
impl Default for EmailDocument {
    fn default() -> Self { Self { blocks: vec![EmailBlock { id: 1, kind: EmailBlockKind::Paragraph(EmailParagraph::default()) }], revision: 0, bytes: 0, runs: 0, next: 2, block_revisions: std::collections::HashMap::from([(1, 0)]), nonempty_blocks: 0, nonblank_blocks: 0, indices: std::collections::HashMap::from([(1, 0)]), changes: Default::default(), changes_floor: 0 } }
}
impl EmailImage {
    pub(crate) fn validate(&self) -> Result<(), EmailError> { if !safe_url(&self.source, true) || self.width == Some(0) || self.height == Some(0) { Err(invalid("Invalid image properties")) } else { Ok(()) } }
}
impl EmailStyle {
    pub(crate) fn validate(&self) -> Result<(), EmailError> {
        if self.size.is_some_and(|s| !(8..=72).contains(&s)) { return Err(invalid("Font size must be 8–72 pt")); }
        if self.font.as_ref().is_some_and(|s| !safe_font(s)) { return Err(invalid("Unsafe font family")); }
        if self.link.as_ref().is_some_and(|s| !safe_url(s, false)) { return Err(invalid("Unsafe link URL")); }
        Ok(())
    }
    pub(crate) fn apply(&mut self, format: &EmailFormat) -> Result<(), EmailError> {
        match format { EmailFormat::Bold(v) => self.bold = *v, EmailFormat::Italic(v) => self.italic = *v, EmailFormat::Underline(v) => self.underline = *v, EmailFormat::Strike(v) => self.strike = *v, EmailFormat::Code(v) => self.code = *v, EmailFormat::Font(v) => self.font = v.clone(), EmailFormat::Size(v) => self.size = *v, EmailFormat::Color(v) => self.color = *v, EmailFormat::Link(v) => self.link = v.clone(), EmailFormat::Clear => *self = Self::default() }
        self.validate()
    }
}
pub(crate) fn safe_font(value: &str) -> bool { !value.is_empty() && value.len() <= 128 && value.chars().all(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | ',')) }
pub(crate) fn safe_url(value: &str, image: bool) -> bool {
    if value.is_empty() || value.len() > MAX_BYTES || value.chars().any(|c| c.is_control() || c.is_whitespace() || matches!(c, '<' | '>' | '"' | '\'' | '\\')) { return false; }
    let lower = value.to_ascii_lowercase();
    if lower.starts_with("https://") || lower.starts_with("http://") { return value.split_once("://").is_some_and(|(_, host)| !host.is_empty() && !host.starts_with('/')); }
    if !image { return lower.starts_with("mailto:") && value.len() > 7; }
    if lower.starts_with("cid:") || lower.starts_with("asset:") { return value.len() > value.find(':').unwrap_or(0) + 1; }
    ["data:image/png;base64,", "data:image/jpeg;base64,", "data:image/gif;base64,", "data:image/webp;base64,"].iter().any(|prefix| lower.starts_with(prefix) && value[prefix.len()..].bytes().all(|c| c.is_ascii_alphanumeric() || matches!(c, b'+' | b'/' | b'=')))
}
impl EmailText {
    pub(crate) fn byte_len(&self) -> usize { self.text.len() + self.images.values().map(|image| image.source.len() + image.alt.len()).sum::<usize>() + self.runs.iter().map(|r| r.style.font.as_ref().map_or(0, String::len) + r.style.link.as_ref().map_or(0, String::len)).sum::<usize>() }
    /// Construct literal unformatted text.
    pub fn plain(text: &str) -> Self { Self { text: text.into(), runs: if text.is_empty() { vec![] } else { vec![EmailRun { range: 0..text.len(), style: EmailStyle::default() }] }, images: Default::default() } }
    /// Look up a byte offset's style in logarithmic time; end-of-text inherits the final run.
    pub fn style_at(&self, offset: usize) -> EmailStyle {
        let index = self.runs.partition_point(|run| run.range.end <= offset);
        self.runs.get(index).filter(|run| run.range.contains(&offset))
            .or_else(|| self.runs.last().filter(|_| offset == self.text.len()))
            .map(|run| run.style.clone()).unwrap_or_default()
    }
    pub(crate) fn check_range(&self, range: &Range<usize>) -> Result<(), EmailError> { if range.start > range.end || range.end > self.text.len() || !self.text.is_char_boundary(range.start) || !self.text.is_char_boundary(range.end) { Err(invalid("Invalid UTF-8 text range")) } else { Ok(()) } }
    pub(crate) fn slice(&self, range: Range<usize>) -> Result<Self, EmailError> {
        self.check_range(&range)?;
        self.validate()?;
        Ok(Self { text: self.text[range.clone()].into(), runs: self.runs.iter().filter_map(|r| { let start = r.range.start.max(range.start); let end = r.range.end.min(range.end); (start < end).then(|| EmailRun { range: start - range.start..end - range.start, style: r.style.clone() }) }).collect(), images: self.images.range(range.clone()).map(|(offset, image)| (offset - range.start, image.clone())).collect() })
    }
    pub(crate) fn append(&mut self, other: &Self) { let offset = self.text.len(); self.text.push_str(&other.text); for r in &other.runs { self.push_run(offset + r.range.start..offset + r.range.end, r.style.clone()); } for (at, image) in &other.images { self.images.insert(offset + at, image.clone()); } }
    fn push_run(&mut self, range: Range<usize>, style: EmailStyle) { if range.is_empty() { return; } if let Some(last) = self.runs.last_mut() { if last.range.end == range.start && last.style == style { last.range.end = range.end; return; } } self.runs.push(EmailRun { range, style }); }
    /// Replace a byte range, retaining unaffected styles and merging adjacent runs.
    pub fn replace(&mut self, range: Range<usize>, text: &str, style: &EmailStyle) -> Result<(), EmailError> {
        self.check_range(&range)?; style.validate()?;
        if self.text.len() - range.len() + text.len() > MAX_BYTES { return Err(EmailError::Limit("text bytes")); }
        let mut result = self.slice(0..range.start)?;
        let mut inserted = Self::plain(text); for run in &mut inserted.runs { run.style = style.clone(); }
        result.append(&inserted); result.append(&self.slice(range.end..self.text.len())?);
        if result.byte_len() > MAX_BYTES { return Err(EmailError::Limit("expanded text bytes")); }
        if result.runs.len() > MAX_RUNS { return Err(EmailError::Limit("style runs")); }
        *self = result; Ok(())
    }
    /// Apply formatting without inserting source markup.
    pub fn format(&mut self, range: Range<usize>, format: &EmailFormat) -> Result<(), EmailError> {
        self.check_range(&range)?; let mut probe = EmailStyle::default(); probe.apply(format)?;
        let mut projected = self.text.len();
        for run in &self.runs { let mut style = run.style.clone(); if run.range.start < range.end && run.range.end > range.start { style.apply(format)?; } projected += style.font.as_ref().map_or(0, String::len) + style.link.as_ref().map_or(0, String::len); if projected > MAX_BYTES { return Err(EmailError::Limit("expanded text bytes")); } }
        let mut result = self.slice(0..range.start)?;
        let mut middle = self.slice(range.clone())?; for run in &mut middle.runs { run.style.apply(format)?; }
        result.append(&middle); result.append(&self.slice(range.end..self.text.len())?);
        if result.runs.len() > MAX_RUNS { return Err(EmailError::Limit("style runs")); }
        if result.byte_len() > MAX_BYTES { return Err(EmailError::Limit("expanded text bytes")); }
        *self = result; Ok(())
    }
    pub(crate) fn validate(&self) -> Result<(), EmailError> {
        let mut end = 0; let mut previous = None;
        for (offset, image) in &self.images { if self.text.get(*offset..offset.saturating_add(3)) != Some("\u{fffc}") { return Err(invalid("Inline image lacks an object token")); } image.validate()?; }
        for run in &self.runs { self.check_range(&run.range)?; run.style.validate()?; if run.range.is_empty() || run.range.start != end || previous == Some(&run.style) { return Err(invalid("Noncanonical style runs")); } end = run.range.end; previous = Some(&run.style); }
        if end != self.text.len() { return Err(invalid("Incomplete style cover")); } Ok(())
    }
}
impl EmailDocument {
    /// Construct and validate a structured document, rebuilding private editing budgets.
    /// An empty block vector becomes one empty paragraph.
    pub fn from_blocks(blocks: Vec<EmailBlock>) -> Result<Self, EmailError> {
        if blocks.is_empty() { return Ok(Self::default()); }
        let mut document = Self { blocks, revision: 0, bytes: 0, runs: 0, next: 1, block_revisions: std::collections::HashMap::new(), nonempty_blocks: 0, nonblank_blocks: 0, indices: Default::default(), changes: Default::default(), changes_floor: 0 }; document.validate()?;
        for block in &document.blocks { let (bytes, runs) = Self::block_budget(block); document.bytes += bytes; document.runs += runs; document.next = document.next.max(block.id.checked_add(1).ok_or_else(|| invalid("Block identity exhausted"))?); document.block_revisions.insert(block.id, 0); }
        document.indices = document.blocks.iter().enumerate().map(|(index, block)| (block.id, index)).collect();
        document.nonempty_blocks = document.blocks.iter().filter(|block| Self::block_nonempty(block)).count();
        document.nonblank_blocks = document.blocks.iter().filter(|block| Self::block_nonblank(block)).count();
        Ok(document)
    }
    /// Return all local splices since a revision, or `None` if the bounded journal cannot cover it.
    /// Current revision returns an empty list; future revisions are rejected.
    pub fn changes_since(&self, revision: u64) -> Option<Vec<EmailSpliceChange>> {
        if revision < self.changes_floor || revision > self.revision { return None; }
        Some(self.changes.iter().filter(|change| change.revision > revision).copied().collect())
    }
    /// Look up a stable block ID without scanning document content.
    pub fn index_by_id(&self, id: u64) -> Option<usize> { self.indices.get(&id).copied() }
    /// Read a block by its current ordered index.
    pub fn block_at(&self, index: usize) -> Option<&EmailBlock> { self.blocks.get(index) }
    fn record_change(&mut self, start: usize, removed: usize, inserted: usize) {
        self.changes.push_back(EmailSpliceChange { revision: self.revision, start, removed, inserted });
        while self.changes.len() > 256 { if let Some(old) = self.changes.pop_front() { self.changes_floor = old.revision; } }
    }
    /// Read the immutable ordered blocks.
    pub fn blocks(&self) -> &[EmailBlock] { &self.blocks }
    /// Read the monotonic editor revision.
    pub fn revision(&self) -> u64 { self.revision }
    /// Read the cached UTF-8 text, image metadata, and per-run style-string byte budget.
    /// This constant-time estimate excludes HTML markup, allocation overhead, and attachments.
    pub fn estimated_bytes(&self) -> usize { self.bytes }
    /// Whether the document contains neither text nor structural content, in constant time.
    /// Literal whitespace and paragraph separators count as text; images, tables and rules count as content.
    pub fn is_empty(&self) -> bool { self.nonempty_blocks == 0 && self.blocks.len() == 1 }
    fn block_nonempty(block: &EmailBlock) -> bool { match &block.kind { EmailBlockKind::Paragraph(paragraph) => !paragraph.content.text.is_empty(), _ => true } }
    /// Whether every paragraph contains only Unicode whitespace and no image, table or rule exists.
    /// Uses a locally maintained cache and never scans the document at query time.
    pub fn is_blank(&self) -> bool { self.nonblank_blocks == 0 }
    fn block_nonblank(block: &EmailBlock) -> bool { match &block.kind { EmailBlockKind::Paragraph(paragraph) => !paragraph.content.text.trim().is_empty(), _ => true } }
    /// Read a block's local change epoch in constant time, or `None` after removal.
    /// Epochs are document-local cache keys, not serialized content identities.
    pub fn block_revision(&self, id: u64) -> Option<u64> { self.block_revisions.get(&id).copied() }
    /// Enumerate all images in document order, including table-cell inline objects.
    pub fn images(&self) -> Vec<&EmailImage> {
        let mut images = Vec::new();
        for block in &self.blocks { match &block.kind { EmailBlockKind::Image(image) => images.push(image), EmailBlockKind::Paragraph(p) => images.extend(p.content.images.values()), EmailBlockKind::Table(t) => for cell in &t.cells { images.extend(cell.images.values()); }, EmailBlockKind::Rule => {} } }
        images
    }
    pub(crate) fn inline_image_offset(text: &EmailText, offset: usize) -> Option<usize> { if text.images.contains_key(&offset) { Some(offset) } else { offset.checked_sub(3).filter(|at| text.images.contains_key(at)) } }
    /// Resolve an inline image at the point or immediately before its caret, or a block image.
    pub fn image_at(&self, point: EmailPoint) -> Option<&EmailImage> {
        let block = self.blocks.get(*self.indices.get(&point.block)?)?;
        if let EmailBlockKind::Image(image) = &block.kind { return (point.cell == 0 && point.offset <= 1).then_some(image); }
        let text = self.text_at(point)?; text.images.get(&Self::inline_image_offset(text, point.offset)?)
    }
    /// Atomically map image metadata, validating every result before updating the document.
    pub fn try_map_images(&mut self, mut mapper: impl FnMut(&mut EmailImage) -> Result<(), EmailError>) -> Result<(), EmailError> {
        let mut changes = Vec::new(); let mut bytes = self.bytes;
        for (index, block) in self.blocks.iter().enumerate() {
            let has_images = match &block.kind { EmailBlockKind::Image(_) => true, EmailBlockKind::Paragraph(p) => !p.content.images.is_empty(), EmailBlockKind::Table(t) => t.cells.iter().any(|cell| !cell.images.is_empty()), EmailBlockKind::Rule => false };
            if !has_images { continue; }
            let mut changed = block.clone();
            match &mut changed.kind { EmailBlockKind::Image(image) => mapper(image)?, EmailBlockKind::Paragraph(p) => for image in p.content.images.values_mut() { mapper(image)?; }, EmailBlockKind::Table(t) => for cell in &mut t.cells { for image in cell.images.values_mut() { mapper(image)?; } }, EmailBlockKind::Rule => {} }
            Self::from_blocks(vec![changed.clone()])?; bytes = bytes - Self::block_budget(block).0 + Self::block_budget(&changed).0; changes.push((index, changed));
        }
        if bytes > MAX_BYTES { return Err(EmailError::Limit("document bytes")); }
        self.revision = self.revision.wrapping_add(1); for (index, block) in changes { self.block_revisions.insert(block.id, self.revision); self.blocks[index] = block; self.record_change(index, 1, 1); } self.bytes = bytes; Ok(())
    }
    pub(crate) fn block_budget(block: &EmailBlock) -> (usize, usize) { match &block.kind { EmailBlockKind::Paragraph(p) => (p.content.byte_len(), p.content.runs.len()), EmailBlockKind::Table(t) => t.cells.iter().fold((0, 0), |(b, r), c| (b + c.byte_len(), r + c.runs.len())), EmailBlockKind::Image(i) => (i.source.len() + i.alt.len(), 0), EmailBlockKind::Rule => (0, 0) } }
    pub(crate) fn splice_blocks(&mut self, start: usize, end: usize, blocks: Vec<EmailBlock>) -> Result<(), EmailError> {
        if start > end || end > self.blocks.len() { return Err(invalid("Invalid block splice")); }
        let empty = blocks.is_empty(); let checked = Self::from_blocks(blocks)?; let blocks = if empty { Vec::new() } else { checked.blocks };
        if self.blocks.len() - (end - start) + blocks.len() == 0 { return Err(invalid("Document must retain one block")); }
        if self.blocks.len() - (end - start) + blocks.len() > MAX_BLOCKS { return Err(EmailError::Limit("document blocks")); }
        for block in &blocks { if self.indices.get(&block.id).is_some_and(|index| *index < start || *index >= end) { return Err(invalid("Splice duplicates an unaffected block ID")); } }
        let removed = self.blocks[start..end].iter().fold((0, 0), |(b, r), block| { let (x, y) = Self::block_budget(block); (b + x, r + y) });
        let bytes = self.bytes - removed.0 + checked.bytes; let runs = self.runs - removed.1 + checked.runs;
        if bytes > MAX_BYTES { return Err(EmailError::Limit("document bytes")); } if runs > MAX_RUNS { return Err(EmailError::Limit("style runs")); }
        self.nonempty_blocks = self.nonempty_blocks - self.blocks[start..end].iter().filter(|block| Self::block_nonempty(block)).count() + checked.nonempty_blocks;
        self.nonblank_blocks = self.nonblank_blocks - self.blocks[start..end].iter().filter(|block| Self::block_nonblank(block)).count() + checked.nonblank_blocks;
        self.revision = self.revision.wrapping_add(1);
        for block in &self.blocks[start..end] { self.block_revisions.remove(&block.id); self.indices.remove(&block.id); }
        for block in &blocks { self.block_revisions.insert(block.id, self.revision); }
        let structural = blocks.len() != end - start; let replaced = blocks.len();
        self.blocks.splice(start..end, blocks);
        let limit = if structural { self.blocks.len() } else { start + replaced };
        for index in start..limit { self.indices.insert(self.blocks[index].id, index); }
        self.record_change(start, end - start, replaced);
        self.bytes = bytes; self.runs = runs; self.next = self.next.max(checked.next); Ok(())
    }
    /// Import plain text literally; newlines delimit paragraphs.
    pub fn from_plain(text: &str) -> Result<Self, EmailError> {
        if text.len() > MAX_BYTES { return Err(EmailError::Limit("import bytes")); }
        let text = text.replace("\r\n", "\n").replace('\r', "\n");
        if text.split('\n').count() > MAX_BLOCKS { return Err(EmailError::Limit("document blocks")); }
        let blocks = text.split('\n').enumerate().map(|(i, text)| EmailBlock { id: i as u64 + 1, kind: EmailBlockKind::Paragraph(EmailParagraph { content: EmailText::plain(text), ..Default::default() }) }).collect();
        Self::from_blocks(blocks)
    }
    /// Verify IDs, Unicode runs, supported properties, and resource limits.
    pub fn validate(&self) -> Result<(), EmailError> {
        if self.blocks.is_empty() || self.blocks.len() > MAX_BLOCKS { return Err(EmailError::Limit("document blocks")); }
        let mut ids = std::collections::HashSet::new(); let mut bytes = 0; let mut runs = 0;
        for block in &self.blocks {
            if block.id == 0 || !ids.insert(block.id) { return Err(invalid("Duplicate or zero block ID")); }
            match &block.kind {
                EmailBlockKind::Paragraph(p) => { if p.indent > 32 || matches!(p.kind, EmailParagraphKind::Heading(n) if !(1..=6).contains(&n)) { return Err(invalid("Invalid paragraph properties")); } p.content.validate()?; bytes += p.content.byte_len(); runs += p.content.runs.len(); }
                EmailBlockKind::Image(i) => { if !safe_url(&i.source, true) || i.width == Some(0) || i.height == Some(0) { return Err(invalid("Invalid image properties")); } bytes += i.source.len() + i.alt.len(); }
                EmailBlockKind::Table(t) => { t.validate()?; for cell in &t.cells { bytes += cell.byte_len(); runs += cell.runs.len(); } }
                EmailBlockKind::Rule => {}
            }
        }
        if bytes > MAX_BYTES { return Err(EmailError::Limit("document bytes")); } if runs > MAX_RUNS { return Err(EmailError::Limit("style runs")); } Ok(())
    }
    pub(crate) fn index(&self, point: EmailPoint) -> Result<usize, EmailError> {
        let index = *self.indices.get(&point.block).ok_or_else(|| invalid("Unknown block"))?;
        if let Some(text) = self.text_at(point) { text.check_range(&(point.offset..point.offset))?; } else if point.cell != 0 || point.offset > 1 || matches!(self.blocks[index].kind, EmailBlockKind::Paragraph(_) | EmailBlockKind::Table(_)) { return Err(invalid("Invalid document point")); } Ok(index)
    }
    /// Return the text addressed by a paragraph or table point.
    pub fn text_at(&self, point: EmailPoint) -> Option<&EmailText> { match &self.blocks.get(*self.indices.get(&point.block)?)?.kind { EmailBlockKind::Paragraph(p) if point.cell == 0 => Some(&p.content), EmailBlockKind::Table(t) if t.spans.get(point.cell).copied().unwrap_or(0) > 0 => t.cells.get(point.cell), _ => None } }
    pub(crate) fn point(block: &EmailBlock, end: bool) -> EmailPoint { let (cell, offset) = match &block.kind { EmailBlockKind::Paragraph(p) => (0, if end { p.content.text.len() } else { 0 }), EmailBlockKind::Table(t) => { let cell = if end { t.spans.iter().rposition(|s| *s > 0).unwrap_or(0) } else { 0 }; (cell, if end { t.cells.get(cell).map_or(0, |c| c.text.len()) } else { 0 }) }, _ => (0, usize::from(end)) }; EmailPoint { block: block.id, cell, offset } }
    /// Beginning of the document.
    pub fn first_point(&self) -> EmailPoint { self.blocks.first().map(|b| Self::point(b, false)).unwrap_or(EmailPoint { block: 0, cell: 0, offset: 0 }) }
    /// End of the document.
    pub fn end_point(&self) -> EmailPoint { self.blocks.last().map(|b| Self::point(b, true)).unwrap_or(EmailPoint { block: 0, cell: 0, offset: 0 }) }
    /// Validate and order endpoints without losing selection direction in the caller.
    pub fn ordered_selection(&self, selection: &EmailSelection) -> Result<(EmailPoint, EmailPoint), EmailError> { let a = self.index(selection.anchor)?; let h = self.index(selection.head)?; Ok(if (a, selection.anchor.cell, selection.anchor.offset) <= (h, selection.head.cell, selection.head.offset) { (selection.anchor, selection.head) } else { (selection.head, selection.anchor) }) }
    pub(crate) fn next_id(&self) -> Result<u64, EmailError> { if self.next == u64::MAX { Err(invalid("Block identity exhausted")) } else { Ok(self.next) } }
}
impl EmailTable {
    pub(crate) fn validate(&self) -> Result<(), EmailError> {
        if self.columns == 0 || self.columns > 64 || self.cells.is_empty() || self.cells.len() > 4096 || self.cells.len() % self.columns != 0 || self.cells.len() / self.columns > 64 || self.spans.len() != self.cells.len() { return Err(invalid("Invalid table dimensions")); }
        for row in 0..self.cells.len() / self.columns { let mut col = 0; while col < self.columns { let index = row * self.columns + col; let span = self.spans[index]; if span == 0 || span > self.columns - col { return Err(invalid("Invalid table span")); } for covered in index + 1..index + span { if self.spans[covered] != 0 || !self.cells[covered].text.is_empty() { return Err(invalid("Nonempty covered table cell")); } } col += span; } }
        for cell in &self.cells { cell.validate()?; } Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cross_block_full_selection_removes_end_table() {
        let mut doc = EmailDocument::from_html("<p>A</p><table><tr><td>B</td></tr></table>").unwrap();
        let selection = EmailSelection { anchor: doc.first_point(), head: doc.end_point() };
        doc.replace(&selection, "", &EmailStyle::default()).unwrap();
        assert_eq!(doc.blocks.len(), 1); assert_eq!(doc.to_plain(), "");
    }
    #[test]
    fn insertion_before_atomic_block_keeps_caret_before_it() {
        let mut doc = EmailDocument::default();
        let rule = doc.insert_rule(&EmailSelection::caret(doc.first_point())).unwrap();
        let inserted = doc.replace(&rule, "text", &EmailStyle::default()).unwrap();
        assert_eq!(doc.text_at(inserted.head).unwrap().text, "text");
        assert_eq!(inserted.head.offset, 4);
        let image = doc.insert_image(&rule, EmailImage { source: "cid:x".into(), alt: "x".into(), width: None, height: None, align: EmailAlign::Left }).unwrap();
        assert!(doc.image_at(image.head).is_some());
    }
    #[test]
    fn atomic_selection_delete_does_not_reinsert_the_rule() {
        let mut doc = EmailDocument::default();
        let at = doc.insert_rule(&EmailSelection::caret(doc.first_point())).unwrap();
        let selected = EmailSelection { anchor: at.head, head: EmailPoint { offset: 1, ..at.head } };
        doc.replace(&selected, "", &EmailStyle::default()).unwrap();
        assert!(!doc.blocks.iter().any(|b| matches!(b.kind, EmailBlockKind::Rule)));
    }

    #[test]
    fn empty_list_enter_exits_without_an_extra_list_item() {
        let mut doc = EmailDocument::default();
        let at = EmailSelection::caret(doc.first_point());
        doc.paragraph(&at, EmailParagraphKind::Bullet, EmailAlign::Left, 0).unwrap();
        doc.replace(&at, "\n", &EmailStyle::default()).unwrap();
        assert_eq!(doc.blocks.len(), 1);
        assert!(matches!(&doc.blocks[0].kind, EmailBlockKind::Paragraph(p) if p.kind == EmailParagraphKind::Normal));
    }

    #[test]
    fn failed_table_change_preserves_content_and_revision() {
        let mut doc = EmailDocument::default();
        let at = doc.insert_table(&EmailSelection::caret(doc.first_point()), 1, 2).unwrap();
        let before = doc.clone();
        assert!(doc.table_edit(at.head.block, &EmailTableEdit::DeleteRow(0)).is_err());
        assert_eq!(doc, before);
    }
    use super::super::history::EmailHistory;

    #[test]
    fn cross_run_and_paragraph_replace_restores_selection() {
        let mut doc = EmailDocument::from_plain("你好 world\nsecond 🦀").unwrap();
        let selection = EmailSelection { anchor: EmailPoint { block: 1, cell: 0, offset: 6 }, head: EmailPoint { block: 2, cell: 0, offset: 7 } };
        doc.format(&selection, &EmailFormat::Bold(true)).unwrap();
        let before = doc.clone();
        let checkpoint = EmailHistory::begin(&doc, &selection).unwrap();
        let after = doc.replace(&selection, "X\nY", &EmailStyle::default()).unwrap();
        assert_eq!(doc.to_plain(), "你好X\nY🦀");
        let mut history = EmailHistory::default();
        history.commit(&doc, checkpoint, after.clone()).unwrap();
        assert_eq!(history.undo(&mut doc), Some(selection));
        assert_eq!(doc.blocks, before.blocks);
        assert_eq!(history.redo(&mut doc), Some(after));
        assert_eq!(doc.to_plain(), "你好X\nY🦀");
    }

    #[test]
    fn invalid_unicode_boundary_is_atomic() {
        let mut text = EmailText::plain("a🦀中");
        let original = text.clone();
        assert!(text.replace(2..5, "x", &EmailStyle::default()).is_err());
        assert_eq!(text, original);
        text.format(1..5, &EmailFormat::Bold(true)).unwrap();
        assert_eq!(text.runs.len(), 3);
        text.replace(1..5, "e\u{301}", &EmailStyle::default()).unwrap();
        assert_eq!(text.text, "ae\u{301}中");
    }

    #[test]
    fn plain_newline_and_hardbreak_are_distinct() {
        let doc = EmailDocument::from_plain("*literal*\n\n  x\u{2028}y").unwrap();
        assert_eq!(doc.blocks.len(), 3);
        assert_eq!(doc.to_plain(), "*literal*\n\n  x\ny");
        assert!(doc.to_html().contains("x<br>y"));
    }

    #[test]
    fn table_merge_and_structure_remain_rectangular() {
        let mut doc = EmailDocument::default();
        let selection = EmailSelection::caret(doc.first_point());
        let at = doc.insert_table(&selection, 2, 3).unwrap();
        doc.table_edit(at.head.block, &EmailTableEdit::Merge { row: 0, column: 0, columns: 2 }).unwrap();
        doc.table_edit(at.head.block, &EmailTableEdit::InsertColumn(1)).unwrap();
        doc.validate().unwrap();
        doc.table_edit(at.head.block, &EmailTableEdit::DeleteRow(1)).unwrap();
        doc.validate().unwrap();
    }
}

#[cfg(test)]
mod budget_tests {
    use super::*;
    #[test]
    fn format_rejects_expanded_metadata_atomically() {
        let mut text = EmailText::plain(&"x".repeat(100));
        for index in (0..100).step_by(2) { text.format(index..index + 1, &EmailFormat::Bold(true)).unwrap(); }
        let before = text.clone();
        assert!(text.format(0..100, &EmailFormat::Link(Some(format!("https://example.com/{}", "a".repeat(100_000))))).is_err());
        assert_eq!(text, before);
    }
    #[test]
    fn cached_budgets_follow_undo_and_structural_edits() {
        let mut document = EmailDocument::from_plain("one\ntwo").unwrap();
        let selection = EmailSelection::caret(document.first_point());
        let checkpoint = super::super::history::EmailHistory::begin(&document, &selection).unwrap();
        let after = document.replace(&selection, "X\nY", &EmailStyle::default()).unwrap();
        let mut history = super::super::history::EmailHistory::default(); history.commit(&document, checkpoint, after).unwrap();
        history.undo(&mut document).unwrap();
        let rebuilt = EmailDocument::from_blocks(document.blocks.clone()).unwrap();
        assert_eq!(document.bytes, rebuilt.bytes); assert_eq!(document.runs, rebuilt.runs);
    }
}

#[cfg(test)]
mod revision_tests {
    use super::*;
    #[test]
    fn block_revision_only_changes_for_the_edited_splice() {
        let mut document = EmailDocument::from_plain("one\ntwo").unwrap();
        let first = document.first_point(); let second = document.end_point();
        let first_revision = document.block_revision(first.block).unwrap();
        let second_revision = document.block_revision(second.block).unwrap();
        document.replace(&EmailSelection::caret(first), "x", &EmailStyle::default()).unwrap();
        assert_ne!(document.block_revision(first.block), Some(first_revision));
        assert_eq!(document.block_revision(second.block), Some(second_revision));
        assert_eq!(document.block_revision(u64::MAX), None);
    }
}

#[cfg(test)]
mod empty_tests {
    use super::*;
    #[test]
    fn semantic_emptiness_counts_whitespace_and_atomic_blocks() {
        assert!(EmailDocument::default().is_empty());
        assert!(!EmailDocument::from_plain(" ").unwrap().is_empty());
        assert!(!EmailDocument::from_plain("\n").unwrap().is_empty());
        let mut document = EmailDocument::default();
        let selection = EmailSelection::caret(document.first_point());
        document.insert_table(&selection, 1, 1).unwrap();
        assert!(!document.is_empty());
        let selection = EmailSelection { anchor: document.first_point(), head: document.end_point() };
        document.replace(&selection, "", &EmailStyle::default()).unwrap();
        assert!(document.is_empty());
    }
}

#[cfg(test)]
mod lookup_tests {
    use super::*;
    #[test]
    fn style_lookup_observes_run_boundaries_and_end_inheritance() {
        let mut text = EmailText::plain("a中🦀");
        text.format(1..4, &EmailFormat::Bold(true)).unwrap();
        text.format(4..8, &EmailFormat::Italic(true)).unwrap();
        assert_eq!(text.style_at(0), EmailStyle::default());
        assert!(text.style_at(1).bold); assert!(text.style_at(3).bold);
        assert!(text.style_at(4).italic); assert!(text.style_at(8).italic);
        assert_eq!(text.style_at(9), EmailStyle::default());
        assert_eq!(EmailText::default().style_at(0), EmailStyle::default());
    }
}

#[cfg(test)]
mod estimate_tests {
    use super::*;
    #[test]
    fn cached_estimate_tracks_unicode_text_and_style_metadata() {
        let mut document = EmailDocument::from_plain("中").unwrap();
        assert_eq!(document.estimated_bytes(), 3);
        let selection = EmailSelection { anchor: document.first_point(), head: document.end_point() };
        document.format(&selection, &EmailFormat::Link(Some("https://example.com".into()))).unwrap();
        assert_eq!(document.estimated_bytes(), 3 + "https://example.com".len());
    }
}

#[cfg(test)]
mod blank_tests {
    use super::*;
    #[test]
    fn blankness_ignores_whitespace_but_not_structural_content() {
        let mut document = EmailDocument::from_plain(" \t\n\u{2003}").unwrap();
        assert!(document.is_blank()); assert!(!document.is_empty());
        let at = EmailSelection::caret(document.first_point());
        document.replace(&at, "x", &EmailStyle::default()).unwrap(); assert!(!document.is_blank());
        let all = EmailSelection { anchor: document.first_point(), head: document.end_point() };
        document.replace(&all, " ", &EmailStyle::default()).unwrap(); assert!(document.is_blank());
        document.insert_rule(&EmailSelection::caret(document.first_point())).unwrap(); assert!(!document.is_blank());
    }
}

#[cfg(test)]
mod inline_tests {
    use super::*;
    fn image() -> EmailImage { EmailImage { source: "cid:photo".into(), alt: "photo".into(), width: Some(20), height: Some(10), align: EmailAlign::Left } }
    #[test]
    fn inline_insertion_replacement_and_undo_preserve_flow() {
        let mut document = EmailDocument::from_plain("left right").unwrap();
        let at = EmailSelection::caret(EmailPoint { offset: 5, ..document.first_point() });
        let checkpoint = super::super::history::EmailHistory::begin(&document, &at).unwrap();
        let after = document.insert_image(&at, image()).unwrap();
        assert_eq!(document.blocks().len(), 1); assert_eq!(document.to_plain(), "left photoright");
        assert!(document.image_at(after.head).is_some());
        let mut history = super::super::history::EmailHistory::default(); history.commit(&document, checkpoint, after.clone()).unwrap();
        history.undo(&mut document).unwrap(); assert_eq!(document.to_plain(), "left right");
        history.redo(&mut document).unwrap();
        let mut replacement = image(); replacement.source = "cid:new".into(); document.update_image(after.head, replacement).unwrap();
        assert_eq!(document.images()[0].source, "cid:new");
        document.delete_image(after.head).unwrap(); assert_eq!(document.to_plain(), "left right");
    }
    #[test]
    fn inline_slice_shift_and_format_keep_object_metadata() {
        let mut document = EmailDocument::from_plain("ab").unwrap();
        let after = document.insert_image(&EmailSelection::caret(EmailPoint { offset: 1, ..document.first_point() }), image()).unwrap();
        let selection = EmailSelection { anchor: EmailPoint { offset: 1, ..after.head }, head: after.head };
        document.format(&selection, &EmailFormat::Bold(true)).unwrap();
        let fragment = document.fragment(&selection).unwrap(); assert_eq!(fragment.to_plain(), "photo"); assert_eq!(fragment.images().len(), 1);
        document.replace(&EmailSelection::caret(document.first_point()), "中", &EmailStyle::default()).unwrap();
        let text = document.text_at(document.first_point()).unwrap(); assert!(text.images.contains_key(&4));
        document.validate().unwrap();
    }
    #[test]
    fn inline_html_and_table_objects_roundtrip_in_place() {
        let document = EmailDocument::from_html("<p>left <img src='cid:x' alt='X'> right</p><table><tr><td>a<img src='cid:y' alt='Y'>b</td></tr></table>").unwrap();
        assert_eq!(document.to_plain(), "left X right\naYb"); assert_eq!(document.images().len(), 2);
        assert_eq!(EmailDocument::from_html(&document.to_html()).unwrap().blocks(), document.blocks());
    }
}

#[cfg(test)]
mod inline_safety_tests {
    use super::*;
    #[test]
    fn image_mapping_reaches_table_objects_and_is_atomic() {
        let mut document = EmailDocument::from_html("<img src='cid:block'><p>a<img src='cid:inline'>b</p><table><tr><td><img src='cid:cell'></td></tr></table>").unwrap();
        assert_eq!(document.images().len(), 3);
        document.try_map_images(|image| { image.alt = "mapped".into(); Ok(()) }).unwrap();
        assert!(document.images().iter().all(|image| image.alt == "mapped"));
        let before = document.clone();
        assert!(document.try_map_images(|image| { image.source = "file:///secret".into(); Ok(()) }).is_err());
        assert_eq!(document, before);
    }
    #[test]
    fn object_byte_splitting_and_invalid_image_metadata_are_rejected() {
        let mut text = EmailText::plain("a\u{fffc}b");
        text.images.insert(1, EmailImage { source: "cid:x".into(), alt: "X".into(), width: None, height: None, align: EmailAlign::Left });
        let before = text.clone(); assert!(text.replace(2..4, "", &EmailStyle::default()).is_err()); assert_eq!(text, before);
        let mut document = EmailDocument::from_blocks(vec![EmailBlock { id: 1, kind: EmailBlockKind::Paragraph(EmailParagraph { content: text, ..Default::default() }) }]).unwrap();
        let after = document.delete_image(EmailPoint { block: 1, cell: 0, offset: 4 }).unwrap();
        assert_eq!(after.head.offset, 1); assert_eq!(document.to_plain(), "ab"); assert!(document.images().is_empty());
    }
}

#[cfg(test)]
mod index_tests {
    use super::*;
    #[test]
    fn structural_splice_and_undo_keep_stable_id_indices() {
        let mut document = EmailDocument::from_plain("a\nb\nc").unwrap();
        let last = document.end_point();
        let at = EmailSelection::caret(document.first_point());
        let checkpoint = super::super::history::EmailHistory::begin(&document, &at).unwrap();
        let after = document.replace(&at, "x\ny", &EmailStyle::default()).unwrap();
        assert_eq!(document.index(last).unwrap(), 3); assert_eq!(document.text_at(last).unwrap().text, "c");
        let mut history = super::super::history::EmailHistory::default(); history.commit(&document, checkpoint, after).unwrap();
        history.undo(&mut document).unwrap(); assert_eq!(document.index(last).unwrap(), 2);
        history.redo(&mut document).unwrap(); assert_eq!(document.index(last).unwrap(), 3);
    }
}

#[cfg(test)]
mod journal_tests {
    use super::*;
    #[test]
    fn splice_journal_tracks_local_structural_and_history_changes() {
        let mut document = EmailDocument::from_plain("a\nb").unwrap();
        let at = EmailSelection::caret(document.first_point());
        let checkpoint = super::super::history::EmailHistory::begin(&document, &at).unwrap();
        let after = document.replace(&at, "x\ny", &EmailStyle::default()).unwrap();
        assert_eq!(document.changes_since(0).unwrap(), vec![EmailSpliceChange { revision: 1, start: 0, removed: 1, inserted: 2 }]);
        let mut history = super::super::history::EmailHistory::default(); history.commit(&document, checkpoint, after).unwrap(); history.undo(&mut document).unwrap();
        assert_eq!(document.changes_since(1).unwrap()[0], EmailSpliceChange { revision: 2, start: 0, removed: 2, inserted: 1 });
        assert_eq!(document.index_by_id(2), Some(1)); assert_eq!(document.block_at(1).unwrap().id, 2);
    }
    #[test]
    fn journal_expires_old_revisions_without_partial_results() {
        let mut document = EmailDocument::default();
        for _ in 0..257 { document.replace(&EmailSelection::caret(document.first_point()), "x", &EmailStyle::default()).unwrap(); }
        assert!(document.changes_since(0).is_none());
        assert_eq!(document.changes_since(1).unwrap().len(), 256);
        assert_eq!(document.changes_since(document.revision()), Some(Vec::new()));
        assert!(document.changes_since(document.revision() + 1).is_none());
    }
}
