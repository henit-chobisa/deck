//! How the narration is coloured for reading, worked out from the page.
//!
//! The window and the HTML export both set the narration, and they used to
//! each carry their own idea of it: the window stepped the prose's ink by
//! contrast and drew chips on a ground of their own, while the export kept a
//! fixed mix and the pane's colour, so a deck opened in a browser read like a
//! different deck. The rules live here once, and both ask.
//!
//! Every rule takes the inks it needs rather than a palette, because the two
//! callers hold different things: the window has the editor's palette, and an
//! export has the inks a document carries, already made legible on the band.

use deck_core::theme::Rgb;

const WHITE: Rgb = Rgb::new(255, 255, 255);
const BLACK: Rgb = Rgb::new(0, 0, 0);

/// Whether `ground` is dark for the purpose of reading on it: white reads on
/// it better than black does.
///
/// Measured by luminance, the way contrast is, and not by the lightness
/// [`Rgb::is_dark`] uses, which weighs the channels' encoded values rather
/// than the light they make and puts its line at half of that. The two part
/// in the middle greys, where `is_dark` calls a ground dark that black text
/// reads on better than white; the rules here choose a contrast range and a
/// face on the answer, so they ask the question contrast asks.
#[must_use]
pub fn dark(ground: Rgb) -> bool {
    WHITE.contrast(ground) > BLACK.contrast(ground)
}

/// The band's prose: `ink` stepped toward `ground` until its contrast falls
/// inside the range for this kind of theme.
///
/// A step off the page's ink, so the group's title is the brightest line in
/// the band and the prose reads as its body. The step is a contrast, not a
/// fixed mix: a fixed mix that read well on one theme left another's prose
/// grey and hard work.
#[must_use]
pub fn prose(ink: Rgb, ground: Rgb) -> Rgb {
    stepped(ink, ground, (9.5, 11.), (7., 9.))
}

/// The rail's record of the conversation: as [`prose`], a step quieter, so a
/// long answer beside the band reads as a note about it rather than as the
/// subject.
#[must_use]
pub fn rail(ink: Rgb, ground: Rgb) -> Rgb {
    stepped(ink, ground, (8.5, 9.5), (7., 8.))
}

/// `ink` stepped toward `ground` until its contrast falls inside the range
/// for this kind of theme, `dark` or `light`.
///
/// Full ink on a dark ground is around 13:1, and that much glares: the eye
/// sees the letters bloom. Much under 7:1 and a tired reader works for every
/// word. An ink already inside the range, or under it, is kept as it is, and
/// so is one that no step can bring inside it.
fn stepped(ink: Rgb, ground: Rgb, dark_range: (f64, f64), light_range: (f64, f64)) -> Rgb {
    let (low, high) = if dark(ground) {
        dark_range
    } else {
        light_range
    };
    if ink.contrast(ground) <= high {
        return ink;
    }
    // The contrast falls as the ink moves toward the ground: find the least
    // step that brings it to the top of the range.
    let (mut near, mut far) = (0f32, 1f32);
    for _ in 0..24 {
        let mid = f32::midpoint(near, far);
        if ink.mix(ground, mid).contrast(ground) > high {
            near = mid;
        } else {
            far = mid;
        }
    }
    let stepped = ink.mix(ground, far);
    if stepped.contrast(ground) < low {
        ink
    } else {
        stepped
    }
}

/// The bar a picked sentence is drawn on, at full light.
///
/// A quiet tint of the accent, not a block of it: the bar's job is to say
/// where, and the words on it must read better than the words around it,
/// not worse. A strong bar took the words' contrast away, and making the
/// words whiter to win it back only made them glare. Starting at 0.22 of the
/// accent, a little over a fifth, it steps back two hundredths at a time
/// until `ink` reads on it at 7:1, and never goes under a tenth, where it
/// would stop being seen at all.
///
/// Counted in whole hundredths rather than by taking 0.02 off a float: the
/// float's steps drift, the sixth of them lands a hair over 0.1, and the
/// seventh came back as 0.08, under the floor the bar promises.
#[must_use]
pub fn lit_bar(ink: Rgb, ground: Rgb, accent: Rgb) -> Rgb {
    for hundredths in (10_u8..=22).rev().step_by(2) {
        let bar = ground.mix(accent, f32::from(hundredths) / 100.);
        if ink.contrast(bar) >= 7. {
            return bar;
        }
    }
    ground.mix(accent, 0.1)
}

/// A code chip's ground: just enough off `ground` to see, about 1.15:1, so a
/// sentence with three names in it does not turn into a row of blocks.
///
/// Worked out from the page rather than taken from the theme's wash or the
/// pane's colour, which on some themes are the page's own colour and on
/// others a slab.
#[must_use]
pub fn chip(ground: Rgb) -> Rgb {
    let toward = if dark(ground) { WHITE } else { BLACK };
    let mut amount = 0.02;
    while amount < 0.3 && ground.mix(toward, amount).contrast(ground) < 1.15 {
        amount += 0.01;
    }
    ground.mix(toward, amount)
}

/// The padding either side of a chip's name. Its corners are rounded by as
/// much, so it reads as a name set in the sentence and not as a button.
pub const CHIP_PADDING: f32 = 3.;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn darkness_is_read_by_luminance() {
        assert!(dark(Rgb::new(0x28, 0x28, 0x28)));
        assert!(!dark(Rgb::new(0xfb, 0xf1, 0xc7)));
        // A middle grey: `is_dark` calls it dark, and black reads on it
        // better than white does.
        let grey = Rgb::new(0x78, 0x78, 0x78);
        assert!(grey.is_dark());
        assert!(Rgb::new(0, 0, 0).contrast(grey) > Rgb::new(255, 255, 255).contrast(grey));
        assert!(!dark(grey));
    }

    #[test]
    fn the_prose_lands_inside_its_range() {
        let (ink, ground) = (Rgb::new(0xeb, 0xdb, 0xb2), Rgb::new(0x24, 0x24, 0x24));
        let contrast = prose(ink, ground).contrast(ground);
        assert!((9.5..=11.).contains(&contrast), "{contrast}");
        // An ink already under the range is left alone.
        let quiet = Rgb::new(0x90, 0x90, 0x90);
        assert_eq!(prose(quiet, ground), quiet);
    }

    #[test]
    fn the_lit_bar_never_goes_under_a_tenth() {
        let (ground, accent) = (Rgb::new(0x24, 0x24, 0x24), Rgb::new(0xfe, 0x80, 0x19));
        // Ink that reads at 7:1 on the strongest bar takes it.
        let white = Rgb::new(0xff, 0xff, 0xff);
        assert_eq!(lit_bar(white, ground, accent), ground.mix(accent, 0.22));
        // Ink that never does stops at the floor, not a step past it.
        let grey = Rgb::new(0x60, 0x60, 0x60);
        assert_eq!(lit_bar(grey, ground, accent), ground.mix(accent, 0.1));
    }

    #[test]
    fn a_chip_is_just_off_its_ground() {
        for ground in [Rgb::new(0x24, 0x24, 0x24), Rgb::new(0xfb, 0xf1, 0xc7)] {
            let contrast = chip(ground).contrast(ground);
            assert!((1.15..1.3).contains(&contrast), "{contrast}");
        }
    }
}
