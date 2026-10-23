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
//! - **It follows, continuously, and never falls behind the text.** Each
//!   frame the drawn caret closes a fixed fraction of the gap to where the
//!   library says the caret belongs, `1 - exp(-dt / TAU)` of it, the way
//!   smooth-caret terminals do. There is no glide that starts and ends, no
//!   rule about bursts and no special case for typing: a caret that is far
//!   from its target moves fast, one that is near moves slowly, and when the
//!   target moves again mid-flight the follow simply carries on towards the new
//!   one, so a held arrow key or a run of typed letters is a smooth stream
//!   rather than a queue of animations. With [`TAU`] at thirty milliseconds the
//!   caret is within a pixel of a letter about a tenth of a second after it
//!   lands, so it never reads as trailing the text. It lands exactly on the
//!   target once the gap is under a quarter of a pixel. It does snap, at once,
//!   in the four cases where following would be wrong: the first time it
//!   appears, when the field scrolled (the words moved, not the caret in them),
//!   when the move is more than [`SNAP_LINES`] lines down or up (a streak down
//!   the page, not a motion the eye follows), and when the window is in the
//!   background (nobody is watching, so there is nothing to animate).
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
//! neighbours the field. So the overlay clips its own paint to the field's
//! `input_bounds`, never to its own box, and the caret cannot escape however
//! the wrapper around the field is styled.
//!
//! Redraws are paid for only when they show something. While the caret is
//! still following its target, or is held solid after a change, the overlay asks for a frame every display
//! refresh, because the eye can see those. Once it is only breathing, a slow
//! cosine, it stops asking and a timer wakes the window about thirty times a
//! second instead: that is plenty for a curve that takes over a second, and it
//! stops a quiet, focused field from redrawing the whole window at display
//! rate for ever. An unfocused field asks for nothing at all, and neither does a
//! focused field in a window that is in the background: there the caret is
//! drawn solid and still, with no breathing and no timer, so a composer left
//! focused behind another app does not keep repainting deck's window.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gpui_kit::{
    Bounds, ContentMask, Hsla, IntoElement, Styled, Task, canvas, fill, point, px, size,
};

/// The follow's time constant, in seconds. Each frame the caret closes
/// `1 - exp(-dt / TAU)` of the gap to its target, so after `TAU` it has covered
/// about 63% of it and after 100ms about 96%. Small enough that the caret never
/// seems to trail typed text, large enough that a jump is seen as a glide.
const TAU: f32 = 0.030;
/// Longest step of time one frame may follow for, in seconds. After a stall (a
/// long frame, a window that was hidden) the caret catches up over a few frames
/// rather than leaping the whole way in one.
const MAX_DT: f32 = 0.050;
/// The frame time assumed for the first frame of a move that starts from rest.
/// The caret has not been asked for frames while resting, so the time since
/// the last one says nothing about how long a frame takes.
const FIRST_DT: f32 = 1. / 60.;
/// When the caret is this close to its target on both axes, in pixels, it is
/// put exactly on it, so the follow leaves no sub-pixel residue.
const SETTLE: f32 = 0.25;
/// How long the caret stays solid after the last change before it breathes.
const HOLD: f32 = 0.500;
/// One full breath, bright to dim to bright.
const CYCLE: f32 = 1.100;
/// How faint the caret gets at the bottom of a breath. Not zero: a caret that
/// vanishes completely is a caret you lose.
const DIM: f32 = 0.15;
/// Moves longer than this many lines snap rather than follow. Across that
/// distance a follow is a streak down the page, not a motion the eye follows.
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
    /// caret having moved in the text, and that is not worth following.
    pub scroll: (f32, f32),
    /// The caret's byte offset in the text. A target that moves while this
    /// stays the same means the letters moved under the caret, as when a Korean
    /// jamo becomes a wider syllable, which leaves the stamp unchanged.
    pub cursor: usize,
    /// How long the text is, in bytes. A drop of more than a character in one
    /// frame is the text being cleared or cut, not typed away.
    pub len: usize,
    /// Changes whenever the text length or the caret position changes.
    pub stamp: u64,
    /// Whether the window is the active one. In the background the caret is
    /// drawn solid, still, and asks for no frames.
    pub active: bool,
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
                cursor: state.cursor(),
                len: state.text().len(),
                active: $window.is_window_active(),
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

/// The follow and breathe state of one caret.
#[derive(Debug, Default)]
pub struct State {
    /// Where the caret is drawn, once it has been placed at all.
    drawn: Option<(f32, f32)>,
    /// Where the library said the caret belongs, last frame.
    to: (f32, f32),
    /// When the previous frame was drawn: what `dt` is measured from.
    last: Option<Instant>,
    /// When the caret last moved or the text last changed: what holds it solid.
    changed: Option<Instant>,
    cursor: usize,
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
    /// Whether this frame is part of something quick, the caret still
    /// following its target or the solid hold after a change, that needs a
    /// frame every refresh. When false the caret is only breathing and a slow
    /// timer is enough.
    pub quick: bool,
    /// Whether nothing is to be woken at all: the caret is solid and still
    /// because the window is in the background.
    pub still: bool,
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
            self.last = None;
            self.changed = None;
            return None;
        };
        let target = (reading.x, reading.y);
        let secs = |since: Instant| now.saturating_duration_since(since).as_secs_f32();

        let first = self.drawn.is_none();
        let here = self.drawn.unwrap_or(target);
        let scrolled = !first && reading.scroll != self.scroll;
        // A first sight or a new stamp is a change. So is a target that moved
        // while the stamp and caret offset did not: the letters shifted under
        // the caret (a jamo becoming a syllable, same length). Hold it solid
        // like any other change; the follow carries it across the shift.
        let shifted = target != self.to && reading.cursor == self.cursor && !scrolled;
        if first || reading.stamp != self.stamp || shifted {
            self.changed = Some(now);
        }

        // The text was cleared or cut: submit, select-all delete, a long cut.
        // The caret did not travel anywhere, the words it was in went away,
        // so it appears at its new place; sliding up from the last line of
        // what was written reads as a bug. One character (a backspace, even
        // of a multi-byte one) still follows. A text that is empty snaps too:
        // there is only one place the caret can be.
        let cleared = !first
            && (self.len.saturating_sub(reading.len) > 4 || (reading.len == 0 && self.len > 0));
        let far = (target.1 - here.1).abs() > reading.line_height * SNAP_LINES;
        let at_rest = here == self.to;
        let drawn = if first || scrolled || cleared || far || !reading.active {
            // Following would be wrong, or nobody is watching.
            target
        } else {
            // Starting from rest, the time since the last frame is time spent
            // resting, so take one ordinary frame instead of a long stride.
            let dt = if at_rest {
                FIRST_DT
            } else {
                self.last.map_or(FIRST_DT, secs).min(MAX_DT)
            };
            let k = 1. - (-dt / TAU).exp();
            let x = here.0 + (target.0 - here.0) * k;
            let y = here.1 + (target.1 - here.1) * k;
            if (target.0 - x).abs() < SETTLE && (target.1 - y).abs() < SETTLE {
                // Exactly on the target, with no residue from the arithmetic.
                target
            } else {
                (x, y)
            }
        };
        self.drawn = Some(drawn);
        self.to = target;
        self.last = Some(now);
        self.stamp = reading.stamp;
        self.cursor = reading.cursor;
        self.len = reading.len;
        self.scroll = reading.scroll;

        if !reading.active {
            return Some(Frame {
                x: drawn.0,
                y: drawn.1,
                height: reading.height,
                opacity: 1.0,
                quick: false,
                still: true,
            });
        }
        let quiet = self.changed.map_or(0., secs);
        Some(Frame {
            x: drawn.0,
            y: drawn.1,
            height: reading.height,
            opacity: breathe(quiet),
            quick: drawn != target || quiet < HOLD,
            still: false,
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

impl Caret {
    /// Forget where the caret was drawn, so the next frame counts as its first
    /// appearance and it lands on the target without following. For a field
    /// that has just been opened, refocused or emptied.
    pub fn reset(&self) {
        *self.state.borrow_mut() = State::default();
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
            if frame.still {
                // Background window: nothing to animate, so nothing to wake.
                caret.wake.borrow_mut().take();
            } else if frame.quick {
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
            cursor: usize::try_from(stamp).unwrap(),
            len: 0,
            stamp,
            active: true,
        })
    }

    fn at(start: Instant, ms: u64) -> Instant {
        start + Duration::from_millis(ms)
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
    fn first_appearance_snaps() {
        let mut caret = State::default();
        let frame = caret.frame(Instant::now(), reading(40., 12., 1)).unwrap();
        assert_eq!((frame.x, frame.y), (40., 12.));
    }

    #[test]
    fn a_single_move_follows_smoothly_and_lands_exactly() {
        let start = Instant::now();
        let mut caret = State::default();
        caret.frame(start, reading(0., 0., 1));
        let mut ms = 1000;
        let first = caret.frame(at(start, ms), reading(100., 0., 2)).unwrap();
        assert!(first.x > 0. && first.x < 100., "{}", first.x);
        let mut at_100 = None;
        let mut landed = None;
        while ms < 1000 + 400 {
            ms += 16;
            let frame = caret.frame(at(start, ms), reading(100., 0., 2)).unwrap();
            assert!(frame.x <= 100.);
            if ms - 1000 >= 96 && at_100.is_none() {
                at_100 = Some(frame.x);
            }
            if frame.x == 100. && landed.is_none() {
                landed = Some(ms - 1000);
            }
        }
        assert!(at_100.unwrap() >= 95., "{:?}", at_100);
        assert!(landed.unwrap() <= 200, "{:?}", landed);
    }

    #[test]
    fn a_steady_stream_of_moves_stays_close_behind() {
        let start = Instant::now();
        let mut caret = State::default();
        caret.frame(start, reading(0., 0., 1));
        let mut worst: f32 = 0.;
        let mut ms = 0;
        while ms < 1500 {
            ms += 16;
            let target = 8. * (ms / 60) as f32;
            let frame = caret
                .frame(at(start, ms), reading(target, 0., 2 + ms / 60))
                .unwrap();
            // Judged just before the next move lands, once the follow has had
            // its 60ms; right at a move the gap is by nature a whole step.
            if (ms + 16) / 60 != ms / 60 && ms > 300 {
                worst = worst.max(target - frame.x);
            }
            assert!(target - frame.x < 8. + 1.5, "{}", target - frame.x);
        }
        assert!(worst < 3., "{worst}");
    }

    #[test]
    fn a_far_jump_snaps() {
        let start = Instant::now();
        let mut caret = State::default();
        caret.frame(start, reading(0., 0., 1));
        let frame = caret.frame(at(start, 16), reading(0., 500., 2)).unwrap();
        assert_eq!(frame.y, 500.);
    }

    #[test]
    fn a_scroll_snaps_instead_of_following() {
        let start = Instant::now();
        let mut caret = State::default();
        caret.frame(start, reading(0., 0., 1));
        let scrolled = Field::Caret(Reading {
            clip: None,
            x: 0.,
            y: 10.,
            height: 16.,
            line_height: 20.,
            scroll: (0., 10.),
            cursor: 1,
            len: 0,
            stamp: 1,
            active: true,
        });
        let frame = caret.frame(at(start, 16), scrolled).unwrap();
        assert_eq!(frame.y, 10.);
    }

    #[test]
    fn a_background_window_gets_a_solid_snapped_caret_and_no_wakes() {
        let start = Instant::now();
        let mut caret = State::default();
        caret.frame(start, reading(0., 0., 1));
        let Field::Caret(mut r) = reading(50., 0., 2) else {
            unreachable!()
        };
        r.active = false;
        let frame = caret.frame(at(start, 2000), Field::Caret(r)).unwrap();
        assert_eq!(frame.x, 50.);
        assert_eq!(frame.opacity, 1.0);
        assert!(frame.still && !frame.quick);
    }

    #[test]
    fn letters_shifting_under_a_still_cursor_hold_it_solid() {
        let start = Instant::now();
        let mut caret = State::default();
        caret.frame(start, reading(0., 0., 1));
        let dim = caret.frame(at(start, 1000), reading(0., 0., 1)).unwrap();
        assert!(dim.opacity < 1.0);
        // Same stamp and cursor, but the target moved by a glyph width.
        let shifted = caret.frame(at(start, 1016), reading(3., 0., 1)).unwrap();
        assert_eq!(shifted.opacity, 1.0);
        assert!(shifted.x > 0. && shifted.x < 3.);
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
        assert!(typed.quick);
    }

    #[test]
    fn nothing_is_drawn_or_asked_for_when_not_focused() {
        let mut caret = State::default();
        assert!(caret.frame(Instant::now(), Field::Idle).is_none());
        assert!(caret.frame(Instant::now(), Field::Selecting).is_none());
    }

    /// Three lines of text, the caret at the end of the third.
    fn written(len: usize, x: f32, y: f32, stamp: u64) -> Field {
        let Field::Caret(mut r) = reading(x, y, stamp) else {
            unreachable!()
        };
        r.len = len;
        Field::Caret(r)
    }

    #[test]
    fn clearing_multi_line_text_snaps_to_the_start() {
        let start = Instant::now();
        let mut caret = State::default();
        caret.frame(start, written(30, 60., 40., 30));
        // Submitted: empty, the caret at the start of the first line.
        let frame = caret.frame(at(start, 16), written(0, 0., 0., 0)).unwrap();
        assert_eq!((frame.x, frame.y), (0., 0.));
    }

    #[test]
    fn a_single_backspace_still_follows() {
        let start = Instant::now();
        let mut caret = State::default();
        caret.frame(start, written(30, 60., 0., 30));
        let frame = caret
            .frame(at(start, 16), written(29, 52., 0., 29))
            .unwrap();
        assert!(frame.x > 52. && frame.x < 60., "{}", frame.x);
    }

    #[test]
    fn a_reset_makes_the_next_frame_a_first_appearance() {
        let start = Instant::now();
        let shared = Caret::default();
        shared
            .state
            .borrow_mut()
            .frame(start, written(5, 40., 0., 5));
        shared.reset();
        let frame = shared
            .state
            .borrow_mut()
            .frame(at(start, 16), written(5, 0., 0., 5))
            .unwrap();
        assert_eq!(frame.x, 0.);
    }
}
