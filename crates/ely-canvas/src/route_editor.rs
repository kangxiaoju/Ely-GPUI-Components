use std::rc::Rc;
use gpui::{App, AppContext as _, ElementId, EmptyView, EntityId, InteractiveElement, IntoElement, MouseButton, ParentElement, Pixels, Point, RenderOnce, Role, StatefulInteractiveElement, Styled, Window, div};
use super::Viewport;
use crate::theme::ActiveTheme;

type OnChange=Rc<dyn Fn(Vec<(f32,f32)>,&mut Window,&mut App)>;
type Commit=Rc<dyn Fn(&gpui::Entity<Editing>,&mut Window,&mut App)>;
#[derive(Default)]
struct Editing {
    points:Vec<(f32,f32)>,
    source:Vec<(f32,f32)>,
    grip:Option<(usize,Point<Pixels>,(f32,f32))>,
}
struct DragPoint {owner:EntityId}

/// Native waypoint handles; previews stay local and the owner receives one release commit.
///
/// ```
/// use ely_canvas::{RouteEditor,Viewport};
/// let editor=RouteEditor::new("route",Viewport::new(0.0,0.0,1.0),[(80.0,40.0)],|_points,_,_|{});
/// ```
#[derive(IntoElement)]
pub struct RouteEditor {
    id:ElementId,
    viewport:Viewport,
    points:Vec<(f32,f32)>,
    on_change:OnChange,
}
impl RouteEditor {
    /// Edits world-space stored waypoints through `viewport`, committing on release.
    pub fn new(id:impl Into<ElementId>,viewport:Viewport,points:impl IntoIterator<Item=(f32,f32)>,on_change:impl Fn(Vec<(f32,f32)>,&mut Window,&mut App)+'static)->Self {
        Self {id:id.into(),viewport,points:points.into_iter().collect(),on_change:Rc::new(on_change)}
    }
}
impl RenderOnce for RouteEditor {
    fn render(self,window:&mut Window,cx:&mut App)->impl IntoElement {
        let state=window.use_keyed_state((self.id.clone(),"waypoints"),cx,|_,_|Editing::default());
        state.update(cx,|state,_| {
            if state.source!=self.points {state.source=self.points.clone();state.points=self.points;state.grip=None;}
        });
        let theme=cx.theme();
        let side=theme.canvas().handle.to_pixels(window.rem_size());
        let (accent,surface)=(theme.colors.accent,theme.colors.surface);
        let view=self.viewport;
        let owner=state.entity_id();
        let points=state.read(cx).points.clone();
        let handles=points.into_iter().enumerate().map(|(index,p)| {
            let (x,y)=view.to_view(p);
            let pressed=state.clone();
            let keyboard=state.clone();
            let changed=self.on_change.clone();
            div().id((self.id.clone(),format!("waypoint-{index}")))
                .absolute().left(Pixels::from(x)-side/2.0).top(Pixels::from(y)-side/2.0).size(side)
                .bg(surface).border_1().border_color(accent).cursor_crosshair()
                .tab_index(0).role(Role::Slider).aria_label(crate::i18n::text(cx,"canvas.board",&[]))
                .on_mouse_down(MouseButton::Left,move |event,_,cx| {
                    cx.stop_propagation();
                    pressed.update(cx,|state,_|state.grip=Some((index,event.position,p)));
                })
                .on_drag(DragPoint {owner},|_,_,_,cx|cx.new(|_|EmptyView))
                .on_key_down(move |event,window,cx| {
                    let step=if event.keystroke.modifiers.shift {10.0} else {1.0};
                    let delta=match event.keystroke.key.as_str() {"left"=>(-step,0.0),"right"=>(step,0.0),"up"=>(0.0,-step),"down"=>(0.0,step),_=>return};
                    cx.stop_propagation();
                    let next=keyboard.update(cx,|state,cx| {state.points[index].0+=delta.0;state.points[index].1+=delta.1;cx.notify();state.points.clone()});
                    changed(next,window,cx);
                })
        }).collect::<Vec<_>>();
        let moved=state.clone();
        let released=state.clone();
        let outside=state.clone();
        let dropped=state.clone();
        let commit:Commit=Rc::new(move |state,window,cx| {
            let next=state.update(cx,|state,cx|state.grip.take().map(|_|{cx.notify();state.points.clone()}));
            if let Some(next)=next {(self.on_change)(next,window,cx);}
        });
        let on_drop=commit.clone();
        let on_out=commit.clone();
        div().id(self.id).absolute().inset_0()
            .on_drag_move::<DragPoint>(move |event,_,cx| {
                if event.drag(cx).owner!=owner {return;}
                moved.update(cx,|state,cx| {
                    if let Some((index,start,p))=state.grip {
                        let delta=event.event.position-start;
                        state.points[index]=(p.0+f32::from(delta.x)/view.zoom,p.1+f32::from(delta.y)/view.zoom);
                        cx.notify();
                    }
                });
            })
            .on_drop(move |drag:&DragPoint,window,cx| {if drag.owner==owner {on_drop(&dropped,window,cx);}})
            .on_mouse_up(MouseButton::Left,move |_,window,cx|commit(&released,window,cx))
            .on_mouse_up_out(MouseButton::Left,move |_,window,cx|on_out(&outside,window,cx))
            .children(handles)
    }
}
