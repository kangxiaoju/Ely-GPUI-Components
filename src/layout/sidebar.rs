use gpui::{
    Animation, AnimationExt, AnyElement, App, ElementId, InteractiveElement, IntoElement,
    ParentElement, Pixels, RenderOnce, Styled, Window, div, prelude::FluentBuilder as _,
};
use smallvec::SmallVec;
use web_time::Instant;

use crate::{motion, theme::ActiveTheme};

/// The side of the content where a sidebar sits; its divider faces the content.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SidebarPlacement {
    /// The sidebar is before the content, with its divider on the right.
    #[default]
    Left,
    /// The sidebar is after the content, with its divider on the left.
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

pub(crate) fn sidebar_width_at(
    collapsed: bool,
    expanded: Pixels,
    collapsed_width: Pixels,
    t: f32,
) -> Pixels {
    let (from, to) = if collapsed {
        (expanded, collapsed_width)
    } else {
        (collapsed_width, expanded)
    };
    from + (to - from) * t
}

struct Width {
    collapsed: bool,
    generation: u64,
    changed: Instant,
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

    /// Places the sidebar on one side of its content, with the divider facing inward.
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
        let state = window.use_keyed_state(self.id.clone(), cx, |_, _| Width {
            collapsed,
            generation: 0,
            changed: Instant::now(),
        });
        if state.read(cx).collapsed != collapsed {
            state.update(cx, |width, _| {
                width.collapsed = collapsed;
                width.generation += 1;
                width.changed = Instant::now();
            });
        }
        let (generation, changed) = {
            let width = state.read(cx);
            (width.generation, width.changed)
        };
        let theme = cx.theme();
        let wide = self
            .expanded_width
            .unwrap_or_else(|| theme.sidebar_width(false).to_pixels(window.rem_size()));
        let narrow = self
            .collapsed_width
            .unwrap_or_else(|| theme.sidebar_width(true).to_pixels(window.rem_size()));
        let (border_left, border_right) = self.placement.border();
        let duration = motion::duration(motion::BASE, cx);
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
            .border_color(theme.colors.border)
            .bg(theme.colors.bg)
            .children(self.body);
        if generation == 0 || changed.elapsed() >= duration {
            return column
                .w(if collapsed { narrow } else { wide })
                .into_any_element();
        }
        column
            .with_animation(
                ("width", generation),
                Animation::new(duration).with_easing(motion::ease_in_out_cubic),
                move |column, t| column.w(sidebar_width_at(collapsed, wide, narrow, t)),
            )
            .into_any_element()
    }
}
