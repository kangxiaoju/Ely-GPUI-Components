use crate::{feedback::InlineMessage, i18n, layout::{ScrollArea, bring_into_view}, primitives::Severity, theme::ActiveTheme};
use gpui::{prelude::*, AnyElement, App, AvailableSpace, Bounds, Context, Element, ElementId, ElementInputHandler, Entity, GlobalElementId, InspectorElementId, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, Pixels, Role, Style, TextAlign, Window, div, fill, img, point, px, size};
use super::{actions::*, editor::HtmlEmailEditor, model::EmailSelection};

pub(crate) struct EmailBodyElement { editor: Entity<HtmlEmailEditor>, images: Vec<AnyElement> }
impl IntoElement for EmailBodyElement { type Element = Self; fn into_element(self) -> Self { self } }
impl Element for EmailBodyElement {
    type RequestLayoutState = ();
    type PrepaintState = ();
    fn id(&self) -> Option<ElementId> { Some("html-email-layout".into()) }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> { None }
    fn request_layout(&mut self, _: Option<&GlobalElementId>, _: Option<&InspectorElementId>, window: &mut Window, _: &mut App) -> (LayoutId, ()) {
        let editor = self.editor.clone();
        (window.request_measured_layout(Style::default(), move |known, available, window, cx| {
            let width = known.width.unwrap_or_else(|| match available.width { AvailableSpace::Definite(value) => value, _ => px(640.) });
            editor.update(cx, |editor, _| { editor.layout.arrange(&editor.document, width.max(px(24.)), window); size(width, editor.layout.height) })
        }), ())
    }
    fn prepaint(&mut self, _: Option<&GlobalElementId>, _: Option<&InspectorElementId>, bounds: Bounds<Pixels>, _: &mut (), window: &mut Window, cx: &mut App) {
        let images = self.editor.update(cx, |editor, _| {
            editor.layout.origin = bounds.origin;
            let viewport = window.content_mask().bounds;
            let caret = editor.layout.caret_with_affinity(editor.selection.head, editor.affinity);
            editor.layout.set_viewport(viewport.origin.y - bounds.origin.y, viewport.bottom() - bounds.origin.y);
            // Visible-line indices change as blocks enter or leave the viewport.
            if let Some(caret) = caret { editor.affinity = editor.layout.lines.iter().position(|line| line.block == editor.selection.head.block && line.cell == editor.selection.head.cell && bounds.origin.y + line.origin.y == caret.origin.y && line.stops.iter().any(|(offset, _)| *offset == editor.selection.head.offset)); }
            if std::mem::take(&mut editor.reveal_selection) {
                if let Some(caret) = editor.layout.caret_with_affinity(editor.selection.head, editor.affinity) {
                    // GPUI snaps the body's offset to physical pixels. Reveal
                    // one physical pixel beyond the caret so rounding cannot
                    // leave its bottom edge clipped by a fractional pixel.
                    let gutter = px(1.) / window.scale_factor();
                    let target = Bounds::new(caret.origin - point(px(0.), gutter), size(caret.size.width, caret.size.height + gutter * 2.));
                    if bring_into_view(&editor.scroll, target, gpui::Axis::Vertical) {
                        // Prepaint already received the current frame's bounds;
                        // draw the revealed content at its new offset next frame.
                        let entity = self.editor.clone();
                        window.on_next_frame(move |_, cx| entity.update(cx, |_, cx| cx.notify()));
                    }
                }
            }
            editor.layout.images.iter().filter(|image| { let rect = Bounds::new(bounds.origin + image.bounds.origin, image.bounds.size); rect.intersects(&viewport) }).map(|image| (image.clone(), editor.images.get(&image.image.source).cloned())).collect::<Vec<_>>()
        });
        self.images.clear();
        for (image, asset) in images {
            let mut element = if let Some(asset) = asset { img(asset).w(image.bounds.size.width).h(image.bounds.size.height).into_any_element() } else { InlineMessage::new(Severity::Warning, if image.image.alt.is_empty() { i18n::text(cx, "email.image.unavailable", &[]).to_string() } else { image.image.alt.clone() }).into_any_element() };
            element.layout_as_root(size(AvailableSpace::Definite(image.bounds.size.width), AvailableSpace::Definite(image.bounds.size.height)), window, cx);
            element.prepaint_at(bounds.origin + image.bounds.origin, window, cx); self.images.push(element);
        }
    }
    fn paint(&mut self, _: Option<&GlobalElementId>, _: Option<&InspectorElementId>, bounds: Bounds<Pixels>, _: &mut (), _: &mut (), window: &mut Window, cx: &mut App) {
        self.editor.update(cx, |editor, cx| {
            let colors = cx.theme().colors.clone();
            if !editor.disabled { window.handle_input(&editor.focus, ElementInputHandler::new(bounds, cx.entity()), cx); }
            if std::mem::take(&mut editor.text_input_dirty) { window.notify_text_input_changed(&editor.focus); }
            if std::mem::take(&mut editor.selection_dirty) { window.notify_text_selection_changed(&editor.focus); }
            let selection_map = editor.input_map(); let selected = selection_map.range(&editor.selection);
            let viewport = window.content_mask().bounds;
            for border in &editor.layout.borders { let rect = Bounds::new(bounds.origin + border.origin, border.size); window.paint_quad(gpui::outline(rect, colors.border, gpui::BorderStyle::default())); }
            for line in &editor.layout.lines {
                let origin = bounds.origin + line.origin;
                if origin.y + line.height < viewport.origin.y || origin.y > viewport.bottom() { continue; }
                if !selected.is_empty() {
                    for cluster in line.clusters.iter() { let a = selection_map.utf16(super::model::EmailPoint { block: line.block, cell: line.cell, offset: cluster.range.start }); let b = selection_map.utf16(super::model::EmailPoint { block: line.block, cell: line.cell, offset: cluster.range.end }); if a < selected.end && b > selected.start { window.paint_quad(fill(Bounds::new(origin + point(cluster.x, px(0.)), size(cluster.width.max(px(1.)), line.height)), colors.selection)); } }
                }
                for fragment in line.fragments.iter() {
                    for shaped_run in &fragment.shape.runs { for glyph in &shaped_run.glyphs {
                        if glyph.index < fragment.prefix || glyph.index >= fragment.prefix + fragment.end - fragment.start { continue; }
                        let logical = fragment.start + glyph.index - fragment.prefix;
                        let styled = fragment.styles.partition_point(|(range, _)| range.end <= logical);
                        let style = fragment.styles.get(styled).map(|(_,style)| style);
                        let color = style.and_then(|style| style.color).map(|color| gpui::rgb((u32::from(color.red) << 16) | (u32::from(color.green) << 8) | u32::from(color.blue)).into()).unwrap_or(colors.fg);
                        let shift = fragment.glyph_shift(glyph.index);
                        let glyph_origin = origin + point(fragment.x + glyph.position.x + shift, line.ascent + glyph.position.y);
                        let result = if glyph.is_emoji { window.paint_emoji(glyph_origin, shaped_run.font_id, glyph.id, fragment.shape.font_size) } else { window.paint_glyph(glyph_origin, shaped_run.font_id, glyph.id, fragment.shape.font_size, color) };
                        if let Err(error) = result { log::warn!("HTML email glyph painting failed: {error}"); }
                    } }
                    for cluster in &line.clusters[fragment.cluster_range.clone()] { let index = fragment.styles.partition_point(|(range,_)| range.end <= cluster.range.start); if let Some((_,style)) = fragment.styles.get(index) { let color = style.color.map(|color| gpui::rgb((u32::from(color.red)<<16)|(u32::from(color.green)<<8)|u32::from(color.blue)).into()).unwrap_or(colors.fg); if style.underline || style.link.is_some() { window.paint_quad(fill(Bounds::new(origin + point(cluster.x,line.ascent+px(2.)),size(cluster.width,px(1.))),color)); } if style.strike { window.paint_quad(fill(Bounds::new(origin + point(cluster.x,line.ascent*0.65),size(cluster.width,px(1.))),color)); } } }
                }
                if let Some(marker) = &line.marker { let _ = marker.paint(origin + point(-px(24.), line.ascent - marker.ascent), marker.ascent + marker.descent, TextAlign::Left, None, window, cx); }
            }
            if editor.focus.is_focused(window) && editor.selection.anchor == editor.selection.head && !editor.disabled { if let Some(caret) = editor.layout.caret_with_affinity(editor.selection.head, editor.affinity) { window.paint_quad(fill(caret, colors.fg)); } }
        });
        for image in &mut self.images { image.paint(window, cx); }
        self.editor.update(cx, |editor, cx| { let map = editor.input_map(); let selected = map.range(&editor.selection); let colors = cx.theme().colors.clone(); for image in &editor.layout.images { let start = map.utf16(super::model::EmailPoint { block: image.block, cell: image.cell, offset: image.offset }); let end = map.utf16(super::model::EmailPoint { block: image.block, cell: image.cell, offset: image.end }); if start < selected.end && end > selected.start { let rect = Bounds::new(bounds.origin + image.bounds.origin, image.bounds.size); window.paint_quad(gpui::outline(rect, colors.focus, gpui::BorderStyle::default())); } } });
    }
}
pub(crate) fn body(editor: &mut HtmlEmailEditor, _: &mut Window, cx: &mut Context<HtmlEmailEditor>) -> AnyElement {
    let mut root = div().id("html-email-editor").debug_selector(||"html-email-editor".into()).role(Role::MultilineTextInput).aria_label(i18n::text(cx, "email.body", &[])).key_context("HtmlEmailEditor").track_focus(&editor.focus).size_full().flex().flex_col().cursor_text()
        .on_mouse_down(MouseButton::Left, cx.listener(|editor, event: &MouseDownEvent, window, cx| {
            if editor.disabled { return; } window.focus(&editor.focus, cx); if let Some((head, affinity)) = editor.layout.hit_with_affinity(event.position) { let anchor = if event.modifiers.shift { editor.selection.anchor } else { head }; editor.set_selection(EmailSelection { anchor, head }, cx); editor.affinity = affinity; editor.dragging = true; }
        }))
        .on_mouse_move(cx.listener(|editor, event: &MouseMoveEvent, _, cx| { if editor.dragging && !editor.disabled { if let Some((head, affinity)) = editor.layout.hit_with_affinity(event.position) { editor.set_selection(EmailSelection { anchor: editor.selection.anchor, head }, cx); editor.affinity = affinity; } } }))
        .on_mouse_up(MouseButton::Left, cx.listener(|editor, _, _, _| editor.dragging = false))
        .on_action(cx.listener(|editor, _: &NextCell, _, cx| editor.move_cell(true, cx)))
        .on_action(cx.listener(|editor, _: &PreviousCell, _, cx| editor.move_cell(false, cx)))
        .on_mouse_up_out(MouseButton::Left, cx.listener(|editor, _, _, _| editor.dragging = false))
        .on_action(cx.listener(|editor, _: &Left, _, cx| editor.move_horizontal(false, false, cx)))
        .on_action(cx.listener(|editor, _: &Right, _, cx| editor.move_horizontal(true, false, cx)))
        .on_action(cx.listener(|editor, _: &SelectLeft, _, cx| editor.move_horizontal(false, true, cx)))
        .on_action(cx.listener(|editor, _: &SelectRight, _, cx| editor.move_horizontal(true, true, cx)))
        .on_action(cx.listener(|editor, _: &Up, _, cx| editor.move_vertical(false, false, cx)))
        .on_action(cx.listener(|editor, _: &Down, _, cx| editor.move_vertical(true, false, cx)))
        .on_action(cx.listener(|editor, _: &SelectUp, _, cx| editor.move_vertical(false, true, cx)))
        .on_action(cx.listener(|editor, _: &SelectDown, _, cx| editor.move_vertical(true, true, cx)))
        .on_action(cx.listener(|editor, _: &Home, _, cx| editor.move_edge(false, false, cx)))
        .on_action(cx.listener(|editor, _: &End, _, cx| editor.move_edge(true, false, cx)))
        .on_action(cx.listener(|editor, _: &SelectHome, _, cx| editor.move_edge(false, true, cx)))
        .on_action(cx.listener(|editor, _: &SelectEnd, _, cx| editor.move_edge(true, true, cx)))
        .on_action(cx.listener(|editor, _: &Backspace, _, cx| editor.delete_direction(false, cx)))
        .on_action(cx.listener(|editor, _: &Delete, _, cx| editor.delete_direction(true, cx)))
        .on_action(cx.listener(|editor, _: &Enter, _, cx| editor.newline(false, cx)))
        .on_action(cx.listener(|editor, _: &HardBreak, _, cx| editor.newline(true, cx)))
        .on_action(cx.listener(|editor, _: &SelectAll, _, cx| { if !editor.disabled { editor.set_selection(EmailSelection { anchor: editor.document.first_point(), head: editor.document.end_point() }, cx); } }))
        .on_action(cx.listener(|editor, _: &Copy, _, cx| editor.copy_selection(false, cx)))
        .on_action(cx.listener(|editor, _: &Cut, _, cx| editor.copy_selection(true, cx)))
        .on_action(cx.listener(|editor, _: &Paste, window, cx| editor.paste_clipboard(window, cx)))
        .on_action(cx.listener(|editor, _: &Undo, _, cx| editor.undo(cx)))
        .on_action(cx.listener(|editor, _: &Redo, _, cx| editor.redo(cx)))
        .on_action(cx.listener(|editor, _: &Cancel, _, cx| editor.cancel_composition(cx)))
        .child(ScrollArea::new("html-email-scroll").size_full().track_scroll(&editor.scroll).child(EmailBodyElement { editor: cx.entity(), images: Vec::new() }));
    if let Some(problem) = &editor.problem { root = root.child(InlineMessage::new(Severity::Danger, problem.clone())); }
    root.into_any_element()
}
