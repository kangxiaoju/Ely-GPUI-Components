use std::{cell::RefCell, collections::BTreeMap, ops::Range, rc::Rc, time::Duration};

use gpui::{
    Bounds, Context, Entity, IntoElement, Modifiers, ParentElement, Pixels, Render,
    ScrollDelta, ScrollEvent, ScrollSource, Styled, TestAppContext, TouchPhase,
    VisualTestContext, Window, canvas, div, point, px,
};

use super::settle;
use crate::{MessageList, StreamingMarkdown, ThinkingBlock, theme::Theme};

type Frames = Rc<RefCell<BTreeMap<usize, Bounds<Pixels>>>>;

struct Conversation {
    markdown: gpui::SharedString,
    folded: bool,
    count: usize,
    revision: u64,
    changed: Range<usize>,
    frames: Frames,
}

impl Render for Conversation {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.frames.borrow_mut().clear();
        let frames = self.frames.clone();
        let markdown = self.markdown.clone();
        let folded = self.folded;
        div().w(px(360.0)).h(px(240.0)).child(
            MessageList::new("variable-conversation", self.count, move |ix, _, _| {
                let body = if ix == 8 {
                    if folded {
                        ThinkingBlock::new("reasoning", "Done", false, Duration::from_secs(3))
                            .into_any_element()
                    } else {
                        StreamingMarkdown::new("long-answer", markdown.clone(), false)
                            .into_any_element()
                    }
                } else {
                    div().h(px(100.0)).child(format!("Tool result {ix}")).into_any_element()
                };
                let frames = frames.clone();
                div().relative().w_full().child(body).child(
                    canvas(
                        move |bounds, _, _| { frames.borrow_mut().insert(ix, bounds); },
                        |_, _, _, _| {},
                    )
                    .absolute().top_0().left_0().size_full(),
                ).into_any_element()
            })
            .content_revision(self.revision, self.changed.clone()),
        )
    }
}

fn conversation(cx: &mut TestAppContext) -> (Entity<Conversation>, Frames, &mut VisualTestContext) {
    cx.update(|cx| {
        crate::init_for_tests(cx);
        Theme::init(cx);
    });
    let frames = Frames::default();
    let recorded = frames.clone();
    let (view, cx) = cx.add_window_view(move |_, _| Conversation {
        markdown: "A long assistant paragraph with markdown **details** and a tool explanation.\n\n".repeat(40).into(),
        folded: false,
        count: 13,
        revision: 0,
        changed: 0..0,
        frames: recorded,
    });
    settle(cx);
    (view, frames, cx)
}

fn wheel(delta: f32, cx: &mut VisualTestContext) {
    cx.simulate_event(ScrollEvent {
        position: point(px(180.0), px(120.0)),
        delta: ScrollDelta::Pixels(point(px(0.0), px(delta))),
        modifiers: Modifiers::none(),
        touch_phase: TouchPhase::Moved,
        source: ScrollSource::Pointer,
    });
    settle(cx);
}

#[gpui::test]
fn shrinking_a_markdown_anchor_to_folded_reasoning_does_not_resume_following(cx: &mut TestAppContext) {
    let (view, frames, cx) = conversation(cx);
    wheel(500.0, cx);
    let tall = frames.borrow().get(&8).copied().expect("the long answer is visible");
    assert!(tall.top() < px(-400.0), "the reader is deep in the long answer: {tall:?}");

    view.update(cx, |view, cx| {
        view.folded = true;
        view.revision += 1;
        view.changed = 8..9;
        cx.notify();
    });
    settle(cx);
    assert!(frames.borrow().contains_key(&9), "the next tool result stays visible after the anchor shrinks: {:?}", frames.borrow());
    assert!(!frames.borrow().contains_key(&12), "shrinking the anchor must not jump to the newest");

    view.update(cx, |view, cx| {
        view.count += 1;
        view.revision += 1;
        view.changed = 13..14;
        cx.notify();
    });
    settle(cx);
    assert!(frames.borrow().contains_key(&9), "new results preserve the reader's anchor");
    assert!(!frames.borrow().contains_key(&13), "new results do not restart following");
}

#[gpui::test]
fn small_pointer_scrolls_hold_variable_rows_across_redraw_growth_and_removal(cx: &mut TestAppContext) {
    let (view, frames, cx) = conversation(cx);
    for _ in 0..22 {
        wheel(24.0, cx);
    }
    let before = frames.borrow().get(&8).copied().expect("small wheel ticks reach the long answer");
    view.update(cx, |_, cx| cx.notify());
    settle(cx);
    assert_eq!(frames.borrow().get(&8).copied(), Some(before), "a same-count redraw leaves the manual anchor alone");

    view.update(cx, |view, cx| {
        view.markdown = format!("{}\n\nMore streamed **markdown**.\n\n", view.markdown).into();
        view.count -= 1;
        view.revision += 1;
        view.changed = 8..12;
        cx.notify();
    });
    settle(cx);
    let grown = frames.borrow().get(&8).copied().expect("the same answer remains visible");
    assert_eq!(grown.top(), before.top(), "growth and removal of a trailing row retain the absolute offset");
    assert!(grown.size.height > before.size.height, "markdown growth changes real row height");

    wheel(-10000.0, cx);
    view.update(cx, |view, cx| {
        view.count += 1;
        view.revision += 1;
        view.changed = 12..13;
        cx.notify();
    });
    settle(cx);
    assert!(frames.borrow().contains_key(&12), "scrolling back down resumes following new results");
}
