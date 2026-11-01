use std::{rc::Rc, sync::Arc};

use gpui::{
    App, AppContext as _, Bounds, ElementId, EmptyView, Entity, EntityId, InteractiveElement,
    IntoElement, MouseButton, ParentElement, Pixels, Point, RenderOnce, Role, SharedString,
    StatefulInteractiveElement, Styled, Window, canvas, div, prelude::*,
};

use super::{
    edit::{Tool, drawn, snapped},
    gesture::{
        Brush, Gesture, Hand, Handlers, LEAST, OnDraw, OnKeys, OnMove, OnPair, OnResize, PEN,
        REACH, Scene, commit, path, spanned, stroke_shape, taken,
    },
    marks::{SelectionBox, SnapIndicator, TransformHandles, placed},
    paint::{in_view, paint_shape},
    press::{Press, pressed as pressed_with},
    shape::{Corner, Shape, ShapeKind, ShapeAppearance},
    view::{Frame, Viewport},
    writing,
};
use crate::{forms::Editing, i18n, primitives::tab_stop, theme::ActiveTheme};

/// A drag on a tool layer, and whose layer it is.
struct Tracing {
    owner: EntityId,
}

/// A layer over an InfiniteCanvas where its tools work. Select presses a shape, Shift adds to the selection, a drag moves it and snaps its edges to others, a handle resizes it, and a drag on empty space draws a SelectionBox. The shape tools draw by a drag, Shift keeping them square; the pen adds a point per press and a double press ends the path; text drops a line where pressed; the brush strokes a path; a note drops where pressed; the connector links the shape pressed to the one released on. A double press with Select writes on a shape: a text's or a note's words, or any other's label. A press focuses the layer, where Delete or Backspace asks to delete the selection and Escape lets it go. A drag may end anywhere, in the layer or out of it. Everything asks the owner; the hand leaves the canvas to pan.
///
/// ```
/// use ely_canvas::{Frame, Shape, ShapeKind, Tool, ToolLayer, Viewport};
///
/// let shape = Shape::new("box", "Box", ShapeKind::Rect, Frame::new(10.0, 10.0, 80.0, 60.0));
/// let layer = ToolLayer::new("tools", Tool::Select, Viewport::new(0.0, 0.0, 1.0), [shape])
///     .on_draw(|_kind, _frame, _, _| {})
///     .on_move(|_keys, _by, _, _| {});
/// ```
#[derive(IntoElement)]
pub struct ToolLayer {
    id: ElementId,
    tool: Tool,
    viewport: Viewport,
    shapes: Arc<Vec<Shape>>,
    selected: Vec<SharedString>,
    brush: Brush,
    appearance: Option<ShapeAppearance>,
    on_select: Option<OnKeys>,
    on_draw: Option<OnDraw>,
    on_move: Option<OnMove>,
    on_resize: Option<OnResize>,
    on_link: Option<OnPair>,
    on_text: Option<OnPair>,
    on_delete: Option<OnKeys>,
    on_rotate: Option<super::gesture::OnRotate>,
}

impl ToolLayer {
    /// A layer under `id` for `tool`, seen through `viewport`, over `shapes`, nothing selected.
    pub fn new(
        id: impl Into<ElementId>,
        tool: Tool,
        viewport: Viewport,
        shapes: impl IntoIterator<Item = Shape>,
    ) -> Self {
        Self {
            id: id.into(),
            tool,
            viewport,
            shapes: Arc::new(shapes.into_iter().collect()),
            selected: Vec::new(),
            brush: Brush { size: 4.0, hue: 0 },
            appearance: None,
            on_select: None,
            on_draw: None,
            on_move: None,
            on_resize: None,
            on_link: None,
            on_text: None,
            on_delete: None,
            on_rotate: None,
        }
    }

    /// The shapes selected now, by key.
    pub fn selected(mut self, keys: impl IntoIterator<Item = impl Into<SharedString>>) -> Self {
        self.selected = keys.into_iter().map(Into::into).collect();
        self
    }

    /// The brush the brush tool strokes with.
    pub fn brush(mut self, brush: Brush) -> Self {
        self.brush = brush;
        self
    }

    /// Matches drawing previews to the appearance committed by the host.
    pub fn appearance(mut self, appearance: ShapeAppearance) -> Self {
        self.appearance = Some(appearance);
        self
    }

    /// Uses the scene's shared shapes without cloning the document each frame.
    pub fn shared_shapes(mut self, shapes: Arc<Vec<Shape>>) -> Self { self.shapes = shapes; self }

    /// Gets clockwise radians after dragging a shape's rotation handle.
    pub fn on_rotate(mut self, handler: impl Fn(&SharedString, f32, &mut Window, &mut App) + 'static) -> Self {
        self.on_rotate = Some(Rc::new(handler)); self
    }

    /// Gets the keys selected after each press or marquee.
    pub fn on_select(
        mut self,
        handler: impl Fn(&[SharedString], &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    /// Gets a new shape's kind and frame, for the owner to key and keep.
    pub fn on_draw(
        mut self,
        handler: impl Fn(ShapeKind, Frame, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_draw = Some(Rc::new(handler));
        self
    }

    /// Gets the selection's keys and how far it moved, snaps included.
    pub fn on_move(
        mut self,
        handler: impl Fn(&[SharedString], (f32, f32), &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_move = Some(Rc::new(handler));
        self
    }

    /// Gets a resized shape's key and new frame.
    pub fn on_resize(
        mut self,
        handler: impl Fn(&SharedString, Frame, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_resize = Some(Rc::new(handler));
        self
    }

    /// Gets the keys of two shapes the connector joined, from first.
    pub fn on_link(
        mut self,
        handler: impl Fn(&SharedString, &SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_link = Some(Rc::new(handler));
        self
    }

    /// Gets the selected keys when Delete or Backspace is pressed on the layer.
    pub fn on_delete(
        mut self,
        handler: impl Fn(&[SharedString], &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_delete = Some(Rc::new(handler));
        self
    }

    /// Gets a shape's key and the words written on it after a double press.
    pub fn on_text(
        mut self,
        handler: impl Fn(&SharedString, &SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_text = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for ToolLayer {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let hand: Entity<Hand> =
            window.use_keyed_state((self.id.clone(), "hand"), cx, |_, _| Hand::default());
        let editing =
            window.use_keyed_state((self.id.clone(), "writing"), cx, |_, _| Editing::default());
        if self.tool != Tool::Pen && !hand.read(cx).pen.is_empty() {
            let points = hand.update(cx, |hand, _| std::mem::take(&mut hand.pen));
            end_path(points, self.on_draw.clone(), window, cx);
        }
        if self.tool == Tool::Hand {
            return div().into_any_element();
        }
        let (view, tool) = (self.viewport, self.tool);
        let shapes = self.shapes;
        let selected = Rc::new(self.selected);
        let others: Vec<Frame> = shapes
            .iter()
            .filter(|shape| !selected.contains(&shape.key))
            .map(|shape| shape.frame)
            .collect();
        let whole = spanned(&shapes, &selected);
        let state = hand.read(cx);
        let gesture = state.gesture.clone();
        let square = state.square;
        let pen = state.pen.clone();
        let theme = cx.theme();
        let (palette, stroke) = (
            theme.colors.clone(),
            theme.canvas().stroke.to_pixels(window.rem_size()),
        );
        let side = f32::from(theme.canvas().handle.to_pixels(window.rem_size()));
        let (nudge, guides) = match &gesture {
            Gesture::Moving { from, to } => {
                let moved = whole.map(|frame| {
                    Frame::new(
                        frame.x + to.0 - from.0,
                        frame.y + to.1 - from.1,
                        frame.w,
                        frame.h,
                    )
                });
                moved.map_or(((0.0, 0.0), Vec::new()), |moved| {
                    snapped(&moved, &others, REACH / view.zoom)
                })
            }
            _ => ((0.0, 0.0), Vec::new()),
        };
        let mut preview: Vec<Shape> = match &gesture {
            Gesture::Rotating { key, center, start, from, to } => shapes.iter().filter(|s| s.key == *key).map(|s| {
                let mut s = s.clone();
                s.rotation = start + (to.1-center.1).atan2(to.0-center.0) - (from.1-center.1).atan2(from.0-center.0);
                s
            }).collect(),
            Gesture::Drawing { kind, from, to } => {
                let (kind, frame) = drawn(kind.clone(), *from, *to, square);
                vec![Shape::new("+draft", "Draft", kind, frame)]
            }
            Gesture::Moving { from, to } => shapes
                .iter()
                .filter(|shape| selected.contains(&shape.key))
                .map(|shape| {
                    let mut moved = shape.clone();
                    moved.frame.x += to.0 - from.0 + nudge.0;
                    moved.frame.y += to.1 - from.1 + nudge.1;
                    moved
                })
                .collect(),
            Gesture::Resizing {
                key,
                handle,
                from,
                to,
            } => shapes
                .iter()
                .filter(|shape| shape.key == *key)
                .map(|shape| {
                    let mut sized = shape.clone();
                    sized.frame = super::edit::resized_shape(shape, *handle, *from, *to, LEAST, square);
                    sized
                })
                .collect(),
            Gesture::Stroke { points } if points.len() > 1 => {
                vec![stroke_shape(points, self.brush.size, self.brush.hue)]
            }
            Gesture::Linking { from, to } => match shapes.iter().find(|shape| shape.key == *from) {
                Some(start) => {
                    let start = start.frame.center();
                    let (kind, frame) =
                        drawn(ShapeKind::Arrow(Corner::BottomRight), start, *to, false);
                    vec![Shape::new("+link", "Link", kind, frame)]
                }
                None => {
                    log::error!("tool layer: the link's shape {from} left the canvas");
                    Vec::new()
                }
            },
            _ if tool == Tool::Pen && pen.len() > 1 => {
                vec![stroke_shape(&pen, PEN, self.brush.hue)]
            }
            _ => Vec::new(),
        };
        if let Some(appearance) = self.appearance {
            for shape in &mut preview {
                if shape.key.as_ref().starts_with('+') {
                    *shape = shape.clone().appearance(appearance);
                }
            }
        }
        let preview_scene=super::links::CanvasScene::new(preview.clone(),[]);
        let image_preview:Vec<_>=preview.iter().filter_map(|shape| {
            let image=preview_scene.images.get(&shape.key)?;
            let frame=in_view(&view,&shape.bounds());
            Some(gpui::img(image.clone()).absolute().left(Pixels::from(frame.x)).top(Pixels::from(frame.y))
                .w(Pixels::from(frame.w)).h(Pixels::from(frame.h)).opacity(shape.appearance.map_or(1.0,|s|s.opacity)))
        }).collect();
        let text_preview:Vec<_>=preview.iter().filter_map(|shape| {
            let text=shape.words()?;
            let frame=in_view(&view,&shape.frame);
            let style=shape.appearance.unwrap_or_default();
            let family=style.font.family(cx);
            if shape.rotation != 0.0 {
                let escaped=|text:&str|text.replace('&',"&amp;").replace('<',"&lt;").replace('>',"&gt;").replace('"',"&quot;").replace('\'',"&apos;");
                let spans=text.lines().enumerate().map(|(ix,line)|format!("<tspan x='0' y='{}'>{}</tspan>",style.font_size*(1.0+ix as f32*1.3),escaped(line))).collect::<String>();
                let svg=format!("<svg xmlns='http://www.w3.org/2000/svg' width='{}' height='{}'><text font-family='{}' font-size='{}'>{}</text></svg>",shape.frame.w,shape.frame.h,escaped(&family),style.font_size,spans);
                return Some(gpui::svg().data(svg.as_bytes()).with_transformation(gpui::Transformation::rotate(gpui::radians(shape.rotation)))
                    .absolute().left(Pixels::from(frame.x)).top(Pixels::from(frame.y)).w(Pixels::from(frame.w)).h(Pixels::from(frame.h)).text_color(palette.fg).opacity(style.opacity).into_any_element());
            }
            Some(placed(&frame).overflow_hidden().font_family(family).text_size(Pixels::from(style.font_size*view.zoom)).line_height(Pixels::from(style.font_size*view.zoom*1.3))
                .text_color(palette.fg).opacity(style.opacity).child(text.to_string()).into_any_element())
        }).collect();
        let marquee = match &gesture {
            Gesture::Marquee { from, to } => Some(SelectionBox::new(in_view(
                &view,
                &Frame::spanning(*from, *to),
            ))),
            _ => None,
        };
        let chosen = (tool == Tool::Select && matches!(gesture, Gesture::Idle))
            .then_some(whole)
            .flatten()
            .map(|frame| in_view(&view, &frame));
        let handles = chosen
            .filter(|_| selected.len() == 1)
            .map(|frame| TransformHandles::new(frame).rotation(shapes.iter().find(|s| s.key == selected[0]).map_or(0.0,|s|s.rotation)));
        let rotation_handle = (tool == Tool::Select && selected.len() == 1).then(|| {
            let shape = shapes.iter().find(|s| s.key == selected[0])?;
            let knob = shape.rotate_point((shape.frame.x + shape.frame.w/2.0, shape.frame.y - 24.0/view.zoom), shape.rotation);
            let (x,y) = view.to_view(knob);
            Some(placed(&Frame::new(x-side/2.0,y-side/2.0,side,side)).rounded_full()
                .bg(palette.surface).border_1().border_color(palette.accent))
        }).flatten();
        let outline = chosen
            .filter(|_| selected.len() > 1)
            .map(|frame| placed(&frame).border_1().border_color(palette.accent));
        let snap = (!guides.is_empty()).then(|| SnapIndicator::new(guides.clone(), view));
        let writing_on = hand.read(cx).writing.clone();
        let field = writing_on
            .as_ref()
            .and_then(|key| shapes.iter().find(|shape| shape.key == *key))
            .and_then(|shape| writing::field(&editing, in_view(&view, &shape.frame), shape.appearance.unwrap_or_default(),view.zoom,cx));
        if field.is_none() && writing_on.is_some() && editing.read(cx).field().is_some() {
            let closing = editing.clone();
            log::info!("tool layer: the shape written on left the canvas");
            window.defer(cx, move |window, cx| {
                closing.update(cx, |editing, cx| editing.finish(true, window, cx))
            });
        }
        let point_of = move |at: Point<Pixels>, bounds: Bounds<Pixels>| {
            let local = at - bounds.origin;
            view.to_canvas((f32::from(local.x), f32::from(local.y)))
        };
        let owner = hand.entity_id();
        let focus = tab_stop((self.id.clone(), "focus").into(), true, window, cx);
        let (ring, held_focus) = (
            if focus.is_focused(window) {
                palette.focus
            } else {
                gpui::transparent_black()
            },
            focus.clone(),
        );
        let (gone, on_delete, let_go) = (
            selected.clone(),
            self.on_delete.clone(),
            self.on_select.clone(),
        );
        let (pressed, moved, dropped, released) =
            (hand.clone(), hand.clone(), hand.clone(), hand.clone());
        let handlers = Handlers {
            on_select: self.on_select,
            on_draw: self.on_draw,
            on_move: self.on_move,
            on_resize: self.on_resize,
            on_link: self.on_link,
            on_text: self.on_text,
            on_delete: self.on_delete,
            on_rotate: self.on_rotate,
        };
        let scene = Rc::new(Scene {
            shapes,
            selected,
            others,
            view,
        });
        let handlers = Rc::new(handlers);
        let select_on_up = handlers.on_select.clone();
        let (press_scene, press_handlers, press_editing) =
            (scene.clone(), handlers.clone(), editing.clone());
        let erasing_scene = press_scene.clone();
        let erasing_handlers = press_handlers.clone();
        let brush = self.brush;
        let (out_hand, out_scene, out_handlers) = (hand.clone(), scene.clone(), handlers.clone());
        let measured = hand.clone();
        div()
            .id(self.id)
            .role(Role::Canvas)
            .aria_label(i18n::text(cx, "canvas.board", &[]))
            .track_focus(&focus)
            .absolute()
            .inset_0()
            .child(div().absolute().inset_0().border_1().border_color(ring))
            .when(tool != Tool::Select, |layer| layer.cursor_crosshair())
            .on_key_down(
                move |event, window, cx| match event.keystroke.key.as_str() {
                    "backspace" | "delete" if !gone.is_empty() => {
                        cx.stop_propagation();
                        log::info!("tool layer: delete {gone:?}");
                        if let Some(on_delete) = &on_delete {
                            on_delete(&gone, window, cx);
                        }
                    }
                    "escape" if !gone.is_empty() => {
                        cx.stop_propagation();
                        log::info!("tool layer: nothing selected");
                        if let Some(on_select) = &let_go {
                            on_select(&[], window, cx);
                        }
                    }
                    _ => {}
                },
            )
            .on_mouse_down(MouseButton::Left, move |event, window, cx| {
                cx.stop_propagation();
                window.focus(&held_focus, cx);
                let bounds = pressed.read(cx).bounds;
                let at = point_of(event.position, bounds);
                let local = event.position - bounds.origin;
                let view_at = (f32::from(local.x), f32::from(local.y));
                let press = Press {
                    tool,
                    at,
                    view_at,
                    count: event.click_count,
                    shift: event.modifiers.shift,
                    side,
                    back: held_focus.clone(),
                };
                let gesture = pressed_with(
                    &press,
                    &press_scene,
                    &press_handlers,
                    &pressed,
                    &press_editing,
                    window,
                    cx,
                );
                pressed.update(cx, |hand, cx| {
                    hand.gesture = gesture;
                    hand.square = event.modifiers.shift;
                    cx.notify();
                });
            })
            .on_drag(Tracing { owner }, |_, _, _, cx| cx.new(|_| EmptyView))
            .on_drag_move::<Tracing>(move |event, window, cx| {
                if event.drag(cx).owner != owner {
                    return;
                }
                let bounds = moved.read(cx).bounds;
                let at = point_of(event.event.position, bounds);
                if tool == Tool::Eraser {
                    if let Some(ix) = super::edit::hit(&erasing_scene.shapes, at, REACH / view.zoom) {
                        if let Some(delete) = &erasing_handlers.on_delete {
                            delete(&[erasing_scene.shapes[ix].key.clone()], window, cx);
                        }
                    }
                }
                moved.update(cx, |hand, cx| {
                    hand.square = event.event.modifiers.shift;
                    match &mut hand.gesture {
                        Gesture::Marquee { to, .. }
                        | Gesture::Moving { to, .. }
                        | Gesture::Rotating { to, .. }
                        | Gesture::Resizing { to, .. }
                        | Gesture::Drawing { to, .. }
                        | Gesture::Linking { to, .. } => *to = at,
                        Gesture::Stroke { points } => points.push(at),
                        Gesture::Idle => {}
                    }
                    cx.notify();
                });
            })
            .on_drop(move |_: &Tracing, window, cx| {
                let (gesture, square) = taken(&dropped, cx);
                commit(gesture, square, &scene, &handlers, brush, window, cx);
            })
            .on_mouse_up_out(MouseButton::Left, move |_, window, cx| {
                let (gesture, square) = taken(&out_hand, cx);
                commit(
                    gesture,
                    square,
                    &out_scene,
                    &out_handlers,
                    brush,
                    window,
                    cx,
                );
            })
            .on_mouse_up(MouseButton::Left, move |_, window, cx| {
                if cx.has_active_drag() {
                    return;
                }
                let gesture = released.update(cx, |hand, cx| {
                    cx.notify();
                    std::mem::take(&mut hand.gesture)
                });
                if matches!(gesture, Gesture::Marquee { .. }) {
                    log::info!("tool layer: nothing selected");
                    if let Some(on_select) = &select_on_up {
                        on_select(&[], window, cx);
                    }
                }
            })
            .child(
                canvas(
                    move |bounds, window, cx| {
                        if measured.read(cx).bounds != bounds {
                            measured.update(cx, |hand, _| hand.bounds = bounds);
                            window.request_animation_frame();
                        }
                    },
                    move |bounds, _, window, _| {
                        for shape in &preview {
                            paint_shape(shape, &view, bounds.origin, &palette, stroke, window);
                        }
                    },
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full(),
            )
            .children(marquee)
            .children(handles)
            .children(image_preview)
            .children(text_preview)
            .children(rotation_handle)
            .children(outline)
            .children(field)
            .children(snap)
            .into_any_element()
    }
}

/// Ends a pen path the tool left: two points or more go to the owner after this frame, a lone point is dropped.
fn end_path(points: Vec<(f32, f32)>, on_draw: Option<OnDraw>, window: &mut Window, cx: &mut App) {
    if points.len() < 2 {
        log::info!("tool layer: the pen left a lone point, dropped");
        return;
    }
    window.defer(cx, move |window, cx| {
        let (kind, frame) = path(&points, PEN);
        log::info!("tool layer: the pen left a path of {} points", points.len());
        if let Some(on_draw) = &on_draw {
            on_draw(kind, frame, window, cx);
        }
    });
}
