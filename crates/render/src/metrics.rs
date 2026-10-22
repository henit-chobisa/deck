//! The window's measurements, in one place both it and every export read.
//!
//! An export is meant to be the window on paper: the band as tall as it
//! pads, the group's number in the column it hangs from, code at the size and
//! the pitch a pane sets it in. Written down twice, those numbers drift — the
//! window's band gains a pixel of air in one release, and every page printed
//! afterwards looks cramped beside it without anybody having touched the
//! export. So they are written once, here, and the window builds itself from
//! them too: `crates/app/src/view.rs` for the band, the stem and the rail,
//! `pane.rs` for a code row's pitch, `prose.rs` for how the narration is set,
//! `palette.rs` for the faces' sizes.
//!
//! Every value is in the window's own pixels, at its default zoom.

/// The size code is set in: the component theme's monospace size, which a
/// pane's rows are measured from.
pub const MONO_SIZE: f32 = 12.;

/// A code row's height, as a multiple of [`MONO_SIZE`].
pub const LINE_HEIGHT: f32 = 1.62;

/// The window's own text size: the component theme's, which anything without
/// a size of its own inherits.
pub const SANS_SIZE: f32 = 13.2;

/// How wide the group's number sits, and the line that runs down from it.
pub const STEM: f32 = 30.;

/// The gap between the group's number and the titles beside it, which the
/// prose under them keeps too, so the three share one left edge.
pub const STEM_GAP: f32 = 14.;

/// The shortest the band is on its own: room for a short narration to
/// breathe, and for the review column beside it to show a handful of rows.
pub const BAND_LEAST: f32 = 231.;

/// The tallest the band grows on its own before its prose scrolls.
pub const BAND_MOST: f32 = 300.;

/// The band's padding above the group's number and titles.
pub const BAND_TOP: f32 = 15.;

/// The band's padding at either side.
pub const BAND_SIDE: f32 = 18.;

/// The gap between the band's titles and the prose under them.
pub const BAND_GAP: f32 = 9.;

/// The band's padding under the prose.
pub const BAND_BOTTOM: f32 = 14.;

/// The group's number, up to nine.
pub const NUMBER_SIZE: f32 = 36.;

/// The group's number from ten on: a step smaller, so two digits stay in the
/// column the line hangs from.
pub const NUMBER_SIZE_TENS: f32 = 27.;

/// The group's title.
pub const TITLE_SIZE: f32 = 15.;

/// The deck's title, the quieter line over the group's.
pub const DECK_TITLE_SIZE: f32 = 12.;

/// The confidence's level, beside the group's title.
pub const CONFIDENCE_SIZE: f32 = 11.;

/// The narration in the band: its size, its line pitch, and the space
/// between its paragraphs.
pub const PROSE_SIZE: f32 = 13.;

/// See [`PROSE_SIZE`].
pub const PROSE_LEADING: f32 = 20.;

/// See [`PROSE_SIZE`].
pub const PROSE_SPACING: f32 = 10.;

/// A code chip in the narration, in the code face: see [`chip_size`].
pub const CHIP_SIZE: f32 = chip_size(PROSE_SIZE);

/// The size of a code chip in prose set at `prose`: 0.88 of it, to the
/// nearest tenth of a point, because JetBrains Mono's x-height is taller
/// than Plex's and at the same size its words read bigger than the sentence
/// around them.
///
/// One function for the band, the rail and the export, so a chip cannot be
/// one size in the window and another on the page.
#[must_use]
pub const fn chip_size(prose: f32) -> f32 {
    // Rounded by hand: `f32::round` is not a `const fn`. Every size here is
    // positive, so adding a half and truncating is rounding.
    #[allow(clippy::cast_possible_truncation)]
    let tenths = (prose * 8.8 + 0.5) as i32;
    #[allow(clippy::cast_precision_loss)]
    let size = tenths as f32 / 10.;
    size
}

/// The rail beside the panes and the band, before anyone drags it.
pub const RAIL: f32 = 365.;

/// What the rail's own record of the conversation is set in, and its pitch.
pub const RAIL_SIZE: f32 = 12.5;

/// See [`RAIL_SIZE`].
pub const RAIL_LEADING: f32 = 19.;

/// The strip along the bottom: its text size, its padding across and down,
/// and the gap between what it says.
pub const STRIP_SIZE: f32 = 10.5;

/// See [`STRIP_SIZE`].
pub const STRIP_SIDE: f32 = 14.;

/// See [`STRIP_SIZE`].
pub const STRIP_ROW: f32 = 7.;

/// See [`STRIP_SIZE`].
pub const STRIP_GAP: f32 = 14.;

#[cfg(test)]
mod tests {
    use super::*;

    /// The numbers the window was measured at when these were moved here.
    ///
    /// Not a test of anything that could compute differently — it is the
    /// record. A change to one of these moves the window and every export
    /// with it, which is the point, but it should be a change somebody made
    /// on purpose and then looked at in both.
    #[test]
    fn the_window_is_measured_as_it_was() {
        let measured = [
            (MONO_SIZE, 12.),
            (LINE_HEIGHT, 1.62),
            (SANS_SIZE, 13.2),
            (STEM, 30.),
            (STEM_GAP, 14.),
            (BAND_LEAST, 231.),
            (BAND_MOST, 300.),
            (BAND_TOP, 15.),
            (BAND_SIDE, 18.),
            (BAND_GAP, 9.),
            (BAND_BOTTOM, 14.),
            (NUMBER_SIZE, 36.),
            (NUMBER_SIZE_TENS, 27.),
            (TITLE_SIZE, 15.),
            (DECK_TITLE_SIZE, 12.),
            (CONFIDENCE_SIZE, 11.),
            (PROSE_SIZE, 13.),
            (PROSE_LEADING, 20.),
            (PROSE_SPACING, 10.),
            (CHIP_SIZE, 11.4),
            (RAIL, 365.),
            (RAIL_SIZE, 12.5),
            (RAIL_LEADING, 19.),
            (STRIP_SIZE, 10.5),
            (STRIP_SIDE, 14.),
            (STRIP_ROW, 7.),
            (STRIP_GAP, 14.),
        ];
        for (now, was) in measured {
            assert!((now - was).abs() < f32::EPSILON, "{now} is not {was}");
        }
    }

    /// A code row lands on a whole pixel at the window's size, as the pane
    /// rounds it.
    #[test]
    fn a_code_row_is_a_whole_number_of_pixels() {
        assert!(((MONO_SIZE * LINE_HEIGHT).round() - 19.).abs() < f32::EPSILON);
    }
}
