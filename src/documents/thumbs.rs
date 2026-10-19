use std::rc::Rc;

use gpui::{
    App, ElementId, Entity, InteractiveElement, IntoElement, MouseButton, ParentElement, RenderOnce,
    StatefulInteractiveElement, Styled, Window, div, prelude::*,
};

use super::viewer::{
    DocPage, PageCache, PageRequestKind, PageSource, StaticPages, page_picture, page_resource,
};
use crate::{
    forms::Pick,
    theme::{ActiveTheme, Radius, TextSize},
    typography::tabular,
};

/// Share of the label column a thumbnail spans.
const THUMB: f32 = 0.75;

/// Pages as small pictures down a column, each numbered, the current one outlined; a press goes to a page.
#[derive(IntoElement)]
pub struct PageThumbnailList {
    id: ElementId,
    source: Rc<dyn PageSource>,
    cache: Option<Entity<PageCache>>,
    current: usize,
    on_pick: Option<Pick>,
}

impl PageThumbnailList {
    pub fn new(
        id: impl Into<ElementId>,
        pages: impl Into<Rc<Vec<DocPage>>>,
        current: usize,
    ) -> Self {
        Self {
            id: id.into(),
            source: Rc::new(StaticPages(pages.into())),
            cache: None,
            current,
            on_pick: None,
        }
    }

    pub(crate) fn source(
        id: impl Into<ElementId>,
        source: Rc<dyn PageSource>,
        cache: Entity<PageCache>,
        current: usize,
    ) -> Self {
        Self {
            id: id.into(),
            source,
            cache: Some(cache),
            current,
            on_pick: None,
        }
    }

    pub fn on_pick(mut self, handler: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_pick = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for PageThumbnailList {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let width = cx.theme().label_width().to_pixels(window.rem_size()) * THUMB;
        let cache = self.cache.unwrap_or_else(|| {
            window.use_keyed_state((self.id.clone(), "page-cache"), cx, |_, _| {
                PageCache::new(64 * 1024 * 1024)
            })
        });
        let visible = self.current.saturating_sub(2)..(self.current + 3).min(self.source.len());
        let requested: std::collections::HashMap<usize, _> = visible
            .map(|ix| {
                let scale = f32::from(width) / self.source.size(ix).width;
                (
                    ix,
                    page_resource(
                        &self.source,
                        &cache,
                        ix,
                        scale,
                        PageRequestKind::Thumbnail,
                        window,
                        cx,
                    ),
                )
            })
            .collect();
        let theme = cx.theme();
        let colors = theme.colors.clone();
        div()
            .id(self.id.clone())
            .h_full()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .items_center()
            .gap_3()
            .p_3()
            .children((0..self.source.len()).map(|ix| {
                let shape = self.source.size(ix);
                let current = ix == self.current;
                let pick = self.on_pick.clone();
                div()
                    .id((self.id.clone(), format!("thumb-{ix}")))
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_1()
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
                    .when_some(pick, |thumb, pick| {
                        thumb.on_click(move |_, window, cx| {
                            log::info!("page thumbnails: page {ix}");
                            pick(ix, window, cx)
                        })
                    })
                    .child(
                        div()
                            .w(width)
                            .h(width * (shape.height / shape.width))
                            .rounded(theme.radius(Radius::Sm))
                            .border_1()
                            .border_color(if current { colors.focus } else { colors.border })
                            .bg(colors.paper)
                            .when_some(requested.get(&ix).cloned(), |thumb, resource| {
                                thumb.child(page_picture(
                                    (self.id.clone(), format!("thumb-picture-{ix}")).into(),
                                    resource,
                                    theme,
                                ))
                            }),
                    )
                    .child(
                        tabular(div().text_size(theme.text_size(TextSize::Xs)))
                            .text_color(if current { colors.fg } else { colors.fg_subtle })
                            .child(format!("{}", ix + 1)),
                    )
            }))
    }
}
