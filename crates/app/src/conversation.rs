//! The conversation belongs to a session, not to the window displaying it.
//!
//! Keeping its journal, clock and undelivered remarks together prevents hide
//! from saving the comments while silently discarding their conversation.

use std::time::Instant;

use deck_core::{Moment, When};

#[derive(Clone)]
pub struct Conversation {
    pub transcript: Vec<Moment>,
    pub began: Instant,
    pub asked_at: Option<Instant>,
    held: Vec<Moment>,
}

impl Default for Conversation {
    fn default() -> Self {
        Self {
            transcript: Vec::new(),
            began: Instant::now(),
            asked_at: None,
            held: Vec::new(),
        }
    }
}

impl Conversation {
    /// The record never waits for speech; only delivery waits for a gap.
    pub fn record(&mut self, moment: Moment, speaking: bool) -> Vec<Moment> {
        self.transcript.push(moment.clone());
        if moment.when == When::Interrupt || !speaking {
            let mut ready = self.release();
            ready.push(moment);
            ready
        } else {
            self.held.push(moment);
            Vec::new()
        }
    }

    /// Hiding and interrupting both end a turn, just as speech completion does.
    pub fn release(&mut self) -> Vec<Moment> {
        std::mem::take(&mut self.held)
    }

    pub fn queued(&self) -> bool {
        !self.held.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn records(moments: &[Moment]) -> serde_json::Value {
        serde_json::to_value(moments).unwrap()
    }

    fn remark(text: &str, when: When) -> Moment {
        Moment {
            at_ms: 123,
            what: deck_core::What::Wrote,
            group: Some("g1".into()),
            ref_id: Some("g1r1".into()),
            file: Some("src/lib.rs".into()),
            range: Some(deck_core::LineRange::single(4)),
            text: text.into(),
            kind: Some(deck_core::Kind::Question),
            when,
        }
    }

    #[test]
    fn a_session_handoff_keeps_the_clock_history_and_pending_question() {
        let mut original = Conversation::default();
        let question = remark("why this guard?", When::Queue);
        original.asked_at = Some(Instant::now());
        assert!(original.record(question.clone(), true).is_empty());
        let mut restored = original.clone();
        drop(original);
        assert_eq!(
            records(&restored.transcript),
            records(std::slice::from_ref(&question))
        );
        assert!(restored.asked_at.is_some());
        assert!(restored.began <= restored.asked_at.unwrap());
        assert_eq!(records(&restored.release()), records(&[question]));
        assert!(
            restored.release().is_empty(),
            "reopen must not redeliver the queue"
        );
        assert_eq!(
            restored.transcript.len(),
            1,
            "delivery does not erase the record"
        );
    }

    #[test]
    fn an_interruption_releases_earlier_remarks_before_taking_the_floor() {
        let mut conversation = Conversation::default();
        let queued = remark("after this sentence", When::Queue);
        let interrupt = remark("stop", When::Interrupt);
        assert!(conversation.record(queued.clone(), true).is_empty());
        assert_eq!(
            records(&conversation.record(interrupt.clone(), true)),
            records(&[queued, interrupt])
        );
        assert!(!conversation.queued());
        assert_eq!(conversation.transcript.len(), 2);
    }

    #[test]
    fn a_silent_walk_delivers_without_waiting_for_an_audio_tick() {
        let mut conversation = Conversation::default();
        let question = remark("what about this?", When::Queue);
        assert_eq!(
            records(&conversation.record(question.clone(), false)),
            records(&[question])
        );
        assert!(!conversation.queued());
    }
}
