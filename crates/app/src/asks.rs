//! The questions a group offers: a row of pills under the window.
//!
//! The agent writes up to five per group (`deck group --ask`), as hooks: one
//! line each, from what the claim stands on to what it puts at risk. Pressing
//! one asks it, straight away, the way *Ask now* does. With the secondary
//! modifier held — ⌘ on a Mac, Ctrl elsewhere — it goes into the comment box
//! instead, to be edited first.
//!
//! The row is drawn the same wherever it lives: in a window of its own just
//! below the deck on macOS, and inside the deck above its footer everywhere
//! else, or when there is no room below it. So it is a function of what to
//! show and what to do when pressed, and knows nothing about where it is.

use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use deck_core::theme::Palette;
#[cfg(target_os = "macos")]
use gpui_kit::component::Root;
use gpui_kit::component::*;
use gpui_kit::*;

use crate::palette::paint;

/// How tall a pill is.
pub const PILL: f32 = 30.;

/// The gap between pills, and between a pill and the arrows.
const GAP: f32 = 8.;

/// The widest a pill may be: room for the longest question `deck group`
/// takes, in ordinary words. One of very wide characters is cut short.
const WIDEST: f32 = 680.;

/// What the row shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    /// The group's questions, in the order the agent wrote them.
    pub asks: Vec<SharedString>,
}

/// Pressed: which question, and whether to edit it first.
///
/// The question goes by its words, not its place: the shelf can be a frame
/// behind the deck, and a place in the old row is another question in the new.
pub type Pick = Rc<dyn Fn(SharedString, bool, &mut Window, &mut App)>;

/// What a pill opens to show, beside its question, while the pointer is on
/// it: how to press it. Said in the pill itself, so it is read where the
/// pressing happens, and only by somebody about to press.
#[must_use]
pub fn how() -> &'static str {
    if cfg!(target_os = "macos") {
        "click to ask \u{b7} \u{2318}-click to edit"
    } else {
        "click to ask \u{b7} ctrl-click to edit"
    }
}

/// The room between a question and its hint, in an open pill.
const HINT_GAP: f32 = 12.;

/// How long a pill takes to open, and to close again: on the bar's own
/// curve, a little slower than the bar opens, so it reads as settling.
const OPEN: Duration = Duration::from_millis(700);
const CLOSE: Duration = OPEN;

/// How quickly a pill closes when the pointer has moved on to another, which
/// waits for it.
const HAND: Duration = Duration::from_millis(320);

/// How long the pointer rests on a pill before it opens. Passed over on the
/// way somewhere else, a pill stays shut, and the row does not ripple.
const REST: Duration = Duration::from_millis(90);

/// How long the spotlight waits, once the pointer is off every pill, before
/// it lifts: crossing the gap from one pill to the next is not leaving, and
/// the row should not brighten and dim again on the way.
const GRACE: Duration = Duration::from_millis(250);

/// And how long it takes to lift.
const LIFT: Duration = Duration::from_millis(200);

/// How long the light takes to cross a question, once, as its pill opens:
/// slow enough to be followed, not flashed.
const SHIMMER: Duration = Duration::from_millis(1800);

/// The breath between one crossing of the light and the next.
const PAUSE: Duration = Duration::from_millis(700);

/// How lit a point of a question is — `place` from its start at 0 to its end
/// at 1 — `t` of the way through the shimmer: a soft band of light, crossing
/// left to right, gone by the end.
#[must_use]
fn shimmer(place: f32, t: f32) -> f32 {
    let at = -BAND + (1. + 2. * BAND) * t.clamp(0., 1.);
    let near = (1. - (place - at).abs() / BAND).max(0.);
    near * near * (3. - 2. * near)
}

/// How wide the light is, as a share of the question it crosses.
const BAND: f32 = 0.22;

/// How many slices the light is drawn in. Each is a window onto a lit copy
/// of the question, at its own strength; enough of them and the edge of the
/// light is soft.
const SLICES: usize = 16;

/// How long an arrow's slide takes.
const GLIDE: Duration = Duration::from_millis(320);

/// One slide under way: from where, to where, since when. Offsets are gpui's,
/// zero at the start of the row and going negative as it moves on.
#[derive(Clone, Copy)]
struct Glide {
    from: f32,
    to: f32,
    since: Instant,
}

/// The row's place: every question is in it, and it slides.
///
/// A trackpad scrolls it directly; the arrows slide it one question along,
/// eased, so the eye can follow where the row went. Each place the row is
/// drawn keeps one of these — it is where the row is, not what it holds.
#[derive(Clone, Default)]
pub struct Carousel {
    handle: ScrollHandle,
    glide: Rc<Cell<Option<Glide>>>,
    /// When the row was last drawn. A row that stops being drawn — moved
    /// from inside the deck out to the shelf with a pill open — hears no more
    /// of the pointer, and must not keep the window drawing for it.
    drawn: Rc<Cell<Option<Instant>>>,
    /// Until when the window is kept drawing, and whether something is.
    drawing: Rc<Cell<Option<Instant>>>,
    /// The pill opening, or open, under the pointer; and the one closing
    /// behind it, so moving across the row hands one to the next smoothly.
    open: Rc<Cell<Option<Opening>>>,
    closing: Rc<Cell<Option<Opening>>>,
    spot: Rc<Cell<Option<Spot>>>,
    /// How much wider a pill is, open: its hint, as last measured.
    hint: Rc<Cell<f32>>,
}

/// One pill opening or closing: which, since when, and how open it was then.
#[derive(Clone, Copy)]
struct Opening {
    ix: usize,
    since: Instant,
    from: f32,
    /// How far along the row was when it began to open, and whether the row
    /// is still moving with it to bring the whole of it into view.
    at: f32,
    follow: bool,
    /// How long its opening or closing takes.
    span: Duration,
}

/// The spotlight on the row: on while a pill is open, and since when, from
/// how bright it was then.
#[derive(Clone, Copy)]
struct Spot {
    on: bool,
    since: Instant,
    from: f32,
}

/// The bar's curve: quick away, a long settle.
fn ease(t: f32) -> f32 {
    1. - (1. - t.clamp(0., 1.)).powi(5)
}

/// How far `since` is behind, or nothing yet if it is still to come.
fn age(since: Instant) -> f32 {
    Instant::now()
        .saturating_duration_since(since)
        .as_secs_f32()
}

impl Carousel {
    /// Back to the first question, at once: a new group's row starts there.
    pub fn reset(&self) {
        self.glide.set(None);
        self.open.set(None);
        self.closing.set(None);
        self.spot.set(None);
        self.drawn.set(None);
        self.handle.set_offset(point(px(0.), px(0.)));
    }

    /// Slide one question back, or one on.
    fn slide(&self, by: isize, window: &Window, cx: &mut App) {
        self.step(by);
        self.drive(Duration::ZERO, window, cx);
    }

    /// Keep the window drawing, frame by frame, for `until` and while a slide
    /// is under way.
    ///
    /// Driven by a timer rather than by asking for the next frame from inside
    /// a render: that way each frame waits for the one before it to be drawn
    /// and then for another, and a slide came out at half the screen's rate
    /// with frames dropped — five pictures in a quarter of a second.
    fn drive(&self, until: Duration, window: &Window, cx: &mut App) {
        let end = Instant::now() + until + Duration::from_millis(40);
        let running = self.drawing.get();
        self.drawing
            .set(Some(running.map_or(end, |was| was.max(end))));
        if running.is_some() {
            return;
        }
        let handle = window.window_handle();
        let carousel = self.clone();
        cx.spawn(async move |cx| {
            let mut asked: Option<Instant> = None;
            loop {
                let busy = carousel.glide.get().is_some() || carousel.open.get().is_some();
                let ending = carousel
                    .drawing
                    .get()
                    .is_none_or(|end| Instant::now() >= end);
                if !busy && ending {
                    break;
                }
                // Frames asked for and not drawn for half a second: the row
                // is no longer on screen, and nobody is looking at it.
                if carousel
                    .drawn
                    .get()
                    .zip(asked)
                    .is_some_and(|(drawn, asked)| drawn >= asked)
                {
                    asked = None;
                }
                if asked.is_some_and(|asked| asked.elapsed() > Duration::from_millis(500)) {
                    carousel.reset();
                    break;
                }
                if !carousel.resting() {
                    if handle.update(cx, |_, window, _| window.refresh()).is_err() {
                        break;
                    }
                    asked.get_or_insert_with(Instant::now);
                }
                // About a frame of a fast screen: often enough for any of
                // them, without asking for frames no screen can show.
                cx.background_executor()
                    .timer(Duration::from_millis(8))
                    .await;
            }
            carousel.drawing.set(None);
        })
        .detach();
    }

    /// Whether nothing on the row is moving this frame: a pill held fully
    /// open, the light between crossings, nothing sliding, closing or
    /// changing brightness. The loop keeps watching, but asks for no frames.
    fn resting(&self) -> bool {
        let Some(open) = self.open.get() else {
            return false;
        };
        let settled = |since: Instant, span: Duration| age(since) > span.as_secs_f32();
        self.glide.get().is_none()
            && self
                .closing
                .get()
                .is_none_or(|closing| settled(closing.since, closing.span))
            && settled(open.since, open.span)
            && !open.follow
            && self
                .spot
                .get()
                .is_none_or(|spot| spot.on && settled(spot.since, OPEN))
            && self.light(open.ix).is_none()
    }

    /// How open pill `ix` is, from shut at 0 to showing its hint at 1.
    fn openness(&self, ix: usize) -> f32 {
        if let Some(open) = self.open.get().filter(|open| open.ix == ix) {
            let t = age(open.since) / open.span.as_secs_f32();
            return open.from + (1. - open.from) * ease(t);
        }
        if let Some(closing) = self.closing.get().filter(|closing| closing.ix == ix) {
            let t = age(closing.since) / closing.span.as_secs_f32();
            return closing.from * (1. - ease(t));
        }
        0.
    }

    /// How far the light has crossed pill `ix`, while it is crossing.
    fn light(&self, ix: usize) -> Option<f32> {
        let open = self.open.get().filter(|open| open.ix == ix)?;
        // Again and again while the pointer stays, with a breath between.
        let round = SHIMMER + PAUSE;
        let t = (age(open.since) % round.as_secs_f32()) / SHIMMER.as_secs_f32();
        (t < 1.).then_some(t)
    }

    /// How much of pill `ix`'s hint shows. It fades in behind the widening,
    /// as the bar's words do, so it arrives in room already made for it; and
    /// goes ahead of the narrowing, so it is never squeezed.
    fn said(&self, ix: usize) -> f32 {
        let open = self.openness(ix);
        if self.open.get().is_some_and(|open| open.ix == ix) {
            ((open - 0.3) / 0.7).clamp(0., 1.).powf(0.7)
        } else {
            ((open - 0.5) / 0.5).clamp(0., 1.)
        }
    }

    /// How strongly the row is given over to one pill, from none at 0 to
    /// all at 1.
    fn spotlight(&self) -> f32 {
        match self.spot.get() {
            None => 0.,
            Some(spot) if spot.on => {
                spot.from + (1. - spot.from) * ease(age(spot.since) / OPEN.as_secs_f32())
            }
            Some(spot) => {
                let after = age(spot.since) - GRACE.as_secs_f32();
                if after <= 0. {
                    spot.from
                } else {
                    spot.from * (1. - ease(after / LIFT.as_secs_f32()))
                }
            }
        }
    }

    /// How far pill `ix` stands out of the spotlit row. Handing over to
    /// another, as far as it is still open; with the pointer off the row
    /// altogether, the last one keeps its place until the light lifts.
    fn standing(&self, ix: usize) -> f32 {
        if self.closing.get().is_some_and(|closing| closing.ix == ix) {
            // The one handing over keeps the light until the next takes it.
            return self
                .open
                .get()
                .map_or(1., |open| 1. - self.openness(open.ix));
        }
        self.openness(ix)
    }

    /// Pill `ix` closes, if it is the one open.
    fn shut(&self, ix: usize) {
        if let Some(open) = self.open.get().filter(|open| open.ix == ix) {
            let now = Instant::now();
            self.closing.set(Some(Opening {
                from: self.openness(ix),
                since: now,
                follow: false,
                span: CLOSE,
                ..open
            }));
            self.open.set(None);
            self.spot.set(Some(Spot {
                on: false,
                since: now,
                from: self.spotlight(),
            }));
        }
    }

    /// The pointer came onto pill `ix`, or left it.
    fn peek(&self, ix: usize, on: bool, window: &Window, cx: &mut App) {
        let now = Instant::now();
        if on {
            if self.open.get().is_some_and(|open| open.ix == ix) {
                return;
            }
            let from = self.openness(ix);
            if self.closing.get().is_some_and(|closing| closing.ix == ix) {
                self.closing.set(None);
            }
            // One at a time. The pill being left closes first, quickly, and
            // this one opens once it has: two pills changing width at once
            // pushed the row both ways and read as a wobble.
            let leaving = self
                .open
                .get()
                .or(self.closing.get())
                .filter(|leaving| leaving.ix != ix && self.openness(leaving.ix) > 0.);
            let since = if let Some(leaving) = leaving {
                self.closing.set(Some(Opening {
                    from: self.openness(leaving.ix),
                    since: now,
                    follow: false,
                    span: HAND,
                    ..leaving
                }));
                now + HAND
            } else if from == 0. && self.spotlight() == 0. {
                // From cold, it waits for the pointer to rest.
                now + REST
            } else {
                now
            };
            if !self.spot.get().is_some_and(|spot| spot.on) {
                self.spot.set(Some(Spot {
                    on: true,
                    since,
                    from: self.spotlight(),
                }));
            }
            self.open.set(Some(Opening {
                ix,
                since,
                from,
                at: -f32::from(self.handle.offset().x),
                follow: true,
                span: OPEN,
            }));
        } else {
            self.shut(ix);
        }
        self.drive(REST + HAND + OPEN.max(CLOSE + GRACE + LIFT), window, cx);
    }

    /// Start sliding one question back, or one on.
    fn step(&self, by: isize) {
        let now = f32::from(self.handle.offset().x);
        let view = self.handle.bounds();
        let lefts: Vec<f32> = (0..)
            .map_while(|ix| self.handle.bounds_for_item(ix))
            // Laid out where they would be unscrolled: the row's own terms.
            .map(|item| f32::from(item.origin.x - view.origin.x))
            .collect();
        let max = f32::from(self.handle.max_offset().x);
        let from = self.glide.get().map_or(now, |glide| glide.to);
        let to = -next_left(&lefts, -from, max, by);
        if to != from {
            self.glide.set(Some(Glide {
                from: now,
                to,
                since: Instant::now(),
            }));
        }
    }

    /// Bring the row up to date for this frame: where its slide has got
    /// to, where the opening pill has taken it, and whether the pointer is
    /// still on that pill at all.
    fn advance(&self, window: &Window) {
        self.drawn.set(Some(Instant::now()));
        if let Some(glide) = self.glide.get() {
            let done = glide.since.elapsed().as_secs_f32() / GLIDE.as_secs_f32();
            let t = done.min(1.);
            let eased = 1. - (1. - t).powi(3);
            let x = glide.from + (glide.to - glide.from) * eased;
            self.handle.set_offset(point(px(x), px(0.)));
            if t >= 1. {
                self.glide.set(None);
            }
        } else {
            self.follow();
        }
        self.let_go(window);
    }

    /// Move the row with a pill as it opens, so the whole of it — question
    /// and hint — ends up in view, in step with its widening. A pill cut off
    /// at either edge is brought in; one already in view does not move.
    fn follow(&self) {
        let Some(open) = self.open.get().filter(|open| open.follow) else {
            return;
        };
        let Some(item) = self.handle.bounds_for_item(open.ix) else {
            return;
        };
        let view = self.handle.bounds();
        let left = f32::from(item.origin.x - view.origin.x);
        let now = self.openness(open.ix);
        let whole = f32::from(item.size.width) + self.hint.get() * (1. - now);
        let want = if left < open.at {
            left
        } else {
            open.at
                .max(left + whole - f32::from(view.size.width))
                .min(left)
        };
        let gone = if open.from >= 1. {
            1.
        } else {
            ((now - open.from) / (1. - open.from)).clamp(0., 1.)
        };
        let at = open.at + (want - open.at) * gone;
        self.handle.set_offset(point(px(-at), px(0.)));
        if gone >= 1. {
            self.open.set(Some(Opening {
                follow: false,
                ..open
            }));
        }
    }

    /// Close the open pill if the pointer has left the window. A window only
    /// a pill high is easily left without a word to it, and a pill that stayed
    /// open, lit and spotlit, over a pointer long gone is the bug this ends.
    #[cfg_attr(not(target_os = "macos"), allow(clippy::unused_self))]
    fn let_go(&self, window: &Window) {
        #[cfg(target_os = "macos")]
        if let Some(open) = self.open.get()
            && crate::mac::pointer_over(window) == Some(false)
        {
            self.shut(open.ix);
        }
        #[cfg(not(target_os = "macos"))]
        let _ = window;
    }
}

/// Where the row's left edge goes after one press of an arrow, in the row's
/// own terms: `lefts` is each question's left edge, `at` how far along the row
/// is now, and `max` how far along it can go. One on is the first question
/// starting past where the row is; one back the last starting before it.
#[must_use]
pub fn next_left(lefts: &[f32], at: f32, max: f32, by: isize) -> f32 {
    let target = if by > 0 {
        lefts.iter().copied().find(|left| *left > at + 1.)
    } else {
        lefts.iter().copied().rev().find(|left| *left < at - 1.)
    };
    target
        .unwrap_or(if by > 0 { max } else { 0. })
        .clamp(0., max.max(0.))
}

/// The row, as an element, `wide` across. `corner` is the window's own
/// radius, which every pill and both arrows share, so they read as made of
/// the same thing.
pub fn render(
    row: &Row,
    palette: &Palette,
    corner: f32,
    wide: f32,
    carousel: &Carousel,
    pick: &Pick,
    window: &Window,
) -> Div {
    carousel.advance(window);
    let hint = how();
    let hint_size = px(12.);
    let hint_w = {
        let font = window.text_style().font();
        let run = TextRun {
            len: hint.len(),
            font,
            color: paint(palette.muted),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        f32::from(
            window
                .text_system()
                .shape_line(hint.into(), hint_size, &[run], None)
                .width,
        ) + HINT_GAP
            + 2.
    };
    carousel.hint.set(hint_w);
    let spotlight = carousel.spotlight();
    // The hint is quieter than the question it sits beside.
    let tertiary = palette.muted.mix(palette.band, 0.15);
    let surface = |id: ElementId| {
        div()
            .id(id)
            .flex_none()
            .h(px(PILL))
            .flex()
            .items_center()
            .rounded(px(corner))
            .border_1()
            .border_color(paint(palette.edge))
            .bg(paint(palette.band))
            .text_color(paint(palette.fg))
            .cursor_pointer()
            .hover(|style| style.bg(paint(palette.wash)))
    };
    let arrow = |id: &'static str, glyph: &'static str, by: isize| {
        let carousel = carousel.clone();
        surface(ElementId::from(id))
            .w(px(36.))
            .justify_center()
            .text_size(px(15.))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(move |_, window, cx| carousel.slide(by, window, cx))
            .child(glyph)
    };
    let pills = row.asks.iter().enumerate().map(|(ix, ask)| {
        let pick = pick.clone();
        let question = ask.clone();
        let open = carousel.openness(ix);
        // The others step back by their words alone: a pill whose border and
        // ground went see-through read as broken, not as quieter.
        let dim = 0.6 * spotlight * (1. - carousel.standing(ix));
        let words_in = palette.fg.mix(palette.band, dim);
        // The light is drawn over the question, not into it. Coloured a
        // letter at a time, the question was shaped in pieces, cut wherever
        // the colour changed, and lost its kerning at every cut: its width
        // shifted under the light as it crossed. So the question is shaped
        // once, in one colour, and the light is a lit copy laid exactly on it,
        // seen through slices that are brighter at the middle of the band.
        let light = carousel.light(ix).map(|t| {
            let font = window.text_style().font();
            let run = TextRun {
                len: ask.len(),
                font,
                color: paint(palette.accent),
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            let wide = f32::from(
                window
                    .text_system()
                    .shape_line(ask.clone(), px(13.), &[run], None)
                    .width,
            );
            let at = -BAND + (1. + 2. * BAND) * t;
            let slice = 2. * BAND * wide / SLICES as f32;
            let start = (at - BAND) * wide;
            div()
                .absolute()
                .inset_0()
                .children((0..SLICES).filter_map(move |n| {
                    let left = start + n as f32 * slice;
                    if left + slice < 0. || left > wide {
                        return None;
                    }
                    let lit = shimmer((left + slice / 2.) / wide.max(1.), t);
                    Some(
                        div()
                            .absolute()
                            .top_0()
                            .bottom_0()
                            .left(px(left))
                            .w(px(slice))
                            .overflow_hidden()
                            .opacity(0.75 * lit)
                            .child(
                                div()
                                    .absolute()
                                    .top_0()
                                    .left(px(-left))
                                    .w(px(wide + 2.))
                                    .whitespace_nowrap()
                                    .text_color(paint(palette.accent))
                                    .child(ask.clone()),
                            ),
                    )
                }))
        });
        let shown = carousel.said(ix);
        let carousel = carousel.clone();
        surface(ElementId::from(("ask", ix)))
            .max_w(px(WIDEST + hint_w))
            .px(px(13.))
            .text_size(px(13.))
            .on_hover({
                let carousel = carousel.clone();
                move |hovered, window, cx| carousel.peek(ix, *hovered, window, cx)
            })
            // Movement too, not only hover: when the pointer left without the
            // window being told, gpui still counts the pill hovered, and the
            // pointer's return would not be news to it.
            .on_mouse_move(move |_, window, cx| carousel.peek(ix, true, window, cx))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(move |event, window, cx| {
                pick(question.clone(), event.modifiers().secondary(), window, cx);
            })
            .child(
                div()
                    .relative()
                    .min_w_0()
                    .truncate()
                    .text_color(paint(words_in))
                    .child(ask.clone())
                    .children(light),
            )
            .child(
                // The padding inside, not on the part that opens: a shut pill
                // is exactly as wide as its question.
                div()
                    .flex_none()
                    .w(px(hint_w * open))
                    .overflow_hidden()
                    .child(
                        div()
                            .flex_none()
                            .whitespace_nowrap()
                            .pl(px(HINT_GAP))
                            .text_size(hint_size)
                            .text_color(paint(tertiary))
                            .opacity(shown)
                            .child(hint),
                    ),
            )
    });
    div()
        .w(px(wide))
        .h_flex()
        .gap(px(GAP))
        .child(arrow("ask-back", "\u{2039}", -1))
        .child(
            div()
                .id("asks")
                .flex_1()
                .min_w_0()
                .h(px(PILL))
                .overflow_x_scroll()
                .track_scroll(&carousel.handle)
                .h_flex()
                .gap(px(GAP))
                .children(pills),
        )
        .child(arrow("ask-on", "\u{203a}", 1))
}

/// How far the questions hang below the deck.
#[cfg(target_os = "macos")]
pub const HANG: f32 = 10.;

/// The shelf's own height: a pill, and a hair either side so the hover
/// colour and the border are not clipped.
#[cfg(target_os = "macos")]
pub const SHELF: f32 = PILL + 4.;

/// The questions in a window of their own, hung just below the deck.
///
/// macOS only: there a window can be made a child of another and moved with
/// it, which is what keeps this under the deck as the deck is dragged. It
/// holds no state of its own: the deck tells it what to draw.
#[cfg(target_os = "macos")]
pub struct Shelf {
    /// What to draw, or nothing while the deck shows them inside itself.
    pub row: Option<Row>,
    pub carousel: Carousel,
    pub palette: Palette,
    pub corner: f32,
    /// How wide to draw: the deck's own width, told by the deck. The shelf's
    /// viewport lags its native frame by a resize, so it is not asked.
    pub wide: f32,
    pub deck: WeakEntity<crate::view::DeckView>,
    /// The deck's window: a question is asked there, where the comment box
    /// and the keyboard are.
    pub deck_window: AnyWindowHandle,
}

#[cfg(target_os = "macos")]
impl Render for Shelf {
    fn render(&mut self, window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let Some(row) = self.row.clone() else {
            return div();
        };
        let pick: Pick = {
            let deck = self.deck.clone();
            let deck_window = self.deck_window;
            Rc::new(move |question, edit, _window, cx| {
                let deck = deck.clone();
                let _ = deck_window.update(cx, |_, window, cx| {
                    let _ = deck.update(cx, |deck, cx| {
                        deck.ask_suggested(&question, edit, window, cx)
                    });
                });
            })
        };
        div().size_full().flex().items_center().child(render(
            &row,
            &self.palette,
            self.corner,
            self.wide,
            &self.carousel,
            &pick,
            window,
        ))
    }
}

/// Open the shelf for the deck in `deck_window`, and hand it to the deck.
///
/// Opened once, empty and off the screen; the deck hangs it and fills it as
/// groups with questions come and go.
#[cfg(target_os = "macos")]
pub fn open_shelf(deck_window: WindowHandle<Root>, cx: &mut App) {
    let Ok(Some((deck, deck_ns, palette))) = deck_window.update(cx, |root, window, cx| {
        let deck = root
            .view()
            .clone()
            .downcast::<crate::view::DeckView>()
            .ok()?;
        let palette = deck.read(cx).palette();
        Some((deck, crate::mac::ns_window(window)?, palette))
    }) else {
        return;
    };
    let options = WindowOptions {
        titlebar: None,
        window_bounds: Some(WindowBounds::Windowed(Bounds {
            origin: point(px(-4000.), px(-4000.)),
            size: size(px(600.), px(SHELF)),
        })),
        kind: WindowKind::PopUp,
        window_background: WindowBackgroundAppearance::Transparent,
        is_movable: false,
        is_resizable: false,
        focus: false,
        show: false,
        ..Default::default()
    };
    let weak = deck.downgrade();
    let any = deck_window.into();
    let Ok(shelf) = cx.open_window(options, |window, cx| {
        window.set_background_appearance(WindowBackgroundAppearance::Transparent);
        crate::mac::never_key(window);
        cx.new(|_| Shelf {
            row: None,
            carousel: Carousel::default(),
            palette,
            corner: 10.,
            wide: 600.,
            deck: weak,
            deck_window: any,
        })
    }) else {
        return;
    };
    let Ok(Some(shelf_ns)) = shelf.update(cx, |_, window, _| crate::mac::ns_window(window)) else {
        return;
    };
    // gpui makes even a window without a title bar a titled one, and AppKit
    // draws a titled window a light edge along its top: a hairline across the
    // gap under the deck. Borderless has none, and no shadow either.
    shelf_ns.setStyleMask(
        objc2_app_kit::NSWindowStyleMask::Borderless
            | objc2_app_kit::NSWindowStyleMask::NonactivatingPanel,
    );
    shelf_ns.setHasShadow(false);
    // The shelf belongs to this deck and goes with it: put away to the bar,
    // closed, or quit. A deck reopened from the bar opens a shelf of its own.
    cx.observe_release(&deck, move |_, cx| {
        let _ = shelf.update(cx, |_, window, _| window.remove_window());
    })
    .detach();
    deck.update(cx, |deck, cx| {
        deck.adopt_shelf(shelf, deck_ns, shelf_ns, cx)
    });
}

#[cfg(test)]
mod tests {
    // Spelled out: the gpui glob above exports a `test` attribute of its own.
    use core::prelude::v1::test;

    use super::*;

    #[test]
    fn the_light_crosses_once_and_leaves_nothing_lit() {
        let places = [0., 0.25, 0.5, 0.75, 1.];
        let dark = |t: f32| places.iter().all(|place| shimmer(*place, t) == 0.);
        assert!(dark(0.) && dark(1.), "dark either side");
        assert!(shimmer(0.5, 0.5) > 0.9, "lit where the light is");
        assert_eq!(shimmer(0., 0.5), 0., "and only there");
        assert_eq!(shimmer(1., 0.5), 0.);
    }

    #[test]
    fn an_arrow_moves_one_question_and_stops_at_either_end() {
        let lefts = [0., 300., 560., 900.];
        let max = 700.;
        assert_eq!(next_left(&lefts, 0., max, 1), 300.);
        assert_eq!(next_left(&lefts, 300., max, 1), 560.);
        assert_eq!(
            next_left(&lefts, 560., max, 1),
            700.,
            "no further than the end"
        );
        assert_eq!(next_left(&lefts, 700., max, 1), 700.);
        assert_eq!(next_left(&lefts, 700., max, -1), 560.);
        assert_eq!(next_left(&lefts, 150., max, -1), 0.);
        assert_eq!(next_left(&lefts, 0., max, -1), 0.);
        assert_eq!(
            next_left(&lefts, 0., 0., 1),
            0.,
            "a row that fits does not move"
        );
    }
}
