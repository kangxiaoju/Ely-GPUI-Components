//! Direct HTML and plain-text serialization of structured email content.
use super::model::*;
use std::fmt::Write;

pub(crate) fn escape(value: &str) -> String { value.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&#39;") }
fn align(value: EmailAlign) -> &'static str { match value { EmailAlign::Left => "left", EmailAlign::Center => "center", EmailAlign::Right => "right", EmailAlign::Justify => "justify" } }
fn inline_image(image: &EmailImage, out: &mut String) { if !safe_url(&image.source, true) { return; } let _ = write!(out, "<img src=\"{}\" alt=\"{}\" data-email-align=\"{}\"", escape(&image.source), escape(&image.alt), align(image.align)); if let Some(width) = image.width { let _ = write!(out, " width=\"{width}\""); } if let Some(height) = image.height { let _ = write!(out, " height=\"{height}\""); } out.push('>'); }
fn plain_text(text: &EmailText) -> String { let mut output = String::new(); let mut start = 0; for (offset, image) in &text.images { if let Some(value) = text.text.get(start..*offset) { output.push_str(value); } output.push_str(&image.alt); start = offset.saturating_add(3); } if let Some(value) = text.text.get(start..) { output.push_str(value); } output.replace('\u{2028}', "\n") }
fn rich(text: &EmailText, out: &mut String) {
    for run in &text.runs {
        if text.text.get(run.range.clone()).is_none() { continue; } let style = &run.style;
        let link = style.link.as_ref().filter(|url| safe_url(url, false));
        if let Some(link) = link { let _ = write!(out, "<a href=\"{}\">", escape(link)); }
        let mut css = String::new();
        if let Some(font) = style.font.as_ref().filter(|font| safe_font(font)) { let _ = write!(css, "font-family:{};", font); }
        if let Some(size) = style.size.filter(|s| (8..=72).contains(s)) { let _ = write!(css, "font-size:{size}pt;"); }
        if let Some(c) = style.color { let _ = write!(css, "color:#{:02x}{:02x}{:02x};", c.red, c.green, c.blue); }
        if !css.is_empty() { let _ = write!(out, "<span style=\"{}\">", escape(&css)); }
        let tags = [(style.bold, "strong"), (style.italic, "em"), (style.underline, "u"), (style.strike, "s"), (style.code, "code")];
        for (active, tag) in tags { if active { let _ = write!(out, "<{tag}>"); } }
        let mut start = run.range.start;
        for (offset, image) in text.images.range(run.range.clone()) { if let Some(value) = text.text.get(start..*offset) { out.push_str(&escape(value).replace('\u{2028}', "<br>")); } inline_image(image, out); start = offset.saturating_add(3); }
        if let Some(value) = text.text.get(start..run.range.end) { out.push_str(&escape(value).replace('\u{2028}', "<br>")); }
        for (active, tag) in tags.into_iter().rev() { if active { let _ = write!(out, "</{tag}>"); } }
        if !css.is_empty() { out.push_str("</span>"); } if link.is_some() { out.push_str("</a>"); }
    }
}
impl EmailDocument {
    /// Serialize the editable subset directly to escaped, inert HTML.
    pub fn to_html(&self) -> String {
        let mut out = String::new();
        for block in &self.blocks { match &block.kind {
            EmailBlockKind::Paragraph(p) => {
                let (tag, wrapper) = match p.kind { EmailParagraphKind::Normal => ("p".to_string(), ""), EmailParagraphKind::Heading(n) => (format!("h{}", n.clamp(1, 6)), ""), EmailParagraphKind::Quote => ("blockquote".into(), ""), EmailParagraphKind::Bullet | EmailParagraphKind::Task(_) => ("li".into(), "ul"), EmailParagraphKind::Numbered(n) => { let _ = write!(out, "<ol start=\"{n}\">"); ("li".into(), "ol-open") }, EmailParagraphKind::Code => ("pre".into(), "") };
                if wrapper == "ul" { out.push_str("<ul>"); }
                let _ = write!(out, "<{tag} style=\"white-space:pre-wrap;text-align:{};margin-left:{}px\" data-indent=\"{}\"", align(p.align), u32::from(p.indent) * 24, p.indent);
                if let EmailParagraphKind::Task(checked) = p.kind { let _ = write!(out, " data-task=\"true\" data-checked=\"{checked}\""); }
                out.push('>');
                if let EmailParagraphKind::Task(checked) = p.kind { let _ = write!(out, "<span data-email-task-marker=\"true\">{}</span>", if checked { "[x] " } else { "[ ] " }); }
                if p.kind == EmailParagraphKind::Code { out.push_str("<span>"); }
                rich(&p.content, &mut out);
                if p.kind == EmailParagraphKind::Code { out.push_str("</span>"); }
                let _ = write!(out, "</{tag}>");
                if wrapper == "ul" { out.push_str("</ul>"); } if wrapper == "ol-open" { out.push_str("</ol>"); }
            }
            EmailBlockKind::Image(i) => { if safe_url(&i.source, true) { let _ = write!(out, "<div data-email-image=\"true\" style=\"text-align:{}\"><img src=\"{}\" alt=\"{}\"", align(i.align), escape(&i.source), escape(&i.alt)); if let Some(w) = i.width { let _ = write!(out, " width=\"{w}\""); } if let Some(h) = i.height { let _ = write!(out, " height=\"{h}\""); } out.push_str("></div>"); } }
            EmailBlockKind::Table(t) => { if t.columns == 0 { continue; } out.push_str("<table border=\"1\" cellpadding=\"4\" cellspacing=\"0\" style=\"border-collapse:collapse\">"); for (row, spans) in t.cells.chunks(t.columns).zip(t.spans.chunks(t.columns)) { out.push_str("<tr>"); for (text, span) in row.iter().zip(spans) { if *span == 0 { continue; } let _ = write!(out, "<td colspan=\"{span}\" style=\"white-space:pre-wrap\">"); rich(text, &mut out); out.push_str("</td>"); } out.push_str("</tr>"); } out.push_str("</table>"); }
            EmailBlockKind::Rule => out.push_str("<hr>"),
        } }
        out
    }
    /// Serialize semantic text, preserving literal whitespace and table/list structure.
    pub fn to_plain(&self) -> String {
        let mut out = Vec::with_capacity(self.blocks.len());
        for block in &self.blocks { out.push(match &block.kind {
            EmailBlockKind::Paragraph(p) => { let prefix = match p.kind { EmailParagraphKind::Bullet => "• ".into(), EmailParagraphKind::Numbered(n) => format!("{n}. "), EmailParagraphKind::Task(v) => if v { "[x] " } else { "[ ] " }.into(), EmailParagraphKind::Quote => "> ".into(), _ => String::new() }; format!("{}{}{}", "  ".repeat(usize::from(p.indent)), prefix, plain_text(&p.content)) }
            EmailBlockKind::Image(i) => i.alt.clone(),
            EmailBlockKind::Table(t) => if t.columns == 0 { String::new() } else { t.cells.chunks(t.columns).map(|row| row.iter().map(plain_text).collect::<Vec<_>>().join("\t")).collect::<Vec<_>>().join("\n") },
            EmailBlockKind::Rule => "---".into(),
        }); }
        out.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn task_status_is_visible_and_roundtrips() {
        let doc = EmailDocument::from_html("<ul><li data-task='true' data-checked='true'>done</li><li data-task='true' data-checked='false'>next</li></ul>").unwrap();
        let html = doc.to_html(); assert!(html.contains("[x]")); assert!(html.contains("[ ]"));
        assert_eq!(EmailDocument::from_html(&html).unwrap().to_plain(), "[x] done\n[ ] next");
    }
    #[test]
    fn pre_leading_newline_is_not_lost_on_roundtrip() {
        let doc = EmailDocument::from_html("<pre>\n\nX</pre>").unwrap();
        assert_eq!(doc.to_plain(), "\nX");
        assert_eq!(EmailDocument::from_html(&doc.to_html()).unwrap().to_plain(), doc.to_plain());
    }
    #[test]
    fn centered_image_uses_a_real_alignment_container() {
        let doc = EmailDocument::from_blocks(vec![EmailBlock { id: 1, kind: EmailBlockKind::Image(EmailImage { source: "cid:x".into(), alt: "x".into(), width: None, height: None, align: EmailAlign::Center }) }]).unwrap();
        assert!(doc.to_html().starts_with("<div"));
        assert_eq!(EmailDocument::from_html(&doc.to_html()).unwrap().blocks, doc.blocks);
    }
}
