use super::*;

struct LongForm;
impl Render for LongForm {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        Dialog::new("long-form", "Form", |_, _| {})
            .child(div().flex().flex_col().flex_none().gap_3().children((0..20).map(|index| {
                div().flex_none().h(px(48.)).debug_selector(move || format!("long-field-{index}")).child(format!("Field {index}"))
            })))
            .action(|_| Button::new("long-save", "Save"))
    }
}

#[gpui::test]
fn composed_form_fields_scroll_without_compressing_away(cx: &mut TestAppContext) {
    cx.update(crate::init_for_tests);
    let (_, cx) = cx.add_window_view(|_, _| LongForm);
    settle(cx);
    let first = cx.debug_bounds("long-field-0").unwrap();
    let last = cx.debug_bounds("long-field-19").unwrap();
    assert_eq!(first.size.height, px(48.));
    assert_eq!(last.size.height, px(48.));
    let middle = cx.update(|window, _| window.viewport_size()).map(|side| side / 2.);
    cx.simulate_event(ScrollEvent { position: point(middle.width, middle.height), delta: ScrollDelta::Pixels(point(px(0.), px(-300.))), touch_phase: TouchPhase::Moved, modifiers: Modifiers::none(), source: gpui::ScrollSource::Pointer });
    settle(cx);
    assert_eq!(cx.debug_bounds("long-field-0").unwrap().top(), first.top() - px(300.));
}

struct TitledDialog {
    title: &'static str,
    fullscreen: bool,
    closed: bool,
}

impl Render for TitledDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        let dialog = Dialog::new("titled", self.title, move |_, cx| {
            view.update(cx, |view, cx| {
                view.closed = true;
                cx.notify();
            });
        })
        .width(px(900.0))
        .detail("A description that also needs to wrap beside the close button.");
        if self.fullscreen {
            dialog.fullscreen()
        } else {
            dialog
        }
    }
}

fn assert_long_titles_fit(cx: &mut TestAppContext, fullscreen: bool) {
    cx.update(crate::init_for_tests);
    cx.update(|cx| Theme::update(cx, |theme| theme.reduced_motion = true));
    let (view, cx) = cx.add_window_view(|_, _| TitledDialog {
        title: "Short",
        fullscreen,
        closed: false,
    });
    for width in [280.0, 800.0] {
        cx.simulate_resize(gpui::size(px(width), px(900.0)));
        view.update(cx, |view, cx| {
            view.title = "Short";
            cx.notify();
        });
        settle(cx);
        let single_line = cx.debug_bounds("dialog-title").unwrap().size.height;
        for title in [
            "# **协作记录反馈：很长的中文标题应当在弹窗内部自动换行，保留完整内容，同时不能把关闭按钮挤出窗口或覆盖下方说明文字**",
            "# **Collaboration record feedback: a long English Markdown-like title must wrap inside the modal without pushing its close button outside the viewport**",
        ] {
            view.update(cx, |view, cx| {
                view.title = title;
                view.closed = false;
                cx.notify();
            });
            settle(cx);
            let card = cx.debug_bounds("dialog-card").unwrap();
            let heading = cx.debug_bounds("dialog-heading").unwrap();
            let title = cx.debug_bounds("dialog-title").unwrap();
            let detail = cx.debug_bounds("dialog-detail").unwrap();
            let close = cx.debug_bounds("dialog-close").unwrap();
            let button_size = cx.update(|window, cx| {
                cx.theme().control_height(ControlSize::Md).to_pixels(window.rem_size())
            });
            assert!(card.left() >= px(0.0) && card.right() <= px(width));
            assert!(title.size.height > single_line, "the title wraps");
            assert!(title.left() >= card.left() && title.right() <= close.left());
            assert!(title.bottom() <= detail.top(), "wrapped text does not overlap detail");
            assert!(detail.right() <= close.left() && detail.bottom() <= heading.bottom());
            assert_eq!(close.size, gpui::size(button_size, button_size));
            assert!(close.left() >= card.left() && close.right() <= card.right());
            assert!(close.top() >= card.top() && close.bottom() <= card.bottom());
            let center = close.center();
            click(f32::from(center.x), f32::from(center.y), cx);
            assert!(view.read_with(cx, |view, _| view.closed), "the close button still works");
        }
    }
}

#[gpui::test]
fn standard_dialog_wraps_long_titles_and_keeps_close_visible(cx: &mut TestAppContext) {
    assert_long_titles_fit(cx, false);
}

#[gpui::test]
fn fullscreen_dialog_wraps_long_titles_and_keeps_close_visible(cx: &mut TestAppContext) {
    assert_long_titles_fit(cx, true);
}
