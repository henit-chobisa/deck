//! Naming a ref on a command line.
//!
//! A ref is a file, a range and a note, and all three have to survive being one
//! shell argument. The shape is:
//!
//! ```text
//! src/batch.ts:140-148 decremented *twice* when the write fails
//! ```
//!
//! File, colon, range, then — after a single space — whatever is left is the
//! note. It parses from the right rather than the left, because a Windows path
//! has a colon in it and a note may have anything in it at all.

use std::path::PathBuf;

use deck_core::LineRange;

/// What a `--ref` argument said.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Named {
    /// The file, as written.
    pub file: PathBuf,
    /// The lines to light up.
    pub range: LineRange,
    /// The note, if one was given.
    pub note: Option<String>,
}

/// Read one `--ref` argument.
///
/// # Errors
///
/// When there is no `file:range` at the front of it, or the range is not two
/// numbers.
pub fn parse(argument: &str) -> anyhow::Result<Named> {
    let (target, note) = match argument.split_once(' ') {
        Some((target, note)) => (target, Some(note.trim().to_string())),
        None => (argument, None),
    };

    // From the right: `C:\src\batch.ts:140-148` has three colons in it and only
    // the last one separates the file from the lines.
    let (file, span) = target
        .rsplit_once(':')
        .ok_or_else(|| anyhow::anyhow!("expected file:first-last, got `{target}`"))?;
    anyhow::ensure!(!file.is_empty(), "no file in `{argument}`");

    let (first, last) = match span.split_once('-') {
        Some((first, last)) => (first, last),
        // A single line is a range of one, and writing `140` for it is what
        // anybody does when they mean one line.
        None => (span, span),
    };
    let number = |text: &str, which: &str| -> anyhow::Result<u32> {
        text.trim()
            .parse::<u32>()
            .map_err(|_| anyhow::anyhow!("`{text}` is not a {which} line number, in `{argument}`"))
    };

    Ok(Named {
        file: PathBuf::from(file),
        range: LineRange::new(number(first, "first")?, number(last, "last")?),
        note: note.filter(|note| !note.is_empty()),
    })
}
