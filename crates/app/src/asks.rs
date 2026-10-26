//! The questions a group offers: a row of pills under the window.
//!
//! The agent writes up to five per group (`deck group --ask`): one line each,
//! the things the reader was most likely to type next. Pressing
//! one asks it, straight away, the way *Ask now* does. With the secondary
//! modifier held — ⌘ on a Mac, Ctrl elsewhere — it goes into the comment box
//! instead, to be edited first.
//!
//! The row is drawn the same wherever it lives: in a window of its own just
//! below the deck on macOS, and inside the deck above its footer everywhere
//! else, or when there is no room below it. So it is a function of what to
//! show and what to do when pressed, and knows nothing about where it is.

use std::cell::Cell;
use std::collections::HashMap;
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
    /// The group's questions, in the order they are shown: what arrived in
    /// front, then what was already shown (see [`front_first`]).
    pub asks: Vec<SharedString>,
    /// The row these questions are renewing, while it renews.
    pub renewed: Option<Renewal>,
}

/// A row being renewed after an answer: what it was, and since when.
///
/// The questions that stay keep their order and move right to make room, the
/// ones that go sink where they stand, and the new ones come in at the
/// front, gliding in from the left, the side they land on. So nothing slides
/// under a pill that stays, and nothing jumps.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Renewal {
    pub from: Vec<SharedString>,
    pub since: Instant,
}

/// How long a question takes to sink away, one after another.
const SINK: Duration = Duration::from_millis(1000);
const SINK_STAGGER: Duration = Duration::from_millis(120);
/// How small it gets as it goes under.
const SHRINK: f32 = 0.86;
/// How far under it is before the row closes over where it was.
const CLOSE_FROM: f32 = 0.35;

/// When the first new question arrives, and the ones after it.
const ARRIVE_AFTER: Duration = Duration::from_millis(520);
const ARRIVE_STAGGER: Duration = Duration::from_millis(160);
/// The row makes room for it, then it glides in from the left, the side it
/// lands on.
const ROOM: Duration = Duration::from_millis(840);
const SLIDE: Duration = Duration::from_millis(1080);
const SLIDE_AFTER: Duration = Duration::from_millis(160);
const SLIDE_BY: f32 = 38.;

/// `step`, `n` times over: the wait before the `n`th of a staggered few.
fn nth(step: Duration, n: usize) -> Duration {
    step.saturating_mul(u32::try_from(n).unwrap_or(u32::MAX))
}

/// The renewal's curve: it eases in and out, calm at both ends.
fn calm(t: f32) -> f32 {
    let t = t.clamp(0., 1.);
    (1. - (std::f32::consts::PI * t).cos()) / 2.
}

impl Renewal {
    /// How long renewing `self.from` into `to` takes, end to end.
    #[must_use]
    pub fn length(&self, to: &[SharedString]) -> Duration {
        let leaving = self.from.iter().filter(|was| !to.contains(was)).count();
        let arriving = to.iter().filter(|now| !self.from.contains(now)).count();
        let sinking = nth(SINK_STAGGER, leaving.saturating_sub(1)) + SINK;
        let coming = ARRIVE_AFTER
            + nth(ARRIVE_STAGGER, arriving.saturating_sub(1))
            + ROOM.max(SLIDE_AFTER + SLIDE);
        if arriving == 0 {
            sinking
        } else {
            sinking.max(coming)
        }
    }

    /// Whether it has finished, and the row is simply its questions again.
    #[must_use]
    pub fn over(&self, to: &[SharedString]) -> bool {
        self.since.elapsed() >= self.length(to)
    }
}

/// How far under the `k`th sinking question is, `since` the renewal began:
/// from 0, standing, to 1, gone and no longer drawn.
fn under(since: Duration, k: usize) -> f32 {
    let age = since.saturating_sub(nth(SINK_STAGGER, k)).as_secs_f32();
    (age / SINK.as_secs_f32()).clamp(0., 1.)
}

/// One question on a renewing row, and what it is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Part {
    /// It stays: the `n`th of the new row.
    Stays(usize),
    /// It goes: the `k`th to sink.
    Sinks(usize),
    /// It is new: the `n`th of the new row, and the `k`th to arrive.
    Arrives(usize, usize),
}

/// The order a row is shown in after its questions change: what arrived
/// first, in the order it came, then what was already shown, in the order it
/// was shown. A question that went is dropped. Newly arrived questions are
/// the ones in `new` that were not in `prev`, so replacing the whole list
/// puts all of the new questions first, and an unchanged one changes
/// nothing.
#[must_use]
pub fn front_first<T: PartialEq + Clone>(prev: &[T], new: &[T]) -> Vec<T> {
    let arrived = new.iter().filter(|ask| !prev.contains(ask));
    let kept = prev.iter().filter(|ask| new.contains(ask));
    arrived.chain(kept).cloned().collect()
}

/// The order a group's row opens in when the reader comes back to it: the
/// order it was last shown in, from `memory`, with anything that arrived
/// since in front. A group not seen before is as the agent wrote it.
#[must_use]
pub fn remembered(memory: &HashMap<String, Vec<String>>, id: &str, asks: &[String]) -> Vec<String> {
    front_first(memory.get(id).map_or(&[][..], Vec::as_slice), asks)
}

/// The order a renewing row is drawn in: what is new in front, then the old
/// row as it stood, with what goes still in its place.
fn parts(from: &[SharedString], to: &[SharedString]) -> Vec<(SharedString, Part)> {
    let mut out: Vec<(SharedString, Part)> = Vec::new();
    let mut arriving = 0;
    for (n, now) in to.iter().enumerate() {
        if !from.contains(now) {
            out.push((now.clone(), Part::Arrives(n, arriving)));
            arriving += 1;
        }
    }
    let mut sinking = 0;
    for was in from {
        match to.iter().position(|now| now == was) {
            Some(n) => out.push((was.clone(), Part::Stays(n))),
            None => {
                out.push((was.clone(), Part::Sinks(sinking)));
                sinking += 1;
            }
        }
    }
    out
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

/// How long a pill takes to open, and to close again, on the room's curve
/// (`calm`, in and out), so a pill widening and a pill narrowing read as one
/// thing settling. A little quicker than the room itself: at 840 the hint
/// felt like it was waiting to be asked. Every change of a pill's hint runs
/// this long: the first open, the close, and both halves of a hop.
const SPAN: Duration = Duration::from_millis(640);

/// The most anything on a pill may move in one 8ms sample: the steepest
/// point of `calm` over `SPAN`, through the hint's reveal (which runs over
/// the last half of the opening), with a hair of slack. Anything larger is a
/// jump, not motion. Derived from `SPAN` so the tests follow the timing.
#[cfg(test)]
fn step_bound() -> f32 {
    let steepest = std::f32::consts::FRAC_PI_2 * 8. / SPAN.as_millis() as f32;
    steepest / 0.5 * 1.05
}

/// How long the spotlight takes to come on.
const DIM: Duration = Duration::from_millis(700);

/// How long the pointer rests on a pill before it opens. Passed over on the
/// way somewhere else, a pill stays shut, and the row does not ripple.
const REST: Duration = Duration::from_millis(150);

/// How long a pill stays open once the pointer has left it. Brushed off and
/// back, it never closed; left for good, it closes after a breath.
const LEAVE: Duration = Duration::from_millis(150);

/// How long the spotlight waits, once the pointer is off every pill, before
/// it lifts: crossing the gap from one pill to the next is not leaving, and
/// the row should not brighten and dim again on the way.
const GRACE: Duration = Duration::from_millis(250);

/// And how long it takes to lift.
const LIFT: Duration = Duration::from_millis(200);

/// How long the light takes to cross a question, once, as its pill opens:
/// slow enough to be followed, not flashed.
const SHIMMER: Duration = Duration::from_millis(1800);

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
    /// The pill opening, or open, under the pointer, and every pill closing
    /// behind it. Moving across the row hands one to the next at once.
    pills: Rc<std::cell::RefCell<Pills>>,
    spot: Rc<Cell<Option<Spot>>>,
    /// How much wider a pill is, open: its hint, as last measured.
    hint: Rc<Cell<f32>>,
    /// Where each question is drawn among the row's children. The same as
    /// its place in the row, except while the row renews: then questions on
    /// their way out are still drawn among them.
    places: Rc<std::cell::RefCell<Vec<usize>>>,
    /// Until when a renewal is playing: the row is moving even when the pill
    /// under the pointer has settled.
    renewing: Rc<Cell<Option<Instant>>>,
    /// Each renewing question's whole width, measured once as the renewal
    /// starts rather than shaped again on every frame of it.
    widths: Rc<std::cell::RefCell<std::collections::HashMap<SharedString, f32>>>,
}

/// One pill opening or closing: which, since when, and how open it was then.
/// Its openness is a pure function of these and the time: it runs from `from`
/// to 1 (opening) or to 0 (closing) over `span` on `calm`, and holds `from`
/// until `since`.
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

/// How much of a pill's hint shows when it is `open` open. Nothing until the
/// pill is half open, then up to all of it, so it arrives in room already
/// made for it and is gone before the narrowing reaches it: text that is cut
/// off is never more than faintly there. One mapping for both directions, so
/// it never steps when a pill turns round.
fn said_at(open: f32) -> f32 {
    ((open - 0.5) / 0.5).clamp(0., 1.)
}

/// Where the row's scroll should end up for the pill that is opening: `at` is
/// where it was when the pill began to open, `left` the pill's left edge now,
/// `narrowing` how far pills closing before it still have to shrink (its edge
/// is on its way left by that much), `width` its width now, `hint` how much
/// wider it will be, `open` how open it is now, and `view` how wide the row's
/// window is.
///
/// Worked out from where the pill will end up, not where it is: the answer
/// does not move while the pills beside it trade width, so the row never
/// chases a target that is itself moving.
fn follow_target(
    at: f32,
    left: f32,
    narrowing: f32,
    width: f32,
    hint: f32,
    open: f32,
    view: f32,
) -> f32 {
    let left = left - narrowing;
    let whole = width + hint * (1. - open);
    if left < at {
        left
    } else {
        at.max(left + whole - view).min(left)
    }
}

/// How far along the row is, `gone` of the way from `at` to `want`.
fn follow_at(at: f32, want: f32, gone: f32) -> f32 {
    at + (want - at) * gone
}

/// Which pill is open or opening, and every pill closing behind it.
///
/// A pill's openness is a pure function of its own `Opening` and the time, so
/// any number of them can be closing at once after rapid hops, and every
/// animation starts from where the pill is, never from shut or open.
#[derive(Clone, Default)]
struct Pills {
    open: Option<Opening>,
    closing: Vec<Opening>,
}

/// How far along `since` and `span` is at `now`, from 0 to 1.
fn progress(since: Instant, span: Duration, now: Instant) -> f32 {
    (now.saturating_duration_since(since).as_secs_f32() / span.as_secs_f32()).clamp(0., 1.)
}

impl Pills {
    /// How open pill `ix` is at `now`, from shut at 0 to showing its hint at 1.
    /// Exactly 0 or 1 once settled.
    fn openness(&self, ix: usize, now: Instant) -> f32 {
        if let Some(open) = self.open.filter(|open| open.ix == ix) {
            let t = progress(open.since, open.span, now);
            return if t >= 1. {
                1.
            } else {
                open.from + (1. - open.from) * calm(t)
            };
        }
        if let Some(closing) = self.closing.iter().find(|closing| closing.ix == ix) {
            let t = progress(closing.since, closing.span, now);
            return if t >= 1. {
                0.
            } else {
                closing.from * (1. - calm(t))
            };
        }
        0.
    }

    /// The row is renewed: its questions are now `to`, and `map` says where
    /// a pill's question went (`None`: it is gone, and sinks as the renewal
    /// draws it, so it has no pill here to close). Each pill that stays keeps
    /// its own openness, whatever place it moves to, and no stale place is
    /// left pointing at another question. Whether the open pill stayed.
    fn remap(&mut self, now: Instant, map: &dyn Fn(usize) -> Option<usize>) -> bool {
        self.prune(now);
        self.closing = self
            .closing
            .iter()
            .filter_map(|closing| map(closing.ix).map(|ix| Opening { ix, ..*closing }))
            .collect();
        let Some(open) = self.open else {
            return false;
        };
        self.open = map(open.ix).map(|ix| Opening {
            ix,
            follow: false,
            ..open
        });
        self.open.is_some()
    }

    /// How far the pills closing to the left of `ix` still have to narrow, at
    /// `now`, for a hint `hint` wide: where `ix` will end up, relative to now.
    fn narrowing_before(&self, ix: usize, now: Instant, hint: f32) -> f32 {
        self.closing
            .iter()
            .filter(|closing| closing.ix < ix)
            .map(|closing| hint * self.openness(closing.ix, now))
            .sum()
    }

    /// Let go of the pills that have finished closing.
    fn prune(&mut self, now: Instant) {
        self.closing
            .retain(|closing| progress(closing.since, closing.span, now) < 1.);
    }

    /// Whether any pill is still opening or closing at `now`: whether it
    /// needs frames for its width.
    fn moving(&self, now: Instant) -> bool {
        self.open
            .is_some_and(|open| progress(open.since, open.span, now) < 1.)
            || self
                .closing
                .iter()
                .any(|closing| progress(closing.since, closing.span, now) < 1.)
    }

    /// Pill `ix`, if it is the one open, starts to close after `LEAVE`, from
    /// how open it is. Whether it was.
    fn leave(&mut self, ix: usize, now: Instant) -> bool {
        let Some(open) = self.open.filter(|open| open.ix == ix) else {
            return false;
        };
        self.closing.push(Opening {
            from: self.openness(ix, now),
            since: now + LEAVE,
            follow: false,
            span: SPAN,
            ..open
        });
        self.open = None;
        true
    }

    /// The pointer came onto pill `ix` at `now`. `at` is how far along the
    /// row is; `lit` whether the spotlight is already on the row.
    ///
    /// Hopping from another pill is a hand-over: the old one starts closing
    /// and this one starts opening at the same moment, both from where they
    /// are, both over `SPAN` on `calm`, so one's width goes as the other's
    /// comes and the row's length hardly changes. Nothing waits and nothing
    /// jumps. Whatever else is still closing carries on by itself.
    fn hover(&mut self, ix: usize, now: Instant, at: f32, lit: bool) {
        if self.open.is_some_and(|open| open.ix == ix) {
            return;
        }
        self.prune(now);
        let from = self.openness(ix, now);
        // Back before it began to close: it never did. Open as it was, its
        // light already crossed.
        let kept = self
            .closing
            .iter()
            .copied()
            .find(|closing| closing.ix == ix && now < closing.since && closing.from >= 1.);
        self.closing.retain(|closing| closing.ix != ix);
        let mut leaving = false;
        if let Some(prev) = self.open {
            let from = self.openness(prev.ix, now);
            self.open = None;
            if from > 0. {
                leaving = true;
                self.closing.push(Opening {
                    from,
                    since: now,
                    follow: false,
                    span: SPAN,
                    ..prev
                });
            }
        }
        // A pill still waiting out its `LEAVE` is left behind for good: its
        // close starts now, from where it stands.
        for closing in &mut self.closing {
            if closing.since > now {
                closing.since = now;
                closing.span = SPAN;
            }
        }
        leaving |= !self.closing.is_empty();
        self.open = Some(if let Some(kept) = kept {
            Opening {
                since: now.checked_sub(SHIMMER.max(SPAN)).unwrap_or(now),
                from: 1.,
                follow: false,
                span: SPAN,
                ..kept
            }
        } else {
            // From cold, it waits for the pointer to rest.
            let cold = from == 0. && !lit && !leaving;
            Opening {
                ix,
                since: if cold { now + REST } else { now },
                from,
                at,
                follow: true,
                span: SPAN,
            }
        });
    }
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
    /// Its questions are other words, so no place in the old row is a place
    /// in this one: nothing is kept, or a pill closing would be a different
    /// question's. (A renewal keeps what stays: see [`Carousel::renew`].)
    pub fn reset(&self) {
        self.glide.set(None);
        *self.pills.borrow_mut() = Pills::default();
        self.spot.set(None);
        self.drawn.set(None);
        self.handle.set_offset(point(px(0.), px(0.)));
    }

    /// Renew the row from the questions `from` to `to`, and keep the window
    /// drawing for as long as the renewal takes. A pill whose question stays
    /// stays open, as open as it was, wherever the question moves to; one whose
    /// question goes lets the light lift as it sinks.
    ///
    /// If any question is new, the row glides back to the front, where they
    /// land; a renewal that only took questions away leaves the reader where
    /// they were.
    pub fn renew(
        &self,
        length: Duration,
        from: &[SharedString],
        to: &[SharedString],
        window: &Window,
        cx: &mut App,
    ) {
        let arrived = to.iter().any(|ask| !from.contains(ask));
        let now = Instant::now();
        let kept = self.pills.borrow_mut().remap(now, &|ix| {
            from.get(ix)
                .and_then(|ask| to.iter().position(|now| now == ask))
        });
        if !kept && self.spot.get().is_some_and(|spot| spot.on) {
            self.spot.set(Some(Spot {
                on: false,
                since: now,
                from: self.spotlight(),
            }));
        }
        self.renewing.set(Some(Instant::now() + length));
        self.widths.borrow_mut().clear();
        if arrived {
            let now = f32::from(self.handle.offset().x);
            self.glide.set((now != 0.).then(|| Glide {
                from: now,
                to: 0.,
                since: Instant::now(),
            }));
        }
        self.drive(length.max(SPAN), window, cx);
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
                let busy = carousel.glide.get().is_some() || carousel.open().is_some();
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
                // Let go of the pill only: if the window was merely slow, the
                // row stays where the reader had slid it.
                if asked.is_some_and(|asked| asked.elapsed() > Duration::from_millis(500)) {
                    carousel.release();
                    break;
                }
                let resting = carousel.resting();
                let alive = if resting {
                    // No frames while it rests, so the pointer is asked about
                    // here: one gone without a word closes the pill now, not
                    // when the light next wants a frame.
                    handle.update(cx, |_, window, _| carousel.let_go(window))
                } else {
                    handle.update(cx, |_, window, _| window.refresh())
                };
                if alive.is_err() {
                    break;
                }
                if !resting {
                    asked.get_or_insert_with(Instant::now);
                }
                // About a frame of a fast screen while anything moves: often
                // enough for any of them, without asking for frames no screen
                // can show. Resting, a glance every few frames is plenty.
                let tick = if resting { 32 } else { 8 };
                cx.background_executor()
                    .timer(Duration::from_millis(tick))
                    .await;
            }
            carousel.drawing.set(None);
        })
        .detach();
    }

    /// Let go of any pill, open or closing, and the spotlight; the row
    /// stays where it is.
    fn release(&self) {
        *self.pills.borrow_mut() = Pills::default();
        self.spot.set(None);
    }

    /// Whether nothing on the row is moving this frame: a pill held fully
    /// open, its light already crossed, nothing sliding, closing or
    /// changing brightness. The loop keeps watching, but asks for no frames.
    fn resting(&self) -> bool {
        if self
            .renewing
            .get()
            .is_some_and(|until| Instant::now() < until)
        {
            return false;
        }
        let Some(open) = self.open() else {
            return false;
        };
        let now = Instant::now();
        self.glide.get().is_none()
            && !self.pills.borrow().moving(now)
            && !open.follow
            && self
                .spot
                .get()
                .is_none_or(|spot| spot.on && age(spot.since) > DIM.as_secs_f32())
            && self.light(open.ix).is_none()
    }

    /// The pill open, or opening, under the pointer.
    fn open(&self) -> Option<Opening> {
        self.pills.borrow().open
    }

    /// How open pill `ix` is, from shut at 0 to showing its hint at 1.
    fn openness(&self, ix: usize) -> f32 {
        self.pills.borrow().openness(ix, Instant::now())
    }

    /// How far the light has crossed pill `ix`, while it is crossing.
    fn light(&self, ix: usize) -> Option<f32> {
        let open = self.open().filter(|open| open.ix == ix)?;
        // Once, as the pill opens. Crossing again and again while the pointer
        // rested read as a pill asking to be pressed, not one being read.
        let t = age(open.since) / SHIMMER.as_secs_f32();
        (t < 1.).then_some(t)
    }

    /// How much of pill `ix`'s hint shows: `said_at` of how open it is.
    fn said(&self, ix: usize) -> f32 {
        said_at(self.openness(ix))
    }

    /// How strongly the row is given over to one pill, from none at 0 to
    /// all at 1.
    fn spotlight(&self) -> f32 {
        match self.spot.get() {
            None => 0.,
            Some(spot) if spot.on => {
                spot.from + (1. - spot.from) * ease(age(spot.since) / DIM.as_secs_f32())
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

    /// Pill `ix` closes, if it is the one open.
    fn shut(&self, ix: usize) {
        let now = Instant::now();
        if self.pills.borrow_mut().leave(ix, now) {
            self.spot.set(Some(Spot {
                on: false,
                since: now + LEAVE,
                from: self.spotlight(),
            }));
        }
    }

    /// The pointer came onto pill `ix`, or left it.
    fn peek(&self, ix: usize, on: bool, window: &Window, cx: &mut App) {
        if on {
            let now = Instant::now();
            if self.open().is_some_and(|open| open.ix == ix) {
                return;
            }
            let lit = self.spotlight() > 0.;
            let was = self.spotlight();
            self.pills
                .borrow_mut()
                .hover(ix, now, -f32::from(self.handle.offset().x), lit);
            let since = self.open().map_or(now, |open| open.since);
            match self.spot.get() {
                Some(spot) if spot.on => {
                    // Still waiting out the rest on a pill passed over: the
                    // light waits for this one, not the last.
                    if spot.since > now {
                        self.spot.set(Some(Spot { since, ..spot }));
                    }
                }
                _ => self.spot.set(Some(Spot {
                    on: true,
                    since,
                    from: was,
                })),
            }
        } else {
            self.shut(ix);
        }
        self.drive(REST + SPAN.max(LEAVE + SPAN + GRACE + LIFT), window, cx);
    }

    /// Start sliding one question back, or one on.
    fn step(&self, by: isize) {
        let now = f32::from(self.handle.offset().x);
        let view = self.handle.bounds();
        let places = self.places.borrow().clone();
        let lefts: Vec<f32> = places
            .iter()
            .map_while(|drawn| self.handle.bounds_for_item(*drawn))
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
        self.pills.borrow_mut().prune(Instant::now());
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
        let Some(open) = self.open().filter(|open| open.follow) else {
            return;
        };
        let drawn = self
            .places
            .borrow()
            .get(open.ix)
            .copied()
            .unwrap_or(open.ix);
        let over = progress(open.since, open.span, Instant::now()) >= 1.;
        let Some(item) = self.handle.bounds_for_item(drawn) else {
            // Nothing to follow: stop once the opening is over, so a pill
            // that can never be found does not keep the window drawing.
            if over {
                self.pills.borrow_mut().open = Some(Opening {
                    follow: false,
                    ..open
                });
            }
            return;
        };
        let view = self.handle.bounds();
        let left = f32::from(item.origin.x - view.origin.x);
        let now = self.openness(open.ix);
        let want = follow_target(
            open.at,
            left,
            self.pills
                .borrow()
                .narrowing_before(open.ix, Instant::now(), self.hint.get()),
            f32::from(item.size.width),
            self.hint.get(),
            now,
            f32::from(view.size.width),
        );
        let gone = if open.from >= 1. {
            1.
        } else {
            ((now - open.from) / (1. - open.from)).clamp(0., 1.)
        };
        let at = follow_at(open.at, want, gone);
        self.handle.set_offset(point(px(-at), px(0.)));
        if gone >= 1. || over {
            self.pills.borrow_mut().open = Some(Opening {
                follow: false,
                ..open
            });
        }
    }

    /// Close the open pill if the pointer has left the window. A window only
    /// a pill high is easily left without a word to it, and a pill that stayed
    /// open, lit and spotlit, over a pointer long gone is the bug this ends.
    #[cfg_attr(not(target_os = "macos"), allow(clippy::unused_self))]
    fn let_go(&self, window: &Window) {
        #[cfg(target_os = "macos")]
        if let Some(open) = self.open()
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

/// How wide an arrow is.
const ARROW: f32 = 36.;

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
    // Both arrows, always there and always live, as in v0.1.6: the row keeps
    // one shape whether its questions fill it or not, and an arrow at the end
    // of the row simply has nowhere further to slide.
    let arrow = |id: &'static str, glyph: &'static str, by: isize| {
        let carousel = carousel.clone();
        surface(ElementId::from(id))
            .w(px(ARROW))
            .justify_center()
            .text_size(px(15.))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(move |_, window, cx| carousel.slide(by, window, cx))
            .child(glyph)
    };
    let mut made: Vec<Option<Stateful<Div>>> = row
        .asks
        .iter()
        .enumerate()
        .map(|(ix, ask)| {
            let pick = pick.clone();
            let question = ask.clone();
            let open = carousel.openness(ix);
            // The others step back by their words alone: a pill whose border and
            // ground went see-through read as broken, not as quieter.
            let dim = 0.6 * spotlight * (1. - open);
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
        })
        .map(Some)
        .collect();
    let renewing = row
        .renewed
        .as_ref()
        .filter(|renewal| !renewal.over(&row.asks));
    *carousel.places.borrow_mut() = match renewing {
        None => (0..row.asks.len()).collect(),
        Some(renewal) => {
            // Counted as they are drawn: a question gone under is no longer
            // among the children, and must not hold a place.
            let since = renewal.since.elapsed();
            let mut places: Vec<usize> = (0..row.asks.len()).collect();
            let mut drawn = 0;
            for (_, part) in parts(&renewal.from, &row.asks) {
                match part {
                    Part::Stays(n) | Part::Arrives(n, _) => {
                        places[n] = drawn;
                        drawn += 1;
                    }
                    Part::Sinks(k) => {
                        if under(since, k) < 1. {
                            drawn += 1;
                        }
                    }
                }
            }
            places
        }
    };
    let pills = match renewing {
        None => made
            .into_iter()
            .flatten()
            .map(IntoElement::into_any_element)
            .collect::<Vec<_>>(),
        Some(renewal) => {
            let since = renewal.since.elapsed();
            let natural = |ask: &SharedString| {
                if let Some(wide) = carousel.widths.borrow().get(ask) {
                    return *wide;
                }
                let run = TextRun {
                    len: ask.len(),
                    font: window.text_style().font(),
                    color: paint(palette.fg),
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                };
                let words = f32::from(
                    window
                        .text_system()
                        .shape_line(ask.clone(), px(13.), &[run], None)
                        .width,
                );
                // A hair over, so the words are never a pixel short and cut.
                let wide = (words + 2. * 13. + 2. + 1.).min(WIDEST + hint_w);
                carousel.widths.borrow_mut().insert(ask.clone(), wide);
                wide
            };
            parts(&renewal.from, &row.asks)
                .into_iter()
                .filter_map(|(ask, part)| match part {
                    Part::Stays(n) => made[n].take().map(IntoElement::into_any_element),
                    Part::Arrives(n, k) => {
                        let age = since
                            .saturating_sub(ARRIVE_AFTER + nth(ARRIVE_STAGGER, k))
                            .as_secs_f32();
                        let room = calm(age / ROOM.as_secs_f32());
                        let slid = calm((age - SLIDE_AFTER.as_secs_f32()) / SLIDE.as_secs_f32());
                        // Pressable only once it has arrived; until then it is
                        // drawn as the pill it is becoming, rounded at both
                        // ends, its words uncovered as the row opens round it.
                        made[n].take();
                        Some(
                            div()
                                .flex_none()
                                .min_w_0()
                                .w(px(natural(&ask) * room))
                                .ml(px(-GAP * (1. - room)))
                                .h(px(PILL))
                                .relative()
                                .left(px(-SLIDE_BY * (1. - slid)))
                                .opacity(slid)
                                .rounded(px(corner))
                                .border_1()
                                .border_color(paint(palette.edge))
                                .bg(paint(palette.band))
                                .overflow_hidden()
                                .flex()
                                .items_center()
                                .child(
                                    div()
                                        .flex_none()
                                        .pl(px(13.))
                                        .whitespace_nowrap()
                                        .text_size(px(13.))
                                        .text_color(paint(palette.fg))
                                        .child(ask),
                                )
                                .into_any_element(),
                        )
                    }
                    Part::Sinks(k) => {
                        let under = under(since, k);
                        let gone = calm(under);
                        let closed = calm((under - CLOSE_FROM) / (1. - CLOSE_FROM));
                        let size = 1. - (1. - SHRINK) * gone;
                        let wide = natural(&ask) * size * (1. - closed);
                        // Gone under: nothing is left on the row, not even a
                        // sliver of border holding a gap open.
                        if under >= 1. {
                            return None;
                        }
                        // One piece, the pill itself: it grows smaller and
                        // narrower, ends and all, with its words cut inside
                        // it. It shrinks down onto the row's floor rather than
                        // dropping through it — the shelf is a pill tall, and
                        // anything lower is cut off by the window's edge.
                        Some(
                            div()
                                .flex_none()
                                .min_w_0()
                                .w(px(wide))
                                .mr(px(-GAP * closed))
                                .h(px(PILL * size))
                                .relative()
                                .top(px(PILL * (1. - size) / 2.))
                                .opacity(1. - gone)
                                .rounded(px(corner * size))
                                .border_1()
                                .border_color(paint(palette.edge))
                                .bg(paint(palette.band))
                                .overflow_hidden()
                                .flex()
                                .items_center()
                                .child(
                                    div()
                                        .flex_none()
                                        .pl(px(13. * size))
                                        .whitespace_nowrap()
                                        .text_size(px(13. * size))
                                        .text_color(paint(palette.fg))
                                        .child(ask),
                                )
                                .into_any_element(),
                        )
                    }
                })
                .collect()
        }
    };
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

    fn said_pairs(before: &[f32; 4], after: &[f32; 4]) -> Vec<(f32, f32)> {
        before
            .iter()
            .zip(after)
            .map(|(b, a)| (said_at(*b), said_at(*a)))
            .collect()
    }

    fn ms(base: Instant, n: u64) -> Instant {
        base + Duration::from_millis(n)
    }

    fn all(pills: &Pills, now: Instant) -> [f32; 4] {
        [0, 1, 2, 3].map(|ix| pills.openness(ix, now))
    }

    fn said(pills: &Pills, now: Instant) -> [f32; 4] {
        all(pills, now).map(said_at)
    }

    #[test]
    fn a_pill_is_the_same_the_moment_after_it_is_interrupted() {
        let base = Instant::now();
        for cut in 0..1200 {
            for how in 0..3 {
                let mut pills = Pills::default();
                pills.hover(0, base, 0., false);
                pills.hover(1, ms(base, 400), 0., true);
                let now = ms(base, 400 + cut);
                let before = all(&pills, now);
                match how {
                    0 => pills.hover(2, now, 0., true),
                    1 => pills.hover(0, now, 0., true),
                    _ => {
                        pills.leave(1, now);
                    }
                }
                let after = all(&pills, now);
                for (b, a) in said_pairs(&before, &after) {
                    assert!((b - a).abs() < 1e-4, "said jumped {b} -> {a}");
                }
                for ix in 0..4 {
                    assert!(
                        (before[ix] - after[ix]).abs() < 1e-4,
                        "pill {ix} jumped {before:?} -> {after:?} ({how} at {cut})"
                    );
                }
            }
        }
    }

    #[test]
    fn a_hop_trades_width_so_the_row_stays_as_long() {
        let base = Instant::now();
        let mut pills = Pills::default();
        pills.hover(0, base, 0., false);
        let hop = ms(base, 2000);
        assert_eq!(pills.openness(0, hop), 1.);
        pills.hover(1, hop, 0., true);
        for n in 0..=1000 {
            let now = ms(hop, n);
            let sum = pills.openness(0, now) + pills.openness(1, now);
            assert!((sum - 1.).abs() <= 0.02, "sum {sum} at {n}ms");
        }
    }

    #[test]
    fn a_fast_run_across_four_pills_never_jumps() {
        let base = Instant::now();
        for leaving in [false, true] {
            let mut pills = Pills::default();
            let mut last = [0.; 4];
            let mut last_said = [0.; 4];
            let mut hop = 0;
            for step in 0..400_u64 {
                let at = step * 8;
                let now = ms(base, at);
                if at >= hop * 100 && hop < 4 {
                    if leaving && hop > 0 {
                        pills.leave(usize::try_from(hop).unwrap() - 1, now);
                    }
                    pills.hover(usize::try_from(hop).unwrap(), now, 0., hop > 0);
                    hop += 1;
                }
                let here = all(&pills, now);
                for ix in 0..4 {
                    assert!(
                        (here[ix] - last[ix]).abs() <= step_bound(),
                        "pill {ix} jumped {} -> {} at {at}ms",
                        last[ix],
                        here[ix]
                    );
                }
                let spoken = said(&pills, now);
                for ix in 0..4 {
                    assert!(
                        (spoken[ix] - last_said[ix]).abs() <= step_bound(),
                        "said {ix} jumped {} -> {} at {at}ms",
                        last_said[ix],
                        spoken[ix]
                    );
                }
                last = here;
                last_said = spoken;
            }
        }
    }

    #[test]
    fn everything_settles_and_asks_for_nothing() {
        let base = Instant::now();
        let mut pills = Pills::default();
        for (n, ix) in [0, 1, 2, 3].into_iter().enumerate() {
            pills.hover(ix, ms(base, 100 * n as u64), 0., n > 0);
        }
        let last = ms(base, 300);
        assert!(pills.moving(last));
        let done = last + REST + SPAN + Duration::from_millis(1);
        assert_eq!(all(&pills, done), [0., 0., 0., 1.]);
        pills.prune(done);
        assert!(pills.closing.is_empty() && !pills.moving(done));
        pills.leave(3, done);
        let gone = done + LEAVE + SPAN + Duration::from_millis(1);
        assert_eq!(all(&pills, gone), [0.; 4]);
        pills.prune(gone);
        assert!(pills.closing.is_empty() && !pills.moving(gone));
    }

    #[test]
    fn a_hop_onto_a_pill_cut_off_at_the_edge_keeps_it_under_the_pointer() {
        // Six pills of 100, a hint of 80, a window of 300 wide; the row is at
        // 0, and the pointer rests on the fourth, whose right edge is cut off.
        let (base_w, hint, view) = (100., 80., 300.);
        let base = Instant::now();
        let mut pills = Pills::default();
        pills.hover(2, base, 120., false);
        let hop = ms(base, 2000);
        pills.hover(3, hop, 120., true);
        let left_of = |ix: usize, now: Instant| -> f32 {
            (0..ix)
                .map(|j| base_w + hint * pills.openness(j, now))
                .sum()
        };
        let mut at;
        let pointer = left_of(3, hop) - 120. + 30.;
        let mut last_right = f32::MIN;
        for n in 0..=800 {
            let now = ms(hop, n);
            let open = pills.open.unwrap();
            let o = pills.openness(3, now);
            let want = follow_target(
                open.at,
                left_of(3, now),
                pills.narrowing_before(3, now, hint),
                base_w + hint * o,
                hint,
                o,
                view,
            );
            let gone = if open.from >= 1. {
                1.
            } else {
                ((o - open.from) / (1. - open.from)).clamp(0., 1.)
            };
            at = follow_at(open.at, want, gone);
            let (l, r) = (
                left_of(3, now) - at,
                left_of(3, now) + base_w + hint * o - at,
            );
            assert!(
                l <= pointer && pointer <= r,
                "pointer outside {l}..{r} at {n}ms"
            );
            assert!(
                r >= last_right - 1e-3 || last_right > view,
                "edge went back at {n}ms"
            );
            last_right = r;
        }
        assert!(
            (last_right - view).abs() < 1e-3,
            "ends whole in view: {last_right}"
        );
    }

    #[test]
    fn a_hint_is_never_seen_squeezed() {
        for n in 0..=1000 {
            let o = n as f32 / 1000.;
            assert!(
                said_at(o) == 0. || o > 0.5,
                "text shown in a pill under half open"
            );
            // The share of the hint cut off, as bright as the hint is.
            assert!((1. - o) * said_at(o) <= 0.13, "squeezed at {o}");
        }
    }

    #[test]
    fn a_renewal_keeps_the_open_pill_open_wherever_its_question_goes() {
        let base = Instant::now();
        let mut pills = Pills::default();
        pills.hover(1, base, 0., false);
        pills.hover(2, ms(base, 1000), 0., true);
        let now = ms(base, 1200);
        let (a, b) = (pills.openness(1, now), pills.openness(2, now));
        // Two questions arrive in front: the old 1 and 2 are now 3 and 4.
        assert!(pills.remap(now, &|ix| Some(ix + 2)));
        assert_eq!(pills.openness(3, now), a);
        assert_eq!(pills.openness(4, now), b);
        assert_eq!((pills.openness(1, now), pills.openness(2, now)), (0., 0.));
    }

    #[test]
    fn a_renewal_that_drops_the_pill_leaves_no_stale_place() {
        let base = Instant::now();
        let mut pills = Pills::default();
        pills.hover(1, base, 0., false);
        pills.hover(2, ms(base, 1000), 0., true);
        let now = ms(base, 1200);
        // The open one goes, and the closing one moves to the front.
        assert!(!pills.remap(now, &|ix| (ix == 1).then_some(0)));
        assert!(pills.open.is_none());
        assert_eq!(pills.closing.len(), 1);
        assert_eq!(pills.closing[0].ix, 0);
        assert_eq!(pills.openness(2, now), 0.);
    }

    #[test]
    fn following_stops_when_the_opening_is_over() {
        let base = Instant::now();
        let mut pills = Pills::default();
        pills.hover(1, base, 0., false);
        let open = pills.open.unwrap();
        assert!(open.follow);
        assert!(progress(open.since, open.span, ms(base, 150) + SPAN) >= 1.);
    }

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
    fn a_pill_is_lit_once_while_it_opens_and_not_after() {
        let row = Carousel::default();
        let open = |since| {
            row.pills.borrow_mut().open = Some(Opening {
                ix: 2,
                since,
                from: 0.,
                at: 0.,
                follow: false,
                span: SPAN,
            })
        };
        let now = Instant::now();
        open(now + Duration::from_millis(90));
        assert_eq!(row.light(2), Some(0.), "waiting out the rest, not yet lit");
        open(now);
        assert!(row.light(2).is_some(), "crossing as it opens");
        assert_eq!(row.light(1), None, "and only on the pill that opened");
        open(now - SHIMMER);
        assert_eq!(
            row.light(2),
            None,
            "crossed once, and dark while the pointer rests"
        );
    }

    #[test]
    fn a_renewing_row_only_ever_moves_one_way() {
        let row = |asks: &[&str]| {
            asks.iter()
                .map(|ask| SharedString::from(*ask))
                .collect::<Vec<_>>()
        };
        // Keep only the second: the first and the rest sink where they stand,
        // the second stays, and the new ones come in after it.
        let from = row(&["a", "b", "c", "d"]);
        let to = row(&["b", "x", "y"]);
        let drawn: Vec<_> = parts(&from, &to)
            .into_iter()
            .map(|(_, part)| part)
            .collect();
        assert_eq!(
            drawn,
            [
                Part::Arrives(1, 0),
                Part::Arrives(2, 1),
                Part::Sinks(0),
                Part::Stays(0),
                Part::Sinks(1),
                Part::Sinks(2),
            ]
        );
        // Nothing new: only the sinking takes time.
        let renewal = Renewal {
            from: from.clone(),
            since: Instant::now(),
        };
        assert_eq!(renewal.length(&row(&["a", "b"])), SINK_STAGGER * 1 + SINK);
        // New ones: the last one's slide is what ends it.
        assert_eq!(
            renewal.length(&to),
            ARRIVE_AFTER + ARRIVE_STAGGER + SLIDE_AFTER + SLIDE
        );
        assert!(!renewal.over(&to), "it has only just begun");
    }

    #[test]
    fn new_questions_go_first_and_the_shown_keep_their_order() {
        // One new one: in front, the rest as they were shown.
        assert_eq!(
            front_first(&["a", "b", "c"], &["a", "b", "c", "x"]),
            ["x", "a", "b", "c"]
        );
        // A shown question that is gone drops out; new ones keep their order.
        assert_eq!(
            front_first(&["a", "b", "c"], &["x", "c", "y", "a"]),
            ["x", "y", "a", "c"]
        );
        // The shown order wins over the new list's order for what stays.
        assert_eq!(front_first(&["a", "b"], &["b", "a"]), ["a", "b"]);
        // Nothing new: unchanged. Everything replaced: all new, in order.
        assert_eq!(front_first(&["a", "b"], &["a", "b"]), ["a", "b"]);
        assert_eq!(front_first(&["a", "b"], &["x", "y"]), ["x", "y"]);
        assert_eq!(front_first(&[], &["x", "y"]), ["x", "y"]);
        // A fixed point: showing what is already in front-first order again,
        // against the same questions, changes nothing.
        let cases: [(&[&str], &[&str]); 6] = [
            (&["a", "b", "c"], &["a", "b", "c", "x"]),
            (&["a", "b", "c"], &["x", "c", "y", "a"]),
            (&["a", "b"], &["b", "a"]),
            (&["a", "b"], &["x", "y"]),
            (&[], &["x", "y"]),
            (&["a", "b"], &[]),
        ];
        for (prev, new) in cases {
            let once = front_first(prev, new);
            assert_eq!(front_first(&once, new), once, "{prev:?} -> {new:?}");
        }
    }

    #[test]
    fn a_group_comes_back_in_the_order_it_was_left() {
        let mut memory = HashMap::new();
        let first = remembered(&memory, "g", &["a".to_string(), "b".to_string()]);
        assert_eq!(first, ["a", "b"], "nothing remembered: as written");
        memory.insert("g".to_string(), vec!["x".to_string(), "a".to_string()]);
        // Gone and back, with one more arrived meanwhile: it is in front, and
        // what was shown keeps its order.
        let back = remembered(
            &memory,
            "g",
            &["a".to_string(), "x".to_string(), "n".to_string()],
        );
        assert_eq!(back, ["n", "x", "a"]);
        // Another group is not touched by it.
        assert_eq!(remembered(&memory, "h", &["a".to_string()]), ["a"]);
    }

    #[test]
    fn the_renewal_is_calm_at_both_ends() {
        assert_eq!(calm(0.), 0.);
        assert!((calm(1.) - 1.).abs() < 1e-6);
        assert!((calm(0.5) - 0.5).abs() < 1e-6);
        assert!(calm(0.05) < 0.01, "it starts gently");
        assert!(calm(0.95) > 0.99, "and settles gently");
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
