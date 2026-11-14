use gpui::{Bounds, Hsla, PathBuilder, Pixels, Point, Window, fill, point, size};

use super::{
    shape::{Shape, ShapeKind, polygon},
    view::{Frame, Viewport},
};
use crate::theme::Palette;

/// The view pixels of a canvas frame.
pub(crate) fn in_view(view: &Viewport, frame: &Frame) -> Frame {
    let (x, y) = view.to_view((frame.x, frame.y));
    Frame::new(x, y, frame.w * view.zoom, frame.h * view.zoom)
}

#[doc(hidden)]
pub fn at(origin: Point<Pixels>, (x, y): (f32, f32)) -> Point<Pixels> {
    origin + point(Pixels::from(x), Pixels::from(y))
}

pub(crate) fn quad(origin: Point<Pixels>, frame: &Frame) -> Bounds<Pixels> {
    Bounds::new(
        at(origin, (frame.x, frame.y)),
        size(Pixels::from(frame.w), Pixels::from(frame.h)),
    )
}

#[doc(hidden)]
pub fn outline(
    points: &[(f32, f32)],
    origin: Point<Pixels>,
    width: Pixels,
    closed: bool,
) -> PathBuilder {
    let mut path = PathBuilder::stroke(width);
    if let Some(first) = points.first() {
        path.move_to(at(origin, *first));
        points[1..]
            .iter()
            .for_each(|next| path.line_to(at(origin, *next)));
        if closed {
            path.close();
        }
    }
    path
}

pub(crate) fn solid(points: &[(f32, f32)], origin: Point<Pixels>) -> PathBuilder {
    let mut path = PathBuilder::fill();
    if let Some(first) = points.first() {
        path.move_to(at(origin, *first));
        points[1..]
            .iter()
            .for_each(|next| path.line_to(at(origin, *next)));
        path.close();
    }
    path
}

#[doc(hidden)]
pub fn finish(path: PathBuilder, color: Hsla, window: &mut Window) {
    match path.build() {
        Ok(path) => window.paint_path(path, color),
        Err(error) => log::error!("canvas: a path failed to build: {error:#}"),
    }
}

/// Paints a shape's outline and wash in view pixels.
pub(crate) fn paint_shape(
    shape: &Shape,
    view: &Viewport,
    origin: Point<Pixels>,
    palette: &Palette,
    stroke: Pixels,
    window: &mut Window,
) {
    let style = shape.appearance;
    let opacity = style.map_or(1.0, |s| s.opacity.clamp(0.0, 1.0));
    let ink = palette.hue(shape.hue, format_args!("shape {}", shape.key)).alpha(opacity);
    let wash = match style {
        Some(s) => s.fill.map(|hue| palette.hue(hue, "shape fill").alpha(opacity)),
        None => Some(ink.alpha(0.14)),
    };
    let stroke = style.map_or(stroke, |s| Pixels::from(s.stroke_width * view.zoom));
    let frame = &shape.frame;
    if let ShapeKind::Symbol(kind) = shape.kind {
        for (index,mut points) in super::symbol_paths(kind,frame.w,frame.h).into_iter().enumerate() {
            let closed=points.len()>2 && points.first()==points.last();
            if closed { points.pop(); }
            for p in &mut points {p.0+=frame.x;p.1+=frame.y;}
            let roughness=style.map_or(0.0,|s|s.roughness);
            let seed=super::sketch_seed(&shape.key).wrapping_add(index as u64);
            let second=(roughness>0.0).then(||super::sketch_outline(&points,closed,roughness,seed^0x9e3779b97f4a7c15));
            let mut points=super::sketch_outline(&points,closed,roughness,seed);
            for p in &mut points {*p=view.to_view(shape.rotate_point(*p,shape.rotation));}
            if index==0 && closed {if let Some(wash)=wash {finish(solid(&points,origin),wash,window);}}
            let dash=style.map_or(super::Dash::Solid,|s|s.dash);
            patterned(&points,origin,stroke,closed,dash,0.0,ink,window);
            if let Some(mut second)=second {
                for p in &mut second {*p=view.to_view(shape.rotate_point(*p,shape.rotation));}
                patterned(&second,origin,stroke*0.65,closed,dash,0.0,ink.opacity(0.45),window);
            }
        }
        return;
    }
    let mut closed = false;
    let mut points = match &shape.kind {
        ShapeKind::Rect | ShapeKind::Note(_) => {
            closed = true;
            vec![(frame.x, frame.y), (frame.right(), frame.y),
                (frame.right(), frame.bottom()), (frame.x, frame.bottom())]
        }
        ShapeKind::RoundedRect => {
            closed = true;
            super::rounded_rectangle_outline(frame)
        }
        ShapeKind::Ellipse | ShapeKind::Polygon(_) | ShapeKind::Diamond => {
            closed = true;
            let sides = match shape.kind { ShapeKind::Polygon(n) => n, ShapeKind::Diamond => 4, _ => 64 };
            polygon(frame, sides)
        }
        ShapeKind::Line(end) | ShapeKind::Arrow(end) => vec![end.opposite().of(frame), end.of(frame)],
        ShapeKind::Path { points, .. } | ShapeKind::Polyline { points, .. } => points.iter().map(|(x,y)| (frame.x + x, frame.y + y)).collect(),
        ShapeKind::Text(_) | ShapeKind::Image(_) | ShapeKind::Symbol(_) => return,
    };
    let roughness = style.map_or(0.0, |style| style.roughness);
    let second = (roughness > 0.0).then(|| super::sketch_outline(&points,closed,roughness,super::sketch_seed(&shape.key)^0x9e3779b97f4a7c15));
    points = super::sketch_outline(&points,closed,roughness,super::sketch_seed(&shape.key));
    for point in &mut points { *point = view.to_view(shape.rotate_point(*point,shape.rotation)); }
    if closed {
        if let Some(wash) = wash { finish(solid(&points, origin), wash, window); }
    }
    let width = match (&shape.kind, style) {
        (ShapeKind::Path { width, .. }, None) => Pixels::from(width * view.zoom).max(stroke),
        _ => stroke,
    };
    let dash = style.map_or(super::style::Dash::Solid, |s| s.dash);
    patterned(&points, origin, width, closed, dash, 0.0, ink, window);
    if let Some(mut second) = second {
        for point in &mut second { *point = view.to_view(shape.rotate_point(*point,shape.rotation)); }
        patterned(&second,origin,width*0.65,closed,dash,0.0,ink.opacity(0.45),window);
    }
    if matches!(shape.kind, ShapeKind::Arrow(_) | ShapeKind::Polyline { arrow: true, .. }) && points.len() >= 2 {
        finish(solid(&head(points[points.len()-2], points[points.len()-1], width), origin), ink, window);
    }
}

/// A solid, dashed or dotted polyline; phase is measured in view pixels.
pub(crate) fn patterned(points: &[(f32, f32)], origin: Point<Pixels>, width: Pixels,
    closed: bool, dash: super::style::Dash, phase: f32, ink: Hsla, window: &mut Window) {
    use super::style::Dash;
    if dash == Dash::Solid { finish(outline(points, origin, width, closed), ink, window); return; }
    let width_value = f32::from(width).max(0.5);
    let (on, off) = if dash == Dash::Dotted { (width_value, width_value * 3.0) }
        else { (width_value * 5.0, width_value * 3.0) };
    let period = on + off;
    let mut distance = -phase;
    let count = points.len().saturating_sub(1) + usize::from(closed);
    for ix in 0..count {
        let (a,b) = (points[ix], points[(ix+1) % points.len()]);
        let length = (b.0-a.0).hypot(b.1-a.1);
        if length <= f32::EPSILON { continue; }
        let mut cursor = 0.0;
        while cursor < length {
            let offset = (distance + cursor).rem_euclid(period);
            let drawing = offset < on;
            let span = (if drawing { on - offset } else { period - offset }).min(length-cursor).max(0.001);
            if drawing {
                let at = |d:f32| (a.0+(b.0-a.0)*d/length, a.1+(b.1-a.1)*d/length);
                finish(outline(&[at(cursor), at(cursor+span)], origin, width, false), ink, window);
            }
            cursor += span;
        }
        distance += length;
    }
}

/// An arrow's head at `to`, coming from `from`: its tip and its two back corners.
pub(crate) fn head(from: (f32, f32), to: (f32, f32), stroke: Pixels) -> [(f32, f32); 3] {
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let length = (dx * dx + dy * dy).sqrt().max(1.0);
    let (ux, uy) = (dx / length, dy / length);
    let reach = f32::from(stroke) * 5.0;
    let back = (to.0 - ux * reach, to.1 - uy * reach);
    [
        to,
        (back.0 - uy * reach / 2.0, back.1 + ux * reach / 2.0),
        (back.0 + uy * reach / 2.0, back.1 - ux * reach / 2.0),
    ]
}

/// A curve from `from` to `to` that leaves and arrives level, its handles `pull` along the way it runs.
pub(crate) fn wire(
    from: (f32, f32),
    to: (f32, f32),
    pull: f32,
    origin: Point<Pixels>,
    width: Pixels,
) -> PathBuilder {
    let mut path = PathBuilder::stroke(width);
    path.move_to(at(origin, from));
    path.cubic_bezier_to(
        at(origin, to),
        at(origin, (from.0 + pull, from.1)),
        at(origin, (to.0 - pull, to.1)),
    );
    path
}
