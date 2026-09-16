//! Pure state for a live review conversation.
//!
//! The native window, filesystem mailbox, and speech worker all have different
//! lifetimes. Keeping their decisions here makes those adapters report facts
//! instead of each inventing its own version of whether a reader may be moved,
//! which utterance finished, or what a comment was about.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::LineRange;

/// A random identity for one ownership of a deck.
///
/// Reopening creates a new generation. Messages from an owner that crashed can
/// then be rejected without guessing whether its process id has been reused.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Generation(pub String);

/// The identity of one applied arrangement of evidence.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StageId(pub String);

/// The identity of one narration request.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct UtteranceId(pub String);

/// The identity of one public transcript entry.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EntryId(pub String);

/// Code and context currently being shown together.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stage {
    /// Stable identity used by comments and transcript entries.
    pub id: StageId,
    /// The authored group this conversation still belongs to.
    pub group: String,
    /// The authored or live ref, when the stage points at one.
    #[serde(rename = "ref", skip_serializing_if = "Option::is_none")]
    pub ref_id: Option<String>,
    /// The file being shown, when this is code evidence.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<PathBuf>,
    /// The resolved source lines shown to the reader.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<LineRange>,
    /// The exact displayed evidence covered by `range`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub quote: String,
    /// Identity of the immutable source snapshot used for the stage.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<String>,
}

/// A target captured at the instant a reader acts.
///
/// It contains values rather than a pane index. The stage may move, the group
/// may change, and a hidden window may rebuild its panes before the remark is
/// saved; none of those events are allowed to retarget what the reader meant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapturedAnchor {
    /// The group that was visible at capture time.
    pub group: String,
    /// The ref visible at capture time, when there was one.
    #[serde(rename = "ref", skip_serializing_if = "Option::is_none")]
    pub ref_id: Option<String>,
    /// The file visible at capture time, when this was code evidence.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<PathBuf>,
    /// The exact source range visible at capture time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<LineRange>,
    /// Code for a code anchor and prose for a narration anchor.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub quote: String,
    /// Identity of the immutable source snapshot, when code was shown.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot: Option<String>,
    /// The applied stage, never a pending request.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage: Option<StageId>,
    /// The narration entry selected by the reader, when applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry: Option<EntryId>,
}

impl Stage {
    /// Whether two commands name the same displayed evidence.
    ///
    /// Request identity is deliberately excluded: retrying or naming the same
    /// subject again must not reset dwell pacing or repaint the view.
    #[must_use]
    pub fn same_target(&self, other: &Self) -> bool {
        self.group == other.group
            && self.ref_id == other.ref_id
            && self.file == other.file
            && self.range == other.range
            && self.quote == other.quote
            && self.snapshot == other.snapshot
    }

    /// Freeze this stage into a comment target.
    #[must_use]
    pub fn capture(&self, entry: Option<EntryId>) -> CapturedAnchor {
        CapturedAnchor {
            group: self.group.clone(),
            ref_id: self.ref_id.clone(),
            file: self.file.clone(),
            range: self.range,
            quote: self.quote.clone(),
            snapshot: self.snapshot.clone(),
            stage: Some(self.id.clone()),
            entry,
        }
    }
}

/// Whether the native session can currently be interacted with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Lifecycle {
    /// The bar exists, but the reader has not opened the deck.
    Waiting,
    /// The reader has the deck open.
    Open,
    /// The reader put the deck back on the bar.
    Hidden,
    /// The review was durably written.
    Submitted,
    /// The reader closed without submitting.
    Closed,
}

/// Why agent-driven movement yielded to the reader.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PauseReason {
    /// The reader moved or scrolled independently.
    Navigation,
    /// The reader selected evidence or prose.
    Selection,
    /// A composer owns an immutable anchor.
    Composer,
    /// A question or must-fix interrupted the walk.
    Interruption,
    /// The deck is hidden or is not the active window.
    InactiveWindow,
}

impl PauseReason {
    /// Whether time alone is enough to give movement back.
    ///
    /// Scrolling and selecting are things a reader does *while watching*, and a
    /// walk that stopped permanently the first time somebody looked at a pane
    /// would be a mode that silently switches itself off. Those lapse. The other
    /// two do not: a composer owns its anchor until it closes, and a hidden
    /// window has nowhere to show movement.
    #[must_use]
    pub const fn lapses(self) -> bool {
        matches!(
            self,
            Self::Navigation | Self::Selection | Self::Interruption
        )
    }
}

/// How long reader activity keeps priority before movement returns.
///
/// Long enough to finish reading the lines under the pointer, short enough that
/// nobody has to learn a key to undo an accident. `f` is still there for a
/// reader who wants it back immediately.
pub const RESUME_AFTER_MS: u64 = 4_000;

/// Who currently controls movement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "state", content = "reason")]
pub enum Following {
    /// An accepted stage request may move the spotlight.
    Following,
    /// Reader movement wins until they explicitly resume.
    Paused(PauseReason),
}

/// Progress of the one utterance a session may own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AudioPhase {
    /// Nothing is being prepared or played.
    Idle,
    /// A provider or process is starting.
    Preparing,
    /// Audio is reaching the output device.
    Playing,
    /// The cancellable pause between paragraphs.
    ParagraphGap,
    /// The output sink drained naturally.
    Completed,
    /// The reader or lifecycle stopped playback.
    Interrupted,
    /// Synthesis or playback failed.
    Failed,
}

impl AudioPhase {
    /// Whether starting another utterance would overlap this one.
    #[must_use]
    pub fn active(self) -> bool {
        matches!(self, Self::Preparing | Self::Playing | Self::ParagraphGap)
    }
}

/// Pure state retained across hide and reopen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    /// The owner generation whose actions are accepted.
    pub generation: Generation,
    /// Whether the deck is waiting, visible, hidden, or terminal.
    pub lifecycle: Lifecycle,
    /// Whether the agent may move the stage.
    pub following: Following,
    /// The stage actually applied to the view.
    pub stage: Option<Stage>,
    /// The last time a different stage was applied, in caller-provided ms.
    pub last_stage_at_ms: Option<u64>,
    /// When reader activity took priority, in caller-provided ms.
    ///
    /// Only meaningful while `following` is a pause that lapses.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paused_at_ms: Option<u64>,
    /// The anchor owned by an open composer.
    pub composer: Option<CapturedAnchor>,
    /// The current or most recently finished utterance.
    pub utterance: Option<UtteranceId>,
    /// Its current phase.
    pub audio: AudioPhase,
}

impl State {
    /// Whether the reader still holds movement at `at_ms`.
    ///
    /// A pause that lapses stops holding once the reader has been still long
    /// enough — the same rule a show request applies, asked without waiting
    /// for one to arrive.
    #[must_use]
    pub fn holds(&self, at_ms: u64) -> bool {
        match self.following {
            Following::Following => false,
            Following::Paused(reason) => {
                !reason.lapses()
                    || self
                        .paused_at_ms
                        .is_none_or(|at| at_ms.saturating_sub(at) < RESUME_AFTER_MS)
            }
        }
    }

    /// A session whose bar exists and whose reader has not opened it yet.
    #[must_use]
    pub fn new(generation: Generation) -> Self {
        Self {
            generation,
            lifecycle: Lifecycle::Waiting,
            following: Following::Following,
            stage: None,
            last_stage_at_ms: None,
            paused_at_ms: None,
            composer: None,
            utterance: None,
            audio: AudioPhase::Idle,
        }
    }

    /// Apply one fact and return what adapters must do about it.
    ///
    /// Time is data supplied by the caller, so pacing tests do not sleep and a
    /// mailbox deadline can use the same clock as the command that created it.
    pub fn apply(&mut self, action: Action) -> Result<Vec<Effect>, Refusal> {
        if action.generation() != &self.generation {
            return Err(Refusal::StaleGeneration);
        }

        match action {
            Action::Opened { .. } => {
                self.lifecycle = Lifecycle::Open;
                // Opening *is* the reader asking to be shown. Every exit pauses
                // with `InactiveWindow`, and without this a deck that was ever
                // hidden could never be followed again without a keypress the
                // reader has no reason to make.
                self.resume();
                Ok(vec![
                    Effect::LifecycleChanged(Lifecycle::Open),
                    Effect::FollowingResumed,
                ])
            }
            Action::Show {
                stage,
                at_ms,
                source_matches,
                ..
            } => self.show(stage, at_ms, source_matches),
            Action::Pause { reason, at_ms, .. } => {
                // A pause the reader must clear by hand outranks one that
                // lapses: typing a comment should not be undone by a scroll
                // that happened a moment earlier.
                if let Following::Paused(held) = self.following
                    && !held.lapses()
                    && reason.lapses()
                {
                    return Ok(Vec::new());
                }
                self.following = Following::Paused(reason);
                self.paused_at_ms = Some(at_ms);
                Ok(vec![Effect::FollowingPaused(reason)])
            }
            Action::Resume { .. } => {
                if self.lifecycle != Lifecycle::Open {
                    return Err(Refusal::NotVisible);
                }
                self.resume();
                Ok(vec![Effect::FollowingResumed])
            }
            Action::OpenComposer { anchor, .. } => {
                if self.lifecycle != Lifecycle::Open {
                    return Err(Refusal::NotVisible);
                }
                self.following = Following::Paused(PauseReason::Composer);
                self.composer = Some(anchor.clone());
                Ok(vec![
                    Effect::FollowingPaused(PauseReason::Composer),
                    Effect::ComposerCaptured(anchor),
                ])
            }
            Action::CloseComposer { .. } => {
                self.composer = None;
                if self.following == Following::Paused(PauseReason::Composer) {
                    self.resume();
                    return Ok(vec![Effect::FollowingResumed]);
                }
                Ok(Vec::new())
            }
            Action::StartAudio { utterance, .. } => {
                if self.lifecycle != Lifecycle::Open {
                    return Err(Refusal::NotVisible);
                }
                if self.audio.active() {
                    return Err(Refusal::Busy);
                }
                self.utterance = Some(utterance.clone());
                self.audio = AudioPhase::Preparing;
                Ok(vec![Effect::AudioChanged {
                    utterance,
                    phase: AudioPhase::Preparing,
                }])
            }
            Action::AudioChanged {
                utterance, phase, ..
            } => {
                if self.utterance.as_ref() != Some(&utterance) {
                    return Err(Refusal::StaleUtterance);
                }
                self.audio = phase;
                Ok(vec![Effect::AudioChanged { utterance, phase }])
            }
            Action::Hidden { .. } => self.leave_visible(Lifecycle::Hidden),
            Action::Submitted { .. } => self.leave_visible(Lifecycle::Submitted),
            Action::Closed { .. } => self.leave_visible(Lifecycle::Closed),
        }
    }

    /// Capture only what is actually applied, never a request waiting in a mailbox.
    #[must_use]
    pub fn capture(&self, entry: Option<EntryId>) -> Option<CapturedAnchor> {
        self.stage.as_ref().map(|stage| stage.capture(entry))
    }

    fn show(
        &mut self,
        stage: Stage,
        at_ms: u64,
        source_matches: bool,
    ) -> Result<Vec<Effect>, Refusal> {
        if self.lifecycle != Lifecycle::Open {
            return Err(Refusal::NotVisible);
        }
        if !source_matches {
            return Err(Refusal::SourceChanged);
        }
        if let Following::Paused(reason) = self.following {
            // A pause that lapses is reader priority, not a mode. Once the
            // reader has been still for long enough, movement comes back on its
            // own — nobody should have to learn a key to undo a scroll.
            let lapsed = reason.lapses()
                && self
                    .paused_at_ms
                    .is_some_and(|at| at_ms.saturating_sub(at) >= RESUME_AFTER_MS);
            if lapsed {
                self.resume();
            } else {
                return Err(Refusal::Paused(reason));
            }
        }
        if self.composer.is_some() {
            return Err(Refusal::ComposerOwnsAnchor);
        }
        if let Some(current) = self
            .stage
            .as_ref()
            .filter(|current| current.same_target(&stage))
        {
            return Ok(vec![Effect::StageUnchanged(current.id.clone())]);
        }
        if let Some(last) = self.last_stage_at_ms {
            let elapsed = at_ms.saturating_sub(last);
            if elapsed < MINIMUM_DWELL_MS {
                return Err(Refusal::Paced {
                    retry_after_ms: MINIMUM_DWELL_MS - elapsed,
                });
            }
        }

        self.last_stage_at_ms = Some(at_ms);
        self.stage = Some(stage.clone());
        Ok(vec![Effect::StageApplied(stage)])
    }

    /// Give movement back, and forget when it was taken.
    fn resume(&mut self) {
        self.following = Following::Following;
        self.paused_at_ms = None;
    }

    fn leave_visible(&mut self, lifecycle: Lifecycle) -> Result<Vec<Effect>, Refusal> {
        self.lifecycle = lifecycle;
        self.following = Following::Paused(PauseReason::InactiveWindow);
        self.paused_at_ms = None;
        let mut effects = vec![
            Effect::FollowingPaused(PauseReason::InactiveWindow),
            Effect::LifecycleChanged(lifecycle),
        ];
        if self.audio.active() {
            self.audio = AudioPhase::Interrupted;
            if let Some(utterance) = self.utterance.clone() {
                effects.push(Effect::AudioChanged {
                    utterance,
                    phase: AudioPhase::Interrupted,
                });
            }
        }
        Ok(effects)
    }
}

/// Minimum time between two different applied targets.
pub const MINIMUM_DWELL_MS: u64 = 1_000;

/// A fact presented to the pure live state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// The reader opened the deck from its bar.
    Opened {
        /// The owner presenting this fact.
        generation: Generation,
    },
    /// An owner validated a target and its source snapshot.
    Show {
        /// The owner presenting this fact.
        generation: Generation,
        /// The complete target ready to apply.
        stage: Stage,
        /// Caller-provided monotonic milliseconds used for pacing.
        at_ms: u64,
        /// Whether disk still matches the displayed snapshot.
        source_matches: bool,
    },
    /// Reader activity takes control from the agent.
    Pause {
        /// The owner presenting this fact.
        generation: Generation,
        /// The reader activity that took control.
        reason: PauseReason,
        /// Caller-provided monotonic milliseconds, so the pause can lapse.
        at_ms: u64,
    },
    /// The reader explicitly returned control.
    Resume {
        /// The owner presenting this fact.
        generation: Generation,
    },
    /// A composer took ownership of a frozen target.
    OpenComposer {
        /// The owner presenting this fact.
        generation: Generation,
        /// The target captured before the composer opens.
        anchor: CapturedAnchor,
    },
    /// The composer was saved or discarded.
    CloseComposer {
        /// The owner presenting this fact.
        generation: Generation,
    },
    /// One utterance was accepted.
    StartAudio {
        /// The owner presenting this fact.
        generation: Generation,
        /// The accepted utterance.
        utterance: UtteranceId,
    },
    /// The matching worker reported a new phase.
    AudioChanged {
        /// The owner presenting this fact.
        generation: Generation,
        /// The utterance the worker is reporting on.
        utterance: UtteranceId,
        /// The newly observed phase.
        phase: AudioPhase,
    },
    /// The reader put the deck back on its bar.
    Hidden {
        /// The owner presenting this fact.
        generation: Generation,
    },
    /// The review was written durably.
    Submitted {
        /// The owner presenting this fact.
        generation: Generation,
    },
    /// The reader closed without submitting.
    Closed {
        /// The owner presenting this fact.
        generation: Generation,
    },
}

impl Action {
    fn generation(&self) -> &Generation {
        match self {
            Self::Opened { generation }
            | Self::Show { generation, .. }
            | Self::Pause { generation, .. }
            | Self::Resume { generation }
            | Self::OpenComposer { generation, .. }
            | Self::CloseComposer { generation }
            | Self::StartAudio { generation, .. }
            | Self::AudioChanged { generation, .. }
            | Self::Hidden { generation }
            | Self::Submitted { generation }
            | Self::Closed { generation } => generation,
        }
    }
}

/// Work an adapter performs after a transition succeeds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// Paint and reveal this stage, then acknowledge it.
    StageApplied(Stage),
    /// An identical target was already applied; acknowledge without moving.
    StageUnchanged(StageId),
    /// Publish that reader control paused movement.
    FollowingPaused(PauseReason),
    /// Publish that the reader resumed following.
    FollowingResumed,
    /// Give this immutable target to the composer and event journal.
    ComposerCaptured(CapturedAnchor),
    /// Start, update, or cancel the matching utterance in the worker.
    AudioChanged {
        /// The utterance whose late messages must not affect another one.
        utterance: UtteranceId,
        /// The phase now visible to commands and the reader.
        phase: AudioPhase,
    },
    /// Publish a visible or terminal lifecycle change.
    LifecycleChanged(Lifecycle),
}

/// A recoverable or terminal reason an action was not applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    /// The action belongs to an earlier owner.
    StaleGeneration,
    /// Only a visible deck may move, speak, or resume.
    NotVisible,
    /// The reader has priority over agent-driven movement.
    Paused(PauseReason),
    /// A different target moved too recently.
    Paced {
        /// How long the caller should wait before deliberately retrying.
        retry_after_ms: u64,
    },
    /// Disk no longer matches the text shown in the pane.
    SourceChanged,
    /// The composer owns an immutable anchor until it closes.
    ComposerOwnsAnchor,
    /// One utterance is already active.
    Busy,
    /// A late worker message names an obsolete utterance.
    StaleUtterance,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn generation() -> Generation {
        Generation("generation-a".into())
    }

    fn stage(id: &str, line: u32) -> Stage {
        Stage {
            id: StageId(id.into()),
            group: "g1".into(),
            ref_id: Some("g1r1".into()),
            file: Some("src/lib.rs".into()),
            range: Some(LineRange::single(line)),
            quote: format!("line {line}"),
            snapshot: Some("sha256:one".into()),
        }
    }

    fn open() -> State {
        let mut state = State::new(generation());
        state
            .apply(Action::Opened {
                generation: generation(),
            })
            .unwrap();
        state
    }

    #[test]
    fn a_reaction_uses_the_applied_stage_not_the_pending_request() {
        // A request is not state until the view applies it. Capturing between a
        // paced request and its retry must therefore retain what was on screen.
        let mut state = open();
        state
            .apply(Action::Show {
                generation: generation(),
                stage: stage("s1", 10),
                at_ms: 1_000,
                source_matches: true,
            })
            .unwrap();
        assert_eq!(
            state.apply(Action::Show {
                generation: generation(),
                stage: stage("s2", 20),
                at_ms: 1_100,
                source_matches: true,
            }),
            Err(Refusal::Paced {
                retry_after_ms: 900
            })
        );

        let anchor = state.capture(None).expect("the applied stage is captured");
        assert_eq!(anchor.stage, Some(StageId("s1".into())));
        assert_eq!(anchor.range, Some(LineRange::single(10)));
    }

    #[test]
    fn naming_the_same_target_again_does_not_repaint_or_restart_dwell() {
        let mut state = open();
        state
            .apply(Action::Show {
                generation: generation(),
                stage: stage("request-one", 10),
                at_ms: 1_000,
                source_matches: true,
            })
            .unwrap();

        assert_eq!(
            state
                .apply(Action::Show {
                    generation: generation(),
                    stage: stage("request-two", 10),
                    at_ms: 1_001,
                    source_matches: true,
                })
                .unwrap(),
            vec![Effect::StageUnchanged(StageId("request-one".into()))]
        );
        assert_eq!(state.last_stage_at_ms, Some(1_000));
    }

    #[test]
    fn a_composer_keeps_the_anchor_it_opened_with() {
        // Pane indices and the current group may change while somebody types.
        // The state stores a complete value instead of resolving either later.
        let mut state = open();
        state
            .apply(Action::Show {
                generation: generation(),
                stage: stage("s1", 10),
                at_ms: 1_000,
                source_matches: true,
            })
            .unwrap();
        let anchor = state.capture(None).unwrap();
        state
            .apply(Action::OpenComposer {
                generation: generation(),
                anchor: anchor.clone(),
            })
            .unwrap();

        assert_eq!(state.composer, Some(anchor));
        assert_eq!(
            state.apply(Action::Show {
                generation: generation(),
                stage: stage("s2", 20),
                at_ms: 3_000,
                source_matches: true,
            }),
            Err(Refusal::Paused(PauseReason::Composer))
        );
    }

    #[test]
    fn a_paused_reader_cannot_be_moved_by_the_agent() {
        let mut state = open();
        state
            .apply(Action::Pause {
                generation: generation(),
                reason: PauseReason::Navigation,
                at_ms: 0,
            })
            .unwrap();

        assert_eq!(
            state.apply(Action::Show {
                generation: generation(),
                stage: stage("s1", 10),
                at_ms: 1_000,
                source_matches: true,
            }),
            Err(Refusal::Paused(PauseReason::Navigation))
        );
        assert!(state.stage.is_none());
    }

    #[test]
    fn opening_gives_movement_back() {
        // The bug a reader actually hit: every exit pauses with
        // `InactiveWindow`, and `Opened` used to set the lifecycle and nothing
        // else — so a deck that had been hidden once could never be followed
        // again without pressing a key nobody knew about.
        let mut state = open();
        state
            .apply(Action::Hidden {
                generation: generation(),
            })
            .unwrap();
        assert_eq!(
            state.following,
            Following::Paused(PauseReason::InactiveWindow)
        );

        let effects = state
            .apply(Action::Opened {
                generation: generation(),
            })
            .unwrap();

        assert_eq!(state.following, Following::Following);
        assert!(effects.contains(&Effect::FollowingResumed));
        state
            .apply(Action::Show {
                generation: generation(),
                stage: stage("s1", 10),
                at_ms: 10_000,
                source_matches: true,
            })
            .expect("a reopened deck can be moved again");
    }

    #[test]
    fn finishing_a_comment_gives_movement_back() {
        // The same class of bug one layer down: closing the composer dropped
        // the anchor and left the pause, so following died on the reader's
        // first comment and never returned.
        let mut state = open();
        state
            .apply(Action::Show {
                generation: generation(),
                stage: stage("s1", 10),
                at_ms: 1_000,
                source_matches: true,
            })
            .unwrap();
        let anchor = state.capture(None).unwrap();
        state
            .apply(Action::OpenComposer {
                generation: generation(),
                anchor,
            })
            .unwrap();
        state
            .apply(Action::CloseComposer {
                generation: generation(),
            })
            .unwrap();

        assert_eq!(state.following, Following::Following);
        state
            .apply(Action::Show {
                generation: generation(),
                stage: stage("s2", 20),
                at_ms: 5_000,
                source_matches: true,
            })
            .expect("movement returns once the comment is finished");
    }

    #[test]
    fn a_hold_lets_go_without_waiting_for_a_show() {
        // The view asks whether the reader is holding the pane before it
        // follows the voice down the file. A lapsed scroll must answer no by
        // itself — nobody is going to send a show request to notice.
        let mut state = open();
        state
            .apply(Action::Pause {
                generation: generation(),
                reason: PauseReason::Navigation,
                at_ms: 1_000,
            })
            .unwrap();
        assert!(state.holds(1_500));
        assert!(!state.holds(1_000 + RESUME_AFTER_MS));

        state
            .apply(Action::Pause {
                generation: generation(),
                reason: PauseReason::Composer,
                at_ms: 1_000,
            })
            .unwrap();
        assert!(state.holds(1_000_000), "a composer holds until it closes");
    }

    #[test]
    fn a_scroll_lapses_but_a_composer_does_not() {
        // Scrolling is something a reader does while watching. It takes
        // priority, then gives it back. A composer owns its anchor until it
        // closes, however long that takes.
        let mut state = open();
        state
            .apply(Action::Pause {
                generation: generation(),
                reason: PauseReason::Selection,
                at_ms: 1_000,
            })
            .unwrap();

        assert_eq!(
            state.apply(Action::Show {
                generation: generation(),
                stage: stage("s1", 10),
                at_ms: 1_500,
                source_matches: true,
            }),
            Err(Refusal::Paused(PauseReason::Selection)),
            "still the reader's, a moment later"
        );
        state
            .apply(Action::Show {
                generation: generation(),
                stage: stage("s1", 10),
                at_ms: 1_000 + RESUME_AFTER_MS,
                source_matches: true,
            })
            .expect("and the agent's again once the reader has been still");
        assert_eq!(state.following, Following::Following);
    }

    #[test]
    fn a_scroll_does_not_cancel_a_composer() {
        // Ordering matters: a lapsing pause must not displace one the reader
        // has to clear by hand, or a stray scroll would hand the agent the
        // anchor somebody is still typing against.
        let mut state = open();
        state
            .apply(Action::Show {
                generation: generation(),
                stage: stage("s1", 10),
                at_ms: 1_000,
                source_matches: true,
            })
            .unwrap();
        let anchor = state.capture(None).unwrap();
        state
            .apply(Action::OpenComposer {
                generation: generation(),
                anchor,
            })
            .unwrap();
        state
            .apply(Action::Pause {
                generation: generation(),
                reason: PauseReason::Navigation,
                at_ms: 1_100,
            })
            .unwrap();

        assert_eq!(
            state.following,
            Following::Paused(PauseReason::Composer),
            "the composer still holds it"
        );
    }

    #[test]
    fn an_old_generation_cannot_change_a_reopened_session() {
        let mut state = open();
        assert_eq!(
            state.apply(Action::Show {
                generation: Generation("old-owner".into()),
                stage: stage("s1", 10),
                at_ms: 1_000,
                source_matches: true,
            }),
            Err(Refusal::StaleGeneration)
        );
        assert!(state.stage.is_none());
    }

    #[test]
    fn hiding_interrupts_audio_and_stops_following() {
        // Audio cannot continue from a bar with no visible explanation, and a
        // hidden session must not accept movement until the reader reopens it.
        let mut state = open();
        let utterance = UtteranceId("u1".into());
        state
            .apply(Action::StartAudio {
                generation: generation(),
                utterance: utterance.clone(),
            })
            .unwrap();
        let effects = state
            .apply(Action::Hidden {
                generation: generation(),
            })
            .unwrap();

        assert_eq!(state.audio, AudioPhase::Interrupted);
        assert_eq!(
            state.following,
            Following::Paused(PauseReason::InactiveWindow)
        );
        assert!(effects.contains(&Effect::AudioChanged {
            utterance,
            phase: AudioPhase::Interrupted,
        }));
    }

    #[test]
    fn stale_audio_completion_cannot_revive_a_cancelled_voice() {
        let mut state = open();
        state
            .apply(Action::StartAudio {
                generation: generation(),
                utterance: UtteranceId("new".into()),
            })
            .unwrap();

        assert_eq!(
            state.apply(Action::AudioChanged {
                generation: generation(),
                utterance: UtteranceId("old".into()),
                phase: AudioPhase::Completed,
            }),
            Err(Refusal::StaleUtterance)
        );
        assert_eq!(state.audio, AudioPhase::Preparing);
    }

    #[test]
    fn source_changes_are_refused_before_the_spotlight_moves() {
        let mut state = open();
        assert_eq!(
            state.apply(Action::Show {
                generation: generation(),
                stage: stage("s1", 10),
                at_ms: 1_000,
                source_matches: false,
            }),
            Err(Refusal::SourceChanged)
        );
        assert!(state.stage.is_none());
    }
}
