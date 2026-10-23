//! The text caret deck draws itself.
//!
//! Everything else in deck that moves, eases: the bar widens, the rail slides,
//! the lit lines fade. The caret was the one thing that did not. The library
//! paints it as a hard rectangle that is on for half a second and off for half
//! a second, and that jumps to each new position the instant a key lands — a
//! strobe in the middle of an otherwise calm window, right where the eye rests
//! while writing.
//!
//! So deck switches the library's caret off (see [`crate::palette::install`])
//! and paints its own over each field it owns, doing two things differently:
//!
//! - **It glides, but never behind the text.** A caret that lags the letters
//!   being typed reads as the window being slow, so the caret snaps, with no
//!   glide at all, whenever the text changed (typing, deleting, pasting), and
//!   whenever a move comes within [`BURST`] of the last one (a held arrow key,
//!   a quick run of clicks), so moves never queue up glides. Only an isolated
//!   caret-only move glides: a click, a jump by word or line, a single arrow
//!   press after a pause. It travels over [`GLIDE`] with a smoothstep, from
//!   wherever the caret is drawn *now*, and lands exactly on its target.
//! - **It breathes.** Instead of blinking, the opacity follows a cosine from
//!   full down to [`DIM`] and back every [`CYCLE`]. While the reader is typing
//!   or moving the caret it is held at full, so it can be found; the breathing
//!   resumes [`HOLD`] after the last change.
//!
//! The library says where the caret belongs (`cursor_layout`), in window
//! coordinates, with the horizontal scroll already folded in. The vertical
//! scroll is not folded in there — the library adds it when it paints — so
//! [`Reading`] adds it back. Because the position is in window coordinates the
//! overlay can paint straight into the window; it only needs a box over the
//! field so that it paints after the field. Painting into the window does not
//! by itself keep the caret inside the field: when a long text is scrolled so
//! the caret's line is out of the box, the quad would still land on whatever
//! neighbours the field. So the overlay clips its own paint to its own bounds,
//! which are the field's, and the caret cannot escape however the wrapper
//! around the field is styled.
//!
//! Redraws are paid for only when they show something. While the caret glides,
//! or is held solid after a change, the overlay asks for a frame every display
//! refresh, because the eye can see those. Once it is only breathing, a slow
//! cosine, it stops asking and a timer wakes the window about thirty times a
//! second instead: that is plenty for a curve that takes over a second, and it
//! stops a quiet, focused field from redrawing the whole window at display
//! rate for ever. An unfocused field asks for nothing at all.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui_kit::{
    Bounds, ContentMask, Hsla, IntoElement, Styled, Task, canvas, fill, point, px, size,
};

/// How long the caret takes to travel to a new place.
const GLIDE: f32 = 0.080;
/// A move this soon after the previous one (or after typing) snaps instead of
/// gliding. A held arrow key repeats faster than this.
const BURST: f32 = 0.150;
/// How long the caret stays solid after the last change before it breathes.
const HOLD: f32 = 0.500;
/// One full breath, bright to dim to bright.
const CYCLE: f32 = 1.100;
/// How faint the caret gets at the bottom of a breath. Not zero: a caret that
/// vanishes completely is a caret you lose.
const DIM: f32 = 0.15;
/// Moves longer than this many lines snap rather than glide. Across that
/// distance a glide is a streak down the page, not a motion the eye follows.
const SNAP_LINES: f32 = 2.0;
/// How often the window is woken while the caret only breathes. Roughly thirty
/// times a second.
const BREATH_TICK: Duration = Duration::from_millis(33);
/// The caret's width on macOS, in points.
const WIDTH: f32 = 1.5;

/// Where the library says the caret is, this frame.
#[derive(Debug, Clone, Copy)]
pub struct Reading {
    /// Window coordinates of the caret's top-left, scroll included.
    pub x: f32,
    pub y: f32,
    /// The height the library reports for the caret, and its line height.
    pub height: f32,
    pub line_height: f32,
    /// The field's scroll offset. A change in it moves the target without the
    /// caret having moved in the text, and that is not worth gliding for.
    pub scroll: (f32, f32),
    /// The text's length. A change in it means the text changed, as opposed to
    /// the caret merely moving through it.
    pub len: usize,
    /// Changes whenever the text or the caret position changes.
    pub stamp: u64,
    /// The field's own bounds in window coordinates, as the library lays it
    /// out: origin x, origin y, width, height. The caret is clipped to them.
    pub clip: Option<[f32; 4]>,
}

/// What a field is doing, as far as the caret cares: not focused, focused with
/// a selected range (the library draws no caret then, so neither do we), or
/// focused with a caret to draw.
#[derive(Debug, Clone, Copy)]
pub enum Field {
    Idle,
    Selecting,
    Caret(Reading),
}

/// Read a field's state into a [`Field`]. A macro, not a function, because the
/// two kinds of state deck owns are different types of one generic the library
/// does not export a bound for.
#[macro_export]
macro_rules! caret_field {
    ($state:expr, $window:expr, $cx:expr) => {{
        let state = $state.read($cx);
        if !state.focus_handle($cx).is_focused($window) {
            $crate::caret::Field::Idle
        } else if !state.selected_range().is_empty() {
            $crate::caret::Field::Selecting
        } else if let Some((bounds, line_height)) = state.cursor_layout() {
            let scroll = state.scroll_offset();
            $crate::caret::Field::Caret($crate::caret::Reading {
                x: f32::from(bounds.origin.x),
                y: f32::from(bounds.origin.y + scroll.y),
                height: f32::from(bounds.size.height),
                line_height: f32::from(line_height),
                scroll: (f32::from(scroll.x), f32::from(scroll.y)),
                len: state.text().len(),
                stamp: (state.cursor() as u64) << 32 ^ state.text().len() as u64,
                clip: {
                    let field = state.input_bounds();
                    Some([
                        f32::from(field.origin.x),
                        f32::from(field.origin.y),
                        f32::from(field.size.width),
                        f32::from(field.size.height),
                    ])
                },
            })
        } else {
            $crate::caret::Field::Idle
        }
    }};
}

/// Ease in and out: slow, quick, slow. `t` is clamped to `0..=1`.
#[must_use]
pub fn smoothstep(t: f32) -> f32 {
    let t = t.clamp(0., 1.);
    t * t * (3. - 2. * t)
}

/// Where a glide from `from` to `to` has got to, `t` of the way through its
/// time.
#[must_use]
pub fn glide(from: (f32, f32), to: (f32, f32), t: f32) -> (f32, f32) {
    let eased = smoothstep(t);
    (
        from.0 + (to.0 - from.0) * eased,
        from.1 + (to.1 - from.1) * eased,
    )
}

/// How bright the caret is, `quiet` seconds after the last change.
///
/// Solid until [`HOLD`], then a cosine that starts at the top (so there is no
/// step where the breathing takes over) and swings down to [`DIM`] and back.
#[must_use]
pub fn breathe(quiet: f32) -> f32 {
    if quiet < HOLD {
        return 1.0;
    }
    let phase = (quiet - HOLD) / CYCLE;
    let wave = 0.5 + 0.5 * (phase * std::f32::consts::TAU).cos();
    DIM + (1.0 - DIM) * wave
}

/// The glide and breathe state of one caret.
#[derive(Debug, Default)]
pub struct State {
    /// Where the caret is drawn, once it has been placed at all.
    drawn: Option<(f32, f32)>,
    from: (f32, f32),
    to: (f32, f32),
    started: Option<Instant>,
    changed: Option<Instant>,
    /// When the caret last moved or the text last changed: what decides
    /// whether the next move is a burst.
    moved: Option<Instant>,
    len: usize,
    stamp: u64,
    scroll: (f32, f32),
}

/// What to paint this frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frame {
    pub x: f32,
    pub y: f32,
    pub height: f32,
    pub opacity: f32,
    /// Whether this frame is part of something quick, a glide or the solid
    /// hold after a change, that needs a frame every refresh. When false the
    /// caret is only breathing and a slow timer is enough.
    pub quick: bool,
}

impl State {
    /// Advance to `now` given what the field is doing. `None` means draw
    /// nothing and ask for no more frames.
    pub fn frame(&mut self, now: Instant, field: Field) -> Option<Frame> {
        let Field::Caret(reading) = field else {
            // Unfocused, or a range is selected. Forget the place, so the
            // caret reappears where it belongs rather than flying in from
            // wherever it was last seen.
            self.drawn = None;
            self.started = None;
            self.changed = None;
            self.moved = None;
            return None;
        };
        let target = (reading.x, reading.y);
        let secs = |since: Instant| now.saturating_duration_since(since).as_secs_f32();

        if self.drawn.is_none() {
            self.drawn = Some(target);
            self.from = target;
            self.to = target;
            self.changed = Some(now);
            self.stamp = reading.stamp;
            self.len = reading.len;
            self.scroll = reading.scroll;
        }
        // Typing counts as a move for the burst rule below, so a caret whose
        // layout lags the text by a frame still snaps when it catches up.
        let typed = reading.len != self.len;
        self.len = reading.len;
        if typed {
            self.moved = Some(now);
        }
        if reading.stamp != self.stamp {
            self.stamp = reading.stamp;
            self.changed = Some(now);
        }
        if target != self.to {
            // Carry on from the current drawn spot, never the old resting one.
            let here = self.drawn.unwrap_or(target);
            let scrolled = reading.scroll != self.scroll;
            let far = (target.1 - here.1).abs() > reading.line_height * SNAP_LINES;
            // A scroll moves the target without moving the caret in the text:
            // gliding would make it trail behind the words it belongs to.
            let burst = typed || self.moved.is_some_and(|at| secs(at) < BURST);
            let from = if scrolled || far || burst {
                target
            } else {
                here
            };
            self.from = from;
            self.to = target;
            // A snap has no glide to run, so it asks for no glide frames.
            self.started = (from != target).then_some(now);
            self.moved = Some(now);
        }
        self.scroll = reading.scroll;

        let t = self.started.map_or(1., |at| secs(at) / GLIDE);
        // Exactly on the target once the glide is over, with no residue from
        // the arithmetic of the last step.
        let here = if t >= 1. {
            self.to
        } else {
            glide(self.from, self.to, t)
        };
        self.drawn = Some(here);
        let quiet = self.changed.map_or(0., secs);
        Some(Frame {
            x: here.0,
            y: here.1,
            height: reading.height,
            opacity: breathe(quiet),
            quick: t < 1. || quiet < HOLD,
        })
    }
}

/// A caret's state, shared between the view that owns the field and the
/// paint that draws over it.
#[derive(Clone, Default)]
pub struct Caret {
    state: Rc<RefCell<State>>,
    /// The pending wake-up while the caret only breathes. Replacing it drops
    /// (and so cancels) the one before; clearing it cancels it.
    wake: Rc<RefCell<Option<Task<()>>>>,
}

impl std::fmt::Debug for Caret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Caret")
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

/// An overlay to put over a field, as a sibling after it, inside a parent that
/// is `relative()`. It paints the caret, clipped to the field, and asks for
/// the next frame only while a caret is showing.
pub fn overlay(
    caret: &Caret,
    colour: Hsla,
    read: impl Fn(&mut gpui_kit::Window, &mut gpui_kit::App) -> Field + 'static,
) -> impl IntoElement {
    let caret = caret.clone();
    canvas(
        |_, _, _| (),
        move |_, (), window, cx| {
            let field = read(window, cx);
            // Clipped to the field itself, as the library lays it out, never
            // to this overlay's own box: the overlay can be laid out with no
            // size at all, and clipping to it hid the caret entirely.
            let clip = match field {
                Field::Caret(reading) => reading.clip.and_then(|[x, y, w, h]| {
                    (w > 0. && h > 0.).then(|| Bounds::new(point(px(x), px(y)), size(px(w), px(h))))
                }),
                _ => None,
            };
            let frame = caret.state.borrow_mut().frame(Instant::now(), field);
            let Some(frame) = frame else {
                // Not focused: no caret, and no wake-up either.
                caret.wake.borrow_mut().take();
                return;
            };
            let width = if cfg!(target_os = "macos") {
                WIDTH
            } else {
                1.0
            };
            window.with_content_mask(clip.map(|bounds| ContentMask { bounds }), |window| {
                window.paint_quad(fill(
                    Bounds::new(
                        point(px(frame.x), px(frame.y)),
                        size(px(width), px(frame.height)),
                    ),
                    colour.opacity(frame.opacity),
                ));
            });
            if frame.quick {
                caret.wake.borrow_mut().take();
                window.request_animation_frame();
            } else {
                // Only breathing: wake the window from a timer instead of
                // every refresh. A new paint replaces the timer, so there is
                // never more than one pending.
                let task = window.spawn(cx, async move |cx| {
                    cx.background_executor().timer(BREATH_TICK).await;
                    cx.update(|window, _| window.refresh()).ok();
                });
                *caret.wake.borrow_mut() = Some(task);
            }
        },
    )
    .absolute()
    .size_full()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn reading(x: f32, y: f32, stamp: u64) -> Field {
        Field::Caret(Reading {
            clip: None,
            x,
            y,
            height: 16.,
            line_height: 20.,
            scroll: (0., 0.),
            len: 0,
            stamp,
        })
    }

    /// A reading after the text changed to `len` characters.
    fn typed(x: f32, y: f32, stamp: u64, len: usize) -> Field {
        Field::Caret(Reading {
            clip: None,
            x,
            y,
            height: 16.,
            line_height: 20.,
            scroll: (0., 0.),
            len,
            stamp,
        })
    }

    #[test]
    fn smoothstep_is_slow_at_both_ends() {
        assert_eq!(smoothstep(0.), 0.);
        assert_eq!(smoothstep(1.), 1.);
        assert!((smoothstep(0.5) - 0.5).abs() < 1e-6);
        assert!(smoothstep(0.1) < 0.1);
        assert!(smoothstep(0.9) > 0.9);
    }

    #[test]
    fn glide_goes_from_one_end_to_the_other() {
        assert_eq!(glide((0., 0.), (10., 4.), 0.), (0., 0.));
        assert_eq!(glide((0., 0.), (10., 4.), 1.), (10., 4.));
        assert_eq!(glide((0., 0.), (10., 4.), 0.5), (5., 2.));
    }

    #[test]
    fn breathing_stays_in_range_and_holds_while_typing() {
        assert_eq!(breathe(0.), 1.0);
        assert_eq!(breathe(HOLD - 0.01), 1.0);
        assert!((breathe(HOLD) - 1.0).abs() < 1e-6);
        let mut lowest: f32 = 1.0;
        for step in 0..4000 {
            let opacity = breathe(step as f32 * 0.001);
            assert!((DIM - 1e-6..=1.0 + 1e-6).contains(&opacity), "{opacity}");
            lowest = lowest.min(opacity);
        }
        assert!((lowest - DIM).abs() < 1e-3);
        assert!((breathe(HOLD + CYCLE / 2.) - DIM).abs() < 1e-5);
    }

    #[test]
    fn an_isolated_move_glides_and_lands_exactly_on_target() {
        let start = Instant::now();
        let mut caret = State::default();
        caret.frame(start, reading(0., 0., 1));
        // A click a second later, well apart from anything else.
        let click = start + Duration::from_millis(1000);
        let first = caret.frame(click, reading(100.3, 0., 2)).unwrap();
        assert_eq!(first.x, 0.);
        let mid = click + Duration::from_millis(40);
        let halfway = caret.frame(mid, reading(100.3, 0., 2)).unwrap();
        assert!((halfway.x - 50.15).abs() < 1.0, "{}", halfway.x);
        let done = caret.frame(click + Duration::from_millis(80), reading(100.3, 0., 2));
        assert_eq!(done.unwrap().x, 100.3);
    }

    #[test]
    fn a_text_change_snaps_the_caret_at_once() {
        let start = Instant::now();
        let mut caret = State::default();
        caret.frame(start, typed(0., 0., 1, 5));
        // Long after the last move, so only the text change can explain a snap.
        let later = start + Duration::from_millis(2000);
        let frame = caret.frame(later, typed(8., 0., 2, 6)).unwrap();
        assert_eq!(frame.x, 8.);
        assert_eq!(frame.opacity, 1.0);
    }

    #[test]
    fn a_move_soon_after_another_snaps_instead_of_queueing_glides() {
        let start = Instant::now();
        let mut caret = State::default();
        caret.frame(start, reading(0., 0., 1));
        let first = start + Duration::from_millis(1000);
        caret.frame(first, reading(10., 0., 2));
        // 100ms later, like a held arrow key: no glide.
        let second = first + Duration::from_millis(100);
        let frame = caret.frame(second, reading(20., 0., 3)).unwrap();
        assert_eq!(frame.x, 20.);
        // And 200ms after that is isolated again, so it glides.
        let third = second + Duration::from_millis(200);
        let frame = caret.frame(third, reading(40., 0., 4)).unwrap();
        assert_eq!(frame.x, 20.);
    }

    #[test]
    fn a_far_jump_or_a_scroll_snaps() {
        let start = Instant::now();
        let mut caret = State::default();
        caret.frame(start, reading(0., 0., 1));
        let frame = caret.frame(start, reading(0., 500., 2)).unwrap();
        assert_eq!(frame.y, 500.);
    }

    #[test]
    fn a_scroll_moves_the_caret_with_the_words_instead_of_gliding() {
        let start = Instant::now();
        let mut caret = State::default();
        caret.frame(start, reading(0., 0., 1));
        // The same caret place in the text, a short distance down the window
        // because the field scrolled by 10: no glide, it lands at once.
        let scrolled = Field::Caret(Reading {
            clip: None,
            x: 0.,
            y: 10.,
            height: 16.,
            line_height: 20.,
            scroll: (0., 10.),
            len: 0,
            stamp: 1,
        });
        let frame = caret.frame(start, scrolled).unwrap();
        assert_eq!(frame.y, 10.);
    }

    #[test]
    fn typing_holds_it_solid_and_stillness_lets_it_breathe() {
        let start = Instant::now();
        let mut caret = State::default();
        caret.frame(start, reading(0., 0., 1));
        let quiet = start + Duration::from_millis(1000);
        let dim = caret.frame(quiet, reading(0., 0., 1)).unwrap();
        assert!(dim.opacity < 1.0);
        let typed = caret.frame(quiet, reading(8., 0., 2)).unwrap();
        assert_eq!(typed.opacity, 1.0);
    }

    #[test]
    fn nothing_is_drawn_or_asked_for_when_not_focused() {
        let mut caret = State::default();
        assert!(caret.frame(Instant::now(), Field::Idle).is_none());
        assert!(caret.frame(Instant::now(), Field::Selecting).is_none());
    }
}
