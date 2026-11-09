use super::*;

#[gpui::test]
fn clearing_sensitive_text_forgets_both_history_directions(cx: &mut TestAppContext) {
    let (fields, cx) = open_with(1, false, TextInput::masked, cx);
    let field = &fields[0];
    focus(field, cx);
    field.update(cx, |input, cx| {
        input.insert("first-secret", cx);
        input.insert("second-secret", cx);
    });
    cx.simulate_keystrokes("secondary-z");
    assert_eq!(text(field, cx), "first-secret");
    field.update(cx, |input, cx| {
        input.set_masked(false, cx);
        input.clear_sensitive(cx);
        assert!(input.is_empty());
        assert!(!input.is_masked(), "clearing preserves eye reveal");
        assert_eq!(input.selection(), 0..0);
        assert!(input.layout.is_none());
    });
    cx.simulate_keystrokes("secondary-z secondary-shift-z");
    assert_eq!(text(field, cx), "");
    cx.simulate_input("fresh");
    cx.simulate_keystrokes("secondary-z secondary-shift-z");
    assert_eq!(text(field, cx), "fresh");
}

#[gpui::test]
fn clearing_sensitive_text_discards_an_uncommitted_composition(cx: &mut TestAppContext) {
    let (fields, cx) = open_with(1, false, TextInput::masked, cx);
    let field = &fields[0];
    focus(field, cx);
    cx.update(|window, cx| {
        field.update(cx, |input, cx| {
            input.insert("committed-secret", cx);
            input.replace_and_mark_text_in_range(None, "composing-secret", None, window, cx);
            input.clear_sensitive(cx);
            assert_eq!(input.committed(), "");
            assert!(input.marked_text_range(window, cx).is_none());
            input.unmark_text(window, cx);
        });
    });
    cx.simulate_keystrokes("secondary-z secondary-shift-z");
    assert_eq!(text(field, cx), "");
}

#[gpui::test]
fn normal_text_inputs_keep_their_existing_history_behavior(cx: &mut TestAppContext) {
    let (fields, cx) = open(1, false, cx);
    let field = &fields[0];
    focus(field, cx);
    field.update(cx, |input, cx| {
        input.insert("ordinary", cx);
        input.set_text("replacement", cx);
    });
    cx.simulate_keystrokes("secondary-z");
    assert_eq!(text(field, cx), "");
    cx.simulate_keystrokes("secondary-shift-z");
    assert_eq!(text(field, cx), "replacement");
}
