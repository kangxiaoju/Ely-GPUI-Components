//! Read-only, view-space node outlines on an isolated animation surface.
use std::{sync::Arc, time::Duration};

use gpui::{Context, Render, Task, Window, canvas, prelude::*};

use crate::{finish, outline, paint::patterned, style::Dash, theme::ActiveTheme};

const FRAME_INTERVAL: Duration = Duration::from_millis(34);

/// One closed outline in view pixels, with an optional moving dashed highlight.
pub struct NodeEmphasisPath {
    /// Unclosed points relative to the layer's top-left corner, already transformed by the viewport.
    pub points: Vec<(f32, f32)>,
    /// Whether this outline moves; false draws a solid, static outline.
    pub running: bool,
    /// Theme palette hue slot, shared with the corresponding node.
    pub hue: usize,
}

/// An isolated read-only node emphasis layer, paced below 30 frames per second.
///
/// Hosts supply only viewport-visible outlines in view pixels, refresh them after
/// pan/zoom or layout changes, and call `set_paths` with `visible = false` before
/// hiding or unmounting the layer. Clip the containing canvas with `overflow_hidden`.
/// Hiding, empty/static geometry and the theme's reduced-motion preference stop
/// its timer; reduced motion renders running outlines as solid, static paths.
/// Animation only notifies this entity, never its owner or a retained scene.
///
/// ```no_run
/// use ely_canvas::{Frame, NodeEmphasis, NodeEmphasisPath, rounded_rectangle_outline};
/// use gpui::{App, AppContext};
/// fn layer(cx: &mut App) -> gpui::Entity<NodeEmphasis> {
///     let layer = cx.new(|_| NodeEmphasis::new());
///     layer.update(cx, |layer, cx| layer.set_paths(vec![NodeEmphasisPath {
///         points: rounded_rectangle_outline(&Frame::new(16.0, 16.0, 160.0, 64.0)),
///         running: true,
///         hue: 2,
///     }], true, cx));
///     layer
/// }
/// ```
pub struct NodeEmphasis {
    paths: Arc<Vec<NodeEmphasisPath>>,
    visible: bool,
    timer: Option<Task<()>>,
    phase: f32,
}

fn animation_active(paths: &[NodeEmphasisPath], visible: bool, reduced_motion: bool) -> bool {
    visible && !reduced_motion && paths.iter().any(|path| path.running && path.points.len() >= 2)
}

impl NodeEmphasis {
    /// Creates an empty, inactive layer.
    pub fn new() -> Self {
        Self { paths: Arc::new(Vec::new()), visible: false, timer: None, phase: 0.0 }
    }

    /// Replaces viewport-visible paths and immediately cancels the timer when inactive.
    pub fn set_paths(&mut self, paths: Vec<NodeEmphasisPath>, visible: bool, cx: &mut Context<Self>) {
        self.paths = Arc::new(paths);
        self.visible = visible;
        self.schedule(cx);
        cx.notify();
    }

    fn schedule(&mut self, cx: &mut Context<Self>) {
        if !animation_active(&self.paths, self.visible, cx.theme().reduced_motion) {
            self.timer = None;
            return;
        }
        if self.timer.is_some() { return; }
        self.timer = Some(cx.spawn(async move |owner, cx| loop {
            cx.background_executor().timer(FRAME_INTERVAL).await;
            if !owner.update(cx, |surface, cx| {
                if !animation_active(&surface.paths, surface.visible, cx.theme().reduced_motion) {
                    surface.timer = None;
                    return false;
                }
                let period = f32::from(cx.theme().hairline() * 16.0);
                surface.phase = (surface.phase + FRAME_INTERVAL.as_secs_f32() * 24.0).rem_euclid(period);
                cx.notify();
                true
            }).unwrap_or(false) { break; }
        }));
    }
}

impl Default for NodeEmphasis {
    fn default() -> Self { Self::new() }
}

impl Render for NodeEmphasis {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.schedule(cx);
        let paths = self.paths.clone();
        let visible = self.visible;
        let moving = !cx.theme().reduced_motion;
        let phase = self.phase;
        let palette = cx.theme().colors.clone();
        let stroke = cx.theme().hairline() * 2.0;
        canvas(|_, _, _| {}, move |bounds, _, window, _| {
            if !visible { return; }
            for path in paths.iter().filter(|path| path.points.len() >= 2) {
                let ink = palette.hue(path.hue, "node emphasis");
                let running = path.running && moving;
                finish(outline(&path.points, bounds.origin, stroke, true),
                    ink.opacity(if running { 0.35 } else { 1.0 }), window);
                if running {
                    patterned(&path.points, bounds.origin, stroke, true, Dash::Dashed, phase, ink, window);
                }
            }
        }).absolute().top_0().left_0().size_full()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(running: bool) -> NodeEmphasisPath {
        NodeEmphasisPath {
            points: vec![(0.0, 0.0), (120.0, 0.0), (120.0, 60.0)],
            running,
            hue: 2,
        }
    }

    #[test]
    fn only_visible_running_geometry_schedules_frames() {
        assert!(animation_active(&[path(true)], true, false));
        assert!(!animation_active(&[path(true)], false, false));
        assert!(!animation_active(&[path(true)], true, true));
        assert!(!animation_active(&[path(false)], true, false));
        assert!(!animation_active(&[], true, false));
        let mut empty = path(true);
        empty.points.clear();
        assert!(!animation_active(&[empty], true, false));
    }

    #[test]
    fn repaint_interval_never_exceeds_thirty_fps() {
        assert!(FRAME_INTERVAL >= Duration::from_secs_f64(1.0 / 30.0));
    }

    #[cfg(feature = "test-support")]
    #[gpui::test]
    fn hiding_and_reduced_motion_drop_the_owned_timer(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| {
            crate::theme::Theme::init(cx);
            let surface = cx.new(|_| NodeEmphasis::new());
            surface.update(cx, |surface, cx| {
                surface.set_paths(vec![path(true)], true, cx);
                assert!(surface.timer.is_some());
                surface.set_paths(vec![path(true)], false, cx);
                assert!(surface.timer.is_none());
                surface.set_paths(vec![path(false)], true, cx);
                assert!(surface.timer.is_none());
                surface.set_paths(vec![path(true)], true, cx);
                assert!(surface.timer.is_some());
            });
            crate::theme::Theme::update(cx, |theme| theme.reduced_motion = true);
            surface.update(cx, |surface, cx| {
                surface.set_paths(vec![path(true)], true, cx);
                assert!(surface.timer.is_none());
            });
        });
    }

    #[cfg(feature = "test-support")]
    #[gpui::test]
    fn path_phase_advances_only_at_capped_ticks_and_stops_when_hidden(cx: &mut gpui::TestAppContext) {
        let surface = cx.update(|cx| {
            crate::theme::Theme::init(cx);
            let surface = cx.new(|_| NodeEmphasis::new());
            surface.update(cx, |surface, cx| surface.set_paths(vec![path(true)], true, cx));
            surface
        });
        cx.run_until_parked();
        cx.executor().advance_clock(Duration::from_millis(33));
        cx.run_until_parked();
        assert_eq!(cx.read(|cx| surface.read(cx).phase), 0.0);
        cx.executor().advance_clock(Duration::from_millis(1));
        cx.run_until_parked();
        let phase = cx.read(|cx| surface.read(cx).phase);
        assert!(phase > 0.0, "the dash travels along the path");
        cx.update(|cx| surface.update(cx, |surface, cx| surface.set_paths(vec![path(true)], false, cx)));
        cx.executor().advance_clock(Duration::from_secs(1));
        cx.run_until_parked();
        assert_eq!(cx.read(|cx| surface.read(cx).phase), phase);
    }
}
