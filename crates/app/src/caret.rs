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
//! - **It glides.** When the place the caret belongs moves, the drawn caret
//!   travels there over [`GLIDE`] with a smoothstep, starting from wherever it
//!   is *now*. A second move that arrives mid-glide carries on from the
//!   current drawn spot; it never restarts from the old resting place, which
//!   would read as a stutter.
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
//! field so that it paints after, and inside the clip of, the field.
//!
//! The window only redraws while the caret is on screen and breathing, which
//! is to say while a field is focused. An unfocused field asks for no frames.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

use gpui_kit::{Bounds, Hsla, IntoElement, Styled, canvas, fill, point, px, size};

/// How long the caret takes to travel to a new place.
const GLIDE: f32 = 0.100;
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
    /// Changes whenever the text or the caret position changes.
    pub stamp: u64,
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
                stamp: (state.cursor() as u64) << 32 ^ state.text().len() as u64,
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
            self.scroll = reading.scroll;
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
            let from = if scrolled || far { target } else { here };
            self.from = from;
            self.to = target;
            self.started = Some(now);
        }
        self.scroll = reading.scroll;

        let t = self.started.map_or(1., |at| secs(at) / GLIDE);
        let here = glide(self.from, self.to, t);
        self.drawn = Some(here);
        Some(Frame {
            x: here.0,
            y: here.1,
            height: reading.height,
            opacity: breathe(self.changed.map_or(0., secs)),
        })
    }
}

/// A caret's state, shared between the view that owns the field and the
/// paint that draws over it.
#[derive(Debug, Clone, Default)]
pub struct Caret(Rc<RefCell<State>>);

/// An overlay to put over a field, as a sibling after it, inside a parent that
/// is `relative()`. It paints the caret and asks for the next frame only while
/// a caret is showing.
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
            let frame = caret.0.borrow_mut().frame(Instant::now(), field);
            if let Some(frame) = frame {
                let width = if cfg!(target_os = "macos") {
                    WIDTH
                } else {
                    1.0
                };
                window.paint_quad(fill(
                    Bounds::new(
                        point(px(frame.x), px(frame.y)),
                        size(px(width), px(frame.height)),
                    ),
                    colour.opacity(frame.opacity),
                ));
                window.request_animation_frame();
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
            x,
            y,
            height: 16.,
            line_height: 20.,
            scroll: (0., 0.),
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
    fn a_new_move_mid_glide_carries_on_from_where_it_is() {
        let start = Instant::now();
        let mut caret = State::default();
        caret.frame(start, reading(0., 0., 1));
        // Begin a glide to x=100, and look halfway through it.
        let begun = start + Duration::from_millis(10);
        caret.frame(begun, reading(100., 0., 2));
        let mid = begun + Duration::from_millis(50);
        let halfway = caret.frame(mid, reading(100., 0., 2)).unwrap();
        assert!((halfway.x - 50.).abs() < 1.0, "{}", halfway.x);
        // A fresh target arrives: the first frame of it starts from halfway,
        // not from 0 and not from 100.
        let again = caret.frame(mid, reading(200., 0., 3)).unwrap();
        assert!((again.x - halfway.x).abs() < 1e-3, "{}", again.x);
        let later = mid + Duration::from_millis(50);
        let next = caret.frame(later, reading(200., 0., 3)).unwrap();
        assert!(next.x > halfway.x && next.x < 200.);
        let done = caret.frame(later + Duration::from_millis(100), reading(200., 0., 3));
        assert_eq!(done.unwrap().x, 200.);
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
