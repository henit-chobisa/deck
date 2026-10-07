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

/// Where the deck window goes: where it was last left, if that is still on a
/// screen, and otherwise where a deck opens the first time.
///
/// The remembered place was trusted as it was. A deck moved onto a second
/// display came back there after the display was unplugged or rearranged —
/// off every screen, or with a sliver showing at an edge — so Open took the
/// bar away and, as far as the reader could tell, nothing opened (Nikhil).
/// The size is kept when it still fits; only the place is given up.
#[must_use]
pub fn placed(
    remembered: Option<Bounds<Pixels>>,
    screens: &[Bounds<Pixels>],
    primary: Option<Bounds<Pixels>>,
) -> Bounds<Pixels> {
    if let Some(remembered) = remembered
        && reachable(remembered, screens)
    {
        return remembered;
    }
    let mut wanted = remembered.map_or(size(px(1180.), px(860.)), |remembered| remembered.size);
    let mut origin = point(px(120.), px(80.));
    if let Some(screen) = primary {
        wanted.width = wanted.width.min(screen.size.width * 0.92);
        wanted.height = wanted.height.min(screen.size.height * 0.86);
        origin.x = screen.origin.x + (screen.size.width - wanted.width) / 2.;
        origin.y = screen.origin.y + px(80.);
    }
    Bounds {
        origin,
        size: wanted,
    }
}

/// Whether a window here could be taken hold of: enough of its top strip, the
/// part it is moved by, on one screen to see it and drag it back.
#[must_use]
pub fn reachable(window: Bounds<Pixels>, screens: &[Bounds<Pixels>]) -> bool {
    let strip = Bounds {
        origin: window.origin,
        size: size(window.size.width, px(48.)),
    };
    let enough = px(200.).min(window.size.width);
    screens.iter().any(|screen| {
        let seen = screen.intersect(&strip);
        seen.size.width >= enough && seen.size.height >= px(24.)
    })
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

    fn rect(x: f32, y: f32, w: f32, h: f32) -> Bounds<Pixels> {
        Bounds {
            origin: point(px(x), px(y)),
            size: size(px(w), px(h)),
        }
    }

    // A tall main display, and a second one to its left — the arrangement the
    // off-screen deck was reproduced on.
    fn screens() -> Vec<Bounds<Pixels>> {
        vec![rect(0., 0., 1296., 2304.), rect(-900., 481., 900., 1600.)]
    }

    #[test]
    fn a_deck_left_on_a_screen_comes_back_where_it_was() {
        let left = rect(73., 590., 1190., 1448.);
        assert_eq!(placed(Some(left), &screens(), Some(screens()[0])), left);
        // On the second display is still on a screen.
        let side = rect(-850., 600., 800., 900.);
        assert_eq!(placed(Some(side), &screens(), Some(screens()[0])), side);
    }

    #[test]
    fn a_deck_left_with_a_sliver_showing_opens_where_it_can_be_seen() {
        // Reproduced: Open took the bar away and the deck arrived at x 1256 on
        // a 1296-wide screen, forty pixels of it showing.
        let sliver = rect(1256., 300., 1190., 1448.);
        let got = placed(Some(sliver), &screens(), Some(screens()[0]));
        assert!(reachable(got, &screens()), "{got:?}");
        assert_eq!(got.size, sliver.size, "the size it was left at still fits");
    }

    #[test]
    fn a_deck_left_on_a_display_that_is_gone_comes_back_to_the_main_one() {
        let unplugged = rect(3000., 300., 1190., 860.);
        let got = placed(Some(unplugged), &[screens()[0]], Some(screens()[0]));
        assert!(reachable(got, &[screens()[0]]), "{got:?}");
    }

    #[test]
    fn a_deck_too_big_for_the_screen_it_returns_to_is_made_to_fit() {
        let huge = rect(5000., 0., 4000., 3000.);
        let got = placed(Some(huge), &screens(), Some(screens()[0]));
        assert!(
            got.size.width <= px(1296.) && got.size.height <= px(2304.),
            "{got:?}"
        );
    }

    #[test]
    fn a_window_whose_top_is_above_every_screen_cannot_be_taken_hold_of() {
        // Its body may show, but the strip it is dragged by does not.
        assert!(!reachable(rect(100., -400., 1000., 800.), &screens()));
    }
}
