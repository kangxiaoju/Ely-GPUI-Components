use std::{collections::{HashMap, HashSet}, ops::Range, sync::Arc};
use gpui::{font, point, px, size, Bounds, FontStyle, FontWeight, Hsla, Pixels, Point, ShapedLine, TextRun, Window};
use unicode_bidi::BidiInfo;
use unicode_segmentation::UnicodeSegmentation;
use super::model::*;

#[cfg(test)]
thread_local! { static RUN_VISITS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; static SHAPED_BYTES:std::cell::Cell<usize>=const{std::cell::Cell::new(0)}; }
#[cfg(test)]
pub(crate) fn run_visits() -> usize { RUN_VISITS.with(std::cell::Cell::get) }
#[cfg(test)]
pub(crate) fn shaped_bytes()->usize{SHAPED_BYTES.with(std::cell::Cell::get)}
#[derive(Clone)]
pub(crate) struct Fragment { pub shape: ShapedLine, pub x: Pixels, pub start: usize, pub end: usize, pub rtl: bool, pub prefix: usize, pub styles: Vec<(Range<usize>, EmailStyle)>, pub glyph_shifts: HashMap<usize, Pixels>, pub cluster_range: Range<usize>, pub object: Option<EmailImage> }
impl Fragment { pub(crate) fn glyph_shift(&self, index: usize) -> Pixels { self.glyph_shifts.get(&index).copied().unwrap_or(px(0.)) } }
#[derive(Clone)]
pub(crate) struct Cluster { pub range: Range<usize>, pub x: Pixels, pub width: Pixels, pub rtl: bool }
#[derive(Clone)]
pub(crate) struct VisualLine { pub block: u64, pub cell: usize, pub origin: Point<Pixels>, pub height: Pixels, pub ascent: Pixels, pub width: Pixels, pub fragments: Arc<Vec<Fragment>>, pub stops: Arc<Vec<(usize, Pixels)>>, pub clusters: Arc<Vec<Cluster>>, pub marker: Option<ShapedLine> }
#[derive(Clone)]
pub(crate) struct ImageBox { pub image: EmailImage, pub bounds: Bounds<Pixels>, pub block: u64, pub cell: usize, pub offset: usize, pub end: usize }
struct CachedBlock { epoch:u64, lines: Vec<VisualLine>, images: Vec<ImageBox>, borders: Vec<Bounds<Pixels>>, height: Pixels, shapes: usize }
#[derive(Default)]
pub(crate) struct EmailLayout { pub lines: Vec<VisualLine>, pub images: Vec<ImageBox>, pub borders: Vec<Bounds<Pixels>>, pub height: Pixels, pub origin: Point<Pixels>, pub shape_count: usize, cache: HashMap<u64, Arc<CachedBlock>>, revision: Option<u64>, width: Pixels, appearance: Option<(Hsla, gpui::SharedString, Pixels)>, order:Vec<u64>, indices:HashMap<u64,usize>, heights:super::prefix::PrefixIndex, viewport:Option<(Pixels,Pixels)> }
impl EmailLayout {
    pub(crate) fn arrange(&mut self,document:&EmailDocument,width:Pixels,window:&mut Window){
        let style=window.text_style();let appearance=(style.color,style.font_family.clone(),style.font_size.to_pixels(window.rem_size()));
        let changes=self.revision.and_then(|revision|document.changes_since(revision));
        let rebuild=self.appearance.as_ref()!=Some(&appearance)||self.width!=width||changes.as_ref().is_none_or(|changes|changes.iter().any(|change|change.removed!=change.inserted));
        if !rebuild&&self.revision==Some(document.revision){return;}
        if rebuild{
            let reuse=self.appearance.as_ref()==Some(&appearance)&&self.width==width;
            let mut previous=if reuse{std::mem::take(&mut self.cache)}else{HashMap::new()};self.cache.clear();self.order.clear();self.indices.clear();
            for (index,block) in document.blocks.iter().enumerate(){let epoch=document.block_revision(block.id).unwrap_or(0);let cached=if let Some(cached)=previous.remove(&block.id).filter(|cached|cached.epoch==epoch){cached}else{let mut cached=layout_block(block,width,window);cached.epoch=epoch;self.shape_count+=cached.shapes;Arc::new(cached)};self.order.push(block.id);self.indices.insert(block.id,index);self.cache.insert(block.id,cached);}
            self.heights.rebuild(self.order.iter().map(|id|f64::from(f32::from(self.cache[id].height+px(8.)))));
        }else{
            let mut touched=HashSet::new();for change in changes.unwrap_or_default(){touched.extend(change.start..change.start+change.inserted);}
            for index in touched{if let Some(block)=document.block_at(index){self.cache.remove(&self.order[index]);self.indices.remove(&self.order[index]);let mut cached=layout_block(block,width,window);cached.epoch=document.block_revision(block.id).unwrap_or(0);self.shape_count+=cached.shapes;self.heights.set(index,f64::from(f32::from(cached.height+px(8.))));self.order[index]=block.id;self.indices.insert(block.id,index);self.cache.insert(block.id,Arc::new(cached));}}
        }
        self.height=px(self.heights.total() as f32+8.).max(px(36.));self.width=width;self.appearance=Some(appearance);self.revision=Some(document.revision);self.refresh_visible();
    }
    pub(crate) fn set_viewport(&mut self,top:Pixels,bottom:Pixels){let viewport=Some((top,bottom));if self.viewport!=viewport{self.viewport=viewport;self.refresh_visible();}}
    fn refresh_visible(&mut self){self.lines.clear();self.images.clear();self.borders.clear();if self.order.is_empty(){return;}let (first,last)=self.viewport.map(|(top,bottom)|(self.heights.locate(f64::from(f32::from(top.max(px(0.))))),self.heights.locate(f64::from(f32::from(bottom.max(px(0.))))))).unwrap_or((0,self.order.len()-1));for index in first..=last{let cached=&self.cache[&self.order[index]];let y=px(self.heights.sum(index) as f32+8.);for line in &cached.lines{let mut line=line.clone();line.origin.y+=y;self.lines.push(line);}for image in &cached.images{let mut image=image.clone();image.bounds.origin.y+=y;self.images.push(image);}for border in &cached.borders{let mut border=*border;border.origin.y+=y;self.borders.push(border);}}}
    // Search only the cached block at the target height and its neighbors. The
    // painted line list deliberately contains no offscreen blocks.
    pub(crate) fn vertical_neighbor(&self, point: EmailPoint, affinity: Option<usize>, down: bool) -> Option<(EmailPoint, Option<usize>)> {
        let caret = self.caret_with_affinity(point, affinity)?;
        let local = caret.origin - self.origin;
        let target_y = if down { local.y + caret.size.height + px(1.) } else { local.y - px(1.) };
        let index = self.heights.locate(f64::from(f32::from(target_y.max(px(0.)))));
        let mut best: Option<(&VisualLine, Pixels, f32)> = None;
        for index in index.saturating_sub(1)..=(index + 1).min(self.order.len().checked_sub(1)?) {
            let y = px(self.heights.sum(index) as f32 + 8.);
            for line in &self.cache[&self.order[index]].lines {
                let top = line.origin.y + y;
                if if down { top <= local.y } else { top >= local.y } { continue; }
                let dy = if target_y < top { top - target_y } else if target_y > top + line.height { target_y - top - line.height } else { px(0.) };
                let dx = if local.x < line.origin.x { line.origin.x - local.x } else if local.x > line.origin.x + line.width { local.x - line.origin.x - line.width } else { px(0.) };
                let distance = f32::from(dy) * 10000. + f32::from(dx);
                if best.as_ref().is_none_or(|(_, _, previous)| distance < *previous) { best = Some((line, top, distance)); }
            }
        }
        let (line, top, _) = best?;
        let (offset, _) = line.stops.iter().min_by(|a, b| f32::from((line.origin.x + a.1 - local.x).abs()).total_cmp(&f32::from((line.origin.x + b.1 - local.x).abs())))?;
        let affinity = self.lines.iter().position(|visible| visible.block == line.block && visible.cell == line.cell && visible.origin.y == top);
        Some((EmailPoint { block: line.block, cell: line.cell, offset: *offset }, affinity))
    }
    pub(crate) fn hit(&self, position: Point<Pixels>) -> Option<EmailPoint> { self.hit_with_affinity(position).map(|(point,_)| point) }
    pub(crate) fn hit_with_affinity(&self, position: Point<Pixels>) -> Option<(EmailPoint, Option<usize>)> {
        let local = position - self.origin;
        for image in &self.images { if image.bounds.contains(&local) { let owning=self.lines.iter().enumerate().find_map(|(index,line)| if line.block==image.block && line.cell==image.cell { line.clusters.iter().find(|cluster|cluster.range.start==image.offset && cluster.range.end==image.end).map(|cluster|(index,cluster.rtl)) } else {None}); let right=local.x>image.bounds.origin.x+image.bounds.size.width/2.;let rtl=owning.is_some_and(|(_,rtl)|rtl); return Some((EmailPoint{block:image.block,cell:image.cell,offset:if right!=rtl{image.end}else{image.offset}},owning.map(|(index,_)|index))); } }
        let (index, line) = self.lines.iter().enumerate().min_by(|(_,a),(_,b)| { let distance = |line: &VisualLine| { let dy = if local.y < line.origin.y { line.origin.y - local.y } else if local.y > line.origin.y + line.height { local.y - line.origin.y - line.height } else { px(0.) }; let dx = if local.x < line.origin.x { line.origin.x - local.x } else if local.x > line.origin.x + line.width { local.x - line.origin.x - line.width } else { px(0.) }; f32::from(dy) * 10000. + f32::from(dx) }; distance(a).total_cmp(&distance(b)) })?;
        let (offset, _) = line.stops.iter().min_by(|a,b| f32::from((line.origin.x + a.1 - local.x).abs()).total_cmp(&f32::from((line.origin.x + b.1 - local.x).abs())))?;
        Some((EmailPoint { block: line.block, cell: line.cell, offset: *offset }, Some(index)))
    }
    #[cfg(all(test, feature = "test-support"))]
    pub(crate) fn caret(&self, point: EmailPoint) -> Option<Bounds<Pixels>> {
        self.caret_with_affinity(point, None)
    }
    pub(crate) fn caret_with_affinity(&self, point: EmailPoint, affinity: Option<usize>) -> Option<Bounds<Pixels>> {
        if let Some(line) = affinity.and_then(|index| self.lines.get(index)) { if line.block == point.block && line.cell == point.cell { if let Some((_,x)) = line.stops.iter().find(|(offset,_)| *offset == point.offset) { return Some(Bounds::new(self.origin + line.origin + gpui::point(*x,px(0.)), size(px(1.5),line.height))); } } }
        for line in &self.lines { if line.block == point.block && line.cell == point.cell { if let Some((_, x)) = line.stops.iter().find(|(offset, _)| *offset == point.offset) { return Some(Bounds::new(self.origin + line.origin + gpui::point(*x, px(0.)), size(px(1.5), line.height))); } } }
        if let Some(index)=self.indices.get(&point.block).copied(){let cached=&self.cache[&point.block];let y=px(self.heights.sum(index) as f32+8.);for line in &cached.lines{if line.cell==point.cell{if let Some((_,x))=line.stops.iter().find(|(offset,_)|*offset==point.offset){return Some(Bounds::new(self.origin+line.origin+gpui::point(*x,y),size(px(1.5),line.height)));}}}}
        self.images.iter().find(|i| i.block == point.block).map(|i| Bounds::new(self.origin + i.bounds.origin + gpui::point(if point.offset == 0 { px(0.) } else { i.bounds.size.width }, px(0.)), size(px(1.5), i.bounds.size.height)))
    }
}
pub(crate) fn run(text: &str, style: &EmailStyle, window: &Window) -> TextRun {
    let inherited = window.text_style();
    let mut face = font(style.font.clone().map(gpui::SharedString::from).unwrap_or_else(|| if style.code { "monospace".into() } else { inherited.font_family.clone() }));
    face.weight = if style.bold { FontWeight::BOLD } else { FontWeight::NORMAL }; face.style = if style.italic { FontStyle::Italic } else { FontStyle::Normal };
    let color: Hsla = style.color.map(|c| gpui::rgb((u32::from(c.red) << 16) | (u32::from(c.green) << 8) | u32::from(c.blue)).into()).unwrap_or(inherited.color);
    TextRun { len: text.len(), font: face, color, background_color: None, underline: (style.underline || style.link.is_some()).then_some(gpui::UnderlineStyle { thickness: px(1.), color: Some(color), wavy: false }), strikethrough: style.strike.then_some(gpui::StrikethroughStyle { thickness: px(1.), color: Some(color) }) }
}
fn shape_fragment(text: &EmailText, range: Range<usize>, rtl: bool, window: &mut Window) -> Fragment {
    let first = text.style_at(range.start);
    let source = &text.text[range.clone()];
    // Explicit embedding preserves the resolved directional run even when a size
    // boundary leaves a neutral-only fragment. Controls have no visual advance.
    let prefix = if rtl { '\u{202b}'.len_utf8() } else { 0 };
    let shaped_text = if rtl { format!("\u{202b}{source}\u{202c}") } else { source.to_owned() };
    let mut runs = Vec::new(); let mut styles = Vec::new();
    if prefix > 0 { runs.push(run("\u{202b}", &first, window)); }
    let first_run = text.runs.partition_point(|styled| styled.range.end <= range.start);
    let last_run = text.runs.partition_point(|styled| styled.range.start < range.end);
    #[cfg(test)] RUN_VISITS.with(|visits| visits.set(visits.get() + last_run - first_run));
    for styled in &text.runs[first_run..last_run] { let start = styled.range.start.max(range.start); let end = styled.range.end.min(range.end); if start < end { runs.push(run(&text.text[start..end], &styled.style, window)); styles.push((start..end, styled.style.clone())); } }
    if source.is_empty() { runs.push(run("", &first, window)); }
    if prefix > 0 { runs.push(run("\u{202c}", &first, window)); }
    let font_size = first.size.map(|size| px(f32::from(size) * 96. / 72.)).unwrap_or_else(|| window.text_style().font_size.to_pixels(window.rem_size()));
    #[cfg(test)] SHAPED_BYTES.with(|bytes|bytes.set(bytes.get()+shaped_text.len()));
    let shape = window.text_system().shape_line(shaped_text.into(), font_size, &runs, None);
    Fragment { shape, x: px(0.), start: range.start, end: range.end, rtl, prefix, styles, glyph_shifts: HashMap::new(), cluster_range: 0..0, object: None }
}
fn same_metrics(a: &EmailStyle, b: &EmailStyle) -> bool { a.font == b.font && a.size == b.size && a.bold == b.bold && a.italic == b.italic && a.code == b.code }
pub(crate) fn map_clusters(text:&str,range:Range<usize>,x:Pixels,width:Pixels,rtl:bool,mut glyphs:Vec<(usize,Pixels)>)->Vec<Cluster>{
    glyphs.sort_by(|a,b|f32::from(a.1).total_cmp(&f32::from(b.1)));glyphs.dedup_by(|a,b|a.0==b.0);
    let mut logical:Vec<usize>=glyphs.iter().map(|(index,_)|*index).collect();logical.push(range.end-range.start);logical.sort_unstable();logical.dedup();let mut clusters=Vec::new();
    for(index,(offset,left))in glyphs.iter().enumerate(){let right=glyphs.get(index+1).map(|(_,x)|*x).unwrap_or(width);let end=logical.get(logical.partition_point(|boundary|boundary<=offset)).copied().unwrap_or(range.end-range.start);let graphemes:Vec<_>=text[range.start+offset..range.start+end].grapheme_indices(true).collect();let count=graphemes.len().max(1);for(part,(inner,grapheme))in graphemes.into_iter().enumerate(){let cluster_left=*left+(right-*left)*if rtl{(count-part-1)as f32/count as f32}else{part as f32/count as f32};let cluster_width=(right-*left)/count as f32;let start=range.start+offset+inner;clusters.push(Cluster{range:start..start+grapheme.len(),x:x+cluster_left,width:cluster_width,rtl});}}
    clusters
}
fn visual_line(block: u64, cell: usize, text: &EmailText, range: Range<usize>, bidi: &BidiInfo<'_>, window: &mut Window, shapes: &mut usize) -> VisualLine {
    let mut fragments = Vec::new(); let mut x = px(0.); let mut ascent = px(14.); let mut descent = px(4.);
    let directional_runs = if range.is_empty() { Vec::new() } else { bidi.paragraphs.iter().find(|paragraph| paragraph.range.start <= range.start && paragraph.range.end >= range.end).map(|paragraph| bidi.visual_runs(paragraph, range.clone()).1).unwrap_or_default() };
    for directional in directional_runs {
        let rtl = bidi.levels.get(directional.start).is_some_and(|level| level.is_rtl());
        let mut parts: Vec<Range<usize>> = Vec::new();
        let first_run = text.runs.partition_point(|run| run.range.end <= directional.start);
        let last_run = text.runs.partition_point(|run| run.range.start < directional.end);
        #[cfg(test)] RUN_VISITS.with(|visits| visits.set(visits.get() + last_run - first_run));
        for run in &text.runs[first_run..last_run] { let start = run.range.start.max(directional.start); let end = run.range.end.min(directional.end); if start < end { if let Some(last) = parts.last_mut() { let previous = text.runs.partition_point(|run| run.range.end <= last.start); if last.end == start && same_metrics(&text.runs[previous].style, &run.style) { last.end = end; continue; } } parts.push(start..end); } }
        let mut object_parts=Vec::new();
        for part in parts { let mut start=part.start; for (offset,_) in text.images.range(part.clone()) { if start<*offset { object_parts.push(start..*offset); } object_parts.push(*offset..*offset+'\u{fffc}'.len_utf8()); start=*offset+'\u{fffc}'.len_utf8(); } if start<part.end { object_parts.push(start..part.end); } }
        let mut parts=object_parts;
        if rtl { parts.reverse(); }
        for part in parts { let object=text.images.get(&part.start).cloned(); let mut fragment = if object.is_some() { shape_fragment(text,part.start..part.start,rtl,window) } else { shape_fragment(text,part.clone(),rtl,window) }; *shapes += 1; fragment.end=part.end; fragment.object=object; fragment.x=x; if let Some(image)=&fragment.object { let w=px(image.width.unwrap_or(160) as f32); let h=px(image.height.unwrap_or(100) as f32); ascent=ascent.max(h);x+=w; } else { ascent=ascent.max(fragment.shape.ascent); descent=descent.max(fragment.shape.descent);x+=fragment.shape.width; } fragments.push(fragment); }
    }
    if fragments.is_empty() { let fragment = shape_fragment(text, range.clone(), false, window); *shapes += 1; ascent = ascent.max(fragment.shape.ascent); descent = descent.max(fragment.shape.descent); fragments.push(fragment); }
    let mut clusters = Vec::new(); let mut stops = Vec::new();
    for fragment in &mut fragments {
        let cluster_start = clusters.len();
        if let Some(image)=&fragment.object { let width=px(image.width.unwrap_or(160) as f32); clusters.push(Cluster{range:fragment.start..fragment.end,x:fragment.x,width,rtl:fragment.rtl}); stops.push((fragment.start,fragment.x+if fragment.rtl{width}else{px(0.)})); stops.push((fragment.end,fragment.x+if fragment.rtl{px(0.)}else{width})); fragment.cluster_range=cluster_start..clusters.len(); continue; }
        let source_len = fragment.end - fragment.start;
        let glyphs: Vec<(usize, Pixels)> = fragment.shape.runs.iter().flat_map(|run| run.glyphs.iter()).filter(|glyph| glyph.index >= fragment.prefix && glyph.index < fragment.prefix + source_len).map(|glyph| (glyph.index - fragment.prefix, glyph.position.x)).collect();
        let mapped=map_clusters(&text.text,fragment.start..fragment.end,fragment.x,fragment.shape.width,fragment.rtl,glyphs);
        for cluster in mapped {stops.push((cluster.range.start,cluster.x+if cluster.rtl{cluster.width}else{px(0.)}));stops.push((cluster.range.end,cluster.x+if cluster.rtl{px(0.)}else{cluster.width}));clusters.push(cluster);}
        if source_len == 0 { stops.push((fragment.start, fragment.x)); }
        fragment.cluster_range = cluster_start..clusters.len();
    }
    // At bidi boundaries a byte offset can have two affinities. Prefer the
    // preceding logical cluster's trailing edge for the canonical caret map.
    let mut canonical=std::collections::BTreeMap::new();
    for cluster in &clusters { canonical.entry(cluster.range.start).or_insert(cluster.x+if cluster.rtl{cluster.width}else{px(0.)}); }
    for cluster in &clusters { canonical.insert(cluster.range.end,cluster.x+if cluster.rtl{px(0.)}else{cluster.width}); }
    for cluster in clusters.iter().filter(|cluster|cluster.rtl){canonical.insert(cluster.range.start,cluster.x+cluster.width);canonical.insert(cluster.range.end,cluster.x);}
    for (offset,x) in stops { canonical.entry(offset).or_insert(x); }
    let stops=canonical.into_iter().collect();
    VisualLine { block, cell, origin: point(px(8.), px(0.)), height: ascent + descent + px(5.), ascent, width: x, fragments: Arc::new(fragments), stops: Arc::new(stops), clusters: Arc::new(clusters), marker: None }
}
fn text_lines(block: u64, cell: usize, text: &EmailText, width: Pixels, window: &mut Window, shapes: &mut usize) -> Vec<VisualLine> {
    let bidi = BidiInfo::new(&text.text, None);
    // One full-paragraph cluster measurement supplies an O(n) logical advance
    // prefix. Final line slices are reshaped once to preserve contextual forms.
    let measured = visual_line(block, cell, text, 0..text.text.len(), &bidi, window, shapes);
    let mut advances: HashMap<usize, Pixels> = HashMap::new(); for cluster in measured.clusters.iter() { *advances.entry(cluster.range.end).or_default() += cluster.width; }
    let graphemes: Vec<_> = text.text.grapheme_indices(true).map(|(start, g)| (start, start + g.len(), g == "\u{2028}")).collect();
    let breaks: HashSet<_> = unicode_linebreak::linebreaks(&text.text).map(|(offset,_)| offset).collect();
    let mut ranges = Vec::new(); let mut start = 0; let mut width_so_far = px(0.); let mut opportunity = 0; let mut opportunity_width = px(0.);
    for (left, end, hard) in graphemes {
        if hard { ranges.push(start..left); start = end; width_so_far = px(0.); opportunity = end; opportunity_width = px(0.); continue; }
        let advance = advances.get(&end).copied().unwrap_or(px(0.));
        if width_so_far + advance > width && left > start { let split = if opportunity > start { opportunity } else { left }; ranges.push(start..split); start = split; width_so_far = if split == opportunity { width_so_far - opportunity_width } else { px(0.) }; opportunity=start; opportunity_width=px(0.); }
        width_so_far += advance;
        if breaks.contains(&end) { opportunity = end; opportunity_width = width_so_far; }
    }
    if start < text.text.len() || ranges.is_empty() || text.text.ends_with('\u{2028}') { ranges.push(start..text.text.len()); }
    let mut lines = Vec::new(); let mut y = px(0.);
    for range in ranges {
        let mut pending = range;
        loop {
            let mut line = visual_line(block, cell, text, pending.clone(), &bidi, window, shapes);
            if line.width > width && pending.start < pending.end {
                let boundaries: Vec<_> = text.text[pending.clone()].grapheme_indices(true).map(|(offset,_)| pending.start + offset).filter(|offset| *offset > pending.start).collect();
                if !boundaries.is_empty() {
                    // Binary-search a fitting final slice, rather than repeatedly
                    // shaping shrinking prefixes of a long overflowing line.
                    let mut low = 0; let mut high = boundaries.len();
                    while low + 1 < high { let middle = (low + high) / 2; let candidate = visual_line(block, cell, text, pending.start..boundaries[middle], &bidi, window, shapes); if candidate.width <= width { low = middle; } else { high = middle; } }
                    let split = boundaries[low];
                    line = visual_line(block, cell, text, pending.start..split, &bidi, window, shapes);
                    line.origin.y = y; y += line.height; lines.push(line); pending.start = split; continue;
                }
            }
            line.origin.y = y; y += line.height; lines.push(line); break;
        }
    }
    lines
}
fn layout_block(block: &EmailBlock, width: Pixels, window: &mut Window) -> CachedBlock {
    let mut cache = CachedBlock { epoch:0, lines: Vec::new(), images: Vec::new(), borders: Vec::new(), height: px(0.), shapes: 0 };
    match &block.kind {
        EmailBlockKind::Paragraph(p) => {
            let mut content = p.content.clone();
            for run in &mut content.runs { match p.kind { EmailParagraphKind::Heading(level) => { if run.style.size.is_none() { run.style.size = Some(match level { 1 => 28, 2 => 24, 3 => 20, 4 => 18, 5 => 16, _ => 14 }); } run.style.bold = true; }, EmailParagraphKind::Code => run.style.code = true, _ => {} } }
            let marker = match p.kind { EmailParagraphKind::Bullet => Some("•".to_owned()), EmailParagraphKind::Numbered(number) => Some(format!("{number}.")), EmailParagraphKind::Task(checked) => Some(if checked { "☑" } else { "☐" }.to_owned()), _ => None };
            let indent = px(f32::from(p.indent) * 24. + if marker.is_some() || matches!(p.kind, EmailParagraphKind::Quote) { 28. } else { 0. });
            let available = (width - indent - px(16.)).max(px(1.));
            cache.lines = text_lines(block.id, 0, &content, available, window, &mut cache.shapes);
            let line_count = cache.lines.len();
            for (index, line) in cache.lines.iter_mut().enumerate() {
                line.origin.x += indent + match p.align { EmailAlign::Center => (available - line.width) / 2., EmailAlign::Right => available - line.width, _ => px(0.) };
                if p.align == EmailAlign::Justify && index + 1 < line_count { justify(line, &content.text, available); }
            }
            if let (Some(marker), Some(first)) = (marker, cache.lines.first_mut()) { let marker_text = EmailText::plain(&marker); first.marker = Some(shape_fragment(&marker_text, 0..marker.len(), false, window).shape); cache.shapes += 1; }
            cache.height = cache.lines.iter().map(|line| line.origin.y + line.height).fold(px(0.), Pixels::max);
            if matches!(p.kind, EmailParagraphKind::Quote) { cache.borders.push(Bounds::new(point(px(12.) + px(f32::from(p.indent) * 24.), px(0.)), size(px(3.), cache.height))); }
        },
        EmailBlockKind::Table(table) => {
            let columns = table.columns.max(1); let cell_width = (width - px(16.)) / columns as f32; let mut y = px(0.);
            for row in 0..table.cells.len().div_ceil(columns) {
                let mut cells = Vec::new(); let mut row_height = px(28.);
                for column in 0..columns { let cell = row * columns + column; let Some(text) = table.cells.get(cell) else { break; }; let span = table.spans.get(cell).copied().unwrap_or(1); if span == 0 { continue; } let lines = text_lines(block.id, cell, text, (cell_width * span as f32 - px(12.)).max(px(1.)), window, &mut cache.shapes); row_height = row_height.max(lines.iter().map(|l| l.origin.y + l.height + px(8.)).fold(px(0.), Pixels::max)); cells.push((column, span, lines)); }
                for (column, span, mut lines) in cells { for line in &mut lines { line.origin += point(cell_width * column as f32, y + px(4.)); } cache.lines.extend(lines); cache.borders.push(Bounds::new(point(px(8.) + cell_width * column as f32, y), size(cell_width * span as f32, row_height))); }
                y += row_height;
            }
            cache.height = y;
        },
        EmailBlockKind::Image(image) => { let original_w = px(image.width.unwrap_or(320) as f32); let w = original_w.min((width - px(16.)).max(px(1.))); let h = px(image.height.unwrap_or(180) as f32) * (w / original_w.max(px(1.))); let x = match image.align { EmailAlign::Center => (width - w) / 2., EmailAlign::Right => width - w - px(8.), _ => px(8.) }; cache.images.push(ImageBox { image: image.clone(), bounds: Bounds::new(point(x, px(0.)), size(w, h)), block: block.id, cell:0, offset:0, end:1 }); cache.height = h; },
        EmailBlockKind::Rule => { cache.borders.push(Bounds::new(point(px(8.), px(8.)), size(width - px(16.), px(1.)))); let text = EmailText::plain(""); let bidi = BidiInfo::new("", None); let mut line = visual_line(block.id, 0, &text, 0..0, &bidi, window, &mut cache.shapes); line.height = px(18.); line.width = width - px(16.); line.stops = Arc::new(vec![(0,px(0.)),(1,line.width)]); cache.lines.push(line); cache.height = px(18.); }
    }
    for line in &cache.lines { for fragment in line.fragments.iter() { if let Some(image)=&fragment.object { let width=px(image.width.unwrap_or(160) as f32);let height=px(image.height.unwrap_or(100) as f32);let x=line.clusters.get(fragment.cluster_range.start).map(|cluster|cluster.x).unwrap_or(fragment.x);cache.images.push(ImageBox{image:image.clone(),bounds:Bounds::new(line.origin+point(x,line.ascent-height),size(width,height)),block:line.block,cell:line.cell,offset:fragment.start,end:fragment.end}); } } }
    cache
}
fn justify(line: &mut VisualLine, text: &str, width: Pixels) {
    let mut spaces: Vec<Pixels> = line.clusters.iter().filter(|cluster| text[cluster.range.clone()].chars().all(char::is_whitespace)).map(|cluster| cluster.x + cluster.width).collect();
    spaces.sort_by(|a,b| f32::from(*a).total_cmp(&f32::from(*b))); if spaces.is_empty() || line.width >= width { return; }
    let extra = (width - line.width) / spaces.len() as f32;
    let shift = |x: Pixels| extra * spaces.partition_point(|boundary| *boundary <= x) as f32;
    // Split drawing at visual cluster boundaries using per-glyph offsets during
    // painting; the same shifted clusters drive caret and selection geometry.
    for fragment in Arc::make_mut(&mut line.fragments) { for glyph in fragment.shape.runs.iter().flat_map(|run| &run.glyphs) { fragment.glyph_shifts.insert(glyph.index, shift(fragment.x + glyph.position.x)); } }
    for stop in Arc::make_mut(&mut line.stops) { stop.1 += shift(stop.1); }
    for cluster in Arc::make_mut(&mut line.clusters) { let right = cluster.x + cluster.width; cluster.width += shift(right) - shift(cluster.x); cluster.x += shift(cluster.x); }
    line.width = width;
}
