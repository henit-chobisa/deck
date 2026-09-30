//! The comments a reader has written, before they go back.
//!
//! Hiding a deck keeps them: the window is packed into a [`Session`] and put on
//! the bar, and everything it was holding goes with it. Quitting does not, and
//! for most of deck's life that meant a reader who closed the window by
//! accident — or whose machine did it for them — lost every word they had
//! typed. Nothing warned them, because nothing knew.
//!
//! So they are written down as they are made. In the deck's own directory,
//! beside `done` and the review, because that is where everything else about
//! this deck already lives and it goes when the deck does.
//!
//! Not the review. A review is the finished thing, written once, at the moment
//! the reader says so. This is the middle of the work, and its only job is to
//! be there if they come back.

use std::path::Path;

use deck_core::Moment;
use serde::{Deserialize, Serialize};

use crate::view::Remark;

/// What was in hand when the deck last went away.
///
/// Both halves, because they are shown in different places and losing either
/// one looks like losing everything. The remarks are what goes back in the
/// review; the transcript is what the rail draws — so restoring only the
/// remarks put the reader's comments in the eventual review and nowhere they
/// could see them, which reads exactly like nothing having been kept.
#[derive(Default, Serialize, Deserialize)]
pub struct Draft {
    #[serde(default)]
    pub remarks: Vec<Remark>,
    #[serde(default)]
    pub transcript: Vec<Moment>,
}

/// The file remarks are kept in while they are still being written.
const DRAFT: &str = "draft.json";

/// Write down what has been said so far.
///
/// Called on every change rather than on the way out, because the way out is
/// exactly what cannot be relied on: a quit, a crash and a force-close all skip
/// whatever tidy exit path they were supposed to take.
///
/// Through a temporary file, so an interrupted write cannot leave half a draft
/// where a whole one was — and a half-parsed draft would lose the work just as
/// surely as no draft at all.
pub fn save(root: &Path, remarks: &[Remark], transcript: &[Moment]) {
    if remarks.is_empty() {
        clear(root);
        return;
    }
    let held = Draft {
        remarks: remarks.to_vec(),
        transcript: transcript.to_vec(),
    };
    let Ok(json) = serde_json::to_string(&held) else {
        return;
    };
    let tmp = root.join("draft.json.tmp");
    if std::fs::write(&tmp, json).is_ok() {
        let _ = std::fs::rename(&tmp, root.join(DRAFT));
    }
}

/// Read back what was being written when the deck last went away.
#[must_use]
pub fn read(root: &Path) -> Draft {
    std::fs::read_to_string(root.join(DRAFT))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

/// Forget it.
///
/// Once a review is submitted the draft is behind it, and leaving one lying
/// about would put answered comments back in front of the next reader.
pub fn clear(root: &Path) {
    let _ = std::fs::remove_file(root.join(DRAFT));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn remark(text: &str) -> Remark {
        Remark {
            group: "g1".into(),
            ref_id: Some("g1r1".into()),
            file: Some("src/lib.rs".into()),
            range: Some(deck_core::LineRange::single(4)),
            said: None,
            moment: None,
            quote: "fn main() {".into(),
            text: text.into(),
            when: deck_core::When::Queue,
            kind: deck_core::Kind::MustFix,
        }
    }

    #[test]
    fn what_was_written_survives_the_window_going_away() {
        // The report this exists for: quit by accident, reopen, and every
        // comment is gone. Hiding kept them because the window was packed and
        // put on the bar; quitting ended the process, and they had only ever
        // been in memory.
        let room = tempfile::tempdir().expect("a directory");
        let deck = room.path();

        assert!(read(deck).remarks.is_empty(), "nothing said yet");

        let said = Moment {
            at_ms: 12,
            what: deck_core::What::Wrote,
            group: Some("g1".into()),
            ref_id: Some("g1r1".into()),
            file: None,
            range: None,
            text: "this assumes sorted input".into(),
            kind: Some(deck_core::Kind::MustFix),
            when: deck_core::When::Queue,
        };
        save(deck, &[remark("this assumes sorted input")], &[said]);

        let back = read(deck);
        assert_eq!(back.remarks.len(), 1);
        assert_eq!(back.remarks[0].text, "this assumes sorted input");
        assert_eq!(back.remarks[0].kind, deck_core::Kind::MustFix);
        assert_eq!(back.remarks[0].when, deck_core::When::Queue);
        // And the half the rail draws. Without it the comments come back into
        // the review and into nothing the reader can see, which looks exactly
        // like nothing having been kept.
        assert_eq!(back.transcript.len(), 1, "the rail's copy comes back too");

        // Submitting puts the review behind them.
        clear(deck);
        assert!(read(deck).remarks.is_empty());
    }

    #[test]
    fn a_remark_on_the_prose_keeps_the_words_it_was_about() {
        // The mark under the narration is drawn from the word range, so a
        // remark that comes back without one comes back invisible — the
        // comment is in the review and the sentence it answered looks
        // untouched.
        //
        // Its place in the transcript deliberately does not survive. That
        // number indexes a transcript this session built, and the next one
        // builds its own, so a value kept from before would point at whatever
        // happens to sit there now.
        let room = tempfile::tempdir().expect("a directory");
        let deck = room.path();

        let on_prose = Remark {
            said: Some((12, 19)),
            moment: Some(3),
            ..remark("this claim is the part I doubt")
        };
        save(deck, &[on_prose], &[]);

        let back = read(deck);
        assert_eq!(
            back.remarks[0].said,
            Some((12, 19)),
            "the words it answered"
        );
        assert_eq!(back.remarks[0].moment, None, "and not a stale index");
    }

    #[test]
    fn taking_the_last_one_back_leaves_nothing_behind() {
        // Deleting every remark has to remove the file rather than write an
        // empty list, or reopening would restore a draft the reader had
        // already emptied.
        let room = tempfile::tempdir().expect("a directory");
        let deck = room.path();

        save(deck, &[remark("first thoughts")], &[]);
        save(deck, &[], &[]);

        assert!(!deck.join(DRAFT).exists(), "and the file is gone");
        assert!(read(deck).remarks.is_empty());
    }
}
