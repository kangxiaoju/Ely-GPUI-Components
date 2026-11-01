use gpui::{App, SharedString};

use super::{
    shape::{Corner, Guide, Shape, ShapeKind},
    view::Frame,
};

/// What the pointer does on a canvas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    /// Picks and moves shapes.
    Select,
    /// Pans the canvas.
    Hand,
    /// Draws a rectangle.
    Rect,
    /// Draws a rounded rectangle.
    RoundedRect,
    /// Draws an ellipse.
    Ellipse,
    /// Draws a diamond.
    Diamond,
    /// Deletes unlocked shapes along the pointer stroke.
    Eraser,
    /// Draws a polygon.
    Polygon,
    /// Draws a line.
    Line,
    /// Draws an arrow.
    Arrow,
    /// Draws a path, a point per press.
    Pen,
    /// Drops a line of text.
    Text,
    /// Strokes a path freehand.
    Brush,
    /// Drops a note.
    Note,
    /// Links two shapes.
    Connector,
}

impl Tool {
    /// Every tool, in the palette's order.
    pub const ALL: [Tool; 15] = [
        Tool::Select,
        Tool::Hand,
        Tool::Rect,
        Tool::RoundedRect,
        Tool::Ellipse,
        Tool::Polygon,
        Tool::Diamond,
        Tool::Eraser,
        Tool::Line,
        Tool::Arrow,
        Tool::Pen,
        Tool::Text,
        Tool::Brush,
        Tool::Note,
        Tool::Connector,
    ];

    /// Its id, the value its control carries.
    pub fn id(self) -> &'static str {
        match self {
            Tool::Select => "select",
            Tool::Hand => "hand",
            Tool::Rect => "rect",
            Tool::RoundedRect => "rounded_rect",
            Tool::Ellipse => "ellipse",
            Tool::Diamond => "diamond",
            Tool::Eraser => "eraser",
            Tool::Polygon => "polygon",
            Tool::Line => "line",
            Tool::Arrow => "arrow",
            Tool::Pen => "pen",
            Tool::Text => "text",
            Tool::Brush => "brush",
            Tool::Note => "note",
            Tool::Connector => "connector",
        }
    }

    /// Its name in the shown locale.
    pub fn words(self, cx: &App) -> SharedString {
        crate::i18n::text(cx, &format!("canvas.tool.{}", self.id()), &[])
    }

    /// The letter that picks it.
    pub fn key(self) -> &'static str {
        match self {
            Tool::Select => "v",
            Tool::Hand => "h",
            Tool::Rect => "r",
            Tool::RoundedRect => "u",
            Tool::Ellipse => "o",
            Tool::Diamond => "d",
            Tool::Eraser => "e",
            Tool::Polygon => "y",
            Tool::Line => "l",
            Tool::Arrow => "a",
            Tool::Pen => "p",
            Tool::Text => "t",
            Tool::Brush => "b",
            Tool::Note => "n",
            Tool::Connector => "c",
        }
    }

    /// The kind of shape a drag with it draws, if it draws one.
    pub(crate) fn draws(self) -> Option<ShapeKind> {
        match self {
            Tool::Rect => Some(ShapeKind::Rect),
            Tool::RoundedRect => Some(ShapeKind::RoundedRect),
            Tool::Ellipse => Some(ShapeKind::Ellipse),
            Tool::Diamond => Some(ShapeKind::Diamond),
            Tool::Polygon => Some(ShapeKind::Polygon(6)),
            Tool::Line => Some(ShapeKind::Line(Corner::BottomRight)),
            Tool::Arrow => Some(ShapeKind::Arrow(Corner::BottomRight)),
            _ => None,
        }
    }
}

/// A handle on a selection's frame, by the side or corner it holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Handle {
    /// The top side.
    North,
    /// The top right corner.
    NorthEast,
    /// The right side.
    East,
    /// The bottom right corner.
    SouthEast,
    /// The bottom side.
    South,
    /// The bottom left corner.
    SouthWest,
    /// The left side.
    West,
    /// The top left corner.
    NorthWest,
}

impl Handle {
    /// Every handle, clockwise from the top.
    pub const ALL: [Handle; 8] = [
        Handle::North,
        Handle::NorthEast,
        Handle::East,
        Handle::SouthEast,
        Handle::South,
        Handle::SouthWest,
        Handle::West,
        Handle::NorthWest,
    ];

    /// Where it sits on `frame`, as shares of its width and height.
    pub(crate) fn at(self) -> (f32, f32) {
        match self {
            Handle::North => (0.5, 0.0),
            Handle::NorthEast => (1.0, 0.0),
            Handle::East => (1.0, 0.5),
            Handle::SouthEast => (1.0, 1.0),
            Handle::South => (0.5, 1.0),
            Handle::SouthWest => (0.0, 1.0),
            Handle::West => (0.0, 0.5),
            Handle::NorthWest => (0.0, 0.0),
        }
    }
}

/// The topmost shape at a canvas point that is shown and not locked, by its place.
pub(crate) fn hit(shapes: &[Shape], at: (f32, f32), reach: f32) -> Option<usize> {
    shapes.iter().rposition(|shape| {
        let at = shape.rotate_point(at, -shape.rotation);
        let frame = &shape.frame;
        let grown = Frame::new(
            frame.x - reach,
            frame.y - reach,
            frame.w + reach * 2.0,
            frame.h + reach * 2.0,
        );
        let contains = match &shape.kind {
            ShapeKind::Line(end) | ShapeKind::Arrow(end) => {
                distance_to_segment(at,end.opposite().of(frame),end.of(frame)) <= reach + shape.appearance.map_or(1.0,|s|s.stroke_width/2.0)
            }
            ShapeKind::Path {points,..} | ShapeKind::Polyline {points,..} => {
                points.windows(2).any(|p| distance_to_segment(at,(frame.x+p[0].0,frame.y+p[0].1),(frame.x+p[1].0,frame.y+p[1].1)) <= reach + shape.appearance.map_or(2.0,|s|s.stroke_width/2.0))
            }
            ShapeKind::RoundedRect => {
                let radius = super::rounded::radius(frame);
                let (cx, cy) = frame.center();
                let x = ((at.0 - cx).abs() - (frame.w / 2.0 - radius)).max(0.0);
                let y = ((at.1 - cy).abs() - (frame.h / 2.0 - radius)).max(0.0);
                x.hypot(y) <= radius + reach
            }
            ShapeKind::Diamond => {
                let (cx, cy) = frame.center();
                (at.0 - cx).abs() / (frame.w / 2.0 + reach).max(0.01)
                    + (at.1 - cy).abs() / (frame.h / 2.0 + reach).max(0.01) <= 1.0
            }
            ShapeKind::Ellipse => {
                let (cx, cy) = frame.center();
                ((at.0 - cx) / (frame.w / 2.0 + reach).max(0.01)).powi(2)
                    + ((at.1 - cy) / (frame.h / 2.0 + reach).max(0.01)).powi(2) <= 1.0
            }
            _ => grown.contains(at),
        };
        !shape.hidden && !shape.locked && contains
    })
}

fn distance_to_segment(p:(f32,f32),a:(f32,f32),b:(f32,f32)) -> f32 {
    let (dx,dy)=(b.0-a.0,b.1-a.1);
    let t=(((p.0-a.0)*dx+(p.1-a.1)*dy)/(dx*dx+dy*dy).max(0.000001)).clamp(0.0,1.0);
    (p.0-a.0-t*dx).hypot(p.1-a.1-t*dy)
}

/// The shapes wholly inside a marquee, shown and not locked.
pub(crate) fn inside(shapes: &[Shape], marquee: &Frame) -> Vec<SharedString> {
    shapes
        .iter()
        .filter(|shape| !shape.hidden && !shape.locked && shape.bounds().within(marquee))
        .map(|shape| shape.key.clone())
        .collect()
}

/// `frame` with `handle` moved by a canvas distance, never smaller than `least`; with `square`, a corner keeps the frame's shape.
pub(crate) fn resized(
    frame: Frame,
    handle: Handle,
    (dx, dy): (f32, f32),
    least: f32,
    square: bool,
) -> Frame {
    let (sx, sy) = handle.at();
    let (mut left, mut top, mut right, mut bottom) =
        (frame.x, frame.y, frame.right(), frame.bottom());
    match sx {
        0.0 => left = (left + dx).min(right - least),
        1.0 => right = (right + dx).max(left + least),
        _ => {}
    }
    match sy {
        0.0 => top = (top + dy).min(bottom - least),
        1.0 => bottom = (bottom + dy).max(top + least),
        _ => {}
    }
    let mut next = Frame::new(left, top, right - left, bottom - top);
    if square && sx != 0.5 && sy != 0.5 {
        let ratio = frame.w / frame.h.max(least);
        let h = next.w / ratio;
        if sy == 0.0 {
            next.y = next.bottom() - h;
        }
        next.h = h;
    }
    next
}

/// Resizes in rotated local axes, keeping the opposite handle fixed.
pub(crate) fn resized_shape(shape:&Shape, handle:Handle, from:(f32,f32), to:(f32,f32), least:f32, square:bool) -> Frame {
    let from=shape.rotate_point(from,-shape.rotation);
    let to=shape.rotate_point(to,-shape.rotation);
    let mut frame=resized(shape.frame,handle,(to.0-from.0,to.1-from.1),least,square);
    let center=shape.rotate_point(frame.center(),shape.rotation);
    frame.x=center.0-frame.w/2.0; frame.y=center.1-frame.h/2.0;
    frame
}

/// How far to nudge a moving frame so an edge or its middle meets another frame's within `reach`, and the lines it meets.
pub(crate) fn snapped(moving: &Frame, others: &[Frame], reach: f32) -> ((f32, f32), Vec<Guide>) {
    let lines = |frame: &Frame| {
        (
            [frame.x, frame.x + frame.w / 2.0, frame.right()],
            [frame.y, frame.y + frame.h / 2.0, frame.bottom()],
        )
    };
    let (xs, ys) = lines(moving);
    let best = |mine: [f32; 3], theirs: Vec<f32>| {
        mine.iter()
            .flat_map(|at| theirs.iter().map(move |line| (line - at, *line)))
            .filter(|(gap, _)| gap.abs() <= reach)
            .min_by(|a, b| a.0.abs().total_cmp(&b.0.abs()))
    };
    let across = best(xs, others.iter().flat_map(|frame| lines(frame).0).collect());
    let down = best(ys, others.iter().flat_map(|frame| lines(frame).1).collect());
    let mut guides = Vec::new();
    if let Some((_, x)) = across {
        guides.push(Guide::Vertical(x));
    }
    if let Some((_, y)) = down {
        guides.push(Guide::Horizontal(y));
    }
    (
        (
            across.map_or(0.0, |(gap, _)| gap),
            down.map_or(0.0, |(gap, _)| gap),
        ),
        guides,
    )
}

/// The shape a drag from `from` to `to` draws with a tool, its frame spanning both; `square` keeps it as wide as tall.
pub(crate) fn drawn(
    kind: ShapeKind,
    from: (f32, f32),
    to: (f32, f32),
    square: bool,
) -> (ShapeKind, Frame) {
    let to = match square {
        true => {
            let side = (to.0 - from.0).abs().max((to.1 - from.1).abs());
            (
                from.0 + side.copysign(to.0 - from.0),
                from.1 + side.copysign(to.1 - from.1),
            )
        }
        false => to,
    };
    let end = match (to.0 >= from.0, to.1 >= from.1) {
        (true, true) => Corner::BottomRight,
        (true, false) => Corner::TopRight,
        (false, true) => Corner::BottomLeft,
        (false, false) => Corner::TopLeft,
    };
    let kind = match kind {
        ShapeKind::Line(_) => ShapeKind::Line(end),
        ShapeKind::Arrow(_) => ShapeKind::Arrow(end),
        other => other,
    };
    (kind, Frame::spanning(from, to))
}

#[cfg(test)]
mod tests {
    use super::{Handle, drawn, hit, inside, resized, snapped};
    use crate::{Corner, Frame, Guide, Shape, ShapeKind};

    fn shapes() -> Vec<Shape> {
        vec![
            Shape::new(
                "a",
                "A",
                ShapeKind::Rect,
                Frame::new(0.0, 0.0, 100.0, 100.0),
            ),
            Shape::new(
                "b",
                "B",
                ShapeKind::Rect,
                Frame::new(50.0, 50.0, 100.0, 100.0),
            ),
            Shape::new(
                "c",
                "C",
                ShapeKind::Rect,
                Frame::new(300.0, 0.0, 50.0, 50.0),
            )
            .locked(true),
        ]
    }

    #[test]
    fn a_press_takes_the_topmost_shape_it_reaches_and_skips_the_locked() {
        assert_eq!(hit(&shapes(), (60.0, 60.0), 0.0), Some(1), "b lies over a");
        assert_eq!(
            hit(&shapes(), (-3.0, 10.0), 4.0),
            Some(0),
            "within reach of a's edge"
        );
        assert_eq!(hit(&shapes(), (320.0, 20.0), 0.0), None, "c is locked");
    }

    #[test]
    fn a_marquee_takes_what_lies_wholly_inside() {
        let keys = inside(&shapes(), &Frame::new(-10.0, -10.0, 400.0, 130.0));
        assert_eq!(keys, ["a"], "b runs past the bottom and c is locked");
    }

    #[test]
    fn a_handle_moves_its_sides_and_a_square_corner_keeps_the_shape() {
        let frame = Frame::new(0.0, 0.0, 100.0, 50.0);
        assert_eq!(
            resized(frame, Handle::East, (20.0, 99.0), 2.0, false),
            Frame::new(0.0, 0.0, 120.0, 50.0)
        );
        assert_eq!(
            resized(frame, Handle::NorthWest, (10.0, 10.0), 2.0, false),
            Frame::new(10.0, 10.0, 90.0, 40.0)
        );
        assert_eq!(
            resized(frame, Handle::West, (500.0, 0.0), 2.0, false),
            Frame::new(98.0, 0.0, 2.0, 50.0),
            "never below the least"
        );
        assert_eq!(
            resized(frame, Handle::SouthEast, (100.0, 0.0), 2.0, true),
            Frame::new(0.0, 0.0, 200.0, 100.0)
        );
    }

    #[test]
    fn a_moving_frame_snaps_its_nearest_line_within_reach() {
        let others = [Frame::new(200.0, 0.0, 100.0, 100.0)];
        let (nudge, guides) = snapped(&Frame::new(97.0, 150.0, 100.0, 30.0), &others, 5.0);
        assert_eq!(nudge, (3.0, 0.0), "its right edge meets the other's left");
        assert_eq!(guides, [Guide::Vertical(200.0)]);
        assert_eq!(
            snapped(&Frame::new(0.0, 300.0, 10.0, 10.0), &others, 5.0).0,
            (0.0, 0.0)
        );
    }

    #[test]
    fn a_drag_draws_toward_its_corner_and_shift_keeps_it_square() {
        let (kind, frame) = drawn(
            ShapeKind::Arrow(Corner::BottomRight),
            (100.0, 100.0),
            (40.0, 160.0),
            false,
        );
        assert_eq!(kind, ShapeKind::Arrow(Corner::BottomLeft));
        assert_eq!(frame, Frame::new(40.0, 100.0, 60.0, 60.0));
        let (_, square) = drawn(ShapeKind::Rect, (0.0, 0.0), (30.0, -80.0), true);
        assert_eq!(square, Frame::new(0.0, -80.0, 80.0, 80.0));
    }

    #[test]
    fn rounded_tool_draws_resizes_writes_and_hits_real_corners() {
        let kind = super::Tool::RoundedRect.draws().unwrap();
        let (kind, frame) = drawn(kind, (40.0, 40.0), (0.0, 0.0), false);
        let shape = Shape::new("round", "Round", kind, frame).written("Words");
        assert_eq!(shape.words().map(|s| s.as_ref()), Some("Words"));
        assert_eq!(hit(std::slice::from_ref(&shape), (0.0, 0.0), 0.0), None);
        assert_eq!(hit(std::slice::from_ref(&shape), (10.0, 0.0), 0.0), Some(0));
        let rotated = shape.clone().rotation(std::f32::consts::FRAC_PI_4);
        assert_eq!(hit(std::slice::from_ref(&rotated), rotated.rotate_point((0.0, 0.0), rotated.rotation), 0.0), None);
        let next = super::resized_shape(&shape, Handle::SouthEast, (40.0, 40.0), (80.0, 60.0), 1.0, false);
        assert_eq!(next, Frame::new(0.0, 0.0, 80.0, 60.0));
        assert_eq!(inside(&[shape], &frame), [gpui::SharedString::from("round")]);
    }
}
