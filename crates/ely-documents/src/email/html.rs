//! HTML5 tree parsing with a deliberately small, inert editing allowlist.
use super::model::*;
use html5ever::{parse_document, Attribute, ExpandedName, QualName};
use html5ever::tendril::{StrTendril, TendrilSink};
use html5ever::tree_builder::{ElementFlags, NodeOrText, QuirksMode, TreeSink};
use std::borrow::Cow;

#[derive(Default)]
struct Node { name: Option<QualName>, attrs: Vec<Attribute>, text: String, children: Vec<usize>, parent: Option<usize> }
struct Dom { nodes: Vec<Node>, too_deep: bool }
impl Dom {
    fn new() -> Self { Self { nodes: vec![Node::default()], too_deep: false } }
    fn alloc(&mut self, node: Node) -> usize { let id = self.nodes.len(); self.nodes.push(node); id }
    fn detach(&mut self, id: usize) { if let Some(parent) = self.nodes[id].parent.take() { self.nodes[parent].children.retain(|child| *child != id); } }
    fn attach(&mut self, parent: usize, child: NodeOrText<usize>, before: Option<usize>) {
        let id = match child { NodeOrText::AppendNode(id) => { self.detach(id); id }, NodeOrText::AppendText(text) => {
            let previous = if let Some(before) = before { self.nodes[parent].children.iter().position(|id| *id == before).and_then(|index| index.checked_sub(1)).map(|index| self.nodes[parent].children[index]) } else { self.nodes[parent].children.last().copied() };
            if let Some(previous) = previous { if self.nodes[previous].name.is_none() { self.nodes[previous].text.push_str(&text); return; } }
            self.alloc(Node { text: text.to_string(), ..Default::default() })
        } };
        let mut ancestor = Some(parent); let mut depth = usize::from(!matches!(self.tag(id), "" | "html" | "head" | "body"));
        while let Some(current) = ancestor { depth += usize::from(!matches!(self.tag(current), "" | "html" | "head" | "body")); if depth > 32 { self.too_deep = true; break; } ancestor = self.nodes[current].parent; }
        self.nodes[id].parent = Some(parent);
        if let Some(index) = before.and_then(|before| self.nodes[parent].children.iter().position(|id| *id == before)) { self.nodes[parent].children.insert(index, id); } else { self.nodes[parent].children.push(id); }
    }
    fn tag(&self, id: usize) -> &str { self.nodes[id].name.as_ref().map(|n| n.local.as_ref()).unwrap_or("") }
    fn attr(&self, id: usize, name: &str) -> Option<&str> { self.nodes[id].attrs.iter().find(|a| a.name.ns.as_ref().is_empty() && a.name.local.as_ref() == name).map(|a| a.value.as_ref()) }
}
impl TreeSink for Dom {
    type Handle = usize;
    type Output = Self;
    fn finish(self) -> Self { self }
    fn get_document(&mut self) -> usize { 0 }
    fn get_template_contents(&mut self, target: &usize) -> usize { *target }
    fn same_node(&self, a: &usize, b: &usize) -> bool { a == b }
    fn elem_name(&self, target: &usize) -> ExpandedName<'_> { self.nodes[*target].name.as_ref().expect("html5ever requests element names only for elements").expanded() }
    fn create_element(&mut self, name: QualName, attrs: Vec<Attribute>, _: ElementFlags) -> usize { self.alloc(Node { name: Some(name), attrs, ..Default::default() }) }
    fn create_comment(&mut self, _: StrTendril) -> usize { self.alloc(Node::default()) }
    fn create_pi(&mut self, _: StrTendril, _: StrTendril) -> usize { self.alloc(Node::default()) }
    fn append(&mut self, parent: &usize, child: NodeOrText<usize>) { self.attach(*parent, child, None); }
    fn append_before_sibling(&mut self, sibling: &usize, child: NodeOrText<usize>) { if let Some(parent) = self.nodes[*sibling].parent { self.attach(parent, child, Some(*sibling)); } }
    fn append_based_on_parent_node(&mut self, element: &usize, previous: &usize, child: NodeOrText<usize>) { if self.nodes[*element].parent.is_some() { self.append_before_sibling(element, child); } else { self.append(previous, child); } }
    fn parse_error(&mut self, _: Cow<'static, str>) {}
    fn set_quirks_mode(&mut self, _: QuirksMode) {}
    fn append_doctype_to_document(&mut self, _: StrTendril, _: StrTendril, _: StrTendril) {}
    fn add_attrs_if_missing(&mut self, target: &usize, attrs: Vec<Attribute>) { for attr in attrs { if !self.nodes[*target].attrs.iter().any(|a| a.name == attr.name) { self.nodes[*target].attrs.push(attr); } } }
    fn remove_from_parent(&mut self, target: &usize) { self.detach(*target); }
    fn reparent_children(&mut self, node: &usize, parent: &usize) { let children = std::mem::take(&mut self.nodes[*node].children); for child in children { self.attach(*parent, NodeOrText::AppendNode(child), None); } }
}
fn hidden(dom: &Dom, id: usize) -> bool {
    if matches!(dom.tag(id), "script" | "style" | "iframe" | "object" | "embed" | "form" | "input" | "button" | "select" | "textarea" | "template" | "head" | "title" | "svg" | "math" | "noscript") { return true; }
    if dom.attr(id, "hidden").is_some() { return true; }
    if dom.attr(id, "data-email-task-marker") == Some("true") && dom.nodes[id].parent.is_some_and(|parent| dom.attr(parent, "data-task") == Some("true")) { return true; }
    dom.attr(id, "style").is_some_and(|css| css.split(';').any(|property| property.split_once(':').is_some_and(|(key, value)| { let value = value.trim().to_ascii_lowercase(); matches!(key.trim().to_ascii_lowercase().as_str(), "display") && value.starts_with("none") || key.trim().eq_ignore_ascii_case("visibility") && value.starts_with("hidden") })))
}
fn color(value: &str) -> Option<EmailColor> {
    let lower = value.trim().to_ascii_lowercase();
    let rgb = if let Some(hex) = lower.strip_prefix('#') {
        let hex = if hex.len() == 3 { hex.chars().flat_map(|c| [c, c]).collect::<String>() } else { hex.to_string() };
        if hex.len() != 6 || !hex.bytes().all(|c| c.is_ascii_hexdigit()) { return None; } u32::from_str_radix(&hex, 16).ok()?
    } else if let Some(rgb) = lower.strip_prefix("rgb(").and_then(|v| v.strip_suffix(')')) {
        let values = rgb.split(',').map(|v| v.trim().parse::<u8>()).collect::<Result<Vec<_>, _>>().ok()?; if values.len() != 3 { return None; } (u32::from(values[0]) << 16) | (u32::from(values[1]) << 8) | u32::from(values[2])
    } else { match lower.as_str() { "black" => 0, "white" => 0xffffff, "red" => 0xff0000, "green" => 0x008000, "blue" => 0x0000ff, "yellow" => 0xffff00, "gray" | "grey" => 0x808080, "purple" => 0x800080, "orange" => 0xffa500, _ => return None } };
    Some(EmailColor { red: (rgb >> 16) as u8, green: (rgb >> 8) as u8, blue: rgb as u8 })
}
fn alignment(value: &str) -> Option<EmailAlign> { match value.trim().to_ascii_lowercase().as_str() { "left" => Some(EmailAlign::Left), "center" => Some(EmailAlign::Center), "right" => Some(EmailAlign::Right), "justify" => Some(EmailAlign::Justify), _ => None } }
fn properties(dom: &Dom, id: usize, inherited: &EmailStyle) -> (EmailStyle, Option<EmailAlign>) {
    let mut style = inherited.clone(); let mut align = dom.attr(id, "align").and_then(alignment);
    match dom.tag(id) { "b" | "strong" => style.bold = true, "i" | "em" => style.italic = true, "u" => style.underline = true, "s" | "strike" | "del" => style.strike = true, "code" => style.code = true, "a" => style.link = dom.attr(id, "href").filter(|v| safe_url(v, false)).map(str::to_string), _ => {} }
    if dom.tag(id) == "font" { style.font = dom.attr(id, "face").filter(|v| safe_font(v)).map(str::to_string).or(style.font); style.color = dom.attr(id, "color").and_then(color).or(style.color); }
    if let Some(css) = dom.attr(id, "style") { for property in css.split(';') { let Some((key, value)) = property.split_once(':') else { continue; }; let value = value.trim(); match key.trim().to_ascii_lowercase().as_str() {
        "font-family" => { let value = value.trim_matches(['\'', '"']); if safe_font(value) { style.font = Some(value.into()); } }
        "font-size" => { let lower = value.to_ascii_lowercase(); let size = if let Some(v) = lower.strip_suffix("pt") { v.trim().parse::<f32>().ok() } else if let Some(v) = lower.strip_suffix("px") { v.trim().parse::<f32>().ok().map(|v| v * 0.75) } else { None }; if let Some(size) = size.filter(|s| s.is_finite() && (8.0..=72.0).contains(s)) { style.size = Some(size.round() as u16); } }
        "font-weight" => style.bold = value.eq_ignore_ascii_case("bold") || value.parse::<u16>().is_ok_and(|v| v >= 600),
        "font-style" => style.italic = value.eq_ignore_ascii_case("italic") || value.eq_ignore_ascii_case("oblique"),
        "text-decoration" | "text-decoration-line" => { style.underline = value.split_whitespace().any(|v| v == "underline"); style.strike = value.split_whitespace().any(|v| v == "line-through"); }
        "color" => { if let Some(value) = color(value) { style.color = Some(value); } }
        "text-align" => align = alignment(value),
        _ => {}
    } } }
    (style, align)
}
struct Importer<'a> { dom: &'a Dom, blocks: Vec<EmailBlock>, pending: Option<EmailParagraph>, runs: usize, bytes: usize, boundary: bool }
impl<'a> Importer<'a> {
    fn push(&mut self, kind: EmailBlockKind) -> Result<(), EmailError> { if self.blocks.len() == MAX_BLOCKS { return Err(EmailError::Limit("document blocks")); } self.blocks.push(EmailBlock { id: self.blocks.len() as u64 + 1, kind }); Ok(()) }
    fn flush(&mut self) -> Result<(), EmailError> { self.boundary = false; if let Some(p) = self.pending.take() { self.push(EmailBlockKind::Paragraph(p))?; } Ok(()) }
    fn text(&mut self, value: &str, style: &EmailStyle) -> Result<(), EmailError> {
        if value.is_empty() { return Ok(()); }
        if self.boundary { self.boundary = false; if self.pending.as_ref().is_some_and(|p| !p.content.text.is_empty()) { self.text("\u{2028}", style)?; } }
        let p = self.pending.get_or_insert_with(EmailParagraph::default);
        let coalesce = p.content.runs.last().is_some_and(|last| last.style == *style);
        self.bytes += value.len() + if coalesce { 0 } else { style.font.as_ref().map_or(0, String::len) + style.link.as_ref().map_or(0, String::len) };
        if self.bytes > MAX_BYTES { return Err(EmailError::Limit("expanded document bytes")); }
        let start = p.content.text.len(); p.content.text.push_str(value);
        if coalesce { if let Some(last) = p.content.runs.last_mut() { last.range.end = p.content.text.len(); } return Ok(()); }
        self.runs += 1; if self.runs > MAX_RUNS { return Err(EmailError::Limit("style runs")); }
        p.content.runs.push(EmailRun { range: start..p.content.text.len(), style: style.clone() }); Ok(())
    }
    fn image(&mut self, id: usize, style: &EmailStyle, align: EmailAlign, inline: bool) -> Result<(), EmailError> {
        let Some(source) = self.dom.attr(id, "src").filter(|source| safe_url(source, true)) else { return Ok(()); };
        let image = EmailImage { source: source.into(), alt: self.dom.attr(id, "alt").unwrap_or("").into(), width: self.dom.attr(id, "width").and_then(|v| v.parse().ok()).filter(|v| *v > 0), height: self.dom.attr(id, "height").and_then(|v| v.parse().ok()).filter(|v| *v > 0), align: self.dom.attr(id, "data-email-align").and_then(alignment).unwrap_or(align) };
        if inline {
            self.bytes += image.source.len() + image.alt.len(); if self.bytes > MAX_BYTES { return Err(EmailError::Limit("expanded document bytes")); }
            self.text("\u{fffc}", style)?; let paragraph = self.pending.as_mut().ok_or_else(|| invalid("Missing inline image paragraph"))?; paragraph.content.images.insert(paragraph.content.text.len() - 3, image);
        } else { self.flush()?; self.push(EmailBlockKind::Image(image))?; }
        Ok(())
    }
    fn inline(&mut self, id: usize, style: &EmailStyle, depth: usize) -> Result<(), EmailError> {
        if depth > 32 { return Err(EmailError::Limit("HTML depth")); } if hidden(self.dom, id) { return Ok(()); }
        let (style, _) = properties(self.dom, id, style);
        if self.dom.tag(id) == "img" { return self.image(id, &style, EmailAlign::Left, true); }
        let block_boundary = matches!(self.dom.tag(id), "p" | "div" | "blockquote" | "li" | "pre");
        if block_boundary { self.boundary = true; }
        if self.dom.tag(id) == "br" { self.text("\u{2028}", &style)?; } else { self.text(&self.dom.nodes[id].text, &style)?; for child in &self.dom.nodes[id].children { self.inline(*child, &style, depth + 1)?; } }
        if block_boundary { self.boundary = true; } Ok(())
    }
    fn table(&mut self, id: usize, style: &EmailStyle) -> Result<(), EmailError> {
        fn rows(dom: &Dom, id: usize, out: &mut Vec<usize>) { for child in &dom.nodes[id].children { match dom.tag(*child) { "tr" => out.push(*child), "tbody" | "thead" | "tfoot" => rows(dom, *child, out), _ => {} } } }
        let mut row_ids = Vec::new(); rows(self.dom, id, &mut row_ids); if row_ids.len() > 64 { return Err(EmailError::Limit("table rows")); }
        let mut row_data = Vec::new(); let mut columns = 0;
        for row in row_ids { let mut cells = Vec::new(); let mut spans = Vec::new();
            for cell in &self.dom.nodes[row].children { if !matches!(self.dom.tag(*cell), "td" | "th") || hidden(self.dom, *cell) { continue; }
                let span = match self.dom.attr(*cell, "colspan") { Some(v) => v.parse::<usize>().map_err(|_| invalid("Invalid colspan"))?, None => 1 }; if span == 0 || span > 64 || cells.len() + span > 64 { return Err(EmailError::Limit("table columns")); }
                let saved = self.pending.take(); let saved_boundary = self.boundary; self.boundary = false; self.pending = Some(EmailParagraph::default()); self.inline(*cell, style, 0)?; let content = self.pending.take().unwrap_or_default().content; self.pending = saved; self.boundary = saved_boundary;
                cells.push(content); spans.push(span); for _ in 1..span { cells.push(EmailText::default()); spans.push(0); }
            }
            if !cells.is_empty() { columns = columns.max(cells.len()); row_data.push((cells, spans)); }
        }
        if columns == 0 || row_data.is_empty() { return Ok(()); }
        let mut cells = Vec::new(); let mut spans = Vec::new(); for (mut row, mut row_spans) in row_data { row.resize(columns, EmailText::default()); row_spans.resize(columns, 1); cells.extend(row); spans.extend(row_spans); }
        self.push(EmailBlockKind::Table(EmailTable { columns, cells, spans }))
    }
    fn visit(&mut self, id: usize, inherited: &EmailStyle, depth: usize, list: Option<(bool, u32)>, indent: u8, inherited_align: EmailAlign) -> Result<(), EmailError> {
        if depth > 32 { return Err(EmailError::Limit("HTML depth")); } if hidden(self.dom, id) { return Ok(()); }
        let tag = self.dom.tag(id); let (style, align) = properties(self.dom, id, inherited); let align = align.unwrap_or(inherited_align);
        if tag == "div" && self.dom.attr(id, "data-email-image") == Some("true") { self.flush()?; for child in &self.dom.nodes[id].children { self.visit(*child, &style, depth + 1, list, indent, align)?; } return Ok(()); }
        let explicit = matches!(tag, "p" | "div" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "blockquote" | "li" | "pre") && !(matches!(tag, "p" | "div") && self.pending.as_ref().is_some_and(|p| matches!(p.kind, EmailParagraphKind::Quote | EmailParagraphKind::Bullet | EmailParagraphKind::Numbered(_) | EmailParagraphKind::Task(_))));
        let nested_boundary = !explicit && matches!(tag, "p" | "div"); if nested_boundary { self.boundary = true; }
        match tag {
            "img" => { let standalone = self.dom.attr(id, "style").is_some_and(|style| style.split(';').any(|property| property.split_once(':').is_some_and(|(key, value)| key.trim().eq_ignore_ascii_case("display") && value.trim().eq_ignore_ascii_case("block")))); return self.image(id, &style, align, self.pending.is_some() && !standalone); }
            "hr" => { self.flush()?; self.push(EmailBlockKind::Rule)?; return Ok(()); }
            "br" => { self.text("\u{2028}", &style)?; return Ok(()); }
            "table" => { self.flush()?; self.table(id, &style)?; return Ok(()); }
            "ul" | "ol" => { self.flush()?; let ordered = tag == "ol"; let mut number = self.dom.attr(id, "start").and_then(|v| v.parse().ok()).unwrap_or(1); for child in &self.dom.nodes[id].children { self.visit(*child, &style, depth + 1, Some((ordered, number)), if list.is_some() { indent.saturating_add(1) } else { indent }, align)?; if self.dom.tag(*child) == "li" { number = number.saturating_add(1); } } self.flush()?; return Ok(()); }
            _ => {}
        }
        if explicit { self.flush()?; let kind = match tag { "h1" => EmailParagraphKind::Heading(1), "h2" => EmailParagraphKind::Heading(2), "h3" => EmailParagraphKind::Heading(3), "h4" => EmailParagraphKind::Heading(4), "h5" => EmailParagraphKind::Heading(5), "h6" => EmailParagraphKind::Heading(6), "blockquote" => EmailParagraphKind::Quote, "pre" => EmailParagraphKind::Code, "li" if self.dom.attr(id, "data-task") == Some("true") => EmailParagraphKind::Task(self.dom.attr(id, "data-checked") == Some("true")), "li" => match list { Some((true, number)) => EmailParagraphKind::Numbered(self.dom.attr(id, "value").and_then(|v| v.parse().ok()).unwrap_or(number)), _ => EmailParagraphKind::Bullet }, _ => EmailParagraphKind::Normal }; self.pending = Some(EmailParagraph { kind, align, indent: self.dom.attr(id, "data-indent").and_then(|v| v.parse().ok()).unwrap_or(indent).min(32), content: EmailText::default() }); }
        let text = &self.dom.nodes[id].text;
        // Ignore HTML source indentation between blocks, but preserve all authored inline whitespace.
        if !text.chars().all(char::is_whitespace) || self.pending.is_some() { self.text(text, &style)?; }
        for child in &self.dom.nodes[id].children { self.visit(*child, &style, depth + usize::from(!matches!(tag, "html" | "body" | "")), list, indent, align)?; }
        if explicit { self.flush()?; } else if nested_boundary { self.boundary = true; } Ok(())
    }
}
impl EmailDocument {
    /// Parse untrusted HTML5 into the safe editable subset, without fetching resources.
    pub fn from_html(html: &str) -> Result<Self, EmailError> {
        if html.len() > MAX_BYTES { return Err(EmailError::Limit("import bytes")); }
        let mut parser = parse_document(Dom::new(), Default::default());
        let mut start = 0;
        while start < html.len() { let mut end = (start + 4096).min(html.len()); while !html.is_char_boundary(end) { end -= 1; } parser.process(StrTendril::from_slice(&html[start..end])); if parser.tokenizer.sink.sink.too_deep { return Err(EmailError::Limit("HTML depth")); } start = end; }
        let dom = parser.finish();
        if dom.too_deep { return Err(EmailError::Limit("HTML depth")); }
        // Check the complete tree, including discarded subtrees, without recursive stack use.
        let mut stack = vec![(0, 0)]; while let Some((id, depth)) = stack.pop() { let increment = usize::from(!matches!(dom.tag(id), "" | "html" | "head" | "body")); let depth = depth + increment; if depth > 32 { return Err(EmailError::Limit("HTML depth")); } stack.extend(dom.nodes[id].children.iter().map(|child| (*child, depth))); }
        let mut importer = Importer { dom: &dom, blocks: Vec::new(), pending: None, runs: 0, bytes: 0, boundary: false }; importer.visit(0, &EmailStyle::default(), 0, None, 0, EmailAlign::Left)?; importer.flush()?;
        Self::from_blocks(importer.blocks)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn repeated_long_link_runs_obey_metadata_budget() {
        let html = format!("<a href='https://example.com/{}'>{}</a>", "a".repeat(100_000), "<b>x</b>x".repeat(100));
        assert!(EmailDocument::from_html(&html).is_err());
    }
    use super::*;
    #[test]
    fn trailing_text_after_nested_paragraph_keeps_boundary() {
        let document = EmailDocument::from_html("<blockquote><p>A</p>B</blockquote><table><tr><td>A<p>B</p>C</td></tr></table>").unwrap();
        assert_eq!(document.to_plain(), "> A\nB\nA\nB\nC");
    }
    #[test]
    fn nested_quote_and_hidden_css_keep_visible_semantics() {
        let doc = EmailDocument::from_html("<blockquote><p>quote</p></blockquote><p>ok<span style='display:none'>secret</span></p>").unwrap();
        assert_eq!(doc.to_plain(), "> quote\nok");
    }
    #[test]
    fn nested_paragraphs_in_cells_and_quotes_keep_breaks() {
        let doc = EmailDocument::from_html("<blockquote><p>A</p><p>B</p></blockquote><table><tr><td><p>A</p><p>B</p></td></tr></table>").unwrap();
        assert_eq!(doc.to_plain(), "> A\nB\nA\nB");
    }
    #[test]
    fn adjacent_equivalent_styles_coalesce() {
        let doc = EmailDocument::from_html("<p><b>a</b><strong>b</strong><span style='font-weight:700'>c</span></p>").unwrap();
        let text = doc.text_at(doc.first_point()).unwrap();
        assert_eq!(text.text, "abc"); assert_eq!(text.runs.len(), 1);
    }
    #[test]
    fn sanitizer_excludes_hidden_content_and_dangerous_urls() {
        let doc = EmailDocument::from_html("<p onclick='bad()'>A<script>secret</script><style>hidden</style><iframe>gone</iframe><a href='java&#x73;cript:alert(1)'>B</a><img src='data:image/svg+xml;base64,aaaa'>C</p>").unwrap();
        assert_eq!(doc.to_plain(), "ABC");
        let html = doc.to_html();
        assert!(!html.contains("javascript"));
        assert!(!html.contains("onclick"));
        assert!(!html.contains("secret"));
    }
    #[test]
    fn supported_html_roundtrips_without_source_text() {
        let doc = EmailDocument::from_html("<h2 style='text-align:center'><span style='font-family:Arial;font-size:18pt;color:#123456'><strong>中文 &amp; 🦀</strong></span><br>tail</h2><ul><li data-task='true' data-checked='true'>done</li></ul><table><tr><td colspan='2'>wide</td></tr><tr><td>x</td><td>y</td></tr></table><img src='cid:photo@example' alt='photo' width='120'>").unwrap();
        doc.validate().unwrap();
        let roundtrip = EmailDocument::from_html(&doc.to_html()).unwrap();
        assert_eq!(doc.blocks.iter().map(|b| &b.kind).collect::<Vec<_>>(), roundtrip.blocks.iter().map(|b| &b.kind).collect::<Vec<_>>());
        assert!(!doc.to_plain().contains("<strong>"));
    }
    #[test]
    fn import_limits_fail_without_mutating_existing_content() {
        let doc = EmailDocument::from_plain("keep").unwrap();
        assert!(EmailDocument::from_html(&"x".repeat(2 * 1024 * 1024 + 1)).is_err());
        let deep = format!("{}x{}", "<div>".repeat(33), "</div>".repeat(33));
        assert!(EmailDocument::from_html(&deep).is_err());
        assert_eq!(doc.to_plain(), "keep");
    }
}
