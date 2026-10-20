//! The code a deck points at, frozen when it is packed.
//!
//! A deck's refs name lines in files, and those files are on one machine. A
//! `.deck` file is read on another, often by somebody with no checkout at
//! all, so the lines have to travel with it. Not whole files: a file holds
//! code the author never showed, and every line shipped is a line the secret
//! scanner has to pass. What travels is each range a deck shows, with enough
//! around it for a reader to see where it sits — the spans [`keep`] chooses —
//! plus the file's hash and the commit, so a reader inside the same repo can
//! tell whether the code has moved since.
//!
//! In the file, `snap/index.json` lists every source file once, and each one's
//! lines are a plain text entry beside it, `snap/1.txt` and on. The entries
//! are numbered rather than named after their paths so no path in a deck ever
//! becomes a path on the reader's disk.

use serde::{Deserialize, Serialize};

use crate::line::LineRange;

/// How many lines either side of a range are kept by default.
///
/// More than any export shows — a page shows a few — so a format can choose,
/// and the window has something to scroll into before it reaches the edge of
/// what was kept.
pub const CONTEXT: u32 = 20;

/// Every source file a deck froze.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Index {
    /// One per source file, in the order they were first pointed at.
    pub files: Vec<File>,
}

/// One source file, as it was when the deck was packed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct File {
    /// Where it is in the repo, with forward slashes.
    pub path: String,
    /// The entry its kept lines are in, as `snap/1.txt`.
    pub entry: String,
    /// The sha256 of the whole file as it was read, in hex.
    pub sha256: String,
    /// The commit it was read at, when it was in a git repo.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    /// How the file stood against that commit.
    pub state: State,
    /// The spans kept, in order, apart, and within the file.
    pub kept: Vec<LineRange>,
}

/// How a file stood against the commit a deck was packed at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    /// As committed. A reader at that commit has the same file.
    Clean,
    /// Changed since: what the deck showed is not in any commit.
    Modified,
    /// Not in the repo at all, or there was no repo.
    Untracked,
}

/// One source file's kept lines, with what is known about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    /// What the index says about it.
    pub file: File,
    /// Its kept lines, every one ending in a newline, span after span.
    pub text: String,
}

impl Snapshot {
    /// Line `n` of the source file, counted from one, without its newline,
    /// or `None` when it was not kept.
    #[must_use]
    pub fn line(&self, n: u32) -> Option<&str> {
        let mut before = 0u32;
        for span in &self.file.kept {
            if span.contains(n) {
                let at = usize::try_from(before + (n - span.first)).ok()?;
                return self.text.lines().nth(at);
            }
            before += span.len();
        }
        None
    }

    /// Whether every line of `range` was kept.
    #[must_use]
    pub fn covers(&self, range: LineRange) -> bool {
        self.file
            .kept
            .iter()
            .any(|span| span.first <= range.first && range.last <= span.last)
    }

    /// Why this snapshot cannot be what it says, if it cannot.
    ///
    /// # Errors
    ///
    /// When its spans are out of order, overlap or start before line one,
    /// when its path could leave the repo, or when its text does not hold
    /// exactly as many lines as its spans.
    pub fn check(&self) -> Result<(), String> {
        let path = &self.file.path;
        if path.is_empty()
            || path.starts_with('/')
            || path.contains('\\')
            || path
                .split('/')
                .any(|s| s.is_empty() || s == "." || s == "..")
        {
            return Err(format!("{path:?} is not a path inside a repo"));
        }
        let mut last = 0u32;
        let mut lines = 0u64;
        for span in &self.file.kept {
            if span.first == 0 || span.first <= last {
                return Err(format!("{path}: its kept lines are out of order"));
            }
            last = span.last;
            lines += u64::from(span.len());
        }
        if !self.text.is_empty() && !self.text.ends_with('\n') {
            return Err(format!("{path}: its last kept line has no newline"));
        }
        if self.text.lines().count() as u64 != lines {
            return Err(format!("{path}: its text and its spans disagree"));
        }
        Ok(())
    }
}

/// The spans of `source` to keep for `ranges`, each widened by `context`
/// lines and merged where they touch, and those lines' text.
///
/// The text holds the kept lines in order, every one ending in a newline,
/// including a last line the file left without one.
#[must_use]
pub fn keep(source: &str, ranges: &[LineRange], context: u32) -> (Vec<LineRange>, String) {
    let lines: Vec<&str> = source.lines().collect();
    let count = u32::try_from(lines.len()).unwrap_or(u32::MAX);
    if count == 0 {
        return (Vec::new(), String::new());
    }
    let mut wide: Vec<LineRange> = ranges
        .iter()
        .filter(|range| range.first >= 1 && range.first <= count)
        .map(|range| {
            LineRange::new(
                range.first.saturating_sub(context).max(1),
                range.last.saturating_add(context).min(count),
            )
        })
        .collect();
    wide.sort_by_key(|range| range.first);

    let mut kept: Vec<LineRange> = Vec::new();
    for range in wide {
        match kept.last_mut() {
            Some(last) if range.first <= last.last.saturating_add(1) => {
                last.last = last.last.max(range.last);
            }
            _ => kept.push(range),
        }
    }
    let mut text = String::new();
    for span in &kept {
        for line in &lines[span.to_zero_based()] {
            text.push_str(line);
            text.push('\n');
        }
    }
    (kept, text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(lines: u32) -> String {
        (1..=lines).map(|n| format!("line {n}\n")).collect()
    }

    fn snapshot(source: &str, ranges: &[LineRange], context: u32) -> Snapshot {
        let (kept, text) = keep(source, ranges, context);
        Snapshot {
            file: File {
                path: "src/a.rs".to_string(),
                entry: "snap/1.txt".to_string(),
                sha256: String::new(),
                commit: None,
                state: State::Clean,
                kept,
            },
            text,
        }
    }

    #[test]
    fn a_range_keeps_its_context_and_no_more() {
        let snap = snapshot(&source(100), &[LineRange::new(40, 42)], 5);
        assert_eq!(snap.file.kept, vec![LineRange::new(35, 47)]);
        assert_eq!(snap.line(35), Some("line 35"));
        assert_eq!(snap.line(47), Some("line 47"));
        assert_eq!(snap.line(34), None);
        assert_eq!(snap.line(48), None);
        assert!(snap.check().is_ok());
    }

    #[test]
    fn ranges_that_touch_are_one_span_and_ranges_apart_are_two() {
        let snap = snapshot(
            &source(200),
            &[
                LineRange::new(50, 50),
                LineRange::new(10, 12),
                LineRange::new(60, 61),
                LineRange::new(150, 150),
            ],
            5,
        );
        assert_eq!(
            snap.file.kept,
            vec![
                LineRange::new(5, 17),
                LineRange::new(45, 66),
                LineRange::new(145, 155)
            ]
        );
        // A line in the second span is found past the first one's lines.
        assert_eq!(snap.line(60), Some("line 60"));
        assert_eq!(snap.line(150), Some("line 150"));
        assert_eq!(snap.line(30), None);
        assert!(snap.covers(LineRange::new(45, 66)));
        assert!(!snap.covers(LineRange::new(16, 46)));
        assert!(snap.check().is_ok());
    }

    #[test]
    fn context_stops_at_the_ends_of_the_file() {
        let snap = snapshot(
            &source(10),
            &[LineRange::new(1, 2), LineRange::new(9, 10)],
            20,
        );
        assert_eq!(snap.file.kept, vec![LineRange::new(1, 10)]);
    }

    #[test]
    fn a_last_line_with_no_newline_is_kept_with_one() {
        let snap = snapshot("a\nb", &[LineRange::new(2, 2)], 0);
        assert_eq!(snap.text, "b\n");
        assert_eq!(snap.line(2), Some("b"));
        assert!(snap.check().is_ok());
    }

    #[test]
    fn a_range_past_the_end_keeps_nothing() {
        let snap = snapshot(&source(5), &[LineRange::new(9, 12)], 3);
        assert!(snap.file.kept.is_empty());
        assert_eq!(snap.text, "");
    }

    #[test]
    fn a_snapshot_that_disagrees_with_itself_is_caught() {
        let mut snap = snapshot(&source(20), &[LineRange::new(5, 6)], 1);
        snap.text.push_str("one more\n");
        assert!(snap.check().is_err());

        let mut snap = snapshot(&source(20), &[LineRange::new(5, 6)], 1);
        snap.file.path = "../outside.rs".to_string();
        assert!(snap.check().is_err());

        let mut snap = snapshot(
            &source(20),
            &[LineRange::new(5, 6), LineRange::new(15, 16)],
            1,
        );
        snap.file.kept.reverse();
        assert!(snap.check().is_err());
    }
}
