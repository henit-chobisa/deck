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
/// The `.deck` the page carries is `bundle` packed again: without its
/// review when the comments are left out, and with only the groups asked for
/// when some are ([`only`]). Choosing groups is choosing what to share, and a
/// file whose page showed group two while its `.deck` held all three would
/// hand the other two to anybody who unpacked it.
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
    let carried = only(&bundle, &options.groups)?;
    let bytes = carried.pack()?;
    // The page is written from the whole deck and shows only what was
    // asked for, so its groups keep the numbers they have in the deck —
    // `2 of 3`, not `1 of 1` — while the file it carries holds nothing else.
    let document = crate::document::build(&bundle, palette, options.dark, syntax)
        .map_err(|err| err.to_string())?;
    Ok(html(&document, &bytes, options))
}

/// `bundle` holding only the groups numbered in `chosen`, or all of it when
/// `chosen` is empty or names every group.
///
/// The chosen groups keep their files and their names. The code that goes
/// with them is what they still need — the spans of a frozen file their
/// refs, their remarks and their moments land in — and every other span,
/// and every file none of them needs, is left out, because a span kept
/// for another group's range is that group's code. The review keeps the
/// chosen groups' remarks and moments; what was said about the deck as a
/// whole goes only with the whole deck, since it may be about any of it.
///
/// # Errors
///
/// When a part of the bundle does not parse.
pub fn only(bundle: &Bundle, chosen: &[u32]) -> Result<Bundle, String> {
    use std::collections::BTreeMap;

    use deck_core::bundle::snap;
    use deck_core::line::LineRange;
    use deck_core::protocol::Ref;

    let malformed = |err: deck_core::bundle::Unreadable| err.to_string();
    let groups = bundle.groups().map_err(malformed)?;
    // The group files in the order `groups` reads them: by their number.
    let mut names: Vec<(u32, &String)> = bundle
        .files
        .keys()
        .filter_map(|name| {
            let n = name.strip_prefix("deck/g")?.strip_suffix(".json")?;
            Some((n.parse().ok()?, name))
        })
        .collect();
    names.sort();
    let kept: Vec<bool> = (1..=groups.len())
        .map(|n| u32::try_from(n).is_ok_and(|n| chosen.is_empty() || chosen.contains(&n)))
        .collect();
    if kept.iter().all(|&keep| keep) {
        return Ok(bundle.clone());
    }

    let mut out = bundle.clone();
    let mut ids = Vec::new();
    for ((group, (_, name)), &keep) in groups.iter().zip(&names).zip(&kept) {
        if keep {
            ids.push(group.id.clone());
        } else {
            out.files.remove(*name);
        }
    }
    let path = |file: &std::path::Path| file.to_string_lossy().replace('\\', "/");

    // Every range a chosen group still needs, by file.
    let mut needed: BTreeMap<String, Vec<LineRange>> = BTreeMap::new();
    for (group, _) in groups.iter().zip(&kept).filter(|(_, keep)| **keep) {
        for one in &group.refs {
            if let Ref::Code(code) = one {
                needed.entry(path(&code.file)).or_default().push(code.range);
            }
        }
    }
    if let Some(mut review) = bundle.review().map_err(malformed)? {
        review
            .comments
            .retain(|comment| ids.contains(&comment.group));
        review
            .transcript
            .retain(|moment| moment.group.as_ref().is_some_and(|id| ids.contains(id)));
        for (file, range) in review
            .comments
            .iter()
            .map(|c| (&c.file, c.range))
            .chain(review.transcript.iter().map(|m| (&m.file, m.range)))
        {
            if let (Some(file), Some(range)) = (file, range) {
                needed.entry(path(file)).or_default().push(range);
            }
        }
        out.files.insert(
            "review.json".to_string(),
            serde_json::to_vec_pretty(&review).map_err(|err| err.to_string())?,
        );
    }

    // The frozen code, cut to the spans those ranges land in, renumbered.
    let snapshots = bundle.snapshots().map_err(malformed)?;
    out.files.retain(|name, _| !name.starts_with("snap/"));
    let mut index = snap::Index::default();
    for snapshot in snapshots {
        let Some(ranges) = needed.get(&snapshot.file.path) else {
            continue;
        };
        let spans: Vec<LineRange> = snapshot
            .file
            .kept
            .iter()
            .copied()
            .filter(|span| {
                ranges
                    .iter()
                    .any(|range| range.first <= span.last && span.first <= range.last)
            })
            .collect();
        if spans.is_empty() {
            continue;
        }
        let mut text = String::new();
        for no in spans.iter().flat_map(|span| span.first..=span.last) {
            text.push_str(snapshot.line(no).unwrap_or_default());
            text.push('\n');
        }
        let entry = format!("snap/{}.txt", index.files.len() + 1);
        out.files.insert(entry.clone(), text.into_bytes());
        index.files.push(snap::File {
            entry,
            kept: spans,
            ..snapshot.file
        });
    }
    if !index.files.is_empty() {
        out.files.insert(
            "snap/index.json".to_string(),
            serde_json::to_vec_pretty(&index).map_err(|err| err.to_string())?,
        );
    }
    Ok(out)
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
