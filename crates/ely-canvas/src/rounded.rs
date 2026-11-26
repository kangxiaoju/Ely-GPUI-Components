use super::Frame;

pub(crate) fn radius(frame: &Frame) -> f32 {
    12.0_f32.min(frame.w.abs().min(frame.h.abs()) * 0.25)
}

/// Clockwise rounded-rectangle outline in canvas units, without a repeated closing point.
///
/// Uses radius `min(12, min(abs(width), abs(height)) * 0.25)` and eight deterministic
/// segments per quarter-circle, including exact tangent endpoints. Negative extents
/// are normalized. Apply rotation, viewport and `sketch_outline` after sampling so
/// native rendering and vector export share geometry and seeded double traces.
///
/// ```
/// use ely_canvas::{Frame, rounded_rectangle_outline};
/// let points = rounded_rectangle_outline(&Frame::new(0.0, 0.0, 80.0, 40.0));
/// assert_eq!(points[0], (70.0, 0.0));
/// assert_eq!(points.len(), 36);
/// ```
pub fn rounded_rectangle_outline(frame: &Frame) -> Vec<(f32, f32)> {
    let (left, top) = (frame.x.min(frame.right()), frame.y.min(frame.bottom()));
    let (right, bottom) = (frame.x.max(frame.right()), frame.y.max(frame.bottom()));
    let radius = radius(frame);
    if radius == 0.0 {
        return vec![(left, top), (right, top), (right, bottom), (left, bottom)];
    }
    let centers = [(right - radius, top + radius), (right - radius, bottom - radius),
        (left + radius, bottom - radius), (left + radius, top + radius)];
    let mut points = Vec::with_capacity(36);
    for (corner, (cx, cy)) in centers.into_iter().enumerate() {
        for step in 0..=8 {
            let turn = step as f32 * std::f32::consts::FRAC_PI_2 / 8.0;
            let (s, c) = match step { 0 => (0.0, 1.0), 8 => (1.0, 0.0), _ => turn.sin_cos() };
            let (x, y) = match corner { 0 => (s, -c), 1 => (c, s), 2 => (-s, c), _ => (-c, -s) };
            points.push((cx + radius * x, cy + radius * y));
        }
    }
    points
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Shape, ShapeKind};

    #[test]
    fn rounded_points_use_exact_tangents_and_normalized_extents() {
        let frame = Frame::new(3.0, 5.0, 100.0, 40.0);
        let points = rounded_rectangle_outline(&frame);
        assert_eq!(points.len(), 36);
        assert_eq!(points[0], (93.0, 5.0));
        assert_eq!(points[8], (103.0, 15.0));
        assert_eq!(points[9], (103.0, 35.0));
        assert_eq!(points[35], (13.0, 5.0));
        assert!(points.iter().all(|point| frame.contains(*point)));
        assert_eq!(points, rounded_rectangle_outline(&Frame { x: 103.0, y: 45.0, w: -100.0, h: -40.0 }));
        assert_eq!(radius(&Frame::new(0.0, 0.0, 200.0, 100.0)), 12.0);
    }

    #[test]
    fn rounded_outlines_share_deterministic_standard_and_double_traces() {
        let points = rounded_rectangle_outline(&Frame::new(0.0, 0.0, 80.0, 40.0));
        let seed = crate::sketch_seed("rounded");
        assert_eq!(crate::sketch_outline(&points, true, 0.0, seed), points);
        let first = crate::sketch_outline(&points, true, 1.2, seed);
        let second = crate::sketch_outline(&points, true, 1.2, seed ^ 0x9e3779b97f4a7c15);
        assert_eq!(first, crate::sketch_outline(&points, true, 1.2, seed));
        assert_ne!(first, points);
        assert_ne!(first, second);
    }

    #[test]
    fn rounded_bindings_reach_the_corner_circle_after_rotation() {
        let shape = Shape::new("rounded", "Rounded", ShapeKind::RoundedRect, Frame::new(0.0, 0.0, 40.0, 40.0));
        let expected = 30.0 + 10.0 / 2.0_f32.sqrt();
        let point = shape.outline_point((100.0, 100.0));
        assert!((point.0 - expected).abs() < 0.0001);
        assert!((point.1 - expected).abs() < 0.0001);
        let rotated = shape.rotation(std::f32::consts::FRAC_PI_4);
        let destination = rotated.rotate_point((100.0, 100.0), rotated.rotation);
        let point = rotated.rotate_point(rotated.outline_point(destination), -rotated.rotation);
        assert!((point.0 - expected).abs() < 0.0001);
        assert!((point.1 - expected).abs() < 0.0001);
    }
}
