use std::rc::Rc;

use gpui::{
    AnyElement, App, ElementId, FontWeight, Hsla, InteractiveElement, IntoElement, MouseButton,
    ParentElement, RenderOnce, Role, SharedString, StatefulInteractiveElement, Styled, Window, div,
    prelude::*,
};

use crate::{
    primitives::{FocusRing, Icon, IconName, Severity},
    theme::{ActiveTheme, ControlSize, IconSize, Palette, Radius, TextSize},
    typography::{AnimatedNumber, Ellipsis},
};

/// How a badge or a tag is tinted: quiet, the accent, or a severity.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tone {
    /// Quiet: muted text on a soft fill.
    #[default]
    Neutral,
    /// The accent.
    Accent,
    /// News that needs no action.
    Info,
    /// Something worked.
    Success,
    /// Something may go wrong.
    Warning,
    /// Something failed.
    Danger,
}

impl From<Severity> for Tone {
    fn from(severity: Severity) -> Self {
        match severity {
            Severity::Info => Self::Info,
            Severity::Success => Self::Success,
            Severity::Warning => Self::Warning,
            Severity::Danger => Self::Danger,
        }
    }
}

impl Tone {
    /// The fill, then the text on it.
    pub(crate) fn colors(self, colors: &Palette) -> (Hsla, Hsla) {
        let severity = |severity: Severity| (severity.subtle(colors), severity.color(colors));
        match self {
            Self::Neutral => (colors.hover, colors.fg_muted),
            Self::Accent => (colors.accent, colors.on_accent),
            Self::Info => severity(Severity::Info),
            Self::Success => severity(Severity::Success),
            Self::Warning => severity(Severity::Warning),
            Self::Danger => severity(Severity::Danger),
        }
    }

    /// The color a dot of this tone takes.
    pub(super) fn dot(self, colors: &Palette) -> Hsla {
        match self {
            Self::Neutral => colors.fg_subtle,
            Self::Accent => colors.accent,
            other => other.colors(colors).1,
        }
    }
}

/// Sets `badge` on the top-right corner of `over`, just past its edge.
fn corner(over: AnyElement, badge: impl IntoElement) -> AnyElement {
    div()
        .relative()
        .flex_none()
        .child(over)
        .child(div().absolute().top_neg_1().right_neg_1().child(badge))
        .into_any_element()
}

/// A small label for a status or a kind, tinted by its tone, with a dot when it marks a state.
///
/// ```
/// use ely_core::data_display::{Badge, Tone};
///
/// let beta = Badge::new("Beta").tone(Tone::Accent);
/// ```
#[derive(IntoElement)]
pub struct Badge {
    label: SharedString,
    tone: Tone,
    dot: bool,
}

impl Badge {
    /// A neutral badge that reads `label`.
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            tone: Tone::default(),
            dot: false,
        }
    }

    /// Its tint.
    pub fn tone(mut self, tone: impl Into<Tone>) -> Self {
        self.tone = tone.into();
        self
    }

    /// Leads with a dot of the tone's color, as a status does.
    pub fn dot(mut self) -> Self {
        self.dot = true;
        self
    }
}

impl RenderOnce for Badge {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let (fill, text) = self.tone.colors(&theme.colors);
        let dot = match self.tone {
            Tone::Accent => theme.colors.on_accent,
            tone => tone.dot(&theme.colors),
        };
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap_1()
            .px_1p5()
            .h(theme.control_height(ControlSize::Sm) * 0.75)
            .rounded(theme.radius(Radius::Sm))
            .bg(fill)
            .text_size(theme.text_size(TextSize::Xs))
            .font_weight(FontWeight::MEDIUM)
            .text_color(text)
            .when(self.dot, |badge| {
                badge.child(div().size(theme.status_dot()).rounded_full().bg(dot))
            })
            .child(self.label)
    }
}

/// What a count badge reads past its cap, as 99+.
fn capped(count: usize, max: usize) -> Option<String> {
    (count > max).then(|| format!("{max}+"))
}

/// A number in a pill, for unread items or a queue. Its digits roll as it changes; past `max` it reads 99+. Zero hides it. Set `over` an icon or an avatar, it sits on the top-right corner.
///
/// ```
/// use ely_core::{buttons::IconButton, data_display::CountBadge, primitives::IconName};
///
/// let unread = CountBadge::new("unread", 12).over(IconButton::new("inbox", IconName::Inbox).tooltip("Inbox"));
/// ```
#[derive(IntoElement)]
pub struct CountBadge {
    id: ElementId,
    count: usize,
    max: usize,
    over: Option<AnyElement>,
}

impl CountBadge {
    /// A count under `id`; 99+ past 99, hidden at zero.
    pub fn new(id: impl Into<ElementId>, count: usize) -> Self {
        Self {
            id: id.into(),
            count,
            max: 99,
            over: None,
        }
    }

    /// The largest count shown as is; above it reads `max+`.
    pub fn max(mut self, max: usize) -> Self {
        assert!(max > 0, "a count badge needs room for one");
        self.max = max;
        self
    }

    /// Pins it to the corner of `element`.
    pub fn over(mut self, element: impl IntoElement) -> Self {
        self.over = Some(element.into_any_element());
        self
    }
}

impl RenderOnce for CountBadge {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let (fill, text) = Tone::Accent.colors(&theme.colors);
        let height = theme.control_height(ControlSize::Sm) * 0.67;
        let pill = (self.count > 0).then(|| {
            div()
                .flex()
                .items_center()
                .justify_center()
                .min_w(height)
                .h(height)
                .px_1()
                .rounded_full()
                .bg(fill)
                .text_color(text)
                .text_size(theme.text_size(TextSize::Xs))
                .font_weight(FontWeight::MEDIUM)
                .child(if let Some(capped) = capped(self.count, self.max) {
                    capped.into_any_element()
                } else {
                    AnimatedNumber::new(self.id, self.count as f64)
                        .size(TextSize::Xs)
                        .color(text)
                        .into_any_element()
                })
        });
        match self.over {
            Some(over) => corner(over, div().children(pill)),
            None => div().children(pill).into_any_element(),
        }
    }
}

/// A dot in `color` centered in an icon's box, as a label or an event wears its hue in a row.
#[doc(hidden)]
pub fn color_mark(color: Hsla, cx: &App) -> gpui::Div {
    let theme = cx.theme();
    div()
        .flex_none()
        .size(theme.icon_size(IconSize::Sm))
        .flex()
        .items_center()
        .justify_center()
        .child(div().size(theme.status_dot()).rounded_full().bg(color))
}

/// A small dot that says something is new or live. Set `over` an icon, it sits on the top-right corner.
///
/// ```
/// use ely_core::{
///     data_display::{DotBadge, Tone},
///     primitives::{Icon, IconName},
/// };
///
/// let alert = DotBadge::new().tone(Tone::Danger).over(Icon::new(IconName::Bell));
/// ```
#[derive(IntoElement)]
pub struct DotBadge {
    tone: Tone,
    over: Option<AnyElement>,
}

impl DotBadge {
    /// An accent dot.
    pub fn new() -> Self {
        Self {
            tone: Tone::Accent,
            over: None,
        }
    }

    /// Its tint.
    pub fn tone(mut self, tone: impl Into<Tone>) -> Self {
        self.tone = tone.into();
        self
    }

    /// Pins it to the corner of `element`.
    pub fn over(mut self, element: impl IntoElement) -> Self {
        self.over = Some(element.into_any_element());
        self
    }
}

impl Default for DotBadge {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for DotBadge {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let dot = div()
            .size(theme.status_dot() * 1.34)
            .rounded_full()
            .border_1()
            .border_color(theme.colors.bg)
            .bg(self.tone.dot(&theme.colors));
        match self.over {
            Some(over) => corner(over, dot),
            None => dot.into_any_element(),
        }
    }
}

type OnPress = Rc<dyn Fn(&mut Window, &mut App)>;

/// A short label for a kind, a topic or a filter: an optional icon, the label, and an x when it can be removed. It is never wider than its container; a long label ends in an ellipsis.
///
/// ```
/// use ely_core::{
///     data_display::{Tag, Tone},
///     primitives::IconName,
/// };
///
/// let label = Tag::new("bug", "Bug").icon(IconName::Bug).tone(Tone::Danger);
/// let chip = Tag::new("work", "Work").hue(2).outlined();
/// let filter = Tag::new("work-filter", "Work")
///     .hue(2)
///     .outlined()
///     .selected(true)
///     .on_click(|_, _| {});
/// ```
#[derive(IntoElement)]
pub struct Tag {
    id: ElementId,
    label: SharedString,
    icon: Option<IconName>,
    tone: Tone,
    hue: Option<usize>,
    outlined: bool,
    selected: bool,
    on_click: Option<OnPress>,
    on_remove: Option<OnPress>,
}

impl Tag {
    /// A neutral tag under `id` that reads `label`.
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: None,
            tone: Tone::Neutral,
            hue: None,
            outlined: false,
            selected: false,
            on_click: None,
            on_remove: None,
        }
    }

    /// An icon before the label.
    pub fn icon(mut self, icon: IconName) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Its tint.
    pub fn tone(mut self, tone: impl Into<Tone>) -> Self {
        self.tone = tone.into();
        self
    }

    /// Uses the theme's chart hue instead of its tone; the index must be in 0..8.
    pub fn hue(mut self, hue: usize) -> Self {
        self.hue = Some(hue);
        self
    }

    /// Uses colored text and a border instead of a fill; selection adds a soft wash.
    pub fn outlined(mut self) -> Self {
        self.outlined = true;
        self
    }

    /// Marks it as chosen with a border and a stronger fill.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Makes it a named, focusable button, pressed by click, Enter or Space.
    pub fn on_click(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }

    /// Adds an x that runs `handler`; the press stays out of focus.
    pub fn on_remove(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_remove = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for Tag {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = cx.theme();
        let colors = &theme.colors;
        let (fill, text) = match self.hue {
            Some(hue) => {
                let color = colors.hue(hue, format_args!("tag {:?}", self.id));
                (color.opacity(0.14), color)
            }
            None => match self.tone {
                Tone::Neutral => (colors.hover, colors.fg),
                other => other.colors(colors),
            },
        };
        let border = self.hue.map_or_else(|| self.tone.dot(colors), |_| text);
        let fill = if self.outlined {
            if self.selected {
                border.opacity(0.14)
            } else {
                gpui::transparent_black()
            }
        } else if self.selected {
            self.hue.map_or(fill, |_| text.opacity(0.24))
        } else {
            fill
        };
        let text = if self.outlined { border } else { text };
        let label = self.label.clone();
        let (remove, named) = (self.id.clone(), self.id.clone());
        let tag = div()
            .debug_selector(move || format!("tag {named}"))
            .flex()
            .flex_none()
            .max_w_full()
            .items_center()
            .gap_1()
            .pl_2()
            .when(self.on_remove.is_none(), |tag| tag.pr_2())
            .when(self.on_remove.is_some(), |tag| tag.pr_1())
            .h(theme.control_height(ControlSize::Sm))
            .rounded(theme.radius(Radius::Sm))
            .when(self.outlined || self.selected || self.on_click.is_some(), |tag| {
                tag.border_1().border_color(if self.outlined || self.selected {
                    border
                } else {
                    gpui::transparent_black()
                })
            })
            .when(self.selected, |tag| tag.font_weight(FontWeight::MEDIUM))
            .bg(fill)
            .text_size(theme.text_size(TextSize::Sm))
            .text_color(text)
            .when_some(self.icon, |tag, icon| {
                tag.child(
                    div()
                        .flex_none()
                        .child(Icon::new(icon).size(IconSize::Xs).color(text)),
                )
            })
            .child(div().flex_1().min_w_0().child(Ellipsis::new(self.label)))
            .when_some(self.on_remove, |tag, remove_it| {
                tag.child(
                    div()
                        .id(remove)
                        .flex_none()
                        .rounded(theme.radius(Radius::Sm))
                        .cursor_pointer()
                        .hover(|style| style.bg(colors.active))
                        .on_mouse_down(MouseButton::Left, |_, window, cx| {
                            window.prevent_default();
                            cx.stop_propagation();
                        })
                        .on_click(move |_, window, cx| {
                            cx.stop_propagation();
                            remove_it(window, cx)
                        })
                        .child(
                            Icon::new(IconName::X)
                                .size(IconSize::Xs)
                                .color(colors.fg_muted),
                        ),
                )
            });
        match self.on_click {
            Some(click) => tag
                .id(self.id)
                .role(Role::Button)
                .aria_label(label)
                .aria_selected(self.selected)
                .tab_index(0)
                .focus_ring(cx)
                .cursor_pointer()
                .hover(|style| style.bg(if self.selected { fill } else { colors.hover }))
                .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
                .on_click(move |_, window, cx| click(window, cx))
                .into_any_element(),
            None => tag.into_any_element(),
        }
    }
}

impl crate::primitives::Selectable for Tag {
    fn selected(self, selected: bool) -> Self {
        Tag::selected(self, selected)
    }

    fn is_selected(&self) -> bool {
        self.selected
    }
}

#[cfg(test)]
mod tests {
    use super::capped;

    #[test]
    fn a_count_past_its_cap_reads_the_cap_and_a_plus() {
        assert_eq!(capped(128, 99), Some("99+".to_string()));
        assert_eq!(capped(99, 99), None);
        assert_eq!(capped(10, 9), Some("9+".to_string()));
    }
}
