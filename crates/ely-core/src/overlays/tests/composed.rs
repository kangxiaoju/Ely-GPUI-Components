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
