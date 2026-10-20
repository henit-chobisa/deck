//! A deck written out for somebody who does not have deck.
//!
//! Everything here writes from a [`Document`]: what a group shows and in what
//! order, which numeral sits beside which line and in what ink, have all been
//! settled by [`deck_core::document`], and a format only lays them out. What
//! a format adds is the page: the window's band, panes and strip drawn at the
//! window's own measurements ([`crate::metrics`]), the faces it is set in, and
//! the `.deck` file itself, so the person it was sent to can open it in deck
//! and answer.
//!
//! HTML is the first format. The PDF follows it, from the same document.

mod face;
mod html;

pub use html::html;

use deck_core::bundle::Bundle;
use deck_core::theme::{Palette, Syntax};

/// How an export is to be written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    /// The groups to show, by number from one, in the order they are in the
    /// deck. Empty shows them all.
    pub groups: Vec<u32>,
    /// Whether what the reader and the agent said goes with it.
    ///
    /// Off, there is no argument anywhere in the export, and the `.deck` it
    /// carries has no review in it either: a file that hid the comments on
    /// the page and handed them over to anybody who unpacked it would be
    /// telling its reader something untrue about what they were sent.
    pub comments: bool,
    /// Whether the palette is a dark one, for the parts of the page the
    /// palette does not paint: the browser's own scrollbars and controls.
    pub dark: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            groups: Vec::new(),
            comments: true,
            dark: true,
        }
    }
}

impl Options {
    /// Whether group `n` is one of those asked for.
    #[must_use]
    pub fn shows(&self, n: u32) -> bool {
        self.groups.is_empty() || self.groups.contains(&n)
    }
}

/// `bundle` as one self-contained HTML file, painted in `palette` with the
/// window's highlighter.
///
/// The `.deck` the page carries is `bundle` packed again — without its
/// review, when the comments are left out — and the page is written from that
/// same packed deck, so what the page shows and what it carries are one deck.
///
/// # Errors
///
/// When the bundle does not pack or a part of it does not parse.
pub fn deck(
    bundle: &Bundle,
    palette: &Palette,
    syntax: Syntax,
    options: &Options,
) -> Result<String, String> {
    let mut bundle = bundle.clone();
    if !options.comments {
        bundle.files.remove("review.json");
    }
    let bytes = bundle.pack()?;
    let (packed, _) = Bundle::open(&bytes).map_err(|err| err.to_string())?;
    let document = crate::document::build(&packed, palette, options.dark, syntax)
        .map_err(|err| err.to_string())?;
    Ok(html(&document, &bytes, options))
}

/// The groups `--groups` names: numbers and ranges, joined by commas, as
/// `1,3-5`. In order, each once.
///
/// # Errors
///
/// When a part is not a number or a range of them, a range runs backwards,
/// or a group is numbered zero.
pub fn groups(list: &str) -> Result<Vec<u32>, String> {
    let mut out = Vec::new();
    for part in list.split(',').map(str::trim) {
        let number = |text: &str| {
            text.trim()
                .parse::<u32>()
                .ok()
                .filter(|&n| n > 0)
                .ok_or_else(|| {
                    format!("`{part}` is not a group: groups are numbered from 1, as `1,3-5`")
                })
        };
        let (first, last) = match part.split_once('-') {
            Some((first, last)) => (number(first)?, number(last)?),
            None => {
                let n = number(part)?;
                (n, n)
            }
        };
        // No deck holds anywhere near this many, and a range written by
        // mistake as `1-4000000000` should be an error rather than a list
        // four billion long.
        if last > 10_000 {
            return Err(format!("`{part}` is more groups than any deck holds"));
        }
        if last < first {
            return Err(format!(
                "`{part}` runs backwards: write it as `{last}-{first}`"
            ));
        }
        out.extend(first..=last);
    }
    out.sort_unstable();
    out.dedup();
    Ok(out)
}

#[cfg(test)]
mod tests;
