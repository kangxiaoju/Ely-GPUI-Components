use std::rc::Rc;

use gpui::{
    App, ElementId, FontWeight, Hsla, IntoElement, ParentElement, RenderOnce, SharedString, Styled,
    Window, div,
};

use super::{
    panel::{OnEdit, caption, editing, pair},
    shape::Shape,
};
use crate::{
    feedback::EmptyState,
    forms::{ColorPalette, ScrubInput, Switch},
    primitives::{Icon, IconName},
    theme::{ActiveTheme, HUE_NAMES, IconSize},
};
use ely_core::typography::Ellipsis;

/// How far from the origin a figure may lie, in canvas units either way.
const REACH: f64 = 100_000.0;

/// The selected layer's name and kind, its frame, its fill among the chart hues, and whether it shows and is locked. Each edit hands the owner the changed shape; with none or several selected it says so.
///
/// ```
/// use ely_canvas::{Frame, InspectorPanel, Shape, ShapeKind};
///
/// let shape = Shape::new("box", "Box", ShapeKind::Rect, Frame::new(10.0, 10.0, 80.0, 60.0));
/// let inspector = InspectorPanel::new("inspect", [shape], |_shape, _, _| {});
/// ```
#[derive(IntoElement)]
pub struct InspectorPanel {
    id: ElementId,
    selection: Vec<Shape>,
    on_change: OnEdit<Shape>,
    read_only: bool,
    defaults: Option<(super::ShapeAppearance, OnEdit<super::ShapeAppearance>)>,
}

impl InspectorPanel {
    /// An inspector under `id` for `selection`; `on_change` gets each edited shape.
    pub fn new(
        id: impl Into<ElementId>,
        selection: impl IntoIterator<Item = Shape>,
        on_change: impl Fn(Shape, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            selection: selection.into_iter().collect(),
            on_change: Rc::new(on_change),
            read_only: false,
            defaults: None,
        }
    }

    /// Shows non-editable property values for a read-only document.
    pub fn read_only(mut self, read_only: bool) -> Self { self.read_only=read_only; self }

    /// Shows the same appearance controls for new shapes when nothing is selected.
    pub fn defaults(mut self, appearance: super::ShapeAppearance, on_change: impl Fn(super::ShapeAppearance, &mut Window, &mut App) + 'static) -> Self {
        self.defaults = Some((appearance, Rc::new(on_change)));
        self
    }
}

impl RenderOnce for InspectorPanel {
    fn render(mut self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let defaults = self.selection.is_empty() && self.defaults.is_some();
        if defaults {
            if let Some((appearance, handler)) = self.defaults.take() {
                self.selection.push(Shape::new("default-style", crate::i18n::text(cx,"canvas.defaults",&[]), super::ShapeKind::Rect, super::Frame::new(0.0,0.0,1.0,1.0)).appearance(appearance));
                self.on_change = Rc::new(move |shape,window,cx| { if let Some(style) = shape.appearance { handler(style,window,cx); } });
            }
        }
        let on_change = self.on_change;
        let id = self.id;
        let [shape] = self.selection.as_slice() else {
            let title: SharedString = match self.selection.len() {
                0 => crate::i18n::text(cx,"canvas.empty",&[]),
                count => crate::i18n::text(cx,"canvas.selected_count",&[("count",&count.to_string())]),
            };
            return EmptyState::new((id, "empty"), IconName::MousePointer2, title)
                .body(crate::i18n::text(cx,"canvas.empty_hint",&[]))
                .into_any_element();
        };
        if self.read_only {
            return div().flex().flex_col().gap_2().child(caption(crate::i18n::text(cx,"canvas.read_only",&[]),cx))
                .child(caption(shape.name.clone(),cx))
                .child(caption(format!("{} · {} × {} · {}°",shape.kind.words(cx),shape.frame.w,shape.frame.h,shape.rotation.to_degrees()),cx)).into_any_element();
        }
        let on_change = &on_change;
        let at = |edit: fn(&mut Shape, f64)| editing("inspector", shape, on_change, edit);
        let scrub = |name: &'static str, value: f32, least: f64| {
            ScrubInput::new((id.clone(), name), crate::i18n::text(cx,match name { "X" => "canvas.x", "Y" => "canvas.y", "W" => "canvas.width", _ => "canvas.height" },&[]), value as f64)
                .range(least, REACH)
                .precision(0)
        };
        let frame = shape.frame;
        let theme = cx.theme();
        let chart = theme.colors.chart;
        let fill = theme
            .colors
            .hue(shape.hue, format_args!("layer {}", shape.key));
        let hue = editing("inspector", shape, on_change, move |shape, color: Hsla| {
            shape.hue = chart
                .iter()
                .position(|hue| *hue == color)
                .expect("the fill is a chart hue");
            if let Some(style) = &mut shape.appearance { style.stroke = shape.hue; }
        });
        let shown = editing("inspector", shape, on_change, |shape, on: bool| {
            shape.hidden = !on
        });
        let locked = editing("inspector", shape, on_change, |shape, on: bool| {
            shape.locked = on
        });
        let appearance = shape.appearance.unwrap_or_default();
        let opacity = editing("inspector", shape, on_change, |s, v:f64| s.appearance.get_or_insert_with(Default::default).opacity = v as f32);
        let width = editing("inspector", shape, on_change, |s, v:f64| s.appearance.get_or_insert_with(Default::default).stroke_width = v as f32);
        let roughness = editing("inspector", shape, on_change, |s, v:f64| s.appearance.get_or_insert_with(Default::default).roughness = v as f32);
        let geometry = editing("inspector", shape, on_change, |s, value: SharedString| {
            let hue = s.hue;
            let style = s.appearance.get_or_insert_with(|| super::ShapeAppearance { stroke: hue, ..Default::default() });
            if value.as_ref() == "standard" { style.roughness = 0.0; }
            else if style.roughness <= 0.0 { style.roughness = 1.2; }
        });
        let font = editing("inspector", shape, on_change, |s, v:f64| s.appearance.get_or_insert_with(Default::default).font_size = v as f32);
        let rotate = editing("inspector", shape, on_change, |s, v:f64| s.rotation = (v as f32).to_radians());
        let filled = editing("inspector", shape, on_change, |s, v:bool| { let hue=s.hue; s.appearance.get_or_insert_with(Default::default).fill = v.then_some(hue); });
        let fill_hue = editing("inspector", shape, on_change, move |s, color:Hsla| { s.appearance.get_or_insert_with(Default::default).fill = chart.iter().position(|h| *h == color); });
        let dash = editing("inspector", shape, on_change, |s, value:SharedString| {
            s.appearance.get_or_insert_with(Default::default).dash = match value.as_ref() { "dashed" => super::style::Dash::Dashed, "dotted" => super::style::Dash::Dotted, _ => super::style::Dash::Solid };
        });
        let family = editing("inspector", shape, on_change, |s, value:SharedString| {
            s.appearance.get_or_insert_with(Default::default).font = match value.as_ref() { "handwritten" => super::CanvasFont::Handwritten, "monospace" => super::CanvasFont::Monospace, _ => super::CanvasFont::Default };
        });
        let part = |title: SharedString| div().flex().flex_col().gap_2().child(caption(title, cx));
        let word = |key:&str| crate::i18n::text(cx,key,&[]);
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Icon::new(shape.kind.icon())
                            .size(IconSize::Sm)
                            .color(theme.colors.fg_muted),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(Ellipsis::new(shape.name.clone())),
                    )
                    .children((!defaults).then(|| caption(shape.kind.words(cx), cx).flex_none())),
            )
            .children((!defaults).then(||
                part(word("canvas.frame"))
                    .child(pair(
                        scrub("X", frame.x, -REACH).on_change(at(|s, v| s.frame.x = v as f32)),
                        scrub("Y", frame.y, -REACH).on_change(at(|s, v| s.frame.y = v as f32)),
                    ))
                    .child(pair(
                        scrub("W", frame.w, 0.0).on_change(at(|s, v| s.frame.w = v as f32)),
                        scrub("H", frame.h, 0.0).on_change(at(|s, v| s.frame.h = v as f32)),
                    ))
            ))
            .child(
                part(word("canvas.stroke")).child(
                    ColorPalette::new((id.clone(), "stroke"), HUE_NAMES.iter().enumerate().map(|(ix,_)| (word(&format!("canvas.hue.{ix}")),chart[ix])))
                        .selected(fill)
                        .on_change(hue),
                ),
            )
            .child(div().flex().flex_col().gap_2().child(caption(word("canvas.appearance"),cx))
                .child(caption(word("canvas.geometry"),cx))
                .child(crate::buttons::SegmentedControl::new((id.clone(), "geometry-mode"), if appearance.roughness > 0.0 { "handdrawn" } else { "standard" })
                    .segment("standard",word("canvas.geometry.standard"),None)
                    .segment("handdrawn",word("canvas.geometry.handdrawn"),None)
                    .on_change(move |value,window,cx| geometry(value.clone(),window,cx)))
                .child(ScrubInput::new((id.clone(), "rotation"), word("canvas.rotation"), shape.rotation.to_degrees() as f64).range(-360.0,360.0).on_change(rotate))
                .child(ScrubInput::new((id.clone(), "width"), word("canvas.stroke_width"), appearance.stroke_width as f64).range(0.1,100.0).on_change(width))
                .child(ScrubInput::new((id.clone(), "opacity"), word("canvas.opacity"), appearance.opacity as f64).range(0.0,1.0).precision(2).on_change(opacity))
                .child(ScrubInput::new((id.clone(), "roughness"), word("canvas.roughness"), appearance.roughness as f64).range(0.0,10.0).on_change(roughness))
                .child(ScrubInput::new((id.clone(), "font"), word("canvas.font_size"), appearance.font_size as f64).range(1.0,512.0).on_change(font))
                .child(Switch::new((id.clone(), "filled"), appearance.fill.is_some()).label(word("canvas.fill")).on_change(filled))
                .child(ColorPalette::new((id.clone(), "fill-hue"), HUE_NAMES.iter().enumerate().map(|(ix,_)| (word(&format!("canvas.hue.{ix}")),chart[ix]))).selected(appearance.fill.map(|hue| chart[hue % chart.len()]).unwrap_or(chart[0])).on_change(fill_hue))
                .child(caption(word("canvas.font"),cx))
                .child(crate::buttons::SegmentedControl::new((id.clone(), "font-family"), match appearance.font { super::CanvasFont::Default => "default", super::CanvasFont::Handwritten => "handwritten", super::CanvasFont::Monospace => "monospace" })
                    .segment("handwritten",word("canvas.font.handwritten"),None).segment("default",word("canvas.font.default"),None).segment("monospace",word("canvas.font.monospace"),None)
                    .on_change(move |value,window,cx| family(value.clone(),window,cx)))
                .child(crate::buttons::SegmentedControl::new((id.clone(), "dash"), match appearance.dash { super::style::Dash::Solid => "solid", super::style::Dash::Dashed => "dashed", super::style::Dash::Dotted => "dotted" })
                    .segment("solid", word("canvas.solid"), None).segment("dashed", word("canvas.dashed"), None).segment("dotted", word("canvas.dotted"), None).on_change(move |value,window,cx| dash(value.clone(),window,cx))))
            .children((!defaults).then(||
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        Switch::new((id.clone(), "visible"), !shape.hidden)
                            .label(word("canvas.visible"))
                            .on_change(shown),
                    )
                    .child(
                        Switch::new((id, "locked"), shape.locked)
                            .label(word("canvas.locked"))
                            .on_change(locked),
                    )
            ))
            .into_any_element()
    }
}
