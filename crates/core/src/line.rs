//! Line coordinates.
//!
//! Every off-by-one bug in a tool like this comes from the same place: the
//! protocol counts lines the way a person reads them, and buffers count the way
//! an array is indexed. Those are two different number lines, and the moment
//! they mix in a bare `- 1` the mistake stops being visible.
//!
//! So there is one type for the first convention, and one method to leave it.

use serde::{Deserialize, Serialize};

/// A range of lines, 1-based and inclusive on both ends.
///
/// `LineRange { first: 1, last: 1 }` is the first line of a file, and it has a
/// length of one. This matches how the protocol, editors, and people talk about
/// lines. To index a slice with it, call [`LineRange::to_zero_based`].
///
/// On the wire it is a two-element array, `[120, 134]`, which is what the
/// `serde(into)` and `serde(from)` attributes below convert to and from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(into = "[u32; 2]", from = "[u32; 2]")]
pub struct LineRange {
    /// The first line, counting from 1.
    pub first: u32,
    /// The last line, included in the range.
    pub last: u32,
}

impl LineRange {
    /// A range covering `first` through `last`.
    ///
    /// The arguments are corrected rather than rejected: line 0 does not exist
    /// so it becomes line 1, and a backwards range is swapped. A ref that names
    /// its lines the wrong way round is a sloppy deck, not a reason to show the
    /// reader nothing.
    #[must_use]
    pub fn new(first: u32, last: u32) -> Self {
        let (first, last) = if first <= last {
            (first, last)
        } else {
            (last, first)
        };
        Self {
            first: first.max(1),
            last: last.max(1),
        }
    }

    /// A range covering the single line `line`.
    #[must_use]
    pub fn single(line: u32) -> Self {
        Self::new(line, line)
    }

    /// How many lines the range covers. Never zero.
    // No `is_empty` to pair with this: both ends are inclusive, so the
    // shortest range there is still covers one line. A method that could
    // only ever return `false` would be a worse answer than its absence.
    #[allow(clippy::len_without_is_empty)]
    #[must_use]
    pub fn len(&self) -> u32 {
        self.last - self.first + 1
    }

    /// Whether `line` falls inside the range.
    #[must_use]
    pub fn contains(&self, line: u32) -> bool {
        self.first <= line && line <= self.last
    }

    /// The same range as a half-open, 0-based [`std::ops::Range`], ready to
    /// index a slice of lines.
    ///
    /// This is the only sanctioned way to leave 1-based counting.
    #[must_use]
    pub fn to_zero_based(&self) -> std::ops::Range<usize> {
        let start = (self.first - 1) as usize;
        let end = self.last as usize;
        start..end
    }

    /// The range, shortened so it cannot point past a file of `line_count`
    /// lines.
    ///
    /// A deck is written against a file the reader may have edited since, so a
    /// range that runs off the end is expected, not exceptional.
    #[must_use]
    pub fn clamp_to(&self, line_count: u32) -> Self {
        let limit = line_count.max(1);
        Self::new(self.first.min(limit), self.last.min(limit))
    }
}

impl From<[u32; 2]> for LineRange {
    fn from([first, last]: [u32; 2]) -> Self {
        Self::new(first, last)
    }
}

impl From<LineRange> for [u32; 2] {
    fn from(range: LineRange) -> Self {
        [range.first, range.last]
    }
}

impl std::fmt::Display for LineRange {
    /// Renders as `120-134`, or just `120` when the range is one line. This is
    /// the form that appears on a pane label.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.first == self.last {
            write!(f, "{}", self.first)
        } else {
            write!(f, "{}-{}", self.first, self.last)
        }
    }
}
