use gpui::{
    Animation, AnimationExt, AnyElement, App, ElementId, InteractiveElement, IntoElement,
    ParentElement, Pixels, RenderOnce, Styled, Window, div, prelude::FluentBuilder as _,
};
use smallvec::SmallVec;
use web_time::Instant;

use crate::{motion, theme::ActiveTheme};

/// Which edge draws the divider facing the content. The caller places the sidebar before or after
/// the content in its flex layout; this value does not reorder elements.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SidebarPlacement {
    /// The caller placed the sidebar before the content, so the divider draws on the right.
    #[default]
    Left,
    /// The caller placed the sidebar after the content, so the divider draws on the left.
    Right,
}

impl SidebarPlacement {
    pub(crate) fn border(self) -> (bool, bool) {
        match self {
            Self::Left => (false, true),
            Self::Right => (true, false),
        }
    }
}

pub(crate) fn sidebar_width_at(from: Pixels, to: Pixels, t: f32) -> Pixels {
    from + (to - from) * t
}

struct Width {
    from: Pixels,
    to: Pixels,
    generation: u64,
    changed: Instant,
}

impl Width {
    fn current(&self, duration: std::time::Duration) -> Pixels {
        let share = (self.changed.elapsed().as_secs_f32()
            / duration.as_secs_f32().max(f32::EPSILON))
        .min(1.0);
        sidebar_width_at(self.from, self.to, motion::ease_in_out_cubic(share))
    }
}

/// Side column that folds to an icon rail, width animated.
#[derive(IntoElement)]
pub struct Sidebar {
    id: ElementId,
    collapsed: bool,
    expanded_width: Option<Pixels>,
    collapsed_width: Option<Pixels>,
    placement: SidebarPlacement,
    body: SmallVec<[AnyElement; 4]>,
}

impl Sidebar {
    /// Children should render their own compact form when `collapsed`.
    pub fn new(id: impl Into<ElementId>, collapsed: bool) -> Self {
        Self {
            id: id.into(),
            collapsed,
            expanded_width: None,
            collapsed_width: None,
            placement: SidebarPlacement::Left,
            body: SmallVec::new(),
        }
    }

    /// A fixed width for the expanded sidebar instead of the theme width.
    pub fn expanded_width(mut self, width: Pixels) -> Self {
        self.expanded_width = Some(width);
        self
    }

    /// A fixed width for the collapsed sidebar instead of the theme width.
    pub fn collapsed_width(mut self, width: Pixels) -> Self {
        self.collapsed_width = Some(width);
        self
    }

    /// Chooses which edge draws the content-side divider. The caller still places the sidebar
    /// before or after the content in its flex layout.
    pub fn placement(mut self, placement: SidebarPlacement) -> Self {
        self.placement = placement;
        self
    }
}

impl ParentElement for Sidebar {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.body.extend(elements);
    }
}

impl RenderOnce for Sidebar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let collapsed = self.collapsed;
        let (wide, narrow, border_color, background) = {
            let theme = cx.theme();
            (
                self.expanded_width
                    .unwrap_or_else(|| theme.sidebar_width(false).to_pixels(window.rem_size())),
                self.collapsed_width
                    .unwrap_or_else(|| theme.sidebar_width(true).to_pixels(window.rem_size())),
                theme.colors.border,
                theme.colors.bg,
            )
        };
        let target = if collapsed { narrow } else { wide };
        let duration = motion::duration(motion::BASE, cx);
        let state = window.use_keyed_state(self.id.clone(), cx, |_, _| Width {
            from: target,
            to: target,
            generation: 0,
            changed: Instant::now(),
        });
        if state.read(cx).to != target {
            state.update(cx, |width, _| {
                width.from = width.current(duration);
                width.to = target;
                width.generation += 1;
                width.changed = Instant::now();
            });
        }
        let (from, to, generation, changed) = {
            let width = state.read(cx);
            (width.from, width.to, width.generation, width.changed)
        };
        let (border_left, border_right) = self.placement.border();
        let column = div()
            .id(self.id)
            .debug_selector(|| "sidebar".into())
            .flex()
            .flex_col()
            .flex_none()
            .h_full()
            .overflow_hidden()
            .when(border_left, |column| column.border_l_1())
            .when(border_right, |column| column.border_r_1())
            .border_color(border_color)
            .bg(background)
            .children(self.body);
        if generation == 0 || changed.elapsed() >= duration {
            return column.w(to).into_any_element();
        }
        column
            .with_animation(
                ("width", generation),
                Animation::new(duration).with_easing(motion::ease_in_out_cubic),
                move |column, t| column.w(sidebar_width_at(from, to, t)),
            )
            .into_any_element()
    }
}
