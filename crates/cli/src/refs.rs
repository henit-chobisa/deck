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
    /// What the range should become, when the ref is a proposed change rather
    /// than something to look at. Set by the caller, not by the syntax.
    pub after: Option<String>,
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
        // The syntax carries no replacement — `--after` is a flag of its own,
        // because a replacement is several lines and a ref is one.
        after: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ref_carries_no_replacement_of_its_own() {
        // `--after` is a flag rather than part of the syntax, because a
        // replacement is several lines and a ref is one. Parsing must not
        // invent one from a note that happens to look like code.
        let named = parse("src/batch.ts:140-148 pending -= 1;").expect("parses");
        assert_eq!(named.after, None);
        assert_eq!(named.note.as_deref(), Some("pending -= 1;"));
    }

    fn named(argument: &str) -> Named {
        parse(argument).expect("this argument parses")
    }

    #[test]
    fn a_file_a_range_and_a_note() {
        let got = named("src/batch.ts:140-148 decremented *twice*");
        assert_eq!(got.file, PathBuf::from("src/batch.ts"));
        assert_eq!(got.range, LineRange::new(140, 148));
        assert_eq!(got.note.as_deref(), Some("decremented *twice*"));
    }

    #[test]
    fn one_number_is_one_line() {
        assert_eq!(named("a.rs:12").range, LineRange::new(12, 12));
    }

    #[test]
    fn a_note_is_optional() {
        assert_eq!(named("a.rs:1-2").note, None);
    }

    #[test]
    fn a_path_may_have_colons_in_it() {
        // Read from the right, or a Windows path loses its drive letter and a
        // ref points at a file nobody has.
        let got = named(r"C:\src\batch.ts:140-148");
        assert_eq!(got.file, PathBuf::from(r"C:\src\batch.ts"));
        assert_eq!(got.range, LineRange::new(140, 148));
    }

    #[test]
    fn a_note_may_have_colons_in_it_too() {
        let got = named("a.rs:9 note: this one");
        assert_eq!(got.file, PathBuf::from("a.rs"));
        assert_eq!(got.note.as_deref(), Some("note: this one"));
    }

    #[test]
    fn a_backwards_range_is_corrected_rather_than_refused() {
        // The same forgiveness the model has everywhere else: an agent that
        // wrote the numbers the other way round meant the lines between them.
        assert_eq!(named("a.rs:20-10").range, LineRange::new(10, 20));
    }

    #[test]
    fn what_is_not_a_ref_is_said_plainly() {
        assert!(parse("src/batch.ts").is_err(), "no range");
        assert!(parse("src/batch.ts:many").is_err(), "not a number");
        assert!(parse(":10-20").is_err(), "no file");
    }
}
