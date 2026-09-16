//! The native process's ownership of live control sessions.
//!
//! A session starts with the bar, not with the first deck render, and survives
//! hide/reopen. Filesystem polling stays off the GPUI thread. A visible view
//! takes validated command envelopes from a small in-memory queue, applies
//! them, and returns facts for the owner to acknowledge.

use std::collections::VecDeque;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};

use deck_cli::live::{
    Owner, ReceiveError, Request, RequestBody, Response, ResponseStatus, ShowReceipt,
};
use deck_core::{
    Generation, Lifecycle, LiveAction, LiveEffect, LiveRefusal, LiveState, PauseReason, Stage,
};

struct Control {
    state: Mutex<LiveState>,
    pending: Mutex<VecDeque<ShowCommand>>,
    wake: Mutex<Option<futures::channel::mpsc::UnboundedSender<()>>>,
    publishing: mpsc::Sender<deck_core::Moment>,
    started: Instant,
    stop: AtomicBool,
}

/// A live owner retained by a waiting, visible, or hidden session.
///
/// Clones move with the session while a window is being replaced by the bar.
/// The final handle asks the worker to stop; the worker then drops the advisory
/// lock, so another `deck open` can own this deck without pid-based recovery.
pub struct Handle {
    control: Arc<Control>,
}

impl Clone for Handle {
    fn clone(&self) -> Self {
        Self {
            control: Arc::clone(&self.control),
        }
    }
}

impl Handle {
    /// Start ownership below Deck's normal runtime directory.
    ///
    /// # Errors
    ///
    /// When no home directory exists or the deck cannot be claimed.
    pub fn start(deck: &Path) -> anyhow::Result<Self> {
        let runtime = deck_core::home::deck()
            .ok_or_else(|| anyhow::anyhow!("no home directory for live Deck state"))?;
        Self::start_at(&runtime, deck)
    }

    fn start_at(runtime: &Path, deck: &Path) -> anyhow::Result<Self> {
        let owner = Arc::new(Owner::claim(runtime, deck)?);
        // Publishing used to write and sync event files inside mouse handlers
        // and render. A slow disk must never hold the reader's next frame.
        let (publishing, events) = mpsc::channel();
        let journal = Arc::clone(&owner);
        std::thread::Builder::new()
            .name("deck-live-journal".into())
            .spawn(move || {
                for moment in events {
                    if let Err(error) = journal.publish(moment) {
                        eprintln!("deck: reader event could not be published: {error}");
                    }
                }
            })?;
        let control = Arc::new(Control {
            state: Mutex::new(LiveState::new(owner.generation())),
            pending: Mutex::new(VecDeque::new()),
            wake: Mutex::new(None),
            publishing,
            started: Instant::now(),
            stop: AtomicBool::new(false),
        });
        let worker_control = Arc::clone(&control);
        std::thread::Builder::new()
            .name("deck-live-owner".into())
            .spawn(move || serve(&owner, &worker_control))?;
        Ok(Self { control })
    }

    /// Report that the reader has opened the deck.
    pub fn ready(&self) {
        let generation = self.generation();
        let _ = self.state().apply(LiveAction::Opened { generation });
    }

    /// Report that the reader put the deck back on the bar.
    pub fn hidden(&self) {
        let generation = self.generation();
        let _ = self.state().apply(LiveAction::Hidden { generation });
    }

    /// Give reader navigation priority over later show requests.
    ///
    /// Scrolling and selecting lapse on their own; the clock is this owner's,
    /// the same one `apply_stage` paces against.
    pub fn pause(&self, reason: PauseReason) {
        let generation = self.generation();
        let at_ms = self.now_ms();
        let _ = self.state().apply(LiveAction::Pause {
            generation,
            reason,
            at_ms,
        });
    }

    /// Tell whoever is listening what the reader just did.
    ///
    /// Best effort on purpose. The moment is already in the review's transcript
    /// by the time this runs, so a failure here costs the agent a nudge and
    /// costs the reader nothing — and a full disk must not stop somebody
    /// reacting to a deck.
    pub fn publish(&self, moment: deck_core::Moment) {
        let _ = self.control.publishing.send(moment);
    }

    /// Whether the agent may currently move the reader, for the indicator.
    #[must_use]
    pub fn following(&self) -> deck_core::Following {
        self.state().following
    }

    /// Rebuilding a window must restore the applied spotlight, or an unchanged
    /// retry would acknowledge evidence the replacement pane is not showing.
    pub fn stage(&self) -> Option<Stage> {
        self.state().stage.clone()
    }

    fn now_ms(&self) -> u64 {
        u64::try_from(self.control.started.elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    /// The composer is finished with the anchor it was holding.
    ///
    /// A composer pause does not lapse — nobody should have the ground move
    /// while they are writing about it — so it has to be handed back
    /// explicitly. Without this, opening one composer paused the agent for the
    /// rest of the session and every `show` came back refused.
    pub fn composer_closed(&self) {
        let generation = self.generation();
        let _ = self.state().apply(LiveAction::CloseComposer { generation });
    }

    /// Return movement to the live driver when the reader asks explicitly.
    pub fn follow(&self) {
        let generation = self.generation();
        let _ = self.state().apply(LiveAction::Resume { generation });
    }

    /// Wake a visible view only when there is a command to apply.
    ///
    /// Replacing the subscription on reopen avoids retaining a hidden view.
    /// The pending check covers a command arriving before the view subscribed.
    pub fn subscribe(&self) -> futures::channel::mpsc::UnboundedReceiver<()> {
        let (send, receive) = futures::channel::mpsc::unbounded();
        *self.control.wake.lock().unwrap_or_else(|p| p.into_inner()) = Some(send.clone());
        if !self
            .control
            .pending
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .is_empty()
        {
            let _ = send.unbounded_send(());
        }
        receive
    }

    /// Take the next show command for the visible GPUI view.
    pub fn next_show(&self) -> Option<ShowCommand> {
        self.control
            .pending
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .pop_front()
    }

    /// Ask the pure reducer whether a resolved stage may move now.
    pub fn apply_stage(
        &self,
        stage: Stage,
        source_matches: bool,
    ) -> Result<Vec<LiveEffect>, LiveRefusal> {
        let generation = self.generation();
        let at_ms = self.now_ms();
        self.state().apply(LiveAction::Show {
            generation,
            stage,
            at_ms,
            source_matches,
        })
    }

    fn generation(&self) -> Generation {
        self.control
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .generation
            .clone()
    }

    fn state(&self) -> std::sync::MutexGuard<'_, LiveState> {
        self.control
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        // The worker owns the other Arc. More than two means another Session or
        // DeckView still carries the owner through a hide/reopen handoff.
        if Arc::strong_count(&self.control) == 2 {
            self.control.stop.store(true, Ordering::Release);
        }
    }
}

/// A show request waiting for the visible view to resolve and paint it.
pub struct ShowCommand {
    request: Request,
    answer: mpsc::SyncSender<ShowAnswer>,
    cancelled: Arc<AtomicBool>,
}

impl ShowCommand {
    /// The stable request identity, also used to identify its resulting stage.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.request.id
    }

    /// The file, range, optional group, and optional pane requested.
    #[must_use]
    pub fn target(&self) -> Option<(&Path, deck_core::LineRange, Option<&str>, Option<&str>)> {
        match &self.request.body {
            RequestBody::Show {
                file,
                range,
                group,
                pane,
                ..
            } => Some((file, *range, group.as_deref(), pane.as_deref())),
            RequestBody::Say { .. }
            | RequestBody::Doing { .. }
            | RequestBody::Clear
            | RequestBody::Fold { .. }
            | RequestBody::Bring { .. }
            | RequestBody::Status => None,
        }
    }

    /// What the agent wants the reader to know it is doing right now.
    #[must_use]
    pub fn doing(&self) -> Option<&str> {
        match &self.request.body {
            RequestBody::Doing { text } => Some(text),
            _ => None,
        }
    }

    /// What the agent wants said, when this is a `say` rather than a `show`.
    #[must_use]
    pub fn saying(&self) -> Option<(&str, bool)> {
        match &self.request.body {
            RequestBody::Say { text, aloud } => Some((text, *aloud)),
            _ => None,
        }
    }

    /// What the shown range is being proposed to become, if anything.
    #[must_use]
    pub fn proposing(&self) -> Option<&str> {
        match &self.request.body {
            RequestBody::Show { after, .. } => after.as_deref(),
            _ => None,
        }
    }

    /// What this asks to be folded away, or opened again.
    #[must_use]
    pub fn folding(&self) -> Option<(&[String], bool, bool)> {
        match &self.request.body {
            RequestBody::Fold { panes, group, open } => Some((panes, *group, *open)),
            _ => None,
        }
    }

    /// The file this asks to be brought into the room, and what it displaces.
    #[must_use]
    pub fn bringing(&self) -> Option<&RequestBody> {
        matches!(self.request.body, RequestBody::Bring { .. }).then_some(&self.request.body)
    }

    /// Whether this asks to stop pointing.
    #[must_use]
    pub fn clearing(&self) -> bool {
        matches!(self.request.body, RequestBody::Clear)
    }

    /// Whether its caller has already withdrawn an unapplied movement.
    #[must_use]
    pub fn expired(&self) -> bool {
        self.cancelled.load(Ordering::Acquire) || self.request.expired()
    }

    /// Return the applied or refused result to the mailbox worker.
    pub fn finish(self, answer: ShowAnswer) {
        let _ = self.answer.send(answer);
    }
}

/// What the view learned while resolving and applying a show request.
pub struct ShowAnswer {
    status: ResponseStatus,
    reason: Option<String>,
    retry_after_ms: Option<u64>,
    show: Option<Box<ShowReceipt>>,
}

impl ShowAnswer {
    /// A stage was painted, or was already the current stage.
    #[must_use]
    pub fn shown(stage: Stage, changed: bool) -> Self {
        Self {
            status: if changed {
                ResponseStatus::Applied
            } else {
                ResponseStatus::Unchanged
            },
            reason: None,
            retry_after_ms: None,
            show: Some(Box::new(ShowReceipt {
                stage,
                applied: true,
                // Applying updates the model immediately; whether the next
                // frame laid it out is a different fact and remains unknown.
                layout_confirmed: false,
                in_view: None,
            })),
        }
    }

    /// The agent's words landed in the walk.
    #[must_use]
    pub fn said() -> Self {
        Self {
            status: ResponseStatus::Applied,
            reason: None,
            retry_after_ms: None,
            show: None,
        }
    }

    /// A target was refused without moving the reader.
    #[must_use]
    pub fn refused(status: ResponseStatus, reason: impl Into<String>) -> Self {
        Self {
            status,
            reason: Some(reason.into()),
            retry_after_ms: None,
            show: None,
        }
    }

    /// A target arrived before the dwell interval had elapsed.
    #[must_use]
    pub fn paced(retry_after_ms: u64) -> Self {
        Self {
            status: ResponseStatus::Paced,
            reason: Some("a different stage moved less than one second ago".into()),
            retry_after_ms: Some(retry_after_ms),
            show: None,
        }
    }
}

fn serve(owner: &Owner, control: &Control) {
    const EVERY: Duration = Duration::from_millis(50);

    while !control.stop.load(Ordering::Acquire) {
        let request = match owner.receive(EVERY) {
            Ok(request) => request,
            Err(ReceiveError::Timeout) => continue,
            Err(ReceiveError::Io(error)) => {
                eprintln!("deck: live control stopped: {error}");
                return;
            }
        };

        match request.body {
            RequestBody::Status => {
                let status = status_of(&control.state);
                let _ = owner.acknowledge(&request, status);
            }
            RequestBody::Say { .. }
            | RequestBody::Doing { .. }
            | RequestBody::Show { .. }
            | RequestBody::Clear
            | RequestBody::Fold { .. }
            | RequestBody::Bring { .. } => {
                let status = status_of(&control.state);
                if status != ResponseStatus::Ready {
                    let mut response = Response::status(&request, owner.generation(), status);
                    response.reason = Some(match status {
                        ResponseStatus::Waiting => "the reader has not opened the deck".into(),
                        ResponseStatus::Hidden => "the reader hid the deck".into(),
                        _ => "the deck cannot currently be moved".into(),
                    });
                    let _ = owner.acknowledge_with(response);
                    continue;
                }

                let (send, receive) = mpsc::sync_channel(1);
                let cancelled = Arc::new(AtomicBool::new(false));
                control
                    .pending
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .push_back(ShowCommand {
                        request: request.clone(),
                        answer: send,
                        cancelled: Arc::clone(&cancelled),
                    });
                if let Some(wake) = control
                    .wake
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .as_ref()
                {
                    let _ = wake.unbounded_send(());
                }
                // Hiding can destroy the view before it takes this command.
                // Do not hold ownership until the caller's deadline, or let a
                // refused command replay against a subsequently reopened view.
                let answer = loop {
                    // An applied result remains true even if the reader hides
                    // immediately afterwards. Prefer that fact over lifecycle.
                    if let Ok(answer) = receive.try_recv() {
                        break answer;
                    }
                    let status = status_of(&control.state);
                    if status != ResponseStatus::Ready || control.stop.load(Ordering::Acquire) {
                        cancelled.store(true, Ordering::Release);
                        break ShowAnswer::refused(
                            ResponseStatus::Hidden,
                            "the reader hid or closed the deck",
                        );
                    }
                    let Some(left) = request.remaining() else {
                        cancelled.store(true, Ordering::Release);
                        break ShowAnswer::refused(
                            ResponseStatus::NotFound,
                            "the command expired before application",
                        );
                    };
                    match receive.recv_timeout(left.min(Duration::from_millis(20))) {
                        Ok(answer) => break answer,
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                        Err(mpsc::RecvTimeoutError::Disconnected) => {
                            break ShowAnswer::refused(
                                ResponseStatus::Hidden,
                                "the view was closed",
                            );
                        }
                    }
                };
                let response = Response {
                    request_id: request.id.clone(),
                    generation: owner.generation(),
                    request_hash: {
                        // Response::status owns the transport's canonical hash
                        // calculation, so start there and retain its result.
                        Response::status(&request, owner.generation(), answer.status).request_hash
                    },
                    status: answer.status,
                    reason: answer.reason,
                    retry_after_ms: answer.retry_after_ms,
                    show: answer.show.map(|receipt| *receipt),
                };
                let _ = owner.acknowledge_with(response);
            }
        }
    }
}

fn status_of(state: &Mutex<LiveState>) -> ResponseStatus {
    match state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .lifecycle
    {
        Lifecycle::Waiting => ResponseStatus::Waiting,
        Lifecycle::Open => ResponseStatus::Ready,
        Lifecycle::Hidden => ResponseStatus::Hidden,
        Lifecycle::Submitted | Lifecycle::Closed => ResponseStatus::Hidden,
    }
}

/// Turn reducer refusals into stable command outcomes.
#[must_use]
pub fn refusal(error: LiveRefusal) -> ShowAnswer {
    match error {
        LiveRefusal::Paused(reason) => ShowAnswer::refused(
            ResponseStatus::Paused,
            format!("reader movement has priority: {reason:?}"),
        ),
        LiveRefusal::Paced { retry_after_ms } => ShowAnswer::paced(retry_after_ms),
        LiveRefusal::SourceChanged => ShowAnswer::refused(
            ResponseStatus::SourceChanged,
            "source changed after it was shown",
        ),
        LiveRefusal::NotVisible => {
            ShowAnswer::refused(ResponseStatus::Hidden, "the deck is not visible")
        }
        LiveRefusal::ComposerOwnsAnchor => ShowAnswer::refused(
            ResponseStatus::Paused,
            "the reader is writing a comment on the current target",
        ),
        LiveRefusal::StaleGeneration => {
            ShowAnswer::refused(ResponseStatus::NotFound, "the live session was replaced")
        }
        LiveRefusal::Busy | LiveRefusal::StaleUtterance => {
            ShowAnswer::refused(ResponseStatus::Paused, "the live session is busy")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use deck_cli::live::{Client, RequestBody};

    fn deck(root: &std::path::Path) -> std::path::PathBuf {
        let deck = root.join("d-live.deck");
        std::fs::create_dir_all(&deck).unwrap();
        std::fs::write(
            deck.join("deck.json"),
            r#"{"v":1,"id":"d-live","title":"test"}"#,
        )
        .unwrap();
        deck
    }

    fn status(client: &Client) -> ResponseStatus {
        client
            .request(RequestBody::Status, Duration::from_secs(1))
            .unwrap()
            .status
    }

    #[test]
    fn status_follows_the_session_across_open_hide_and_reopen() {
        // The owner belongs to the session, not a short-lived DeckView. Hiding
        // changes the answer without dropping the mailbox or its generation.
        let temp = tempfile::tempdir().unwrap();
        let runtime = temp.path().join("runtime");
        let deck = deck(temp.path());
        let owner = Handle::start_at(&runtime, &deck).unwrap();
        let client = Client::connect(&runtime, &deck).unwrap();

        assert_eq!(status(&client), ResponseStatus::Waiting);
        owner.ready();
        assert_eq!(status(&client), ResponseStatus::Ready);
        owner.hidden();
        assert_eq!(status(&client), ResponseStatus::Hidden);
        owner.ready();
        assert_eq!(status(&client), ResponseStatus::Ready);
    }

    #[test]
    fn a_hidden_show_is_refused_without_waiting_for_a_view() {
        let temp = tempfile::tempdir().unwrap();
        let runtime = temp.path().join("runtime");
        let deck = deck(temp.path());
        let owner = Handle::start_at(&runtime, &deck).unwrap();
        let client = Client::connect(&runtime, &deck).unwrap();
        owner.ready();
        owner.hidden();

        let response = client
            .request(
                RequestBody::Show {
                    file: "src/lib.rs".into(),
                    range: deck_core::LineRange::single(1),
                    group: None,
                    pane: None,
                },
                Duration::from_secs(1),
            )
            .unwrap();
        assert_eq!(response.status, ResponseStatus::Hidden);
        assert!(owner.next_show().is_none());
    }

    #[test]
    fn a_visible_show_is_acknowledged_only_after_the_view_applies_it() {
        let temp = tempfile::tempdir().unwrap();
        let runtime = temp.path().join("runtime");
        let deck = deck(temp.path());
        let owner = Handle::start_at(&runtime, &deck).unwrap();
        let client = Client::connect(&runtime, &deck).unwrap();
        owner.ready();

        std::thread::scope(|scope| {
            let caller = scope.spawn(|| {
                client
                    .request(
                        RequestBody::Show {
                            file: "src/lib.rs".into(),
                            range: deck_core::LineRange::single(3),
                            group: None,
                            pane: None,
                        },
                        Duration::from_secs(1),
                    )
                    .unwrap()
            });
            let until = Instant::now() + Duration::from_secs(1);
            let command = loop {
                if let Some(command) = owner.next_show() {
                    break command;
                }
                assert!(Instant::now() < until);
                std::thread::sleep(Duration::from_millis(5));
            };
            let stage = Stage {
                id: deck_core::StageId(command.id().into()),
                group: "g1".into(),
                ref_id: Some("g1r1".into()),
                file: Some("src/lib.rs".into()),
                range: Some(deck_core::LineRange::single(3)),
                quote: "line three".into(),
                snapshot: Some("snapshot:one".into()),
            };
            let effects = owner.apply_stage(stage.clone(), true).unwrap();
            assert!(effects.contains(&LiveEffect::StageApplied(stage.clone())));
            command.finish(ShowAnswer::shown(stage, true));

            let response = caller.join().unwrap();
            assert_eq!(response.status, ResponseStatus::Applied);
            assert!(response.show.is_some_and(|show| show.applied));
        });
    }

    #[test]
    fn hiding_with_a_command_in_flight_refuses_it_and_never_replays_it() {
        let temp = tempfile::tempdir().unwrap();
        let runtime = temp.path().join("runtime");
        let deck = deck(temp.path());
        let owner = Handle::start_at(&runtime, &deck).unwrap();
        let client = Client::connect(&runtime, &deck).unwrap();
        owner.ready();
        let mut wake = owner.subscribe();
        std::thread::scope(|scope| {
            let caller = scope.spawn(|| {
                client
                    .request(
                        RequestBody::Say {
                            text: "do not replay me".into(),
                            aloud: false,
                        },
                        Duration::from_secs(5),
                    )
                    .unwrap()
            });
            assert_eq!(
                futures::executor::block_on(futures::StreamExt::next(&mut wake)),
                Some(())
            );
            owner.hidden();
            assert_eq!(caller.join().unwrap().status, ResponseStatus::Hidden);
            owner.ready();
            assert!(owner.next_show().unwrap().expired());
        });
    }

    #[test]
    fn reader_events_survive_hide_and_reopen_in_order() {
        let temp = tempfile::tempdir().unwrap();
        let runtime = temp.path().join("runtime");
        let deck = deck(temp.path());
        let owner = Handle::start_at(&runtime, &deck).unwrap();
        let client = Client::connect(&runtime, &deck).unwrap();
        owner.ready();
        for text in ["before hiding", "after reopening"] {
            owner.publish(deck_core::Moment {
                at_ms: 1,
                what: deck_core::What::Wrote,
                group: Some("g1".into()),
                ref_id: None,
                file: None,
                range: None,
                text: text.into(),
                kind: Some(deck_core::Kind::Question),
                when: deck_core::When::Queue,
            });
            owner.hidden();
            owner.ready();
        }
        let first = client
            .events(None, Duration::from_secs(1))
            .unwrap()
            .unwrap();
        let second = client
            .events(Some(first.seq), Duration::from_secs(1))
            .unwrap()
            .unwrap();
        assert_eq!(first.moment.text, "before hiding");
        assert_eq!(second.moment.text, "after reopening");
    }

    #[test]
    fn a_note_about_what_the_agent_is_doing_reaches_the_visible_view() {
        // It goes down the same queue as a turn, because it has to arrive in
        // the order the agent sent it relative to what it says — but it is a
        // separate request so the view can tell the two apart.
        let temp = tempfile::tempdir().unwrap();
        let runtime = temp.path().join("runtime");
        let deck = deck(temp.path());
        let owner = Handle::start_at(&runtime, &deck).unwrap();
        let client = Client::connect(&runtime, &deck).unwrap();
        owner.ready();

        std::thread::scope(|scope| {
            let caller = scope.spawn(|| {
                client
                    .request(
                        RequestBody::Doing {
                            text: "reading the retry loop".into(),
                        },
                        Duration::from_secs(5),
                    )
                    .unwrap()
            });
            let until = Instant::now() + Duration::from_secs(5);
            let command = loop {
                if let Some(command) = owner.next_show() {
                    break command;
                }
                assert!(Instant::now() < until, "the note never arrived");
                std::thread::sleep(Duration::from_millis(5));
            };
            assert_eq!(command.doing(), Some("reading the retry loop"));
            assert!(command.saying().is_none(), "and it is not a turn");
            command.finish(ShowAnswer::said());
            assert_eq!(caller.join().unwrap().status, ResponseStatus::Applied);
        });
    }

    #[test]
    fn the_final_session_handle_releases_ownership() {
        let temp = tempfile::tempdir().unwrap();
        let runtime = temp.path().join("runtime");
        let deck = deck(temp.path());
        let owner = Handle::start_at(&runtime, &deck).unwrap();
        let kept = owner.clone();
        drop(owner);
        assert!(Client::connect(&runtime, &deck).is_ok());
        drop(kept);

        let until = std::time::Instant::now() + Duration::from_secs(1);
        while Client::connect(&runtime, &deck).is_ok() && std::time::Instant::now() < until {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(Client::connect(&runtime, &deck).is_err());
    }
}
