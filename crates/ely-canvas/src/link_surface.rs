use std::time::Instant;
use gpui::{Context, IntoElement, Render, Styled, Window, canvas, fill, point, size, Bounds, Pixels};
use super::{links::{LinkAnimation, FlowMode, FlowDirection}, paint::{at, patterned, finish, solid, head}, style::Dash};
use crate::theme::Palette;

pub(crate) struct LinkSurface {
    pub lines: Vec<(Vec<(f32,f32)>, Option<LinkAnimation>, f32)>,
    pub palette: Palette,
    pub stroke: gpui::Pixels,
    pub zoom: f32,
    pub active: bool,
    pub started: Instant,
}

pub(crate) fn advanced_phase(frozen:f32, elapsed:f32, animation:Option<&LinkAnimation>, active:bool, zoom:f32) -> f32 {
    animation.filter(|a| active && !a.paused).map_or(frozen, |a| frozen+elapsed*a.speed*zoom)
}

fn particle(points:&[(f32,f32)], distance:f32, reverse:bool) -> Option<(f32,f32)> {
    let length:f32 = points.windows(2).map(|p| (p[1].0-p[0].0).hypot(p[1].1-p[0].1)).sum();
    if length<=f32::EPSILON { return None; }
    let travel=distance.rem_euclid(length*1.35);
    if travel>length { return None; }
    let mut remaining=if reverse {length-travel} else {travel};
    for pair in points.windows(2) {
        let (a,b)=(pair[0],pair[1]);
        let segment=(b.0-a.0).hypot(b.1-a.1);
        if segment>0.0 && remaining<=segment {
            let t=remaining/segment;
            return Some((a.0+(b.0-a.0)*t,a.1+(b.1-a.1)*t));
        }
        remaining-=segment;
    }
    points.last().copied()
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
                    let phase=advanced_phase(*frozen,elapsed,animation.as_ref(),state.active,state.zoom);
                    let (dash,sign) = animation.as_ref().map_or((Dash::Solid,1.0), |a| {
                        let dash = if a.speed == 0.0 { Dash::Solid } else { match a.mode { FlowMode::Dashes => Dash::Dashed, FlowMode::Dots => Dash::Dotted } };
                        (dash,if a.direction==FlowDirection::Forward {1.0} else {-1.0})
                    });
                    patterned(points,bounds.origin,state.stroke,false,dash,phase*sign,ink,window);
                    if points.len() >= 2 { finish(solid(&head(points[points.len()-2],points[points.len()-1],state.stroke),bounds.origin),ink,window); }
                    if let Some(animation)=animation.as_ref().filter(|a|a.glow) {
                        if let Some(position)=particle(points,phase,animation.direction==FlowDirection::Reverse) {
                            let radius=f32::from(state.stroke).max(1.0)*2.0;
                            for (scale,alpha) in [(3.0,0.06),(2.0,0.14),(1.0,0.45),(0.45,1.0)] {
                                let r=Pixels::from(radius*scale);
                                window.paint_quad(fill(Bounds::new(at(bounds.origin,position)-point(r,r),size(r*2.0,r*2.0)),ink.opacity(alpha)).corner_radii(r));
                            }
                        }
                    }
                }
            }
        }).absolute().top_0().left_0().size_full()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn particle_uses_route_length_direction_and_an_intermittent_gap() {
        let points=[(0.0,0.0),(100.0,0.0),(100.0,200.0)];
        assert_eq!(particle(&points,150.0,false),Some((100.0,50.0)));
        assert_eq!(particle(&points,50.0,true),Some((100.0,150.0)));
        assert_eq!(particle(&points,350.0,false),None);
        assert_eq!(particle(&points,405.0,false),Some((0.0,0.0)));
    }
    #[test]
    fn pause_and_hidden_surface_keep_the_accumulated_phase() {
        let mut animation=LinkAnimation {key:"a".into(),mode:FlowMode::Dots,direction:FlowDirection::Forward,speed:40.0,paused:false,hue:0,glow:true};
        assert_eq!(advanced_phase(20.0,2.0,Some(&animation),true,1.0),100.0);
        assert_eq!(advanced_phase(20.0,2.0,Some(&animation),false,1.0),20.0);
        animation.paused=true;
        assert_eq!(advanced_phase(20.0,2.0,Some(&animation),true,1.0),20.0);
    }
}
