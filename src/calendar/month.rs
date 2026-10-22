use std::rc::Rc;

use gpui::{
    AnyElement, App, ElementId, FontWeight, InteractiveElement, IntoElement, MouseButton,
    ParentElement, Pixels, Rems, RenderOnce, StatefulInteractiveElement, Styled, Window, div,
    prelude::*, relative, rems,
};
use jiff::{Timestamp, ToSpan, civil::Date, tz::TimeZone};

use super::{
    chip::EventChip,
    event::{Event, unique},
    head::{OnDay, Step, head},
    weeks::{Bar, bars, day_rows, singles, weeks},
};
use crate::{
    forms::OnValue,
    i18n,
    layout::seeded::use_seeded,
    primitives::{FocusRing, tab_stop},
    theme::{ActiveTheme, Radius, TextSize},
    typography::{Ellipsis, format, fresh},
};

/// Builds the muted note beside a day's number (lunar date, holiday, …); `None` leaves the day bare.
pub type DayNote = Rc<dyn Fn(Date) -> Option<gpui::SharedString>>;

/// Builds the short badge at a day's top-right corner, such as "休" for a day off; `None` leaves the corner bare.
pub type DayBadge = Rc<dyn Fn(Date) -> Option<gpui::SharedString>>;

/// i18n keys for the weekday header, Monday first.
const WEEKDAY_KEYS: [&str; 7] = [
    "calendar.weekday.mon",
    "calendar.weekday.tue",
    "calendar.weekday.wed",
    "calendar.weekday.thu",
    "calendar.weekday.fri",
    "calendar.weekday.sat",
    "calendar.weekday.sun",
];

/// A month of days in week rows, each with its events: what spans days lies across them as bars, the rest by the hour as chips, and what does not fit waits behind "N more". Arrows move a day, Page keys turn the month, and Enter or a double press opens the day.
#[derive(IntoElement)]
pub struct CalendarMonthView {
    id: ElementId,
    month: Date,
    events: Vec<Event>,
    today: Option<Date>,
    zone: Option<TimeZone>,
    header_leading: Option<AnyElement>,
    chip: Option<Rems>,
    day_rows: Option<usize>,
    on_day: Option<OnDay>,
    on_select: Option<OnDay>,
    on_event: Option<OnValue>,
    on_month: Option<OnDay>,
    day_note: Option<DayNote>,
    day_badge: Option<DayBadge>,
}

impl CalendarMonthView {
    pub fn new(
        id: impl Into<ElementId>,
        month: Date,
        events: impl IntoIterator<Item = Event>,
    ) -> Self {
        Self {
            id: id.into(),
            month,
            events: unique(events),
            today: None,
            zone: None,
            on_day: None,
            on_select: None,
            on_event: None,
            on_month: None,
            day_note: None,
            day_badge: None,
            header_leading: None,
            chip: None,
            day_rows: None,
        }
    }

    /// The day marked as today; the zone's own otherwise.
    pub fn today(mut self, date: Date) -> Self {
        self.today = Some(date);
        self
    }

    /// The zone its days and hours read in; the system's otherwise.
    pub fn zone(mut self, zone: TimeZone) -> Self {
        self.zone = Some(zone);
        self
    }

    /// Gets each day as the cursor lands on it: a press, the arrow keys, or a page turn's first day.
    pub fn on_select(mut self, handler: impl Fn(Date, &mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    /// Gets a day opened: Enter on it, a double press, or its "N more".
    pub fn on_day(mut self, handler: impl Fn(Date, &mut Window, &mut App) + 'static) -> Self {
        self.on_day = Some(Rc::new(handler));
        self
    }

    /// Gets the key of an event pressed.
    pub fn on_event(
        mut self,
        handler: impl Fn(&gpui::SharedString, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_event = Some(Rc::new(handler));
        self
    }

    /// Gets the first day of each month turned to.
    pub fn on_month(mut self, handler: impl Fn(Date, &mut Window, &mut App) + 'static) -> Self {
        self.on_month = Some(Rc::new(handler));
        self
    }

    /// A short badge at each day's top-right corner, such as "休" for a day off.
    pub fn day_badge(
        mut self,
        badge: impl Fn(Date) -> Option<gpui::SharedString> + 'static,
    ) -> Self {
        self.day_badge = Some(Rc::new(badge));
        self
    }

    /// The height of an event chip under a day's number; the theme's otherwise.
    pub fn chip_height(mut self, chip: Rems) -> Self {
        self.chip = Some(chip);
        self
    }

    /// How many event rows a day holds under its number; the theme's otherwise.
    pub fn day_rows(mut self, rows: usize) -> Self {
        self.day_rows = Some(rows);
        self
    }

    /// A control shown at the header's left, with the month title centered beside it.
    pub fn header_leading(mut self, element: impl IntoElement) -> Self {
        self.header_leading = Some(element.into_any_element());
        self
    }

    /// A muted note shown beside each day's number, such as its lunar date or a holiday; the day stays bare where the builder returns `None`.
    pub fn day_note(mut self, note: impl Fn(Date) -> Option<gpui::SharedString> + 'static) -> Self {
        self.day_note = Some(Rc::new(note));
        self
    }
}

/// What a week row draws with: the view's id and zone, its events, today, the cursor, and what presses do.
struct Row<'a> {
    id: &'a ElementId,
    events: &'a [Event],
    zone: &'a TimeZone,
    shown: Date,
    today: Date,
    cursor: Option<Date>,
    pick: OnDay,
    open: OnDay,
    on_event: Option<OnValue>,
    day_note: Option<DayNote>,
    day_badge: Option<DayBadge>,
}

/// Sizes of a month's rows in pixels: a chip, the gap between rows, and the cell's inset.
struct Metrics {
    chip: Pixels,
    gap: Pixels,
    pad: Pixels,
    rows: usize,
}

impl Metrics {
    fn pitch(&self) -> Pixels {
        self.chip + self.gap
    }

    /// A day's height: its number, then its rows of events.
    fn height(&self) -> Pixels {
        self.pad * 2.0 + self.chip + self.pitch() * self.rows as f32
    }
}

impl Row<'_> {
    fn chip(&self, id: String, event: &Event) -> EventChip {
        let chip = EventChip::new((self.id.clone(), id), event.clone())
            .zone(self.zone.clone())
            .tooltip(event.title.clone());
        match self.on_event.clone() {
            Some(on_event) => {
                let key = event.key.clone();
                chip.on_click(move |window, cx| on_event(&key, window, cx))
            }
            None => chip,
        }
    }

    fn bar(&self, bar: &Bar, monday: Date, sizes: &Metrics) -> AnyElement {
        let event = &self.events[bar.event];
        let span = (bar.to - bar.from + 1) as f32;
        div()
            .absolute()
            .top(sizes.pad + sizes.pitch() * (bar.lane + 1) as f32)
            .left(relative(bar.from as f32 / 7.0))
            .w(relative(span / 7.0))
            .when(!bar.before, |slot| slot.pl_1())
            .when(!bar.after, |slot| slot.pr_1())
            .child(
                self.chip(format!("bar-{}-{monday}", event.key), event)
                    .open(bar.before, bar.after),
            )
            .into_any_element()
    }

    fn day(
        &self,
        day: Date,
        column: usize,
        lanes: usize,
        hidden: usize,
        sizes: &Metrics,
        cx: &App,
    ) -> AnyElement {
        let theme = cx.theme();
        let colors = &theme.colors;
        let on = singles(self.events, day, self.zone);
        let (shown, more) = day_rows(on.len(), hidden, sizes.rows - lanes);
        let label = match day.day() {
            1 => i18n::text(
                cx,
                "calendar.monthday",
                &[
                    (
                        "month",
                        &i18n::text(cx, &format!("calendar.short_month.{}", day.month()), &[]),
                    ),
                    ("day", &day.day().to_string()),
                ],
            )
            .to_string(),
            number => number.to_string(),
        };
        let note = self
            .day_note
            .as_ref()
            .and_then(|note| note(day))
            .map(|text| {
                div()
                    .min_w_0()
                    .px_1p5()
                    .text_size(theme.text_size(TextSize::Xs))
                    .text_color(colors.fg_subtle)
                    .child(Ellipsis::new(text))
            });
        let badge = self
            .day_badge
            .as_ref()
            .and_then(|badge| badge(day))
            .map(|text| {
                div()
                    .flex_none()
                    .px_1()
                    .rounded(theme.radius(Radius::Sm))
                    .bg(colors.danger_subtle)
                    .text_size(theme.text_size(TextSize::Xs))
                    .text_color(colors.danger)
                    .child(text)
            });
        let has_badge = badge.is_some();
        let fg = if day.first_of_month() == self.shown {
            colors.fg
        } else {
            colors.fg_subtle
        };
        let (pick, open) = (self.pick.clone(), self.open.clone());
        let more = (more > 0).then(|| {
            let open = self.open.clone();
            div()
                .id((self.id.clone(), format!("more-{day}")))
                .debug_selector(move || format!("more {day}"))
                .h(sizes.chip)
                .flex()
                .items_center()
                .px_1p5()
                .rounded(theme.radius(Radius::Sm))
                .border_1()
                .border_color(gpui::transparent_black())
                .focus_ring(cx)
                .tab_index(0)
                .text_size(theme.text_size(TextSize::Xs))
                .text_color(colors.fg_muted)
                .cursor_pointer()
                .hover(|style| style.bg(colors.hover))
                .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
                .on_click(move |_, window, cx| open(day, window, cx))
                .child(div().min_w_0().child(Ellipsis::new(i18n::text(
                    cx,
                    "calendar.more",
                    &[("n", &more.to_string())],
                ))))
        });
        div()
            .id((self.id.clone(), format!("day-{day}")))
            .debug_selector(move || format!("day {day}"))
            .absolute()
            .top_0()
            .left(relative(column as f32 / 7.0))
            .w(relative(1.0 / 7.0))
            .h_full()
            .overflow_hidden()
            .p_1()
            .flex()
            .flex_col()
            .gap_0p5()
            .border_r_1()
            .border_b_1()
            .border_color(colors.border)
            .on_mouse_down(MouseButton::Left, move |event, window, cx| {
                if event.click_count == 2 {
                    open(day, window, cx);
                } else {
                    pick(day, window, cx);
                }
            })
            .when(self.cursor == Some(day), |cell| {
                cell.child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full()
                        .border_1()
                        .border_color(colors.focus),
                )
            })
            .child(
                div()
                    .h(sizes.chip)
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_1()
                    .child(
                        div()
                            .px_1p5()
                            .rounded_full()
                            .text_size(theme.text_size(TextSize::Xs))
                            .text_color(fg)
                            .map(|number| match day == self.today {
                                true => number
                                    .bg(colors.accent)
                                    .text_color(colors.on_accent)
                                    .font_weight(FontWeight::SEMIBOLD),
                                false => number,
                            })
                            .child(label),
                    )
                    .when(has_badge, |row| row.pr_6())
                    .children(note),
            )
            .children(badge.map(|badge| badge.absolute().top_1().right_1()))
            .children((0..lanes).map(|_| div().flex_none().h(sizes.chip)))
            .children(on.iter().take(shown).map(|&ix| {
                let event = &self.events[ix];
                self.chip(format!("chip-{}", event.key), event)
            }))
            .children(more)
            .into_any_element()
    }

    fn week(&self, monday: Date, sizes: &Metrics, cx: &App) -> AnyElement {
        let bars = bars(self.events, monday, 7, self.zone);
        let lanes = bars
            .iter()
            .map(|bar| bar.lane + 1)
            .max()
            .unwrap_or(0)
            .min(sizes.rows - 1);
        let days = (0..7).map(|column| {
            let day = monday
                .checked_add((column as i64).days())
                .expect("a week has seven days");
            let hidden = bars
                .iter()
                .filter(|bar| bar.lane >= lanes && bar.from <= column && column <= bar.to)
                .count();
            self.day(day, column, lanes, hidden, sizes, cx)
        });
        div()
            .relative()
            .h(sizes.height())
            .children(days.collect::<Vec<_>>())
            .children(
                bars.iter()
                    .filter(|bar| bar.lane < lanes)
                    .map(|bar| self.bar(bar, monday, sizes)),
            )
            .into_any_element()
    }
}

impl RenderOnce for CalendarMonthView {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        if self.today.is_none() {
            fresh((self.id.clone(), "clock"), window, cx);
        }
        let zone = self
            .zone
            .unwrap_or_else(|| format::system_zone("calendar month view"));
        let today = self
            .today
            .unwrap_or_else(|| Timestamp::now().to_zoned(zone.clone()).date());
        let month = use_seeded(
            (self.id.clone(), "month"),
            self.month.first_of_month(),
            window,
            cx,
        );
        let shown = month.read(cx).value;
        let start = match today.first_of_month() == shown {
            true => today,
            false => shown,
        };
        let cursor = window.use_keyed_state((self.id.clone(), "cursor"), cx, |_, _| start);
        if cursor.read(cx).first_of_month() != shown {
            cursor.update(cx, |cursor, _| *cursor = start);
        }
        let focus = tab_stop((self.id.clone(), "focus").into(), true, window, cx);
        let at = *cursor.read(cx);
        let turn: OnDay = {
            let (id, month, on_month) = (self.id.clone(), month.clone(), self.on_month.clone());
            Rc::new(move |day, window, cx| {
                let first = day.first_of_month();
                if month.read(cx).value == first {
                    return;
                }
                log::info!("calendar month view {id:?}: {}", first.strftime("%B %Y"));
                month.update(cx, |month, cx| {
                    month.value = first;
                    cx.notify();
                });
                if let Some(on_month) = &on_month {
                    on_month(first, window, cx);
                }
            })
        };
        let pick: OnDay = {
            let cursor = cursor.clone();
            let (on_select, turn) = (self.on_select.clone(), turn.clone());
            Rc::new(move |day, window, cx| {
                cursor.update(cx, |cursor, cx| {
                    *cursor = day;
                    cx.notify();
                });
                turn(day, window, cx);
                if let Some(on_select) = &on_select {
                    on_select(day, window, cx);
                }
            })
        };
        let open: OnDay = {
            let (id, on_day) = (self.id.clone(), self.on_day.clone());
            Rc::new(move |day, window, cx| {
                log::info!("calendar month view {id:?}: open {day}");
                if let Some(on_day) = &on_day {
                    on_day(day, window, cx);
                }
            })
        };
        let step: Step = {
            let pick = pick.clone();
            Rc::new(move |by, window, cx| {
                let day = match by {
                    0 => today,
                    by => match super::stepped(at, by.months(), "calendar month view") {
                        Some(day) => day,
                        None => return,
                    },
                };
                pick(day, window, cx)
            })
        };
        let keys = {
            let (focus, pick, open) = (focus.clone(), pick.clone(), open.clone());
            move |event: &gpui::KeyDownEvent, window: &mut Window, cx: &mut App| {
                if !focus.is_focused(window) {
                    return;
                }
                let by = match event.keystroke.key.as_str() {
                    "left" => 1.day().negate(),
                    "right" => 1.day(),
                    "up" => 1.week().negate(),
                    "down" => 1.week(),
                    "pageup" => 1.month().negate(),
                    "pagedown" => 1.month(),
                    "enter" => {
                        cx.stop_propagation();
                        return open(at, window, cx);
                    }
                    _ => return,
                };
                cx.stop_propagation();
                if let Some(day) = super::stepped(at, by, "calendar month view") {
                    pick(day, window, cx)
                }
            }
        };
        let theme = cx.theme();
        let colors = &theme.colors;
        let rem = window.rem_size();
        let sizes = Metrics {
            chip: self.chip.unwrap_or(theme.calendar().chip).to_pixels(rem),
            gap: rems(0.125).to_pixels(rem),
            pad: rems(0.25).to_pixels(rem),
            rows: self.day_rows.unwrap_or(theme.calendar().day_rows),
        };
        let row = Row {
            id: &self.id,
            events: &self.events,
            zone: &zone,
            shown,
            today,
            cursor: focus.is_focused(window).then_some(at),
            pick,
            open,
            on_event: self.on_event.clone(),
            day_note: self.day_note.clone(),
            day_badge: self.day_badge.clone(),
        };
        let weekday_names = WEEKDAY_KEYS.map(|key| i18n::text(cx, key, &[]));
        let weekdays = div().flex().children(weekday_names.map(|name| {
            div()
                .flex_none()
                .w(relative(1.0 / 7.0))
                .px_1p5()
                .pb_1()
                .text_size(theme.text_size(TextSize::Xs))
                .text_color(colors.fg_subtle)
                .child(name)
        }));
        let rows: Vec<AnyElement> = weeks(shown)
            .into_iter()
            .map(|monday| row.week(monday, &sizes, cx))
            .collect();
        let title = i18n::text(
            cx,
            "calendar.title",
            &[
                (
                    "month",
                    &i18n::text(cx, &format!("calendar.month.{}", shown.month()), &[]),
                ),
                ("year", &shown.year().to_string()),
            ],
        )
        .to_string();
        let back = i18n::text(cx, "calendar.prev_month", &[]).to_string();
        let on = i18n::text(cx, "calendar.next_month", &[]).to_string();
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(head(
                &self.id,
                title,
                &back,
                &on,
                step,
                self.header_leading,
                cx,
            ))
            .child(
                div().flex().flex_col().child(weekdays).child(
                    div()
                        .id((self.id.clone(), "grid"))
                        .debug_selector(|| "month-grid".into())
                        .track_focus(&focus)
                        .on_key_down(keys)
                        .border_t_1()
                        .border_l_1()
                        .border_color(colors.border)
                        .children(rows),
                ),
            )
    }
}
