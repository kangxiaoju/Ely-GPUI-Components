use std::{collections::HashMap, sync::Arc};
use gpui::SharedString;

use super::view::Frame;

/// A line from one shape to another that follows them as they move: straight or turning at right angles, with an arrow at its end and words at its middle.
#[derive(Clone, Debug, PartialEq)]
pub struct Link {
    /// Tells it apart from the others.
    pub key: SharedString,
    /// The key of the shape it leaves.
    pub from: SharedString,
    /// The key of the shape it reaches.
    pub to: SharedString,
    /// Its words at its middle.
    pub label: Option<SharedString>,
    /// Whether it turns at right angles.
    pub elbow: bool,
}

impl Link {
    /// A straight link keyed `key` from `from` to `to`; panics when both are one shape.
    pub fn new(
        key: impl Into<SharedString>,
        from: impl Into<SharedString>,
        to: impl Into<SharedString>,
    ) -> Self {
        let (from, to) = (from.into(), to.into());
        assert!(from != to, "a link from {from} to itself");
        Self {
            key: key.into(),
            from,
            to,
            label: None,
            elbow: false,
        }
    }

    /// Its words at its middle.
    pub fn label(mut self, text: impl Into<SharedString>) -> Self {
        self.label = Some(text.into());
        self
    }

    /// Turns at right angles instead of running straight.
    pub fn elbow(mut self) -> Self {
        self.elbow = true;
        self
    }
}

/// Prepared world-space geometry and stroke style, independent of flow animation.
#[derive(Clone, Debug, PartialEq)]
pub struct LinkPresentation {
    /// The link to override.
    pub key: SharedString,
    /// Full route in canvas coordinates, including endpoint attachments.
    pub points: Vec<(f32,f32)>,
    /// Theme hue slot.
    pub stroke: usize,
    /// Width in canvas units.
    pub stroke_width: f32,
    /// Static or animated stroke pattern.
    pub dash: super::Dash,
    /// Arrow head at the route's first point.
    pub start_arrow: bool,
    /// Arrow head at the route's last point.
    pub end_arrow: bool,
}

/// The moving pattern painted along a connector.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlowMode {
    /// Moving dashes.
    Dashes,
    /// Moving dots.
    Dots,
}

/// Travel relative to a link's source and destination.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlowDirection {
    /// From source to destination.
    Forward,
    /// From destination to source.
    Reverse,
}

/// Independent presentation keyed by link; existing `Link` literals remain valid.
#[derive(Clone, Debug, PartialEq)]
pub struct LinkAnimation {
    /// Link key.
    pub key: SharedString,
    /// Moving pattern.
    pub mode: FlowMode,
    /// Travel direction.
    pub direction: FlowDirection,
    /// Canvas units per second.
    pub speed: f32,
    /// Freezes the pattern without requesting frames.
    pub paused: bool,
    /// Stroke hue slot.
    pub hue: usize,
    /// Paints an intermittent luminous particle traveling along the entire route.
    pub glow: bool,
}

/// Immutable shared scene. Indexing and binding routes are computed once per content revision,
/// never by an animation frame. Retain its `Arc` until document content changes.
pub struct CanvasScene {
    /// Shapes in paint order.
    pub shapes: Arc<Vec<super::shape::Shape>>,
    /// Connectors in paint order.
    pub links: Arc<Vec<Link>>,
    pub(crate) routes: Vec<Option<Vec<(f32, f32)>>>,
    pub(crate) images: HashMap<SharedString, Arc<gpui::Image>>,
    pub(crate) presentations: HashMap<SharedString, LinkPresentation>,
}

impl CanvasScene {
    /// Builds the shape index and bound connector routes once.
    pub fn new(shapes: impl IntoIterator<Item = super::shape::Shape>, links: impl IntoIterator<Item = Link>) -> Self {
        let shapes: Vec<_> = shapes.into_iter().collect();
        let links: Vec<_> = links.into_iter().collect();
        let index: HashMap<_, _> = shapes.iter().map(|s| (s.key.clone(), s)).collect();
        let routes = links.iter().map(|link| {
            let (from,to) = (index.get(&link.from)?, index.get(&link.to)?);
            if from.hidden || to.hidden { return None; }
            let (a,b)=(from.frame.center(),to.frame.center());
            Some(if link.elbow {
                let start=from.outline_point((a.0+if b.0>=a.0 {1.0} else {-1.0},a.1));
                let end=to.outline_point((b.0+if a.0>=b.0 {1.0} else {-1.0},b.1));
                let x=(a.0+b.0)/2.0;
                vec![start,(x,start.1),(x,end.1),end]
            } else { vec![from.outline_point(b),to.outline_point(a)] })
        }).collect();
        let images = shapes.iter().filter_map(|shape| {
            let super::shape::ShapeKind::Image(image) = &shape.kind else { return None; };
            if shape.rotation == 0.0 { return Some((shape.key.clone(), image.clone())); }
            use base64::Engine as _;
            let bounds = shape.bounds();
            let svg = format!("<svg xmlns='http://www.w3.org/2000/svg' xmlns:xlink='http://www.w3.org/1999/xlink' width='{}' height='{}'><g transform='translate({} {}) rotate({})'><image x='{}' y='{}' width='{}' height='{}' xlink:href='data:{};base64,{}'/></g></svg>",
                bounds.w, bounds.h, bounds.w/2.0, bounds.h/2.0, shape.rotation.to_degrees(),
                -shape.frame.w/2.0, -shape.frame.h/2.0, shape.frame.w, shape.frame.h,
                image.format.mime_type(), base64::engine::general_purpose::STANDARD.encode(&image.bytes));
            Some((shape.key.clone(), Arc::new(gpui::Image::from_bytes(gpui::ImageFormat::Svg, svg.into_bytes()))))
        }).collect();
        Self { shapes: Arc::new(shapes), links: Arc::new(links), routes, images, presentations: HashMap::new() }
    }

    /// Replaces routes and styles by link key; empty routes intentionally hide a link.
    /// Missing or hidden endpoint shapes remain invisible. `Link` literals are unchanged.
    pub fn with_link_presentations(mut self, presentations: impl IntoIterator<Item=LinkPresentation>) -> Self {
        self.presentations.extend(presentations.into_iter().map(|p|(p.key.clone(),p)));
        for (link,route) in self.links.iter().zip(&mut self.routes) {
            if let Some(p)=self.presentations.get(&link.key) {
                let visible=|key:&SharedString|self.shapes.iter().any(|shape|shape.key==*key && !shape.hidden);
                *route=(visible(&link.from) && visible(&link.to) && p.points.len()>=2).then(||p.points.clone());
            }
        }
        self
    }

    /// Topmost prepared connector within `reach` canvas units of its actual stroke.
    pub fn hit_link(&self, at:(f32,f32), reach:f32) -> Option<&SharedString> {
        self.links.iter().zip(&self.routes).rev().find_map(|(link,route)| {
            let width=self.presentations.get(&link.key).map_or(2.0,|p|p.stroke_width);
            route.as_ref()?.windows(2).any(|p| super::edit::distance_to_segment(at,p[0],p[1])<=reach+width*0.5).then_some(&link.key)
        })
    }
}

/// The points a link runs through, in canvas units: out of the side of `from` that faces `to`, into the side of `to` that faces back. An elbow turns halfway between.
pub(crate) fn route(from: &Frame, to: &Frame, elbow: bool) -> Vec<(f32, f32)> {
    let (a, b) = (from.center(), to.center());
    let across = (b.0 - a.0).abs() >= (b.1 - a.1).abs();
    let (exit, entry) = match (across, b.0 >= a.0, b.1 >= a.1) {
        (true, true, _) => ((from.right(), a.1), (to.x, b.1)),
        (true, false, _) => ((from.x, a.1), (to.right(), b.1)),
        (false, _, true) => ((a.0, from.bottom()), (b.0, to.y)),
        (false, _, false) => ((a.0, from.y), (b.0, to.bottom())),
    };
    match (elbow, across) {
        (false, _) => vec![exit, entry],
        (true, true) => {
            let middle = (exit.0 + entry.0) / 2.0;
            vec![exit, (middle, exit.1), (middle, entry.1), entry]
        }
        (true, false) => {
            let middle = (exit.1 + entry.1) / 2.0;
            vec![exit, (exit.0, middle), (entry.0, middle), entry]
        }
    }
}

/// The point halfway along a run of points.
pub(crate) fn halfway(points: &[(f32, f32)]) -> (f32, f32) {
    let length =
        |(a, b): (&(f32, f32), &(f32, f32))| ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
    let whole: f32 = points.iter().zip(&points[1..]).map(length).sum();
    let mut left = whole / 2.0;
    for (a, b) in points.iter().zip(&points[1..]) {
        let span = length((a, b));
        if left <= span && span > 0.0 {
            let share = left / span;
            return (a.0 + (b.0 - a.0) * share, a.1 + (b.1 - a.1) * share);
        }
        left -= span;
    }
    *points.last().expect("a run has points")
}

#[cfg(test)]
mod tests {
    use super::{halfway, route};
    use crate::Frame;

    #[test]
    fn a_link_leaves_the_side_that_faces_the_other_shape() {
        let (a, b) = (
            Frame::new(0.0, 0.0, 100.0, 50.0),
            Frame::new(200.0, 100.0, 100.0, 50.0),
        );
        assert_eq!(route(&a, &b, false), [(100.0, 25.0), (200.0, 125.0)]);
        assert_eq!(route(&b, &a, false), [(200.0, 125.0), (100.0, 25.0)]);
        let below = Frame::new(0.0, 300.0, 100.0, 50.0);
        assert_eq!(route(&a, &below, false), [(50.0, 50.0), (50.0, 300.0)]);
    }

    #[test]
    fn an_elbow_turns_halfway() {
        let (a, b) = (
            Frame::new(0.0, 0.0, 100.0, 50.0),
            Frame::new(200.0, 100.0, 100.0, 50.0),
        );
        assert_eq!(
            route(&a, &b, true),
            [(100.0, 25.0), (150.0, 25.0), (150.0, 125.0), (200.0, 125.0)]
        );
        assert_eq!(halfway(&route(&a, &b, true)), (150.0, 75.0));
    }
}
