use gpui::{App, Entity, FocusHandle, SharedString, Window};

use super::{
    edit::{Tool, hit},
    gesture::{Gesture, Hand, Handlers, PEN, REACH, Scene, path, spanned},
    marks::handle_boxes,
    paint::in_view,
    shape::ShapeKind,
    view::Frame,
    writing,
};
use crate::forms::Editing;

/// A press on a tool layer: the tool, where it landed in canvas units and in view pixels, how many presses it counts, whether Shift is held, a handle's side, and the layer's focus, which a field hands back.
pub(super) struct Press {
    pub tool: Tool,
    pub at: (f32, f32),
    pub view_at: (f32, f32),
    pub count: usize,
    pub shift: bool,
    pub side: f32,
    pub back: FocusHandle,
}

/// The gesture a press sets going, after what it asks the owner at once: a selection, a dropped text or note, an ended path, or words to rewrite.
pub(super) fn pressed(
    press: &Press,
    scene: &Scene,
    handlers: &Handlers,
    hand: &Entity<Hand>,
    editing: &Entity<Editing>,
    window: &mut Window,
    cx: &mut App,
) -> Gesture {
    let (at, view) = (press.at, scene.view);
    let drop = |kind: ShapeKind, frame: Frame, window: &mut Window, cx: &mut App| {
        log::info!("tool layer: {kind:?} at {at:?}");
        if let Some(on_draw) = &handlers.on_draw {
            on_draw(kind, frame, window, cx);
        }
    };
    match press.tool {
        Tool::Eraser => {
            if let Some(ix) = hit(&scene.shapes, at, REACH / view.zoom) {
                if let Some(delete) = &handlers.on_delete {
                    delete(&[scene.shapes[ix].key.clone()], window, cx);
                }
            }
            Gesture::Idle
        }
        Tool::Hand => Gesture::Idle,
        Tool::Select => {
            if scene.selected.len() == 1 {
                if let Some(shape) = scene.shapes.iter().find(|s| s.key == scene.selected[0]) {
                    let knob = shape.rotate_point((shape.frame.x + shape.frame.w/2.0, shape.frame.y - 24.0/view.zoom), shape.rotation);
                    if (at.0-knob.0).hypot(at.1-knob.1) <= press.side / view.zoom {
                        return Gesture::Rotating { key: shape.key.clone(), center: shape.frame.center(), start: shape.rotation, from: at, to: at };
                    }
                }
            }
            let held = (scene.selected.len() == 1)
                .then(|| spanned(&scene.shapes, &scene.selected))
                .flatten()
                .and_then(|frame| {
                    let unrotated = scene.shapes.iter().find(|s| s.key == scene.selected[0]).map_or(press.view_at, |s| view.to_view(s.rotate_point(at,-s.rotation)));
                    handle_boxes(&in_view(&view, &frame), press.side)
                        .into_iter()
                        .find(|(_, square)| square.contains(unrotated))
                });
            match (held, hit(&scene.shapes, at, REACH / view.zoom)) {
                (Some((handle, _)), _) => Gesture::Resizing {
                    key: scene.selected[0].clone(),
                    handle,
                    from: at,
                    to: at,
                },
                (None, Some(ix)) if press.count >= 2 => {
                    let shape = &scene.shapes[ix];
                    window.prevent_default();
                    hand.update(cx, |hand, _| hand.writing = Some(shape.key.clone()));
                    let words = shape
                        .words()
                        .map(|words| words.to_string())
                        .unwrap_or_default();
                    writing::begin(
                        editing,
                        shape.key.clone(),
                        words,
                        handlers.on_text.clone(),
                        press.back.clone(),
                        window,
                        cx,
                    );
                    Gesture::Idle
                }
                (None, Some(ix)) => {
                    let key = scene.shapes[ix].key.clone();
                    let selected = &scene.selected;
                    let next: Vec<SharedString> = match (press.shift, selected.contains(&key)) {
                        (true, true) => selected
                            .iter()
                            .filter(|each| **each != key)
                            .cloned()
                            .collect(),
                        (true, false) => selected.iter().cloned().chain([key]).collect(),
                        (false, true) => selected.to_vec(),
                        (false, false) => vec![key],
                    };
                    if next != **selected {
                        log::info!("tool layer: select {next:?}");
                        if let Some(on_select) = &handlers.on_select {
                            on_select(&next, window, cx);
                        }
                    }
                    Gesture::Moving { from: at, to: at }
                }
                (None, None) => Gesture::Marquee { from: at, to: at },
            }
        }
        Tool::Connector => match hit(&scene.shapes, at, REACH / view.zoom) {
            Some(ix) => Gesture::Linking {
                from: scene.shapes[ix].key.clone(),
                to: at,
            },
            None => Gesture::Idle,
        },
        Tool::Pen => {
            let ends = press.count >= 2;
            let points = hand.update(cx, |hand, _| {
                if !ends {
                    hand.pen.push(at);
                }
                match ends {
                    true => std::mem::take(&mut hand.pen),
                    false => Vec::new(),
                }
            });
            if ends && points.len() > 1 {
                let (kind, frame) = path(&points, PEN);
                drop(kind, frame, window, cx);
            }
            Gesture::Idle
        }
        Tool::Text => {
            drop(
                ShapeKind::Text("Text".into()),
                Frame::new(at.0, at.1, 160.0, 32.0),
                window,
                cx,
            );
            Gesture::Idle
        }
        Tool::Note => {
            drop(
                ShapeKind::Note("Note".into()),
                Frame::new(at.0, at.1, 160.0, 160.0),
                window,
                cx,
            );
            Gesture::Idle
        }
        Tool::Brush => Gesture::Stroke { points: vec![at] },
        drawer => Gesture::Drawing {
            kind: drawer.draws().expect("the rest draw shapes"),
            from: at,
            to: at,
        },
    }
}
