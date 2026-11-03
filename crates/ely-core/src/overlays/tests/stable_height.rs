use super::*;
struct AsyncBody { ready: bool }
impl Render for AsyncBody {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        Dialog::new("async-detail", "Record title", |_,_| {}).height(px(600.)).child(
            div().h(px(if self.ready {1600.} else {24.})).child(if self.ready {"Ready content"} else {"Loading…"})
        ).action(|_| Button::new("close-async","Close"))
    }
}
#[gpui::test]
fn asynchronous_body_keeps_modal_frame_stable(cx: &mut TestAppContext) {
    cx.update(crate::init_for_tests);
    cx.update(|cx| Theme::update(cx,|theme| theme.reduced_motion=true));
    let (view,cx)=cx.add_window_view(|_,_|AsyncBody{ready:false});
    settle(cx);
    let before=cx.debug_bounds("dialog-card").unwrap();
    view.update(cx,|view,cx|{view.ready=true;cx.notify();});
    settle(cx);
    assert_eq!(before,cx.debug_bounds("dialog-card").unwrap());
}
