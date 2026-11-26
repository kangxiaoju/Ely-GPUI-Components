use super::{layout::EmailLayout, model::*, HtmlEmailEditor};
use gpui::{point, px, TestAppContext};

#[gpui::test]
fn mixed_size_baseline_and_arabic_latin_hit_test(cx: &mut TestAppContext) {
    cx.update(crate::theme::Theme::init);
    let (_, cx) = cx.add_window_view(|window, cx| HtmlEmailEditor::new(EmailDocument::default(), window, cx));
    cx.update(|window, _| {
        let mut document = EmailDocument::from_plain("Latin العربية end").unwrap();
        let block = document.first_point().block;
        document.format(&EmailSelection { anchor: EmailPoint { block, cell: 0, offset: 6 }, head: EmailPoint { block, cell: 0, offset: 20 } }, &EmailFormat::Size(Some(28))).unwrap();
        let mut layout = EmailLayout::default();
        layout.arrange(&document, px(600.), window);
        let line = &layout.lines[0];
        assert!(line.height > px(30.));
        assert!(line.fragments.iter().any(|fragment| fragment.rtl));
        assert!(line.fragments.iter().any(|fragment| !fragment.rtl));
        // GPUI TestPlatform shapes all glyphs LTR, including embedding controls.
        // Exercise the production cluster mapper with visual RTL glyph positions;
        // real-platform shaping is covered by the interactive prototype probe.
        let glyphs=(0..7).map(|index|(index*2,px((6-index)as f32*20.))).collect();
        let clusters=super::layout::map_clusters("Latin العربية end",6..20,px(60.),px(140.),true,glyphs);
        let mut stops=std::collections::BTreeMap::new();
        for cluster in &clusters{stops.insert(cluster.range.start,cluster.x+cluster.width);stops.insert(cluster.range.end,cluster.x);}
        let arabic:Vec<_>=stops.iter().map(|(offset,x)|(*offset,*x)).collect();
        assert!(arabic.windows(2).all(|pair|pair[0].1>=pair[1].1));
        let mut fixture=line.clone();fixture.clusters=std::sync::Arc::new(clusters);fixture.stops=std::sync::Arc::new(arabic.clone());
        let mut fixture_layout=EmailLayout::default();fixture_layout.lines.push(fixture.clone());
        for(offset,x)in arabic{let hit=fixture_layout.hit(point(fixture.origin.x+x,fixture.origin.y+fixture.height/2.)).unwrap();assert_eq!(hit.offset,offset);}
        assert!(line.fragments.iter().map(|fragment| fragment.shape.ascent).any(|ascent| ascent < line.ascent));
    });
}

#[gpui::test]
fn unchanged_revision_reuses_layout_and_long_text_wraps_on_graphemes(cx: &mut TestAppContext) {
    cx.update(crate::theme::Theme::init);
    let (_, cx) = cx.add_window_view(|window, cx| HtmlEmailEditor::new(EmailDocument::default(), window, cx));
    cx.update(|window, _| {
        let document = EmailDocument::from_plain(&"中文 👩‍👩‍👧‍👦 Arabic العربية ".repeat(1000)).unwrap();
        let mut layout = EmailLayout::default();
        let bytes_before=super::layout::shaped_bytes();
        layout.arrange(&document, px(300.), window);
        assert!(layout.lines.len() > 100);
        let shapes = layout.shape_count;
        assert!(super::layout::shaped_bytes()-bytes_before < document.to_plain().len()*12, "total shaped bytes must remain linear, including contextual line reshaping");
        layout.arrange(&document, px(300.), window);
        assert_eq!(layout.shape_count, shapes);
        for line in &layout.lines { assert!(line.width <= px(340.)); }
    });
}

#[gpui::test]
fn ligature_paint_origin_is_not_an_internal_caret_position(cx: &mut TestAppContext) {
    cx.update(crate::theme::Theme::init);
    let (_, cx) = cx.add_window_view(|window, cx| HtmlEmailEditor::new(EmailDocument::default(), window, cx));
    cx.update(|window, _| {
        let document = EmailDocument::from_plain("لا").unwrap();
        let mut layout = EmailLayout::default(); layout.arrange(&document, px(400.), window);
        let line = &layout.lines[0];
        for fragment in line.fragments.iter() {
            for glyph in fragment.shape.runs.iter().flat_map(|run| &run.glyphs).filter(|glyph| glyph.index >= fragment.prefix && glyph.index < fragment.prefix + fragment.end - fragment.start) {
                assert_eq!(fragment.glyph_shift(glyph.index), px(0.), "unjustified ligatures keep their shaped origin");
            }
        }
        assert_eq!(line.fragments[0].shape.font_size, window.text_style().font_size.to_pixels(window.rem_size()));
    });
}

#[gpui::test]
fn wrap_head_affinity_and_style_run_visits_are_explicit(cx: &mut TestAppContext) {
    cx.update(crate::theme::Theme::init);
    let (_, cx) = cx.add_window_view(|window, cx| HtmlEmailEditor::new(EmailDocument::default(), window, cx));
    cx.update(|window, _| {
        let mut content = EmailText::plain(&"word ".repeat(800));
        for index in (0..content.text.len()).step_by(5) { content.format(index..index + 4, &EmailFormat::Bold(index % 10 == 0)).unwrap(); }
        let runs = content.runs.len();
        let document = EmailDocument::from_blocks(vec![EmailBlock { id: 1, kind: EmailBlockKind::Paragraph(EmailParagraph { content, ..EmailParagraph::default() }) }]).unwrap();
        let mut layout = EmailLayout::default(); let visits_before = super::layout::run_visits(); layout.arrange(&document, px(160.), window);
        assert!(super::layout::run_visits() - visits_before < runs * 12, "run traversal must be local to each line and fragment");
        let second = &layout.lines[1];
        let (point, affinity) = layout.hit_with_affinity(second.origin + point(px(0.), second.height / 2.)).unwrap();
        let caret = layout.caret_with_affinity(point, affinity).unwrap();
        assert_eq!(caret.origin.y, second.origin.y);
    });
}

#[gpui::test]
fn inline_image_shares_text_baseline_and_atomic_hit_geometry(cx: &mut TestAppContext) {
    cx.update(crate::theme::Theme::init);
    let (_,cx)=cx.add_window_view(|window,cx|HtmlEmailEditor::new(EmailDocument::default(),window,cx));
    cx.update(|window,_| {
        let mut document=EmailDocument::from_plain("left right").unwrap(); let insertion=EmailPoint{offset:5,..document.first_point()};
        document.insert_image(&EmailSelection::caret(insertion),EmailImage{source:"cid:test".into(),alt:"picture".into(),width:Some(40),height:Some(30),align:EmailAlign::Left}).unwrap();
        let mut layout=EmailLayout::default();layout.arrange(&document,px(500.),window);
        assert_eq!(layout.lines.len(),1);assert_eq!(layout.images.len(),1);
        let image=&layout.images[0];assert_eq!(image.cell,0);assert_eq!(image.offset,5);assert_eq!(image.end,8);
        let before=layout.hit(image.bounds.origin+point(px(1.),px(15.))).unwrap();
        let after=layout.hit(image.bounds.origin+point(px(39.),px(15.))).unwrap();
        assert_eq!(before.offset,5);assert_eq!(after.offset,8);
        assert!(layout.lines[0].height>=px(30.));
    });
}

#[gpui::test]
fn rtl_inline_hits_keep_line_affinity_and_justified_boxes_follow_clusters(cx: &mut TestAppContext) {
    cx.update(crate::theme::Theme::init);
    let (_,cx)=cx.add_window_view(|window,cx|HtmlEmailEditor::new(EmailDocument::default(),window,cx));
    cx.update(|window,_| {
        let mut document=EmailDocument::from_plain("العربية العربية العربية العربية").unwrap();
        let insertion=EmailPoint{offset:15,..document.first_point()};
        document.insert_image(&EmailSelection::caret(insertion),EmailImage{source:"cid:rtl".into(),alt:"image".into(),width:Some(30),height:Some(25),align:EmailAlign::Left}).unwrap();
        document.set_alignment(&EmailSelection{anchor:document.first_point(),head:document.end_point()},EmailAlign::Justify).unwrap();
        let mut layout=EmailLayout::default();layout.arrange(&document,px(250.),window);
        let image=&layout.images[0];let (line_index,line)=layout.lines.iter().enumerate().find(|(_,line)|line.clusters.iter().any(|cluster|cluster.range.start==image.offset)).unwrap();
        let cluster=line.clusters.iter().find(|cluster|cluster.range.start==image.offset).unwrap();
        assert_eq!(image.bounds.origin.x,line.origin.x+cluster.x);
        let (left,affinity)=layout.hit_with_affinity(image.bounds.origin+point(px(1.),px(12.))).unwrap();
        assert_eq!(affinity,Some(line_index));assert_eq!(left.offset,if cluster.rtl{image.end}else{image.offset});
    });
}

#[gpui::test]
fn local_edit_updates_height_prefix_without_rebuilding_other_block_shapes(cx:&mut TestAppContext){
    cx.update(crate::theme::Theme::init);let (_,cx)=cx.add_window_view(|window,cx|HtmlEmailEditor::new(EmailDocument::default(),window,cx));
    cx.update(|window,_|{let mut document=EmailDocument::from_plain(&vec!["unchanged paragraph";1000].join("\n")).unwrap();let mut layout=EmailLayout::default();layout.arrange(&document,px(300.),window);layout.set_viewport(px(0.),px(100.));let before=layout.shape_count;document.replace(&EmailSelection::caret(document.first_point()),"local",&EmailStyle::default()).unwrap();layout.arrange(&document,px(300.),window);assert!(layout.shape_count-before<20);assert!(layout.lines.len()<20);let end=layout.caret(document.end_point()).unwrap();assert!(end.origin.y>px(100.));});
}
