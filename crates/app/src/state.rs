//! What the deck remembers between runs.
//!
//! The window's shape, and which way the reader last turned the page. A deck is
//! opened by an agent rather than by the reader, so the reader never gets to
//! place the window or arrange it before it appears — the only way it can
//! arrive the way they want it is to arrive the way they last left it.
//!
//! Both are written into one file, and each is written without disturbing the
//! other: the shape is known when the window closes and the turn is known the
//! moment it is asked for, so the two never arrive together.
//!
//! Kept beside the config the app will read later, under `~/.deck`, so there is
//! one place to look for anything deck has written about you.

use std::path::PathBuf;

use gpui_kit::{Bounds, Pixels, Point, Size, point, px, size};
use serde::{Deserialize, Serialize};

/// What was last left behind.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Placement {
    /// The window's shape, in logical pixels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    x: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    y: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    width: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    height: Option<f32>,
    /// Which quarter turn the page was on. See `view::quarter`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    turn: Option<u8>,
}

impl Placement {
    /// The shape, if all four numbers were written.
    fn bounds(self) -> Option<Bounds<Pixels>> {
        Some(Bounds {
            origin: Point {
                x: px(self.x?),
                y: px(self.y?),
            },
            size: Size {
                width: px(self.width?),
                height: px(self.height?),
            },
        })
    }
}

/// Where the remembered shape lives.
///
/// Asked rather than spelled out. `HOME` is not set on Windows unless somebody
/// has been living in a Unix shell, and reading it directly meant a deck there
/// forgot its size and its turn every time it closed.
fn path() -> Option<PathBuf> {
    Some(deck_core::home::deck()?.join("window.json"))
}

/// The shape the window had when it was last closed.
///
/// Anything unreadable is treated as nothing remembered: a corrupt file should
/// cost the reader their window size, not their deck.
#[must_use]
pub fn remembered() -> Option<Bounds<Pixels>> {
    read_from(&path()?).bounds()
}

/// The quarter turn the page was last left on.
#[must_use]
pub fn remembered_turn() -> Option<u8> {
    path().and_then(|path| read_from(&path).turn)
}

/// Everything in the file, or nothing if it will not read.
fn read_from(path: &std::path::Path) -> Placement {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// Remember the window's shape.
pub fn remember_bounds(bounds: Bounds<Pixels>) {
    if let Some(path) = path() {
        write_to(&path, Some(bounds), None);
    }
}

/// Remember which way the page was turned.
pub fn remember_turn(turn: u8) {
    if let Some(path) = path() {
        write_to(&path, None, Some(turn));
    }
}

/// Write what is given and leave the rest as it was.
///
/// Read, change, write. The two things remembered here are learned at different
/// moments — the turn when it is asked for, the shape when the window closes —
/// so a writer that put down everything it knew would put down a default over
/// something the other one had just saved.
///
/// Failure is silent on purpose. This runs while the window is closing, and a
/// deck that refused to shut because it could not write a preference would be a
/// worse thing than one that forgets where it was.
fn write_to(path: &std::path::Path, bounds: Option<Bounds<Pixels>>, turn: Option<u8>) {
    let mut placement = read_from(path);

    if let Some(bounds) = bounds {
        placement.x = Some(f32::from(bounds.origin.x));
        placement.y = Some(f32::from(bounds.origin.y));
        placement.width = Some(f32::from(bounds.size.width));
        placement.height = Some(f32::from(bounds.size.height));
    }
    if turn.is_some() {
        placement.turn = turn;
    }

    let Ok(json) = serde_json::to_string_pretty(&placement) else {
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(path, json);
}

/// Where the deck window goes: where it was last left, made to fit the display
/// it opens on, or where a first deck opens when that place cannot be reached.
///
/// Checked against the main display alone, because that is where the window
/// opens. On macOS every display reports itself at the origin and a window's
/// x is saved relative to the display it was on, so a deck left at x 1800
/// on an external monitor came back at x 1800 on a 1512-wide laptop screen,
/// off it entirely. Open took the bar away and nothing seemed to open (#117).
///
/// A place that can be reached is kept, but the window is shrunk to the display
/// and moved onto it, so all of it shows: a size left on a tall monitor would
/// otherwise hang off the bottom of a laptop's. With no main display, as on
/// Wayland, the compositor places the window and what was remembered is passed
/// through as it is.
#[must_use]
pub fn placed(
    remembered: Option<Bounds<Pixels>>,
    primary: Option<Bounds<Pixels>>,
) -> Bounds<Pixels> {
    let first = Bounds {
        origin: point(px(120.), px(80.)),
        size: size(px(1180.), px(860.)),
    };
    let Some(screen) = primary else {
        return remembered.unwrap_or(first);
    };
    if let Some(remembered) = remembered
        && reachable(remembered, screen)
    {
        return fitted(remembered, screen);
    }
    let mut wanted = remembered.map_or(first.size, |remembered| remembered.size);
    wanted.width = wanted.width.min(screen.size.width * 0.92);
    wanted.height = wanted.height.min(screen.size.height * 0.86);
    Bounds {
        origin: point(
            screen.origin.x + (screen.size.width - wanted.width) / 2.,
            screen.origin.y + px(80.),
        ),
        size: wanted,
    }
}

/// Whether enough of a window's top strip, the part it is moved by, shows on
/// the display to see it and drag it back.
#[must_use]
fn reachable(window: Bounds<Pixels>, screen: Bounds<Pixels>) -> bool {
    let strip = Bounds {
        origin: window.origin,
        size: size(window.size.width, px(48.)),
    };
    let seen = screen.intersect(&strip);
    seen.size.width >= px(200.).min(window.size.width) && seen.size.height >= px(24.)
}

/// The window, no larger than the display and moved the least it takes to lie
/// wholly on it.
fn fitted(window: Bounds<Pixels>, screen: Bounds<Pixels>) -> Bounds<Pixels> {
    let width = window.size.width.min(screen.size.width);
    let height = window.size.height.min(screen.size.height);
    // Bounded by hand rather than with `clamp`, which panics when its floor is
    // above its ceiling, and float rounding can put it there by a hair.
    let furthest_x = (screen.origin.x + screen.size.width - width).max(screen.origin.x);
    let furthest_y = (screen.origin.y + screen.size.height - height).max(screen.origin.y);
    Bounds {
        origin: point(
            window.origin.x.max(screen.origin.x).min(furthest_x),
            window.origin.y.max(screen.origin.y).min(furthest_y),
        ),
        size: size(width, height),
    }
}

#[cfg(test)]
mod tests {
    // Spelled out rather than `#[test]`: this module glob-imports GPUI, which
    // exports a `test` attribute of its own and would otherwise shadow the
    // standard one.
    use core::prelude::v1::test;

    use super::*;

    fn scratch(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("deck-state-{name}.json"))
    }

    fn bounds(x: f32, y: f32, w: f32, h: f32) -> Bounds<Pixels> {
        Bounds {
            origin: Point { x: px(x), y: px(y) },
            size: Size {
                width: px(w),
                height: px(h),
            },
        }
    }

    #[test]
    fn a_shape_survives_being_written_and_read_back() {
        let path = scratch("round-trip");
        let _ = std::fs::remove_file(&path);

        write_to(&path, Some(bounds(120., 80., 50., 50.)), None);
        let back = read_from(&path)
            .bounds()
            .expect("a shape was written, so one reads back");

        assert_eq!(f32::from(back.origin.x), 120.);
        assert_eq!(f32::from(back.origin.y), 80.);
        assert_eq!(f32::from(back.size.width), 50.);
        assert_eq!(f32::from(back.size.height), 50.);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn nothing_remembered_is_not_an_error() {
        let nothing = read_from(&scratch("never-written"));
        assert!(nothing.bounds().is_none());
        assert!(nothing.turn.is_none());
    }

    #[test]
    fn a_corrupt_file_costs_the_window_size_and_nothing_else() {
        // Whatever is on disk, opening the deck has to work — so a file that
        // will not parse reads as "nothing remembered" rather than as a
        // failure to be reported.
        let path = scratch("corrupt");
        std::fs::write(&path, "{ this is not json").expect("the scratch file is writable");

        assert!(read_from(&path).bounds().is_none());

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn writing_one_thing_leaves_the_other_alone() {
        // The shape is learned when the window closes and the turn the moment
        // it is asked for, so they are never written together. A writer that
        // put down everything it knew would put a default over whatever the
        // other one had just saved.
        let path = scratch("both");
        let _ = std::fs::remove_file(&path);

        write_to(&path, Some(bounds(10., 20., 30., 40.)), None);
        write_to(&path, None, Some(2));

        let back = read_from(&path);
        assert_eq!(back.turn, Some(2));
        assert_eq!(f32::from(back.bounds().expect("still there").origin.x), 10.);

        write_to(&path, Some(bounds(11., 21., 31., 41.)), None);
        assert_eq!(read_from(&path).turn, Some(2), "the turn survived a resize");

        let _ = std::fs::remove_file(&path);
    }

    // A laptop's main display, in its own coordinates, which is how gpui
    // reports every display on macOS.
    fn laptop() -> Bounds<Pixels> {
        bounds(0., 0., 1512., 982.)
    }

    fn inside(window: Bounds<Pixels>, screen: Bounds<Pixels>) -> bool {
        window.origin.x >= screen.origin.x
            && window.origin.y >= screen.origin.y
            && window.origin.x + window.size.width <= screen.origin.x + screen.size.width
            && window.origin.y + window.size.height <= screen.origin.y + screen.size.height
    }

    #[test]
    fn a_deck_left_on_the_screen_comes_back_where_it_was() {
        let left = bounds(73., 60., 1180., 860.);
        assert_eq!(placed(Some(left), Some(laptop())), left);
    }

    #[test]
    fn a_deck_left_on_an_external_monitor_opens_on_the_laptop_screen() {
        // Saved relative to the external display, reopened on the main one:
        // x 1800 on a 1512-wide screen is off it entirely (#117).
        let got = placed(Some(bounds(1800., 300., 1190., 860.)), Some(laptop()));
        assert!(inside(got, laptop()), "{got:?}");
    }

    #[test]
    fn a_deck_left_with_a_sliver_showing_opens_where_it_can_be_seen() {
        // Reproduced: the deck arrived at x 1256 on a 1296-wide screen, forty
        // pixels of it showing.
        let portrait = bounds(0., 0., 1296., 2304.);
        let got = placed(Some(bounds(1256., 300., 1190., 1448.)), Some(portrait));
        assert!(inside(got, portrait), "{got:?}");
        assert_eq!(
            got.size,
            size(px(1190.), px(1448.)),
            "the size still fits, so it is kept"
        );
    }

    #[test]
    fn a_deck_sized_on_a_tall_monitor_is_made_to_fit_a_shorter_one() {
        // Its top is on the screen, so the place is kept, but 1448 tall on a
        // 982-tall screen would leave the bottom edge out of reach.
        let got = placed(Some(bounds(100., 100., 1190., 1448.)), Some(laptop()));
        assert!(inside(got, laptop()), "{got:?}");
        assert_eq!(got.origin.x, px(100.), "moved no more than it had to");
    }

    #[test]
    fn a_window_whose_top_is_above_the_screen_cannot_be_taken_hold_of() {
        assert!(!reachable(bounds(100., -400., 1000., 800.), laptop()));
    }

    #[test]
    fn a_first_deck_opens_on_the_main_display() {
        let got = placed(None, Some(laptop()));
        assert!(inside(got, laptop()), "{got:?}");
    }

    #[test]
    fn without_a_main_display_the_remembered_shape_passes_through() {
        // Wayland: the compositor places windows, and there is no main display
        // to fit to.
        let left = bounds(3000., 300., 1190., 860.);
        assert_eq!(placed(Some(left), None), left);
    }

    #[test]
    fn a_deck_hanging_off_the_right_edge_is_moved_the_least_it_takes() {
        let got = placed(Some(bounds(1000., 100., 1180., 860.)), Some(laptop()));
        assert_eq!(got, bounds(332., 100., 1180., 860.));
    }

    #[test]
    fn a_deck_hanging_off_the_left_edge_is_moved_back_onto_it() {
        let got = placed(Some(bounds(-300., 100., 1180., 860.)), Some(laptop()));
        assert_eq!(got, bounds(0., 100., 1180., 860.));
    }

    #[test]
    fn fitting_never_panics_on_a_display_not_at_the_origin() {
        // `clamp` panicked here: rounding put its floor above its ceiling.
        let screen = bounds(0.1, 0.1, 1512., 982.);
        let got = fitted(bounds(0.1, 0.1, 1512., 982.), screen);
        assert!(got.size.width <= screen.size.width, "{got:?}");
    }
}
