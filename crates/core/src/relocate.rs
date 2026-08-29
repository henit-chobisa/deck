//! Following a pinned line while the file moves under it.
//!
//! You open a deck. It pins a comment to line 120. While you are reading you
//! notice something else, switch to your editor, add three lines near the top,
//! and save. Line 120 is now line 123, and every range deck is holding is wrong.
//!
//! Deck cannot ride along with those edits — it did not make them, and it is
//! not the editor that did. What it has instead is two versions of the file:
//! the snapshot taken when the deck opened, and whatever is on disk now. From
//! those two it can work out where each pinned line went.
//!
//! Three answers, in descending order of confidence, and the answer always
//! travels with how it was reached:
//!
//! 1. **The diff knows.** The line survived the edit, so a line-level diff maps
//!    it straight across. [`Source::Diff`].
//! 2. **The neighbourhood knows.** The line itself was rewritten, but a line
//!    with the same text and the same neighbours exists elsewhere.
//!    [`Source::Fingerprint`].
//! 3. **Nobody knows.** [`Source::Stale`] — say so, and let the agent search
//!    for the comment's `quote` instead of trusting a number.
//!
//! Never a silent guess. A comment that quietly points at the wrong line is
//! worse than one that admits it is lost, because the reader believes it.

use similar::TextDiff;

use crate::line::LineRange;
use crate::protocol::Source;

/// Where a pinned range ended up, and how much to trust the answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Relocated {
    /// The range in the current text.
    pub range: LineRange,
    /// How the range was arrived at.
    pub source: Source,
}

/// Follow `pins` from the text they were taken against to the text as it is
/// now.
///
/// The diff is computed once for the whole batch, so pass every pin in a file
/// together rather than calling this per comment.
///
/// Identical text is not a special case worth writing: the diff maps every line
/// to itself and each pin comes back unchanged, marked [`Source::Diff`].
#[must_use]
pub fn relocate(snapshot: &str, current: &str, pins: &[LineRange]) -> Vec<Relocated> {
    let was: Vec<&str> = snapshot.lines().collect();
    let now: Vec<&str> = current.lines().collect();
    let moved = line_map(snapshot, current);
    let line_count = clamp_u32(now.len());

    pins.iter()
        .map(|pin| follow(*pin, &was, &now, &moved, line_count))
        .collect()
}

/// Where each line of the snapshot ended up, by 0-based index.
///
/// `None` means the line did not survive: it was deleted, or edited into
/// something the diff no longer considers the same line.
fn line_map(snapshot: &str, current: &str) -> Vec<Option<usize>> {
    let mut map = vec![None; snapshot.lines().count()];

    // Only unchanged lines carry both indices. An insertion has no old index
    // and a deletion has no new one, which is exactly the distinction wanted
    // here — anything that moved without changing is what can be followed.
    for change in TextDiff::from_lines(snapshot, current).iter_all_changes() {
        if let (Some(before), Some(after)) = (change.old_index(), change.new_index())
            && let Some(slot) = map.get_mut(before)
        {
            *slot = Some(after);
        }
    }

    map
}

/// One pin, through the three answers.
fn follow(
    pin: LineRange,
    was: &[&str],
    now: &[&str],
    moved: &[Option<usize>],
    line_count: u32,
) -> Relocated {
    // The pin covers several lines and they need not have moved as a block: an
    // edit inside the range keeps the ends and loses the middle. Taking the
    // span of whatever survived keeps the comment over the code it was about.
    let landed: Vec<usize> = pin
        .to_zero_based()
        .filter_map(|ix| moved.get(ix).copied().flatten())
        .collect();

    if let (Some(&first), Some(&last)) = (landed.first(), landed.last()) {
        return Relocated {
            range: LineRange::new(clamp_u32(first) + 1, clamp_u32(last) + 1),
            source: Source::Diff,
        };
    }

    // Every pinned line was rewritten. The text may still exist somewhere, so
    // look for it by what surrounded it.
    let anchor_ix = (pin.first - 1) as usize;
    if let Some(print) = Fingerprint::of(was, anchor_ix)
        && let Some(found) = print.best_match(now, anchor_ix)
    {
        let first = clamp_u32(found) + 1;
        return Relocated {
            range: LineRange::new(first, first + pin.len() - 1).clamp_to(line_count),
            source: Source::Fingerprint,
        };
    }

    Relocated {
        range: pin.clamp_to(line_count),
        source: Source::Stale,
    }
}

/// A line, remembered by its own text and its two neighbours.
///
/// One line of code is rarely unique — `}` and `});` and a bare `return` occur
/// everywhere. What is closer to unique is a line *between two particular other
/// lines*, which is what makes the neighbours worth carrying.
struct Fingerprint<'a> {
    line: &'a str,
    before: &'a str,
    after: &'a str,
}

impl<'a> Fingerprint<'a> {
    /// Take the fingerprint of line `ix`, or `None` when the line is blank —
    /// a blank line matches every other blank line and identifies nothing.
    fn of(lines: &[&'a str], ix: usize) -> Option<Self> {
        let line = lines.get(ix)?.trim();
        if line.is_empty() {
            return None;
        }
        Some(Self {
            line,
            before: ix
                .checked_sub(1)
                .and_then(|i| lines.get(i))
                .unwrap_or(&"")
                .trim(),
            after: lines.get(ix + 1).unwrap_or(&"").trim(),
        })
    }

    /// The best candidate for this line in `lines`, or `None` when its text
    /// appears nowhere.
    ///
    /// Every line with matching text is a candidate. A matching neighbour is
    /// worth two points, because two lines agreeing is much stronger evidence
    /// than one. Equal scores are broken by staying near where the line used to
    /// be, on the reasoning that most edits are local.
    fn best_match(&self, lines: &[&str], came_from: usize) -> Option<usize> {
        let mut best: Option<(usize, u32, usize)> = None;

        for (ix, candidate) in lines.iter().enumerate() {
            if candidate.trim() != self.line {
                continue;
            }

            let mut score = 0;
            if !self.before.is_empty()
                && ix
                    .checked_sub(1)
                    .and_then(|i| lines.get(i))
                    .is_some_and(|l| l.trim() == self.before)
            {
                score += 2;
            }
            if !self.after.is_empty() && lines.get(ix + 1).is_some_and(|l| l.trim() == self.after) {
                score += 2;
            }

            let drift = ix.abs_diff(came_from);
            let better = match best {
                None => true,
                Some((_, best_score, best_drift)) => {
                    score > best_score || (score == best_score && drift < best_drift)
                }
            };
            if better {
                best = Some((ix, score, drift));
            }
        }

        best.map(|(ix, _, _)| ix)
    }
}

/// A line count as a `u32`.
///
/// A file with more than four billion lines is not a file anyone is reviewing,
/// and saturating here keeps every caller free of a cast that could silently
/// wrap.
fn clamp_u32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}
