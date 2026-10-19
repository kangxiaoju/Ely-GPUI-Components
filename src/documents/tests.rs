use std::{cell::RefCell, rc::Rc};

use gpui::{
    App, AppContext as _, Bounds, Context, Entity, Focusable, ImageSource, IntoElement, Modifiers,
    ParentElement, Pixels, Point, Render, ScrollDelta, ScrollWheelEvent, Size, Styled,
    TestAppContext, TouchPhase, VisualTestContext, Window, div, point, px, size,
};

use super::{
    DocPage, DocumentViewer, PageHit, PageRequestKind, PageResource, PageSource, ZenMode,
};
use crate::{
    editor::FindWidget,
    forms::{Input, TextInput},
    theme::{ActiveTheme, ControlSize, Theme},
};

type Noted = Rc<RefCell<Option<(usize, Point<f32>)>>>;

/// A hit on `page`, 400 points down.
fn hit(page: usize) -> PageHit {
    PageHit {
        page,
        bounds: Bounds::new(point(40.0, 400.0), size(60.0, 12.0)),
    }
}

/// Three tall pages and hits; a pinning press records notes.
struct Reader {
    query: Entity<TextInput>,
    width: f32,
    hits: Vec<PageHit>,
    current: Option<usize>,
    noted: Noted,
}

impl Render for Reader {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let pages: Vec<DocPage> = (0..3)
            .map(|ix| DocPage {
                source: format!("page-{ix}.png").into(),
                width: self.width,
                height: 1000.0,
            })
            .collect();
        let noted = self.noted.clone();
        div().size_full().child(
            DocumentViewer::new("viewer", "lift.pdf", pages)
                .find(
                    FindWidget::new("find", &self.query, self.current, self.hits.len()),
                    self.hits.clone(),
                    self.current,
                )
                .notes(Vec::new(), move |page, at, _, _| {
                    *noted.borrow_mut() = Some((page, at))
                }),
        )
    }
}

fn open(cx: &mut TestAppContext) -> (Entity<Reader>, Noted, &mut VisualTestContext) {
    open_with(400.0, None, cx)
}

/// Pages `width` wide, a hit on page two, maybe current.
fn open_with(
    width: f32,
    current: Option<usize>,
    cx: &mut TestAppContext,
) -> (Entity<Reader>, Noted, &mut VisualTestContext) {
    cx.update(Theme::init);
    let noted = Noted::default();
    let seen = noted.clone();
    let (reader, cx) = cx.add_window_view(move |window, cx| Reader {
        query: cx.new(|cx| TextInput::new(window, cx)),
        width,
        hits: vec![hit(1)],
        current,
        noted: seen,
    });
    settle(cx);
    (reader, noted, cx)
}

fn settle(cx: &mut VisualTestContext) {
    cx.run_until_parked();
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
}

#[derive(Clone)]
struct Requested(Rc<RefCell<Vec<(usize, PageRequestKind)>>>);

impl PageSource for Requested {
    fn len(&self) -> usize {
        1_000
    }

    fn size(&self, _: usize) -> Size<f32> {
        size(400.0, 1_000.0)
    }

    fn request(
        &self,
        page: usize,
        _: f32,
        kind: PageRequestKind,
        _: &mut Window,
        _: &mut App,
    ) -> PageResource {
        self.0.borrow_mut().push((page, kind));
        PageResource::Ready {
            image: ImageSource::from(format!("page-{page}.png")),
            pixels: 400_000,
        }
    }
}

struct LazyReader(Requested, usize);

impl Render for LazyReader {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(
            DocumentViewer::source("lazy-viewer", "long.pdf", self.0.clone())
                .cache_pixel_budget(self.1),
        )
    }
}

#[gpui::test]
fn document_viewer_requests_only_visible_and_neighbor_pages(cx: &mut TestAppContext) {
    cx.update(Theme::init);
    let requests = Rc::new(RefCell::new(Vec::new()));
    let source = Requested(requests.clone());
    let (_, cx) = cx.add_window_view(move |_, _| LazyReader(source, 8_000_000));
    cx.simulate_resize(size(px(840.0), px(520.0)));
    settle(cx);

    let full: Vec<usize> = requests
        .borrow()
        .iter()
        .filter_map(|(page, kind)| (*kind == PageRequestKind::Page).then_some(*page))
        .collect();
    assert!(full.len() < 8, "requested {full:?}");
    assert!(full.iter().all(|page| *page < 8), "requested {full:?}");

    let before = requests.borrow().len();
    settle(cx);
    assert_eq!(requests.borrow().len(), before, "the same viewport reuses its cache");
}

#[gpui::test]
fn document_viewer_evicts_pages_past_its_pixel_budget(cx: &mut TestAppContext) {
    cx.update(Theme::init);
    let requests = Rc::new(RefCell::new(Vec::new()));
    let source = Requested(requests.clone());
    let (_, cx) = cx.add_window_view(move |_, _| LazyReader(source, 500_000));
    cx.simulate_resize(size(px(840.0), px(520.0)));
    settle(cx);
    let first = requests.borrow().len();

    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert!(
        requests.borrow().len() > first,
        "an evicted neighbor is requested again"
    );
}

#[derive(Clone)]
struct LoadingSource(Rc<RefCell<usize>>);

impl PageSource for LoadingSource {
    fn len(&self) -> usize {
        1
    }

    fn size(&self, _: usize) -> Size<f32> {
        size(400.0, 1_000.0)
    }

    fn request(
        &self,
        _: usize,
        _: f32,
        _: PageRequestKind,
        window: &mut Window,
        cx: &mut App,
    ) -> PageResource {
        *self.0.borrow_mut() += 1;
        let image = ImageSource::from("ready.png");
        PageResource::loading(window.spawn(cx, async move |_| PageResource::Ready {
            image,
            pixels: 400_000,
        }))
    }
}

struct LoadingReader(LoadingSource);

impl Render for LoadingReader {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        DocumentViewer::source("loading-viewer", "loading.pdf", self.0.clone())
    }
}

#[gpui::test]
fn document_viewer_deduplicates_an_in_flight_page_request(cx: &mut TestAppContext) {
    cx.update(Theme::init);
    let requests = Rc::new(RefCell::new(0));
    let source = LoadingSource(requests.clone());
    let (_, cx) = cx.add_window_view(move |_, _| LoadingReader(source));
    for _ in 0..4 {
        cx.update(|window, _| window.refresh());
    }
    settle(cx);
    assert_eq!(*requests.borrow(), 1, "one request remains in flight");
}

#[gpui::test]
fn document_viewer_does_not_retain_a_resource_larger_than_its_budget(cx: &mut TestAppContext) {
    cx.update(Theme::init);
    let requests = Rc::new(RefCell::new(Vec::new()));
    let source = Requested(requests.clone());
    let (_, cx) = cx.add_window_view(move |_, _| LazyReader(source, 0));
    settle(cx);
    let first = requests.borrow().len();
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
    assert!(requests.borrow().len() > first, "a zero budget retains no image");
}

struct ThumbnailReader(Requested);

impl Render for ThumbnailReader {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        DocumentViewer::source("thumb-viewer", "long.pdf", self.0.clone()).thumbnails()
    }
}

#[gpui::test]
fn document_viewer_requests_the_scrolled_thumbnail_viewport(cx: &mut TestAppContext) {
    cx.update(Theme::init);
    let requests = Rc::new(RefCell::new(Vec::new()));
    let source = Requested(requests.clone());
    let (_, cx) = cx.add_window_view(move |_, _| ThumbnailReader(source));
    cx.simulate_resize(size(px(840.0), px(520.0)));
    settle(cx);
    requests.borrow_mut().clear();

    cx.simulate_event(ScrollWheelEvent {
        position: point(px(80.0), px(300.0)),
        delta: ScrollDelta::Pixels(point(px(0.0), px(-100_000.0))),
        modifiers: Modifiers::none(),
        touch_phase: TouchPhase::Moved,
    });
    settle(cx);
    let thumbs: Vec<usize> = requests
        .borrow()
        .iter()
        .filter_map(|(page, kind)| (*kind == PageRequestKind::Thumbnail).then_some(*page))
        .collect();
    assert!(thumbs.iter().any(|page| *page > 250), "requested {thumbs:?}");
    assert!(thumbs.iter().all(|page| *page > 250), "requested {thumbs:?}");
}

/// A point on the pages, below the bars, mid-view.
fn on_pages(cx: &mut VisualTestContext) -> Point<Pixels> {
    cx.update(|window, _| point(window.viewport_size().width / 2.0, px(300.0)))
}

fn wheel(down: f32, cx: &mut VisualTestContext) {
    let at = on_pages(cx);
    cx.simulate_event(ScrollWheelEvent {
        position: at,
        delta: ScrollDelta::Pixels(point(px(0.0), px(-down))),
        modifiers: Modifiers::none(),
        touch_phase: TouchPhase::Moved,
    });
    settle(cx);
}

/// Turns pinning on, presses mid-page; the note asked.
fn pin(cx: &mut VisualTestContext, noted: &Noted) -> (usize, Point<f32>) {
    let at = on_pages(cx);
    pin_at(at, cx, noted)
}

fn pin_at(at: Point<Pixels>, cx: &mut VisualTestContext, noted: &Noted) -> (usize, Point<f32>) {
    let toggle = cx.update(|window, cx| {
        let header = cx
            .theme()
            .control_height(ControlSize::Lg)
            .to_pixels(window.rem_size());
        point(
            window.viewport_size().width - px(17.0),
            px(1.0) + header / 2.0,
        )
    });
    cx.simulate_click(toggle, Modifiers::none());
    settle(cx);
    cx.simulate_click(at, Modifiers::none());
    settle(cx);
    noted
        .borrow_mut()
        .take()
        .expect("a press while pinning asks for a note")
}

#[gpui::test]
fn a_note_lands_where_the_press_is_on_a_scrolled_page(cx: &mut TestAppContext) {
    let (_, noted, cx) = open(cx);
    let (page, top) = pin(cx, &noted);
    wheel(150.0, cx);
    let at = on_pages(cx);
    cx.simulate_click(at, Modifiers::none());
    settle(cx);
    let (scrolled_page, scrolled) = noted.borrow_mut().take().expect("pinning stays on");
    assert_eq!((scrolled_page, page), (0, 0));
    assert_eq!(scrolled.x, top.x);
    assert_eq!(scrolled.y, top.y + 150.0, "the press counts the scroll");
}

/// Scrolls `down`, makes the hit current, pins a note.
fn revealed(down: f32, cx: &mut TestAppContext) -> (usize, Point<f32>) {
    let (reader, noted, cx) = open(cx);
    if down > 0.0 {
        wheel(down, cx);
    }
    reader.update(cx, |reader, cx| {
        reader.current = Some(0);
        cx.notify();
    });
    settle(cx);
    pin(cx, &noted)
}

#[gpui::test]
fn a_current_hit_comes_to_one_place_from_any_scroll(cx: &mut TestAppContext) {
    let from_top = revealed(0.0, cx);
    let from_below = revealed(300.0, cx);
    assert_eq!(from_top.0, 1, "the hit's page comes into view");
    assert_eq!(from_top, from_below);
}

#[gpui::test]
fn a_page_wider_than_the_view_starts_at_its_left_edge(cx: &mut TestAppContext) {
    let (_, noted, cx) = open_with(3000.0, None, cx);
    let (page, at) = pin_at(point(px(60.0), px(300.0)), cx, &noted);
    assert_eq!(page, 0);
    assert!(
        at.x < 100.0,
        "the page's left edge is in reach, not at {}",
        at.x
    );
}

#[gpui::test]
fn a_new_hit_at_the_same_index_moves_the_view(cx: &mut TestAppContext) {
    let (reader, noted, cx) = open(cx);
    reader.update(cx, |reader, cx| {
        reader.current = Some(0);
        cx.notify();
    });
    settle(cx);
    reader.update(cx, |reader, cx| {
        reader.hits = vec![hit(2)];
        cx.notify();
    });
    settle(cx);
    assert_eq!(pin(cx, &noted).0, 2, "the view follows the new hit");
}

#[gpui::test]
fn a_hit_current_from_the_first_frame_comes_into_view(cx: &mut TestAppContext) {
    let (_, noted, first) = open_with(400.0, Some(0), cx);
    let at_once = pin(first, &noted);
    let later = revealed(0.0, cx);
    assert_eq!(at_once, later);
}

/// A field outside zen, and the Escapes that left zen.
struct Zen {
    outside: Entity<TextInput>,
    on: bool,
    left: usize,
}

impl Render for Zen {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let me = cx.entity();
        div().size_full().child(Input::new(&self.outside)).child(
            ZenMode::new("zen", self.on)
                .on_exit(move |_, cx| {
                    me.update(cx, |zen, cx| {
                        zen.on = false;
                        zen.left += 1;
                        cx.notify();
                    })
                })
                .child(div().child("A quiet page")),
        )
    }
}

#[gpui::test]
fn escape_leaves_zen_turned_on_from_outside_and_hands_focus_back(cx: &mut TestAppContext) {
    cx.update(|cx| {
        Theme::init(cx);
        crate::forms::bind_keys(cx);
    });
    let (zen, cx) = cx.add_window_view(|window, cx| Zen {
        outside: cx.new(|cx| TextInput::new(window, cx)),
        on: false,
        left: 0,
    });
    cx.update(|window, cx| {
        window.activate_window();
        let outside = zen.read(cx).outside.focus_handle(cx);
        window.focus(&outside, cx);
    });
    settle(cx);
    zen.update(cx, |zen, cx| {
        zen.on = true;
        cx.notify();
    });
    settle(cx);
    cx.simulate_keystrokes("escape");
    settle(cx);
    assert_eq!(zen.read_with(cx, |zen, _| zen.left), 1);
    let back = cx.update(|window, cx| zen.read(cx).outside.focus_handle(cx).is_focused(window));
    assert!(back, "focus returns to where it was");
}

#[gpui::test]
fn hits_past_the_pages_are_left_and_the_current_follows(cx: &mut TestAppContext) {
    let (reader, _, cx) = open_with(400.0, None, cx);
    reader.update(cx, |reader, cx| {
        reader.hits = vec![hit(7), hit(2), hit(9), hit(1)];
        reader.current = Some(3);
        cx.notify();
    });
    settle(cx);
    reader.update(cx, |reader, cx| {
        reader.current = Some(12);
        cx.notify();
    });
    settle(cx);
    let (_, _, cx) = open_with(400.0, Some(usize::MAX), cx);
    settle(cx);
}

/// Versions whose selected one left with a shorter history.
struct Versions(Vec<super::Version>, usize);

impl Render for Versions {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child(super::VersionHistory::new(
            "history",
            self.0.clone(),
            self.1,
        ))
    }
}

#[gpui::test]
fn a_selected_version_past_the_history_selects_none(cx: &mut TestAppContext) {
    cx.update(Theme::init);
    let version = super::Version {
        at: jiff::Timestamp::UNIX_EPOCH,
        author: "Ann".into(),
        markdown: "# Plan".into(),
    };
    let (view, cx) = cx.add_window_view(|_, _| Versions(vec![version], 4));
    settle(cx);
    view.update(cx, |view, cx| {
        *view = Versions(Vec::new(), usize::MAX);
        cx.notify();
    });
    settle(cx);
}

/// Markdown drawn `width` points wide, its pictures read in `base`.
struct Page {
    source: &'static str,
    base: std::path::PathBuf,
    width: f32,
}

impl Render for Page {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .w(px(self.width))
            .child(super::MarkdownRenderer::new("page", self.source).base(self.base.as_path()))
    }
}

const SHOT: &str = "Before

![shot](assets/wide%20shot.png)
";

/// A 200×100 picture in `source`, drawn in a `width` column.
fn drawn_picture(source: &'static str, width: f32, cx: &mut TestAppContext) -> gpui::Size<Pixels> {
    let named = format!(
        "ely-picture-{}-{width}-{}",
        std::process::id(),
        source.len()
    );
    let folder = std::env::temp_dir().join(named);
    std::fs::create_dir_all(folder.join("assets")).expect("a folder");
    let mut png = Vec::new();
    image::RgbaImage::new(200, 100)
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .expect("a png");
    std::fs::write(folder.join("assets/wide shot.png"), png).expect("the picture");
    cx.update(Theme::init);
    let (_, cx) = cx.add_window_view(|_, _| Page {
        source,
        base: folder.clone(),
        width,
    });
    let mut drawn = None;
    // The decode runs on the background executor.
    for _ in 0..50 {
        settle(cx);
        drawn = cx.debug_bounds("markdown-picture");
        if drawn.is_some() {
            break;
        }
    }
    std::fs::remove_dir_all(&folder).expect("cleaned");
    drawn.expect("the picture drew").size
}

#[gpui::test]
fn a_picture_beside_the_file_draws_at_its_ratio_within_the_column(cx: &mut TestAppContext) {
    let shrunk = drawn_picture(SHOT, 120.0, cx);
    assert_eq!(shrunk, size(px(120.0), px(60.0)), "shrunk at 2:1");
}

#[gpui::test]
fn a_picture_narrower_than_the_column_keeps_its_size(cx: &mut TestAppContext) {
    assert_eq!(drawn_picture(SHOT, 400.0, cx), size(px(200.0), px(100.0)));
}

#[gpui::test]
fn a_picture_in_a_table_cell_draws(cx: &mut TestAppContext) {
    let table = "| Picture |
| --- |
| ![shot](assets/wide%20shot.png) |
";
    assert_eq!(drawn_picture(table, 400.0, cx), size(px(200.0), px(100.0)));
}

#[gpui::test]
fn a_picture_in_a_narrow_cell_shrinks_within_it(cx: &mut TestAppContext) {
    let table = "| A | B |
| - | - |
| ![shot](assets/wide%20shot.png) | b |
";
    let drawn = drawn_picture(table, 160.0, cx);
    assert!(drawn.width < px(80.0), "within its half: {drawn:?}");
    assert_eq!(drawn.height, drawn.width / 2.0, "at its ratio");
}

#[gpui::test]
fn a_formula_that_fails_shows_why_and_its_source(cx: &mut TestAppContext) {
    cx.update(Theme::init);
    let (_, cx) = cx.add_window_view(|_, _| Page {
        source: "$$x^2^3$$\n",
        base: std::env::temp_dir(),
        width: 300.0,
    });
    settle(cx);
    assert!(cx.debug_bounds("latex-error").is_some(), "the failure drew");
}
