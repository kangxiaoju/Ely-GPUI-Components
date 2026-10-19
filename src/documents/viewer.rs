use std::{
    collections::{HashMap, VecDeque},
    rc::Rc,
};

use gpui::{
    AnyElement, App, Bounds, ElementId, Entity, ImageSource, InteractiveElement, IntoElement,
    MouseButton, ParentElement, Pixels, Point, RenderOnce, ScrollHandle, SharedString, Size,
    StatefulInteractiveElement, Styled, Window, canvas, div, img, point, prelude::*, size,
};

use super::{
    find::{Find, reveal},
    thumbs::PageThumbnailList,
};
use crate::{
    buttons::{ButtonVariant, IconButton, ToggleButton, ToggleItem},
    chat::ErrorMessage,
    documents::blocks::source,
    editor::FindWidget,
    motion::Spinner,
    overlays::HoverCard,
    primitives::{Icon, IconName},
    shell::ToolbarSeparator,
    theme::{ActiveTheme, ControlSize, Elevation, IconSize, Radius, TextSize},
    typography::tabular,
};

/// Zoom steps, as shares of a page's own size.
const STEPS: [f32; 9] = [0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 2.0, 3.0, 4.0];

/// A page the app drew: its picture, a file or a web address, and its size in points.
#[derive(Clone, Debug, PartialEq)]
pub struct DocPage {
    pub source: SharedString,
    pub width: f32,
    pub height: f32,
}

/// Which representation of a document page the viewer asks its owner to draw.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum PageRequestKind {
    Page,
    Thumbnail,
}

/// The current result of asking a lazy page source for an image.
#[derive(Clone)]
pub enum PageResource {
    Loading,
    Ready { image: ImageSource, pixels: usize },
    Failed(SharedString),
}

/// Pages supplied on demand. Sizes are in document points and resources may change on later asks.
pub trait PageSource {
    fn len(&self) -> usize;
    fn size(&self, page: usize) -> Size<f32>;
    fn request(
        &self,
        page: usize,
        scale: f32,
        kind: PageRequestKind,
        window: &mut Window,
        app: &mut App,
    ) -> PageResource;
}

#[derive(Clone)]
pub(crate) struct StaticPages(pub(crate) Rc<Vec<DocPage>>);

impl PageSource for StaticPages {
    fn len(&self) -> usize {
        self.0.len()
    }

    fn size(&self, page: usize) -> Size<f32> {
        let page = &self.0[page];
        size(page.width, page.height)
    }

    fn request(
        &self,
        page: usize,
        _: f32,
        _: PageRequestKind,
        _: &mut Window,
        _: &mut App,
    ) -> PageResource {
        PageResource::Ready {
            image: source(&self.0[page].source),
            pixels: (self.0[page].width * self.0[page].height) as usize,
        }
    }
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
struct PageKey {
    page: usize,
    scale: u16,
    kind: PageRequestKind,
}

pub(crate) struct PageCache {
    resources: HashMap<PageKey, PageResource>,
    order: VecDeque<PageKey>,
    pixels: usize,
    budget: usize,
}

impl PageCache {
    pub(crate) fn new(budget: usize) -> Self {
        Self {
            resources: HashMap::new(),
            order: VecDeque::new(),
            pixels: 0,
            budget,
        }
    }

    fn get(&mut self, key: PageKey) -> Option<PageResource> {
        let resource = self.resources.get(&key)?.clone();
        self.order.retain(|cached| *cached != key);
        self.order.push_back(key);
        Some(resource)
    }

    fn insert(&mut self, key: PageKey, resource: PageResource) {
        if let Some(PageResource::Ready { pixels, .. }) = self.resources.remove(&key) {
            self.pixels = self.pixels.saturating_sub(pixels);
        }
        self.order.retain(|cached| *cached != key);
        if let PageResource::Ready { pixels, .. } = &resource {
            self.pixels = self.pixels.saturating_add(*pixels);
        }
        self.resources.insert(key, resource);
        self.order.push_back(key);
        while self.pixels > self.budget && self.resources.len() > 1 {
            let Some(oldest) = self.order.pop_front() else { break };
            if let Some(PageResource::Ready { pixels, .. }) = self.resources.remove(&oldest) {
                self.pixels = self.pixels.saturating_sub(pixels);
            }
        }
    }
}

pub(crate) fn page_resource(
    source: &Rc<dyn PageSource>,
    cache: &Entity<PageCache>,
    page: usize,
    scale: f32,
    kind: PageRequestKind,
    window: &mut Window,
    cx: &mut App,
) -> PageResource {
    let key = PageKey {
        page,
        scale: (scale * 100.0).round().clamp(1.0, u16::MAX as f32) as u16,
        kind,
    };
    if let Some(resource) = cache.update(cx, |cache, _| cache.get(key)) {
        return resource;
    }
    let resource = source.request(page, scale, kind, window, cx);
    if !matches!(resource, PageResource::Loading) {
        cache.update(cx, |cache, _| cache.insert(key, resource.clone()));
    }
    resource
}

pub(crate) fn page_picture(
    id: ElementId,
    resource: PageResource,
    theme: &crate::theme::Theme,
) -> AnyElement {
    match resource {
        PageResource::Loading => div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(Spinner::new((id, "loading")).size(IconSize::Sm))
            .into_any_element(),
        PageResource::Ready { image, .. } => img(image).id(id).size_full().into_any_element(),
        PageResource::Failed(error) => div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .p_3()
            .text_size(theme.text_size(TextSize::Sm))
            .child(ErrorMessage::new((id, "error"), "Page could not be loaded").detail(error))
            .into_any_element(),
    }
}

/// A stretch of a page a search found, in the page's points.
#[derive(Clone, Debug, PartialEq)]
pub struct PageHit {
    pub page: usize,
    pub bounds: Bounds<f32>,
}

/// A note pinned to a page, at a point in its points.
#[derive(Clone, Debug, PartialEq)]
pub struct PageNote {
    pub page: usize,
    pub at: Point<f32>,
    pub text: SharedString,
}

type OnNote = Rc<dyn Fn(usize, Point<f32>, &mut Window, &mut App)>;
type OnZoom = Rc<dyn Fn(f32, &mut Window, &mut App)>;

/// The zoom step past `zoom` in `direction`, when there is one.
pub(crate) fn step(zoom: f32, up: bool) -> Option<f32> {
    if up {
        STEPS.into_iter().find(|step| *step > zoom + f32::EPSILON)
    } else {
        STEPS
            .into_iter()
            .rev()
            .find(|step| *step < zoom - f32::EPSILON)
    }
}

/// The first page whose bottom lies past `y`, or the last; the first before any layout.
pub(crate) fn page_at(bottoms: &[Pixels], y: Pixels) -> usize {
    bottoms
        .iter()
        .position(|bottom| *bottom >= y)
        .unwrap_or(bottoms.len().saturating_sub(1))
}

/// The page under the middle of the view, from the last layout.
fn page_in_view(scroll: &ScrollHandle, count: usize) -> usize {
    let middle = scroll.bounds().center().y - scroll.offset().y;
    let bottoms: Vec<Pixels> = (0..count)
        .map_while(|ix| scroll.bounds_for_item(ix))
        .map(|page| page.bottom())
        .collect();
    page_at(&bottoms, middle)
}

/// Pages the app drew, one under another on a desk: the page in view and its count, zoom by steps or to the width, the pages as thumbnails beside, a find bar whose hits light on the page, and notes pinned where a press puts them.
#[derive(IntoElement)]
pub struct DocumentViewer {
    id: ElementId,
    title: SharedString,
    source: Rc<dyn PageSource>,
    cache_pixel_budget: usize,
    zoom: f32,
    on_zoom: Option<OnZoom>,
    thumbnails: bool,
    find: Option<Find>,
    notes: Vec<PageNote>,
    on_note: Option<OnNote>,
}

impl DocumentViewer {
    pub fn new(
        id: impl Into<ElementId>,
        title: impl Into<SharedString>,
        pages: impl Into<Rc<Vec<DocPage>>>,
    ) -> Self {
        let pages = pages.into();
        for page in pages.iter() {
            assert!(
                page.width > 0.0 && page.height > 0.0,
                "page {} has no size",
                page.source
            );
        }
        Self::source(id, title, StaticPages(pages))
    }

    pub fn source(
        id: impl Into<ElementId>,
        title: impl Into<SharedString>,
        source: impl PageSource + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            source: Rc::new(source),
            cache_pixel_budget: 64 * 1024 * 1024,
            zoom: 1.0,
            on_zoom: None,
            thumbnails: false,
            find: None,
            notes: Vec::new(),
            on_note: None,
        }
    }

    /// Caps retained decoded page and thumbnail pixels.
    pub fn cache_pixel_budget(mut self, pixels: usize) -> Self {
        self.cache_pixel_budget = pixels;
        self
    }

    /// Scales pages from their size in points; the owner keeps the zoom the steps and fit ask for.
    pub fn zoom(
        mut self,
        zoom: f32,
        on_zoom: impl Fn(f32, &mut Window, &mut App) + 'static,
    ) -> Self {
        assert!(zoom > 0.0, "a zoom of {zoom} shows nothing");
        self.zoom = zoom;
        self.on_zoom = Some(Rc::new(on_zoom));
        self
    }

    /// Shows the pages small beside the large ones.
    pub fn thumbnails(mut self) -> Self {
        self.thumbnails = true;
        self
    }

    /// The owner's find box, floating over the pages; the owner searches them, and `hits` light where they are, the current one scrolled into view.
    pub fn find(mut self, widget: FindWidget, hits: Vec<PageHit>, current: Option<usize>) -> Self {
        let pages = self.source.len();
        let on_page = |hit: &PageHit| hit.page < pages;
        if current.is_some_and(|current| current >= hits.len()) || !hits.iter().all(on_page) {
            log::error!(
                "document viewer: hit {current:?} of {}, some past {pages} pages; those left",
                hits.len()
            );
        }
        let current = current
            .filter(|current| hits.get(*current).is_some_and(on_page))
            .map(|current| hits[..current].iter().filter(|hit| on_page(hit)).count());
        let hits: Vec<PageHit> = hits.into_iter().filter(on_page).collect();
        self.find = Some(Find {
            widget,
            hits,
            current,
        });
        self
    }

    /// Notes on the pages; with `on_note`, a press on a page while notes are on asks the owner to pin one there.
    pub fn notes(
        mut self,
        notes: Vec<PageNote>,
        on_note: impl Fn(usize, Point<f32>, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.notes = notes;
        self.on_note = Some(Rc::new(on_note));
        self
    }
}

/// The viewer's own state: its scroll, the page in view, the width it fits, whether presses pin notes, and the hit last brought into view.
#[derive(Default)]
struct Desk {
    scroll: ScrollHandle,
    page: usize,
    width: f32,
    pinning: bool,
    shown_hit: Option<PageHit>,
}

impl RenderOnce for DocumentViewer {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let desk = window.use_keyed_state((self.id.clone(), "desk"), cx, |_, _| Desk::default());
        let cache = window.use_keyed_state((self.id.clone(), "page-cache"), cx, |_, _| {
            PageCache::new(self.cache_pixel_budget)
        });
        cache.update(cx, |cache, _| cache.budget = self.cache_pixel_budget);
        let (scroll, page_now, width, pinning) = {
            let desk = desk.read(cx);
            (desk.scroll.clone(), desk.page, desk.width, desk.pinning)
        };
        let zoom = self.zoom;
        let nearby = page_now.saturating_sub(2)..(page_now + 3).min(self.source.len());
        let requested: HashMap<usize, PageResource> = nearby
            .map(|ix| {
                let resource = page_resource(
                    &self.source,
                    &cache,
                    ix,
                    zoom,
                    PageRequestKind::Page,
                    window,
                    cx,
                );
                (ix, resource)
            })
            .collect();
        let theme = cx.theme();
        let colors = theme.colors.clone();
        let count = self.source.len();
        let go = |page: usize, scroll: &ScrollHandle| {
            let scroll = scroll.clone();
            move |_: &gpui::ClickEvent, _: &mut Window, _: &mut App| {
                scroll.scroll_to_top_of_item(page)
            }
        };
        let zoom_to = |next: Option<f32>, icon: IconName, tip: &'static str| {
            let on_zoom = self.on_zoom.clone();
            IconButton::new((self.id.clone(), tip), icon)
                .variant(ButtonVariant::Ghost)
                .size(ControlSize::Sm)
                .tooltip(tip)
                .disabled(next.is_none() || on_zoom.is_none())
                .when_some(next.zip(on_zoom), |button, (next, on_zoom)| {
                    button.on_click(move |_, window, cx| on_zoom(next, window, cx))
                })
        };
        let widest = (0..count).map(|page| self.source.size(page).width).fold(0.0, f32::max);
        let fit = (width > 0.0 && widest > 0.0)
            .then(|| (width / widest).clamp(STEPS[0], STEPS[STEPS.len() - 1]));
        let header = div()
            .flex()
            .items_center()
            .gap_2()
            .px_3()
            .h(theme.control_height(ControlSize::Lg))
            .border_b_1()
            .border_color(colors.border)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_color(colors.fg_muted)
                    .child(self.title.clone()),
            )
            .child(
                IconButton::new((self.id.clone(), "previous"), IconName::ChevronUp)
                    .variant(ButtonVariant::Ghost)
                    .size(ControlSize::Sm)
                    .tooltip("Previous page")
                    .disabled(page_now == 0)
                    .on_click(go(page_now.saturating_sub(1), &scroll)),
            )
            .child(
                tabular(div().text_size(theme.text_size(TextSize::Sm)))
                    .child(format!("{} of {count}", page_now + 1)),
            )
            .child(
                IconButton::new((self.id.clone(), "next"), IconName::ChevronDown)
                    .variant(ButtonVariant::Ghost)
                    .size(ControlSize::Sm)
                    .tooltip("Next page")
                    .disabled(page_now + 1 >= count)
                    .on_click(go((page_now + 1).min(count.saturating_sub(1)), &scroll)),
            )
            .child(ToolbarSeparator)
            .child(zoom_to(step(zoom, false), IconName::ZoomOut, "Zoom out"))
            .child(
                tabular(
                    div()
                        .min_w_10()
                        .text_center()
                        .text_size(theme.text_size(TextSize::Sm)),
                )
                .child(format!("{:.0}%", zoom * 100.0)),
            )
            .child(zoom_to(step(zoom, true), IconName::ZoomIn, "Zoom in"))
            .child(zoom_to(fit, IconName::Maximize2, "Fit to width"))
            .when(self.on_note.is_some(), |header| {
                let desk = desk.clone();
                header.child(
                    ToggleButton::new(
                        (self.id.clone(), "pin"),
                        ToggleItem::new("notes")
                            .icon(IconName::MessageSquare)
                            .tooltip("Pin notes"),
                        pinning,
                    )
                    .size(ControlSize::Sm)
                    .on_toggle(move |on, _, cx| {
                        desk.update(cx, |desk, cx| {
                            desk.pinning = on;
                            cx.notify();
                        })
                    }),
                )
            });
        let (widget, hits, current) = match self.find {
            Some(Find {
                widget,
                hits,
                current,
            }) => (Some(widget), hits, current),
            None => (None, Vec::new(), None),
        };
        let target = current.map(|current| hits[current].clone());
        let pages = (0..count).map(|ix| {
            let shape = self.source.size(ix);
            let (on_note, scroll) = (self.on_note.clone().filter(|_| pinning), scroll.clone());
            div()
                .id((self.id.clone(), format!("page-{ix}")))
                .relative()
                .flex_none()
                .when(width == 0.0 || shape.width * zoom <= width, |page| page.mx_auto())
                .w(Pixels::from(shape.width * zoom))
                .h(Pixels::from(shape.height * zoom))
                .bg(colors.paper)
                .shadow(theme.elevation(Elevation::Raised))
                .when(pinning, |page| page.cursor_crosshair())
                .when_some(on_note, |page, on_note| {
                    page.on_mouse_down(MouseButton::Left, move |event, window, cx| {
                        let Some(bounds) = scroll.bounds_for_item(ix) else {
                            return;
                        };
                        let at = event.position - bounds.origin - scroll.offset();
                        let at = point(f32::from(at.x) / zoom, f32::from(at.y) / zoom);
                        log::info!("document viewer: note asked on page {ix} at {at:?}");
                        on_note(ix, at, window, cx);
                    })
                })
                .when_some(requested.get(&ix).cloned(), |page, resource| {
                    page.child(page_picture(
                        (self.id.clone(), format!("picture-{ix}")).into(),
                        resource,
                        theme,
                    ))
                })
                .children(
                    hits.iter()
                        .enumerate()
                        .filter(|(_, hit)| hit.page == ix)
                        .map(|(at, hit)| {
                            div()
                                .absolute()
                                .left(Pixels::from(hit.bounds.origin.x * zoom))
                                .top(Pixels::from(hit.bounds.origin.y * zoom))
                                .w(Pixels::from(hit.bounds.size.width * zoom))
                                .h(Pixels::from(hit.bounds.size.height * zoom))
                                .rounded(theme.radius(Radius::Sm))
                                .bg(if current == Some(at) {
                                    colors.warning.opacity(0.45)
                                } else {
                                    colors.warning.opacity(0.2)
                                })
                        }),
                )
                .children(
                    self.notes
                        .iter()
                        .enumerate()
                        .filter(|(_, note)| note.page == ix)
                        .map(|(at, note)| {
                            let text = note.text.clone();
                            div()
                                .absolute()
                                .left(Pixels::from(note.at.x * zoom))
                                .top(Pixels::from(note.at.y * zoom))
                                .child(HoverCard::new(
                                    (self.id.clone(), format!("note-{at}")),
                                    div()
                                        .size_6()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .rounded(theme.radius(Radius::Sm))
                                        .bg(colors.warning)
                                        .child(
                                            Icon::new(IconName::MessageSquare)
                                                .size(IconSize::Xs)
                                                .color(colors.on_accent),
                                        ),
                                    move |_, _| div().max_w_64().child(text.clone()),
                                ))
                        }),
                )
        });
        let (tracked, measured) = (desk.clone(), desk.clone());
        let body = div()
            .flex()
            .flex_1()
            .min_h_0()
            .when(self.thumbnails, |body| {
                let scroll = scroll.clone();
                body.child(
                    div()
                        .flex_none()
                        .border_r_1()
                        .border_color(colors.border)
                        .child(
                            PageThumbnailList::source(
                                (self.id.clone(), "thumbs"),
                                self.source.clone(),
                                cache.clone(),
                                page_now,
                            )
                            .on_pick(move |page, _, _| scroll.scroll_to_top_of_item(page)),
                        ),
                )
            })
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_w_0()
                    .bg(colors.sunken)
                    .child(
                        div()
                            .id((self.id.clone(), "pages"))
                            .size_full()
                            .overflow_scroll()
                            .track_scroll(&scroll)
                            .flex()
                            .flex_col()
                            .gap_4()
                            .p_6()
                            .children(pages),
                    )
                    .child(
                        canvas(
                            move |bounds, window, cx| {
                                let (page, width) = {
                                    let desk = tracked.read(cx);
                                    (
                                        page_in_view(&desk.scroll, count),
                                        f32::from(bounds.size.width),
                                    )
                                };
                                if (page, width) != (tracked.read(cx).page, tracked.read(cx).width)
                                {
                                    measured.update(cx, |desk, cx| {
                                        desk.page = page;
                                        desk.width = width;
                                        cx.notify();
                                    });
                                    window.request_animation_frame();
                                }
                                if tracked.read(cx).shown_hit != target {
                                    if let Some(hit) = &target {
                                        log::info!("document viewer: hit on page {}", hit.page);
                                        reveal(&tracked.read(cx).scroll, hit, zoom);
                                        window.request_animation_frame();
                                    }
                                    measured.update(cx, |desk, _| desk.shown_hit = target.clone());
                                }
                            },
                            |_, _, _, _| {},
                        )
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full(),
                    )
                    .children(
                        widget.map(|widget| div().absolute().top_2().right_2().child(widget)),
                    ),
            );
        div()
            .id(self.id)
            .size_full()
            .flex()
            .flex_col()
            .rounded(theme.radius(Radius::Lg))
            .border_1()
            .border_color(colors.border)
            .overflow_hidden()
            .bg(colors.surface)
            .child(header)
            .child(body)
    }
}

#[cfg(test)]
mod tests {
    use gpui::px;

    use super::{page_at, step};

    #[test]
    fn the_page_in_view_is_the_one_under_the_middle() {
        let bottoms = [px(100.0), px(220.0), px(340.0)];
        assert_eq!(page_at(&bottoms, px(50.0)), 0);
        assert_eq!(page_at(&bottoms, px(150.0)), 1);
        assert_eq!(page_at(&bottoms, px(500.0)), 2, "past the end is the last");
        assert_eq!(page_at(&[], px(50.0)), 0, "no layout yet");
    }

    #[test]
    fn zoom_steps_walk_the_scale_and_stop_at_its_ends() {
        assert_eq!(step(1.0, true), Some(1.25));
        assert_eq!(step(1.0, false), Some(0.75));
        assert_eq!(
            step(1.1, false),
            Some(1.0),
            "between steps goes to the nearer side"
        );
        assert_eq!(step(4.0, true), None);
        assert_eq!(step(0.25, false), None);
    }
}
