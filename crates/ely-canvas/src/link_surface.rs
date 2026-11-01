use std::time::Instant;
use gpui::{App, Context, IntoElement, Render, Styled, Window, canvas};
use super::{links::{LinkAnimation, FlowMode, FlowDirection}, paint::{patterned, finish, solid, head}, style::Dash};
use crate::theme::Palette;

pub(crate) struct LinkSurface {
    pub lines: Vec<(Vec<(f32,f32)>, Option<LinkAnimation>, f32)>,
    pub palette: Palette,
    pub stroke: gpui::Pixels,
    pub zoom: f32,
    pub active: bool,
    pub started: Instant,
}

impl Render for LinkSurface {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        canvas(move |_,window,cx| {
            if entity.read(cx).active {
                entity.update(cx, |_,cx| cx.notify());
                window.request_animation_frame();
            }
        }, {
            let entity = cx.entity();
            move |bounds,_,window,cx| {
                let state = entity.read(cx);
                let elapsed = state.started.elapsed().as_secs_f32();
                for (points, animation, frozen) in &state.lines {
                    let ink = animation.as_ref().map_or(state.palette.fg_muted, |a| state.palette.hue(a.hue,"connector"));
                    let (dash,phase) = animation.as_ref().map_or((Dash::Solid,0.0), |a| {
                        let direction = if a.direction == FlowDirection::Forward {1.0} else {-1.0};
                        let dash = if a.speed == 0.0 { Dash::Solid } else { match a.mode { FlowMode::Dashes => Dash::Dashed, FlowMode::Dots => Dash::Dotted } };
                        (dash, if state.active && !a.paused { elapsed * a.speed * state.zoom * direction } else { *frozen })
                    });
                    patterned(points,bounds.origin,state.stroke,false,dash,phase,ink,window);
                    if points.len() >= 2 { finish(solid(&head(points[points.len()-2],points[points.len()-1],state.stroke),bounds.origin),ink,window); }
                }
            }
        }).absolute().top_0().left_0().size_full()
    }
}
