use std::{rc::Rc, sync::Arc, time::Instant};

use gpui::{
    AnyElement, App, AppContext as _, Bounds, ElementId, EmptyView, Entity, EntityId,
    InteractiveElement, IntoElement, ParentElement, Pixels, Point, RenderOnce, ScrollDelta,
    SharedString, StatefulInteractiveElement, Styled, Window, canvas, div, fill, size,
};

use super::{
    links::{Link, CanvasScene, LinkAnimation, FlowMode, FlowDirection, halfway},
    paint::{at, finish, in_view, outline, paint_shape, quad},
    shape::{Artboard, Guide, Shape, ShapeKind},
    view::{Viewport, marks, step},
};
use crate::{
    layout::seeded::use_seeded,
    theme::{ActiveTheme, Radius, TextSize},
};

pub(crate) type OnViewport = Rc<dyn Fn(Viewport, &mut Window, &mut App)>;

/// A link's run in view pixels, with its words.
type Drawn = (Vec<(f32, f32)>, Option<SharedString>, Option<LinkAnimation>);

/// A drag that pans a canvas, and whose canvas it is.
struct Pan {
    owner: EntityId,
}

/// The canvas's box, and the pointer and viewport at the press of a pan under way.
#[derive(Default)]
struct Plane {
    bounds: Bounds<Pixels>,
    grip: Option<(Point<Pixels>, Viewport)>,
}


/// An endless plane: a dot grid that thins as it zooms out, rulers along the top and left, guides, artboards with their names, and shapes. A drag on empty space pans it from where it was pressed, a wheel scrolls it, and Command or Control with the wheel zooms about the pointer. The owner keeps the viewport; a new one from the owner shows at once.
///
/// ```
/// use ely_canvas::{Artboard, Frame, InfiniteCanvas, Shape, ShapeKind, Viewport};
///
/// let canvas = InfiniteCanvas::new("canvas", Viewport::new(0.0, 0.0, 1.0))
///     .artboards([Artboard::new("home", "Home", Frame::new(0.0, 0.0, 390.0, 844.0))])
///     .shapes([Shape::new("logo", "Logo", ShapeKind::Ellipse, Frame::new(20.0, 20.0, 64.0, 64.0))])
///     .rulers()
///     .on_viewport(|_viewport, _, _| {});
/// ```
#[derive(IntoElement)]
pub struct InfiniteCanvas {
    id: ElementId,
    viewport: Viewport,
    shapes: Vec<Shape>,
    links: Vec<Link>,
    artboards: Vec<Artboard>,
    guides: Vec<Guide>,
    rulers: bool,
    layers: Vec<AnyElement>,
    on_viewport: Option<OnViewport>,
    scene: Option<Arc<CanvasScene>>,
    link_animations: Vec<LinkAnimation>,
    animations_active: bool,
    reduced_motion: bool,
    link_hues: std::collections::HashMap<SharedString,usize>,
}

impl InfiniteCanvas {
    /// An empty plane under `id` seen through `viewport`.
    pub fn new(id: impl Into<ElementId>, viewport: Viewport) -> Self {
        Self {
            id: id.into(),
            viewport,
            shapes: Vec::new(),
            links: Vec::new(),
            artboards: Vec::new(),
            guides: Vec::new(),
            rulers: false,
            layers: Vec::new(),
            on_viewport: None,
            scene: None,
            link_animations: Vec::new(),
            animations_active: true,
            reduced_motion: false,
            link_hues: Default::default(),
        }
    }

    /// What lies on it, in paint order.
    pub fn shapes(mut self, shapes: impl IntoIterator<Item = Shape>) -> Self {
        self.shapes = shapes.into_iter().collect();
        self
    }

    /// Lines between shapes that follow them, drawn over the shapes; each names two shapes on the canvas.
    pub fn links(mut self, links: impl IntoIterator<Item = Link>) -> Self {
        self.links = links.into_iter().collect();
        self
    }

    /// Uses an immutable content-revision scene without cloning shapes or recalculating routes.
    pub fn scene(mut self, scene: Arc<CanvasScene>) -> Self { self.scene = Some(scene); self }

    /// Independent flow styles, keyed by connector.
    pub fn link_animations(mut self, animations: impl IntoIterator<Item = LinkAnimation>) -> Self {
        self.link_animations = animations.into_iter().collect(); self
    }

    /// Stops animation scheduling while the owning tab or view is hidden.
    pub fn animations_active(mut self, active: bool) -> Self { self.animations_active = active; self }

    /// Shows static connector patterns without requesting animation frames.
    pub fn reduced_motion(mut self, reduced: bool) -> Self { self.reduced_motion = reduced; self }

    /// Theme hue slots for static connectors, keyed by link.
    pub fn link_hues(mut self, hues: impl IntoIterator<Item=(SharedString,usize)>) -> Self {
        self.link_hues = hues.into_iter().collect(); self
    }

    /// Boards drawn under the shapes.
    pub fn artboards(mut self, artboards: impl IntoIterator<Item = Artboard>) -> Self {
        self.artboards = artboards.into_iter().collect();
        self
    }

    /// Lines across it to line things up with.
    pub fn guides(mut self, guides: impl IntoIterator<Item = Guide>) -> Self {
        self.guides = guides.into_iter().collect();
        self
    }

    /// Rulers along the top and the left, in canvas units.
    pub fn rulers(mut self) -> Self {
        self.rulers = true;
        self
    }

    /// Lays an element over the plane, in its view's pixels, as a selection or handles are.
    pub fn layer(mut self, element: impl IntoElement) -> Self {
        self.layers.push(element.into_any_element());
        self
    }

    /// Gets the viewport after each pan and zoom.
    pub fn on_viewport(
        mut self,
        handler: impl Fn(Viewport, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_viewport = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for InfiniteCanvas {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let scene = self.scene.clone().unwrap_or_else(|| Arc::new(CanvasScene::new(self.shapes, self.links)));
        let id = self.id.clone();
        let local = use_seeded((id.clone(), "view"), self.viewport, window, cx);
        let plane: Entity<Plane> =
            window.use_keyed_state((id.clone(), "plane"), cx, |_, _| Plane::default());
        let view = local.read(cx).value;
        let bounds = plane.read(cx).bounds;
        let (w, h) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
        let set: OnViewport = {
            let (local, on_viewport) = (local.clone(), self.on_viewport.clone());
            Rc::new(move |next, window, cx| {
                local.update(cx, |local, cx| {
                    local.value = next;
                    cx.notify();
                });
                if let Some(on_viewport) = &on_viewport {
                    on_viewport(next, window, cx);
                }
            })
        };
        let theme = cx.theme();
        let (colors, sizes) = (theme.colors.clone(), theme.canvas());
        let rem = window.rem_size();
        let (dot, hairline, stroke) = (sizes.dot, sizes.hairline, sizes.stroke.to_pixels(rem));
        let (ruler, side) = (sizes.ruler.to_pixels(rem), sizes.side.to_pixels(rem));
        let grid = step(view.zoom, f32::from(sizes.grid.to_pixels(rem)));
        let (left, top) = view.to_canvas((0.0, 0.0));
        let (right, bottom) = view.to_canvas((w, h));
        let dots: Vec<(f32, f32)> = marks(left, right, grid)
            .into_iter()
            .flat_map(|x| {
                marks(top, bottom, grid)
                    .into_iter()
                    .map(move |y| view.to_view((x, y)))
            })
            .collect();
        let words = |text: String, color| {
            div()
                .absolute()
                .whitespace_nowrap()
                .text_size(theme.text_size(TextSize::Xs))
                .text_color(color)
                .child(text)
        };
        let names: Vec<AnyElement> = self
            .artboards
            .iter()
            .map(|board| {
                let frame = in_view(&view, &board.frame);
                words(board.name.to_string(), colors.fg_muted)
                    .left(Pixels::from(frame.x))
                    .bottom(Pixels::from(h - frame.y))
                    .pb_1()
                    .into_any_element()
            })
            .collect();
        let routes: Vec<Drawn> = scene.links.iter().zip(&scene.routes).filter_map(|(link, route)| {
            let points: Vec<_> = route.as_ref()?.iter().map(|point| view.to_view(*point)).collect();
            let bounds = points.iter().map(|p| super::view::Frame::new(p.0,p.1,0.0,0.0))
                .reduce(|a,b| a.union(&b))?;
            if bounds.right() < 0.0 || bounds.bottom() < 0.0 || bounds.x > w || bounds.y > h { return None; }
            let animation = self.link_animations.iter().find(|a| a.key == link.key).cloned()
                .or_else(|| self.link_hues.get(&link.key).map(|hue| LinkAnimation { key:link.key.clone(), mode:FlowMode::Dashes, direction:FlowDirection::Forward, speed:0.0, paused:true, hue:*hue }));
            Some((points, link.label.clone(), animation))
        }).collect();
        let flowing = self.animations_active && !self.reduced_motion && !theme.reduced_motion && routes.iter().any(|(_,_,a)| a.as_ref().is_some_and(|a| !a.paused && a.speed > 0.0));
        let chip = theme.text_size(TextSize::Sm).to_pixels(rem) * view.zoom;
        let chips: Vec<AnyElement> = routes
            .iter()
            .filter_map(|(points, label, _)| {
                let (x, y) = halfway(points);
                Some(
                    div()
                        .absolute()
                        .left(Pixels::from(x))
                        .top(Pixels::from(y))
                        .size_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .flex_none()
                                .whitespace_nowrap()
                                .px(chip * 0.5)
                                .rounded(theme.radius(Radius::Sm))
                                .bg(colors.bg)
                                .border_1()
                                .border_color(colors.border)
                                .text_size(chip)
                                .line_height(chip * 1.4)
                                .text_color(colors.fg_muted)
                                .child(label.clone()?),
                        )
                        .into_any_element(),
                )
            })
            .collect();
        let lines = routes;
        let surface = window.use_keyed_state((self.id.clone(), "flow-surface"), cx, |_,_| super::link_surface::LinkSurface {
            lines: Vec::new(), palette: colors.clone(), stroke, zoom: view.zoom, active:false, started:Instant::now()
        });
        surface.update(cx, |surface, _| {
            let elapsed = surface.started.elapsed().as_secs_f32();
            surface.lines = lines.into_iter().map(|(points,_,animation)| {
                let phase = animation.as_ref().map_or(0.0, |a| {
                    let sign = if a.direction == FlowDirection::Forward {1.0} else {-1.0};
                    elapsed*a.speed*view.zoom*sign
                });
                (points,animation,phase)
            }).collect();
            surface.palette=colors.clone(); surface.stroke=stroke; surface.zoom=view.zoom; surface.active=flowing;
        });
        let texts: Vec<AnyElement> = scene.shapes
            .iter()
            .filter(|shape| !shape.hidden)
            .filter(|shape| { let frame=in_view(&view,&shape.bounds()); frame.right()>=0.0 && frame.bottom()>=0.0 && frame.x<=w && frame.y<=h })
            .filter_map(|shape| {
                let frame = in_view(&view, &shape.frame);
                let size = shape.appearance.map_or(theme.text_size(TextSize::Base).to_pixels(rem), |s| Pixels::from(s.font_size)) * view.zoom;
                if shape.rotation != 0.0 && !matches!(shape.kind, ShapeKind::Image(_)) {
                    if let Some(text) = shape.words() {
                        let font_size = shape.appearance.map_or(16.0, |s| s.font_size);
                        let escaped = |s:&str| s.replace('&',"&amp;").replace('<',"&lt;").replace('>',"&gt;").replace('"',"&quot;");
                        let spans = text.lines().enumerate().map(|(ix,line)| format!("<tspan x='0' y='{}'>{}</tspan>", font_size*(1.0+ix as f32*1.3),escaped(line))).collect::<String>();
                        let data = format!("<svg xmlns='http://www.w3.org/2000/svg' width='{}' height='{}'><text font-family='{}' font-size='{}'>{}</text></svg>",shape.frame.w,shape.frame.h,escaped(&theme.font_family),font_size,spans);
                        return Some(gpui::svg().data(data.as_bytes()).with_transformation(gpui::Transformation::rotate(gpui::radians(shape.rotation)))
                            .absolute().left(Pixels::from(frame.x)).top(Pixels::from(frame.y)).w(Pixels::from(frame.w)).h(Pixels::from(frame.h))
                            .text_color(colors.fg).opacity(shape.appearance.map_or(1.0,|s|s.opacity)).into_any_element());
                    }
                }
                let placed = |text: &str| {
                    div()
                        .absolute()
                        .left(Pixels::from(frame.x))
                        .top(Pixels::from(frame.y))
                        .w(Pixels::from(frame.w))
                        .h(Pixels::from(frame.h))
                        .overflow_hidden()
                        .text_size(size)
                        .line_height(size * 1.3)
                        .opacity(shape.appearance.map_or(1.0, |s| s.opacity))
                        .child(text.to_string())
                };
                match &shape.kind {
                    ShapeKind::Image(_) => {
                        let frame = in_view(&view, &shape.bounds());
                        scene.images.get(&shape.key).map(|image| gpui::img(image.clone())
                            .absolute().left(Pixels::from(frame.x)).top(Pixels::from(frame.y))
                            .w(Pixels::from(frame.w)).h(Pixels::from(frame.h))
                            .opacity(shape.appearance.map_or(1.0, |s| s.opacity)).into_any_element())
                    }
                    ShapeKind::Text(text) => {
                        Some(placed(text).text_color(colors.fg).into_any_element())
                    }
                    ShapeKind::Note(text) => {
                        let paper = colors.hue(shape.hue, format_args!("note {}", shape.key));
                        Some(
                            placed(text)
                                .p(sizes.note_inset.to_pixels(rem) * view.zoom)
                                .rounded(sizes.note_corner.to_pixels(rem) * view.zoom)
                                .text_color(colors.fg)
                                .into_any_element(),
                        )
                    }
                    _ => shape.label.as_ref().map(|label| {
                        placed(label)
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_center()
                            .text_color(colors.fg)
                            .into_any_element()
                    }),
                }
            })
            .collect();
        let tick = step(view.zoom, f32::from(sizes.label.to_pixels(rem)));
        let rulers = self.rulers.then(|| {
            let across = marks(left, right, tick).into_iter().map(|x| {
                words(format!("{}", x.round()), colors.fg_subtle)
                    .left(Pixels::from(view.to_view((x, 0.0)).0))
                    .top_0()
                    .pl_1()
                    .into_any_element()
            });
            let down = marks(top, bottom, tick).into_iter().map(|y| {
                words(format!("{}", y.round()), colors.fg_subtle)
                    .left_0()
                    .top(Pixels::from(view.to_view((0.0, y)).1))
                    .pl_0p5()
                    .into_any_element()
            });
            let strip = |element: gpui::Div| {
                element
                    .absolute()
                    .bg(colors.surface)
                    .border_color(colors.border)
            };
            [
                strip(div().top_0().left_0().right_0().h(ruler).border_b_1())
                    .children(across.collect::<Vec<_>>())
                    .into_any_element(),
                strip(div().top_0().left_0().bottom_0().w(side).border_r_1())
                    .children(down.collect::<Vec<_>>())
                    .into_any_element(),
            ]
        });
        let (shapes, artboards, guides, palette) =
            (scene.shapes.clone(), self.artboards, self.guides, colors.clone());
        let painted = canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                let origin = bounds.origin;
                for (x, y) in &dots {
                    window.paint_quad(fill(
                        Bounds::new(at(origin, (*x, *y)), size(dot, dot)),
                        palette.border_strong,
                    ));
                }
                for board in &artboards {
                    let frame = in_view(&view, &board.frame);
                    window.paint_quad(
                        fill(quad(origin, &frame), palette.surface)
                            .border_widths(hairline)
                            .border_color(palette.border),
                    );
                }
                for shape in shapes.iter().filter(|shape| !shape.hidden) {
                    let frame=in_view(&view,&shape.bounds());
                    if frame.right()<0.0 || frame.bottom()<0.0 || frame.x>w || frame.y>h { continue; }
                    paint_shape(shape, &view, origin, &palette, stroke, window);
                }
                let line = palette.hue(3, "guide");
                for guide in &guides {
                    let (from, to) = match guide {
                        Guide::Vertical(x) => {
                            let x = view.to_view((*x, 0.0)).0;
                            ((x, 0.0), (x, h))
                        }
                        Guide::Horizontal(y) => {
                            let y = view.to_view((0.0, *y)).1;
                            ((0.0, y), (w, y))
                        }
                    };
                    finish(
                        outline(&[from, to], origin, stroke / 2.0, false),
                        line,
                        window,
                    );
                }
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full();
        let measured = {
            let plane = plane.clone();
            canvas(
                move |bounds, window, cx| {
                    if plane.read(cx).bounds != bounds {
                        plane.update(cx, |plane, cx| {
                            plane.bounds = bounds;
                            cx.notify();
                        });
                        window.request_animation_frame();
                    }
                },
                |_, _, _, _| {},
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full()
        };
        let owner = plane.entity_id();
        let (wheel, pan, drop) = (set.clone(), set, plane.clone());
        let (scrolled, dragged, pressed) = (plane.clone(), plane.clone(), plane);
        let (live, held) = (local.clone(), local.clone());
        div()
            .id(id)
            .relative()
            .size_full()
            .overflow_hidden()
            .bg(colors.sunken)
            .cursor_grab()
            .on_scroll(move |event, window, cx| {
                let bounds = scrolled.read(cx).bounds;
                let delta = match event.delta {
                    ScrollDelta::Pixels(delta) => (f32::from(delta.x), f32::from(delta.y)),
                    ScrollDelta::Lines(lines) => {
                        let line = f32::from(window.line_height());
                        (lines.x * line, lines.y * line)
                    }
                };
                if delta == (0.0, 0.0) {
                    return;
                }
                let pointer = event.position - bounds.origin;
                let about = (f32::from(pointer.x), f32::from(pointer.y));
                let now = live.read(cx).value;
                let next = match event.modifiers.platform || event.modifiers.control {
                    true => now.zoomed((-delta.1 / 240.0).exp(), about),
                    false => now.panned(delta),
                };
                cx.stop_propagation();
                wheel(next, window, cx);
            })
            .on_mouse_down(gpui::MouseButton::Left, move |event, _, cx| {
                let now = held.read(cx).value;
                pressed.update(cx, |plane, _| plane.grip = Some((event.position, now)))
            })
            .on_drag(Pan { owner }, |_, _, _, cx| cx.new(|_| EmptyView))
            .on_drag_move::<Pan>(move |event, window, cx| {
                if event.drag(cx).owner != owner {
                    return;
                }
                let Some((press, start)) = dragged.read(cx).grip else {
                    return;
                };
                let way = event.event.position - press;
                pan(
                    start.panned((f32::from(way.x), f32::from(way.y))),
                    window,
                    cx,
                );
            })
            .on_drop(move |_: &Pan, _, cx| drop.update(cx, |plane, _| plane.grip = None))
            .child(painted)
            .children(names)
            .children(texts)
            .child(surface)
            .children(chips)
            .children(self.layers)
            .children(rulers.into_iter().flatten())
            .child(measured)
    }
}

impl ParentElement for InfiniteCanvas {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.layers.extend(elements);
    }
}
