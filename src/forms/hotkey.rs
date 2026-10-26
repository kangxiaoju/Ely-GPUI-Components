use std::rc::Rc;

use gpui::{
    App, ElementId, InteractiveElement, IntoElement, Keystroke, ParentElement, RenderOnce, Styled,
    Window, div, prelude::*,
};

use crate::{
    primitives::{FocusRing, Icon, IconName},
    theme::{ActiveTheme, ControlSize, IconSize, Radius},
    typography::KbdCombo,
};

type OnChange = Rc<dyn Fn(Option<Keystroke>, &mut Window, &mut App)>;

/// Records one shortcut, then lets go. Backspace clears; Escape leaves.
#[derive(IntoElement)]
pub struct HotkeyInput {
    id: ElementId,
    value: Option<Keystroke>,
    disabled: bool,
    on_change: Option<OnChange>,
}

impl HotkeyInput {
    pub fn new(id: impl Into<ElementId>, value: Option<Keystroke>) -> Self {
        Self {
            id: id.into(),
            value,
            disabled: false,
            on_change: None,
        }
    }

    pub fn on_change(
        mut self,
        handler: impl Fn(Option<Keystroke>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_change = Some(Rc::new(handler));
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl RenderOnce for HotkeyInput {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let disabled = self.disabled;
        let focus = window
            .use_keyed_state(self.id.clone(), cx, move |_, cx| {
                cx.focus_handle().tab_stop(!disabled)
            })
            .read(cx)
            .clone()
            .tab_stop(!disabled);
        let recording = !disabled && focus.is_focused(window);
        let theme = cx.theme();
        let colors = &theme.colors;
        let on_change = self.on_change;
        div()
            .id(self.id)
            .track_focus(&focus)
            .flex()
            .items_center()
            .justify_between()
            .gap_2()
            .h(theme.control_height(ControlSize::Md))
            .px(theme.control_padding(ControlSize::Md))
            .rounded(theme.radius(Radius::Md))
            .border_1()
            .border_color(colors.border_strong)
            .bg(colors.surface)
            .when(!disabled, |field| {
                field
                    .focus_ring(cx)
                    .cursor_pointer()
                    .on_key_down(move |event, window, cx| {
                        cx.stop_propagation();
                        let stroke = &event.keystroke;
                        let bare = !stroke.modifiers.modified();
                        let next = match stroke.key.as_str() {
                            "escape" if bare => {
                                window.blur(cx);
                                return;
                            }
                            "backspace" | "delete" if bare => None,
                            _ => Some(Keystroke {
                                key_char: None,
                                ..stroke.clone()
                            }),
                        };
                        log::info!(
                            "hotkey input: {}",
                            next.as_ref()
                                .map_or("cleared".into(), |stroke| stroke.unparse())
                        );
                        window.blur(cx);
                        if let Some(on_change) = &on_change {
                            on_change(next, window, cx);
                        }
                    })
            })
            .when(disabled, |field| {
                field
                    .border_color(colors.border)
                    .bg(colors.sunken)
                    .text_color(colors.fg_subtle)
            })
            .map(|field| match (&self.value, recording) {
                (_, true) => field.child(
                    div()
                        .text_color(colors.fg_subtle)
                        .child("Press a shortcut…"),
                ),
                (Some(stroke), false) => field.child(KbdCombo::new(&stroke.unparse())),
                (None, false) => field.child(div().text_color(colors.fg_subtle).child("None")),
            })
            .child(
                Icon::new(IconName::Keyboard)
                    .size(IconSize::Sm)
                    .color(colors.fg_subtle),
            )
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::rc::Rc;

    use gpui::{
        Context, FocusHandle, IntoElement, KeyBinding, KeyUpEvent, Keystroke, ParentElement,
        Render, TestAppContext, VisualTestContext, Window,
    };

    use super::HotkeyInput;
    use crate::{
        primitives::{FocusNext, FocusScope},
        theme::Theme,
    };

    struct Bench {
        root: FocusHandle,
        disabled: bool,
        changes: Rc<Cell<usize>>,
    }

    impl Render for Bench {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let changes = self.changes.clone();
            FocusScope::new(&self.root).root().child(
                HotkeyInput::new("hotkey", None)
                    .disabled(self.disabled)
                    .on_change(move |_, _, _| changes.set(changes.get() + 1)),
            )
        }
    }

    fn press(key: &str, cx: &mut VisualTestContext) {
        cx.simulate_keystrokes(key);
        cx.simulate_event(KeyUpEvent {
            keystroke: Keystroke::parse(key).expect("a key"),
        });
        cx.run_until_parked();
    }

    #[gpui::test]
    fn a_disabled_hotkey_input_is_not_a_tab_stop_and_ignores_keys(cx: &mut TestAppContext) {
        cx.update(|cx| {
            Theme::init(cx);
            cx.bind_keys([KeyBinding::new("tab", FocusNext, None)]);
        });
        let changes = Rc::new(Cell::new(0));
        let observed = changes.clone();
        let (view, cx) = cx.add_window_view(move |_, cx| Bench {
            root: cx.focus_handle(),
            disabled: true,
            changes,
        });
        let root = view.read_with(cx, |bench, _| bench.root.clone());
        cx.update(|window, cx| window.focus(&root, cx));
        press("tab", cx);
        press("ctrl-k", cx);
        assert_eq!(observed.get(), 0);
        assert!(cx.update(|window, _| root.is_focused(window)));
    }
}
