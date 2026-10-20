//! Where a line of deck's words breaks, without a window.
//!
//! A diagram's labels are wrapped to the width of their box, and today the
//! platform's text system decides where: CoreText on a Mac, DirectWrite on
//! Windows, cosmic-text on Linux. None of them is available to an SVG or a
//! PDF, so an export would break the same label somewhere else, and the
//! picture in a document would differ from the one the reader saw.
//!
//! So deck breaks its own lines, the way the window does. A label is plain
//! text, and GPUI lays plain text out by shaping it — kerning included — and
//! breaking it in `compute_wrap_boundaries`, which walks the shaped glyphs and
//! their positions. This shapes the same face with rustybuzz and runs that same
//! walk. A note is cut short by `truncate()`, which keeps the text whole when
//! the shaped text fits and otherwise uses `LineWrapper::truncate_line`, which
//! sums single characters without kerning; [`truncate`] does both.

use std::sync::OnceLock;

/// The narration face, embedded rather than looked for: the regular, medium
/// and semibold cuts of IBM Plex Sans, under the SIL Open Font License
/// (`fonts/LICENSE-IBM-Plex.txt`). The window loads these into its text system;
/// [`wrap`] and [`truncate`] measure with them.
pub const PLEX: &[(&[u8], &str)] = &[
    (include_bytes!("../fonts/IBMPlexSans-400.ttf"), "regular"),
    (include_bytes!("../fonts/IBMPlexSans-500.ttf"), "medium"),
    (include_bytes!("../fonts/IBMPlexSans-600.ttf"), "semibold"),
];

/// Which cut of the face.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Weight {
    /// 400.
    Regular,
    /// 500.
    Medium,
    /// 600, what an accented node's label is set in.
    Semibold,
}

/// The advance of every character this face has, in em, for one weight.
fn face(weight: Weight) -> &'static Face {
    static FACES: OnceLock<[Face; 3]> = OnceLock::new();
    let faces = FACES.get_or_init(|| [Face::load(0), Face::load(1), Face::load(2)]);
    match weight {
        Weight::Regular => &faces[0],
        Weight::Medium => &faces[1],
        Weight::Semibold => &faces[2],
    }
}

/// One cut of Plex, ready to shape and to measure.
struct Face {
    units_per_em: f32,
    /// The parsed font. Owned by the static it lives in.
    font: rustybuzz::Face<'static>,
}

impl Face {
    fn load(ix: usize) -> Self {
        let font = rustybuzz::Face::from_slice(PLEX[ix].0, 0).expect("the embedded Plex parses");
        Self {
            #[allow(clippy::cast_precision_loss)]
            units_per_em: font.units_per_em() as f32,
            font,
        }
    }

    /// How far `c` moves the pen on its own, at `size`: what
    /// `LineWrapper` measures, with no kerning against a neighbour.
    ///
    /// A character Plex has no glyph for falls back to another face in the
    /// window, which this cannot see; it is measured as Plex's missing glyph.
    fn advance(&self, c: char, size: f32) -> f32 {
        let glyph = self
            .font
            .glyph_index(c)
            .unwrap_or(rustybuzz::ttf_parser::GlyphId(0));
        let units = self.font.glyph_hor_advance(glyph).unwrap_or(0);
        f32::from(units) / self.units_per_em * shaped_size(size)
    }

    /// `text` shaped as the window shapes it, kerning and all: each glyph's
    /// byte offset into `text` and its pen position, and the line's width.
    fn shape(&self, text: &str, size: f32) -> (Vec<(usize, f32)>, f32) {
        let mut buffer = rustybuzz::UnicodeBuffer::new();
        buffer.push_str(text);
        buffer.guess_segment_properties();
        let shaped = rustybuzz::shape(&self.font, &[], buffer);
        // Positions are added up in f64 and each rounded to f32 once, as
        // CoreText lays a line out in doubles and GPUI keeps f32.
        let scale = f64::from(shaped_size(size)) / f64::from(self.units_per_em);
        let mut x = 0f64;
        let mut glyphs = Vec::with_capacity(shaped.len());
        for (info, at) in shaped.glyph_infos().iter().zip(shaped.glyph_positions()) {
            #[allow(clippy::cast_possible_truncation)]
            glyphs.push((
                info.cluster as usize,
                (x + f64::from(at.x_offset) * scale) as f32,
            ));
            x += f64::from(at.x_advance) * scale;
        }
        #[allow(clippy::cast_possible_truncation)]
        (glyphs, x as f32)
    }
}

/// Whether Plex has a glyph for every character of `text`.
///
/// Where it does not — CJK, most emoji — the window draws those characters in
/// a fallback face from the machine, which nothing here can measure, so a line
/// holding them may break somewhere else. A tab, and an accent written as a
/// separate combining mark, count as not covered too: the window measures
/// both with widths of its own that the font does not hold. An export embeds Plex and will need
/// a fallback face of its own for them.
#[must_use]
pub fn covers(text: &str) -> bool {
    let face = face(Weight::Regular);
    text.chars().all(|c| {
        c == ' ' || (!c.is_control() && !is_combining(c) && face.font.glyph_index(c).is_some())
    })
}

/// How wide `text` is, at `size` in `weight`, as a sum of single characters.
#[must_use]
pub fn width(text: &str, size: f32, weight: Weight) -> f32 {
    let face = face(weight);
    text.chars().map(|c| face.advance(c, size)).sum()
}

/// The size the window really shapes at: one float step above the size
/// asked for, which GPUI's Mac text system bumps the first run to. Every label
/// is one run, and per-character widths go through the same path, so both the
/// shaped line and the single characters are measured at this size.
fn shaped_size(size: f32) -> f32 {
    size.next_up()
}

/// How wide `text` is once shaped, kerning included: what the window draws.
#[must_use]
pub fn shaped_width(text: &str, size: f32, weight: Weight) -> f32 {
    face(weight).shape(text, size).1
}

/// Where `line` breaks to fit `wrap_width`: the byte offset each new line
/// starts at.
///
/// GPUI's `LineLayout::compute_wrap_boundaries`, step for step, over the line
/// shaped with kerning: a break is measured from the last one to the next
/// glyph's position, and the previous character is recorded even on a break.
#[must_use]
pub fn boundaries(line: &str, size: f32, weight: Weight, wrap_width: f32) -> Vec<usize> {
    let (glyphs, line_width) = face(weight).shape(line, size);
    let mut out = Vec::new();
    let mut first_non_whitespace = false;
    let mut last_candidate: Option<usize> = None;
    let mut last_candidate_x = 0.;
    let mut last_boundary = 0;
    let mut last_boundary_x = 0.;
    let mut prev_ch = '\0';

    for (glyph_ix, &(byte_ix, x)) in glyphs.iter().enumerate() {
        let Some(ch) = line[byte_ix..].chars().next() else {
            continue;
        };
        if ch == '\n' {
            continue;
        }
        if is_word_char(ch) {
            if prev_ch == ' ' && ch != ' ' && first_non_whitespace {
                last_candidate = Some(glyph_ix);
                last_candidate_x = x;
            }
        } else if ch != ' ' && first_non_whitespace {
            last_candidate = Some(glyph_ix);
            last_candidate_x = x;
        }
        if ch != ' ' {
            first_non_whitespace = true;
        }

        let next_x = glyphs.get(glyph_ix + 1).map_or(line_width, |&(_, x)| x);
        if next_x - last_boundary_x > wrap_width && glyph_ix > last_boundary {
            if let Some(candidate) = last_candidate.take() {
                last_boundary = candidate;
                last_boundary_x = last_candidate_x;
            } else {
                last_boundary = glyph_ix;
                last_boundary_x = x;
            }
            out.push(glyphs[last_boundary].0);
        }
        prev_ch = ch;
    }
    out
}

/// `text` broken into the lines the window would draw it in, at `wrap_width`.
///
/// At the window's own zoom of 100%: zoomed, the window scales a label's
/// width and size but not the padding inside its box, so its breaks move.
/// An export draws at 100%.
///
/// Each paragraph — the text between two newlines — is wrapped on its own, as
/// the window lays out each one. A line keeps its trailing space, because the
/// window centres each line with that space included; trim it only to draw a
/// line left-aligned.
#[must_use]
pub fn wrap(text: &str, size: f32, weight: Weight, wrap_width: f32) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        let mut from = 0;
        for at in boundaries(paragraph, size, weight, wrap_width) {
            lines.push(paragraph[from..at].to_string());
            from = at;
        }
        lines.push(paragraph[from..].to_string());
    }
    lines
}

/// `line` cut short with an ellipsis to fit `truncate_width`, as the window's
/// `truncate()` does, or the line itself when it fits.
///
/// As GPUI's text element does it: the line stays whole when, shaped, it
/// fits; otherwise `LineWrapper::truncate_line` from the end, step for step,
/// which sums single characters.
#[must_use]
pub fn truncate(line: &str, size: f32, weight: Weight, truncate_width: f32) -> String {
    const AFFIX: &str = "…";
    let face = face(weight);
    if face.shape(line, size).1 <= truncate_width {
        return line.to_string();
    }
    let suffix_width: f32 = AFFIX.chars().map(|c| face.advance(c, size)).sum();
    let mut width = 0.;
    let mut truncate_ix = 0;
    for (ix, c) in line.char_indices() {
        if width + suffix_width < truncate_width {
            truncate_ix = ix;
        }
        width += face.advance(c, size);
        if width.floor() > truncate_width {
            return format!(
                "{}{AFFIX}",
                line[..truncate_ix]
                    .trim_end_matches(|c: char| c.is_whitespace() || c.is_ascii_punctuation())
            );
        }
    }
    line.to_string()
}

/// A combining mark, written after the letter it sits on.
fn is_combining(c: char) -> bool {
    matches!(c, '\u{0300}'..='\u{036F}' | '\u{1AB0}'..='\u{1AFF}' | '\u{1DC0}'..='\u{1DFF}' | '\u{20D0}'..='\u{20FF}' | '\u{FE20}'..='\u{FE2F}')
}

/// A character that belongs to the word around it, and so is no place to
/// break: GPUI's `is_word_char`, exactly.
fn is_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric()
        || matches!(c, '\u{00C0}'..='\u{00FF}')
        || matches!(c, '\u{0100}'..='\u{017F}')
        || matches!(c, '\u{0180}'..='\u{024F}')
        || matches!(c, '\u{0400}'..='\u{04FF}')
        || matches!(c, '\u{1E00}'..='\u{1EFF}')
        || matches!(c, '\u{0300}'..='\u{036F}')
        || matches!(c, '\u{0980}'..='\u{09FF}')
        || matches!(
            c,
            '-' | '_'
                | '.'
                | '\''
                | '’'
                | '‘'
                | '$'
                | '%'
                | '@'
                | '#'
                | '^'
                | '~'
                | ','
                | '='
                | ':'
                | ';'
        )
        || matches!(c, '!' | ')' | ']' | '}' | '"' | '”' | '»' | '…')
        || matches!(c, '⋯')
        || matches!(c, '\u{202F}' | '\u{00A0}' | '\u{2011}')
}

#[doc(hidden)]
/// Labels of the length a diagram holds, for checking a wrapper against the
/// window's. Public only so the agreement test outside this crate can use it.
///
/// Labels of the length a diagram holds, and the edge cases a wrapper
/// gets wrong: a word longer than the box, punctuation that must not start
/// a line, a path, CJK with no spaces, a leading indent.
pub const CORPUS: &[&str] = &[
    "connect",
    "JiraController",
    "installed?",
    "workspace_credentials",
    "the request stops at the installed check",
    "POST /api/v1/workspaces/connect",
    "a pick has nowhere to land, so it fails",
    "upsert one option row per key",
    "Supercalifragilisticexpialidocious",
    "plz! (see) \"quoted\"",
    "retry, with backoff; then give up…",
    "café naïve résumé",
    "数据库连接池满了",
    "  indented label that wraps",
    "a-b var_name I'm 100% 3.1415 Self::new",
    "   three leading spaces then words",
    "two  spaces  between  words here",
    "Supercalifragilistic word after",
    "e\u{301}cole cafe\u{301} re\u{301}sume\u{301} words",
    "plz! wow!! see (this) now",
    "AVAWAY To Ty Yo Wave Tower",
    "a/b/c/d/e/f/g/h foo?bar=baz&q=1",
    "tab\tseparated\tlabel words",
    "office affirm fluff flight difficult",
    "ffi ffl fi fl ft tt Th",
    "0123456789 1/2 3x4 10:30 v2.0.1",
    "\u{201c}curly\u{201d} \u{2018}single\u{2019} it\u{2019}s won't",
    "-> => != <= >= :: && || --",
    "Total: $1,234.56 (42%) #7 @user",
    "fetchUserProfile() returns Option<T>",
    "WAVE AVATAR LTA PTY Tj Yj ff.",
    "naïve coöperate Æsir Œuvre ß",
    "x\u{2014}y \u{2013} a\u{2026}b \u{00a0}nbsp",
    "deploy/staging → prod",
    "1st 2nd 3rd 4th H2O CO2",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_exactly_as_wide_as_its_box_stays_on_one_line() {
        // The window breaks only when the line is wider than the room, not
        // when it is exactly as wide.
        for text in ["installed?", "JiraController", "upsert one option row"] {
            let w = shaped_width(text, 12., Weight::Regular);
            assert_eq!(
                wrap(text, 12., Weight::Regular, w),
                vec![text.to_string()],
                "{text}"
            );
            assert_eq!(truncate(text, 12., Weight::Regular, w), text, "{text}");
        }
    }

    #[test]
    fn a_line_that_fits_is_one_line_and_a_long_one_is_cut() {
        assert_eq!(wrap("connect", 12., Weight::Regular, 136.), vec!["connect"]);
        let lines = wrap(
            "the request stops at the installed check",
            12.,
            Weight::Regular,
            60.,
        );
        assert!(lines.len() > 1, "{lines:?}");
        assert_eq!(
            lines.concat(),
            "the request stops at the installed check",
            "nothing is lost or added"
        );
        let short = truncate(
            "apps/api/plane/app/views/external_connect/dropdown.py",
            10.5,
            Weight::Regular,
            80.,
        );
        assert!(short.ends_with('…') && short.len() < 40, "{short}");
        assert_eq!(
            truncate("apps/api", 10.5, Weight::Regular, 136.),
            "apps/api"
        );
    }
}
