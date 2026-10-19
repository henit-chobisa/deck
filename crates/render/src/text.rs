//! Where a line of deck's words breaks, without a window.
//!
//! A diagram's labels are wrapped to the width of their box, and today the
//! platform's text system decides where: CoreText on a Mac, DirectWrite on
//! Windows, cosmic-text on Linux. None of them is available to an SVG or a
//! PDF, so an export would break the same label somewhere else, and the
//! picture in a document would differ from the one the reader saw.
//!
//! So deck breaks its own lines. This is GPUI's wrapper, character for
//! character — the same candidates, the same greedy fill, the same truncation —
//! measuring each character by its advance in the IBM Plex face deck embeds,
//! read straight from the font. GPUI's wrapper measures one character at a
//! time and never kerns, so nothing here needs a shaper to agree with it, and
//! the answer is the same on every machine.

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

/// One cut of Plex, as advances.
struct Face {
    units_per_em: f32,
    /// The parsed font. Owned by the static it lives in.
    font: ttf_parser::Face<'static>,
}

impl Face {
    fn load(ix: usize) -> Self {
        let font = ttf_parser::Face::parse(PLEX[ix].0, 0).expect("the embedded Plex parses");
        Self {
            units_per_em: f32::from(font.units_per_em()),
            font,
        }
    }

    /// How far `c` moves the pen at `size`, as GPUI measures it: one character
    /// on its own, so no kerning against its neighbours.
    ///
    /// A character Plex has no glyph for falls back to another face in the
    /// window, which this cannot see; it is measured as Plex's missing glyph,
    /// which is the honest answer for the face deck ships.
    fn advance(&self, c: char, size: f32) -> f32 {
        let glyph = self.font.glyph_index(c).unwrap_or(ttf_parser::GlyphId(0));
        let units = self.font.glyph_hor_advance(glyph).unwrap_or(0);
        f32::from(units) / self.units_per_em * size
    }
}

/// Whether Plex has a glyph for every character of `text`.
///
/// Where it does not — CJK, most emoji — the window draws those characters in
/// a fallback face from the machine, which nothing here can measure, so a line
/// holding them may break somewhere else. An export embeds Plex and will need
/// a fallback face of its own for them.
#[must_use]
pub fn covers(text: &str) -> bool {
    let face = face(Weight::Regular);
    text.chars()
        .all(|c| c.is_whitespace() || face.font.glyph_index(c).is_some())
}

/// How wide `text` is, at `size` in `weight`, as a sum of single characters.
#[must_use]
pub fn width(text: &str, size: f32, weight: Weight) -> f32 {
    let face = face(weight);
    text.chars().map(|c| face.advance(c, size)).sum()
}

/// The most indent a wrapped line keeps, as GPUI's wrapper has it.
const MAX_INDENT: u32 = 256;

/// Where `line` breaks to fit `wrap_width`: the byte offset each new line
/// starts at, and how many columns of indent it keeps.
///
/// GPUI's `LineWrapper::wrap_line`, for plain text, step for step.
#[must_use]
pub fn boundaries(line: &str, size: f32, weight: Weight, wrap_width: f32) -> Vec<(usize, u32)> {
    let face = face(weight);
    let mut out = Vec::new();
    let mut width = 0.;
    let mut first_non_whitespace_ix = None;
    let mut indent: Option<u32> = None;
    let mut last_candidate_ix = 0;
    let mut last_candidate_width = 0.;
    let mut last_wrap_ix = 0;
    let mut prev_c = '\0';
    let mut index = 0;

    for c in line.chars() {
        let ix = index;
        index += c.len_utf8();
        if c == '\n' {
            continue;
        }

        if is_word_char(c) {
            if prev_c == ' ' && c != ' ' && first_non_whitespace_ix.is_some() {
                last_candidate_ix = ix;
                last_candidate_width = width;
            }
        } else if c != ' ' && first_non_whitespace_ix.is_some() {
            // Text that is not separated by spaces, such as CJK, may break
            // before any character.
            last_candidate_ix = ix;
            last_candidate_width = width;
        }
        if c != ' ' && first_non_whitespace_ix.is_none() {
            first_non_whitespace_ix = Some(ix);
        }

        let item_width = face.advance(c, size);
        width += item_width;
        if width > wrap_width && ix > last_wrap_ix {
            if let (None, Some(first)) = (indent, first_non_whitespace_ix) {
                indent =
                    Some(MAX_INDENT.min(u32::try_from(first - last_wrap_ix).unwrap_or(MAX_INDENT)));
            }
            if last_candidate_ix > 0 {
                last_wrap_ix = last_candidate_ix;
                width -= last_candidate_width;
                last_candidate_ix = 0;
            } else {
                last_wrap_ix = ix;
                width = item_width;
            }
            if let Some(indent) = indent {
                #[allow(clippy::cast_precision_loss)]
                {
                    width += face.advance(' ', size) * indent as f32;
                }
            }
            out.push((last_wrap_ix, indent.unwrap_or(0)));
            // GPUI returns the boundary before it records this character as
            // the previous one, so the next one is compared with the one
            // before the break.
            continue;
        }
        prev_c = c;
    }
    out
}

/// `text` broken into the lines the window would draw it in, at `wrap_width`.
///
/// Each paragraph — the text between two newlines — is wrapped on its own, as
/// the window lays out each one. A line keeps no trailing space.
#[must_use]
pub fn wrap(text: &str, size: f32, weight: Weight, wrap_width: f32) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        let mut from = 0;
        for (at, _) in boundaries(paragraph, size, weight, wrap_width) {
            lines.push(paragraph[from..at].trim_end().to_string());
            from = at;
        }
        lines.push(paragraph[from..].trim_end().to_string());
    }
    lines
}

/// `line` cut short with an ellipsis to fit `truncate_width`, as the window's
/// `truncate()` does, or the line itself when it fits.
///
/// GPUI's `truncate_line` from the end, step for step.
#[must_use]
pub fn truncate(line: &str, size: f32, weight: Weight, truncate_width: f32) -> String {
    const AFFIX: &str = "…";
    let face = face(weight);
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
];

#[cfg(test)]
mod tests {
    use super::*;

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
        for line in &lines {
            assert!(!line.ends_with(' '), "{line:?} keeps no trailing space");
        }
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
