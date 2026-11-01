use std::sync::Arc;
use gpui::SharedString;

use super::view::Frame;
use crate::primitives::IconName;

/// What a shape is.
#[derive(Clone, Debug, PartialEq)]
pub enum ShapeKind {
    /// A rectangle.
    Rect,
    /// A diamond with vertices at the frame's side centers.
    Diamond,
    /// An embedded image; the host retains this shared resource between renders.
    Image(Arc<gpui::Image>),
    /// A connected line with frame-relative points and an optional arrow head.
    Polyline {
        /// Points relative to the frame's upper-left corner.
        points: Vec<(f32, f32)>,
        /// Draw an arrow head at the final point.
        arrow: bool,
    },
    /// An ellipse.
    Ellipse,
    /// A regular polygon of so many sides, three or more.
    Polygon(u8),
    /// From the opposite corner of the frame to this one.
    Line(Corner),
    /// A line with a head at the corner it ends at.
    Arrow(Corner),
    /// A line of text.
    Text(SharedString),
    /// Points in canvas units from the frame's top left, a line so wide.
    Path {
        /// The points, in canvas units from the frame's top left.
        points: Vec<(f32, f32)>,
        /// The line's width, in canvas units.
        width: f32,
    },
    /// A square of paper with words on it.
    Note(SharedString),
}

impl ShapeKind {
    /// Its icon in lists and grids.
    pub(crate) fn icon(&self) -> IconName {
        match self {
            ShapeKind::Rect => IconName::Square,
            ShapeKind::Diamond => IconName::Hexagon,
            ShapeKind::Image(_) => IconName::Square,
            ShapeKind::Polyline { .. } => IconName::Spline,
            ShapeKind::Ellipse => IconName::Circle,
            ShapeKind::Polygon(_) => IconName::Hexagon,
            ShapeKind::Line(_) => IconName::Slash,
            ShapeKind::Arrow(_) => IconName::MoveUpRight,
            ShapeKind::Text(_) => IconName::Type,
            ShapeKind::Path { .. } => IconName::PenTool,
            ShapeKind::Note(_) => IconName::StickyNote,
        }
    }

    /// What it is, in a word.
    pub fn word(&self) -> &'static str {
        match self {
            ShapeKind::Rect => "Rectangle",
            ShapeKind::Diamond => "Diamond",
            ShapeKind::Image(_) => "Image",
            ShapeKind::Polyline { .. } => "Polyline",
            ShapeKind::Ellipse => "Ellipse",
            ShapeKind::Polygon(_) => "Polygon",
            ShapeKind::Line(_) => "Line",
            ShapeKind::Arrow(_) => "Arrow",
            ShapeKind::Text(_) => "Text",
            ShapeKind::Path { .. } => "Path",
            ShapeKind::Note(_) => "Note",
        }
    }

    /// Its localized type name, without changing the user's layer name or text.
    pub fn words(&self, cx: &gpui::App) -> SharedString {
        let key = match self {
            Self::Rect => "rect", Self::Diamond => "diamond", Self::Image(_) => "image",
            Self::Polyline { .. } => "polyline", Self::Ellipse => "ellipse",
            Self::Polygon(_) => "polygon", Self::Line(_) => "line", Self::Arrow(_) => "arrow",
            Self::Text(_) => "text", Self::Path { .. } => "path", Self::Note(_) => "note",
        };
        crate::i18n::text(cx, &format!("canvas.kind.{key}"), &[])
    }
}

/// A frame's corner, where a line or an arrow ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Corner {
    /// The top left.
    TopLeft,
    /// The top right.
    TopRight,
    /// The bottom right.
    BottomRight,
    /// The bottom left.
    BottomLeft,
}

impl Corner {
    /// Its point on `frame`.
    pub(crate) fn of(self, frame: &Frame) -> (f32, f32) {
        match self {
            Corner::TopLeft => (frame.x, frame.y),
            Corner::TopRight => (frame.right(), frame.y),
            Corner::BottomRight => (frame.right(), frame.bottom()),
            Corner::BottomLeft => (frame.x, frame.bottom()),
        }
    }

    pub(crate) fn opposite(self) -> Corner {
        match self {
            Corner::TopLeft => Corner::BottomRight,
            Corner::TopRight => Corner::BottomLeft,
            Corner::BottomRight => Corner::TopLeft,
            Corner::BottomLeft => Corner::TopRight,
        }
    }
}

/// Font choice for canvas content, independent of the application's UI font.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CanvasFont {
    /// The theme's proportional font.
    #[default]
    Default,
    /// The host-provided Xiaolai Chinese and Latin handwritten font.
    Handwritten,
    /// The theme's monospace font.
    Monospace,
}

impl CanvasFont {
    pub(crate) fn family(self, cx: &gpui::App) -> SharedString {
        use crate::theme::ActiveTheme;
        match self {
            Self::Default => cx.theme().font_family.clone(),
            Self::Handwritten => "Xiaolai".into(),
            Self::Monospace => cx.theme().mono_family.clone(),
        }
    }
}

/// Theme-relative appearance shared by committed shapes and tool previews.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShapeAppearance {
    /// Stroke hue slot.
    pub stroke: usize,
    /// Optional fill hue slot; `None` leaves the interior transparent.
    pub fill: Option<usize>,
    /// Stroke width in canvas units.
    pub stroke_width: f32,
    /// Overall alpha from zero to one.
    pub opacity: f32,
    /// Deterministic hand-drawn displacement in canvas units.
    pub roughness: f32,
    /// Stroke pattern.
    pub dash: super::style::Dash,
    /// Font size in canvas units.
    pub font_size: f32,
    /// Font family used by shape text, labels and writing previews.
    pub font: CanvasFont,
}

impl Default for ShapeAppearance {
    fn default() -> Self {
        Self { stroke: 0, fill: None, stroke_width: 2.0, opacity: 1.0,
            roughness: 0.0, dash: super::style::Dash::Solid, font_size: 16.0, font: CanvasFont::Default }
    }
}

/// Something on a canvas: its key, its name, what it is, its frame, its hue among the chart colors, and whether it hides or is locked.
#[derive(Clone, Debug, PartialEq)]
pub struct Shape {
    /// Tells it apart from the others.
    pub key: SharedString,
    /// Its name in the layers.
    pub name: SharedString,
    /// What it is.
    pub kind: ShapeKind,
    /// Its box on the canvas.
    pub frame: Frame,
    /// Its place among the chart colors.
    pub hue: usize,
    /// Whether it is hidden.
    pub hidden: bool,
    /// Whether it is locked.
    pub locked: bool,
    /// Words shown in its middle.
    pub label: Option<SharedString>,
    /// Explicit appearance, or the original theme-derived hue appearance.
    pub appearance: Option<ShapeAppearance>,
    /// Clockwise rotation about the frame center, in radians.
    pub rotation: f32,
}

impl Shape {
    /// A shape keyed `key`, named `name`, of `kind` in `frame`: the first hue, shown, unlocked, unlabeled; panics on a polygon of fewer than three sides.
    pub fn new(
        key: impl Into<SharedString>,
        name: impl Into<SharedString>,
        kind: ShapeKind,
        frame: Frame,
    ) -> Self {
        if let ShapeKind::Polygon(sides) = kind {
            assert!(sides >= 3, "a polygon of {sides} sides");
        }
        Self {
            key: key.into(),
            name: name.into(),
            kind,
            frame,
            hue: 0,
            hidden: false,
            locked: false,
            label: None,
            appearance: None,
            rotation: 0.0,
        }
    }

    /// Its place among the chart colors.
    pub fn hue(mut self, hue: usize) -> Self {
        self.hue = hue;
        self
    }

    /// Sets the complete theme-relative appearance.
    pub fn appearance(mut self, appearance: ShapeAppearance) -> Self {
        self.hue = appearance.stroke;
        self.appearance = Some(appearance);
        self
    }

    /// Sets clockwise rotation about the frame center, in radians.
    pub fn rotation(mut self, radians: f32) -> Self {
        self.rotation = radians;
        self
    }

    /// Rotates a canvas point about this shape's center.
    pub fn rotate_point(&self, at: (f32, f32), angle: f32) -> (f32, f32) {
        let (cx, cy) = self.frame.center();
        let (s, c) = angle.sin_cos();
        let (x, y) = (at.0 - cx, at.1 - cy);
        (cx + x * c - y * s, cy + x * s + y * c)
    }

    /// Axis-aligned canvas bounds after rotation.
    pub fn bounds(&self) -> Frame {
        let (s, c) = self.rotation.sin_cos();
        let w = self.frame.w * c.abs() + self.frame.h * s.abs();
        let h = self.frame.w * s.abs() + self.frame.h * c.abs();
        let (x, y) = self.frame.center();
        Frame::new(x - w / 2.0, y - h / 2.0, w, h)
    }

    /// Point on the rotated outline facing a canvas-space destination.
    pub fn outline_point(&self, destination: (f32,f32)) -> (f32,f32) {
        let local = self.rotate_point(destination, -self.rotation);
        let (cx,cy) = self.frame.center();
        let (dx,dy) = (local.0-cx,local.1-cy);
        let (rx,ry) = ((self.frame.w/2.0).max(0.001),(self.frame.h/2.0).max(0.001));
        if dx.hypot(dy) < 0.000001 { return self.rotate_point((cx+rx,cy),self.rotation); }
        let divisor = match self.kind {
            ShapeKind::Ellipse => ((dx/rx).powi(2)+(dy/ry).powi(2)).sqrt(),
            ShapeKind::Diamond => (dx/rx).abs()+(dy/ry).abs(),
            _ => (dx/rx).abs().max((dy/ry).abs()),
        }.max(0.001);
        self.rotate_point((cx+dx/divisor,cy+dy/divisor),self.rotation)
    }

    /// Whether it is hidden.
    pub fn hidden(mut self, hidden: bool) -> Self {
        self.hidden = hidden;
        self
    }

    /// Whether it is locked.
    pub fn locked(mut self, locked: bool) -> Self {
        self.locked = locked;
        self
    }

    /// Words on it.
    pub fn label(mut self, text: impl Into<SharedString>) -> Self {
        self.label = Some(text.into());
        self
    }

    /// The words it shows: a text's or a note's own, or its label.
    pub fn words(&self) -> Option<&SharedString> {
        match &self.kind {
            ShapeKind::Text(text) | ShapeKind::Note(text) => Some(text),
            _ => self.label.as_ref(),
        }
    }

    /// The shape showing `text`: a text or a note says it, and anything else wears it as its label, or none when it is empty.
    pub fn written(&self, text: impl Into<SharedString>) -> Shape {
        let text = text.into();
        let mut next = self.clone();
        match &mut next.kind {
            ShapeKind::Text(words) | ShapeKind::Note(words) => *words = text,
            _ => next.label = (!text.is_empty()).then_some(text),
        }
        next
    }
}

/// A named board on the canvas that designs sit on.
#[derive(Clone, Debug, PartialEq)]
pub struct Artboard {
    /// Tells it apart from the others.
    pub key: SharedString,
    /// Its name, shown above it.
    pub name: SharedString,
    /// Its box on the canvas.
    pub frame: Frame,
}

impl Artboard {
    /// A board keyed `key`, named `name`, in `frame`.
    pub fn new(key: impl Into<SharedString>, name: impl Into<SharedString>, frame: Frame) -> Self {
        Self {
            key: key.into(),
            name: name.into(),
            frame,
        }
    }
}

/// A line across the canvas to line things up with.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Guide {
    /// At this canvas x.
    Vertical(f32),
    /// At this canvas y.
    Horizontal(f32),
}

/// The corners of a regular polygon of `sides` in `frame`, the first at the top.
pub(crate) fn polygon(frame: &Frame, sides: u8) -> Vec<(f32, f32)> {
    let (cx, cy) = frame.center();
    (0..sides)
        .map(|ix| {
            let turn = std::f32::consts::TAU * f32::from(ix) / f32::from(sides)
                - std::f32::consts::FRAC_PI_2;
            (
                cx + frame.w / 2.0 * turn.cos(),
                cy + frame.h / 2.0 * turn.sin(),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{Frame, polygon};

    #[test]
    fn a_polygon_starts_at_the_top_and_goes_round() {
        let corners = polygon(&Frame::new(0.0, 0.0, 100.0, 100.0), 4);
        let round = |(x, y): (f32, f32)| (x.round(), y.round());
        assert_eq!(
            corners.into_iter().map(round).collect::<Vec<_>>(),
            [(50.0, 0.0), (100.0, 50.0), (50.0, 100.0), (0.0, 50.0)]
        );
    }
}
