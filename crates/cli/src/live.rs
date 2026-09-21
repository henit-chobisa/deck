//! Local transport for one live Deck owner and its shell commands.
//!
//! The mailbox is deliberately file-backed: it is inspectable, works wherever
//! `deck open` already works, and does not introduce a platform-specific socket
//! before the live loop has proved useful. Ownership still comes from an OS
//! advisory lock held by an open handle; metadata such as a pid is never used
//! to guess whether an owner is alive.

use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering as AtomicOrdering;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use deck_core::Moment;
use deck_core::protocol::Header;
use deck_core::{Generation, LineRange, Stage};
use fs2::FileExt as _;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

/// Largest control document accepted from the local mailbox.
const MAX_MESSAGE_BYTES: u64 = 1024 * 1024;
/// Polling is only a wake-up mechanism; committed files remain the authority.
const POLL_EVERY: Duration = Duration::from_millis(20);

/// One typed request crossing the local owner mailbox.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "type")]
pub enum RequestBody {
    /// Report whether the owning session is ready to accept live commands.
    Status,
    /// Move the spotlight to source already represented in an authored group.
    Show {
        /// File as the command named it, resolved by the owning deck.
        file: PathBuf,
        /// One-based inclusive source lines.
        range: LineRange,
        /// Authored group to select, or the currently visible group.
        #[serde(skip_serializing_if = "Option::is_none")]
        group: Option<String>,
        /// Ref id used to disambiguate repeated panes for one file.
        #[serde(skip_serializing_if = "Option::is_none")]
        pane: Option<String>,
        /// What the range should become, drawn as a change rather than a
        /// highlight.
        ///
        /// The live half of an authored `--after`: an answer that proposes
        /// code can put the code on the screen instead of describing it. The
        /// file on disk is never touched.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        after: Option<String>,
    },
    /// Say something to the reader, beside whatever is currently on screen.
    ///
    /// Recorded in the transcript whether or not a voice is configured, because
    /// what the agent said is part of the walk even when nobody heard it.
    Say {
        /// Markdown, as a group's `say` is.
        text: String,
        /// Whether to read it aloud, when the reader has a voice set up.
        aloud: bool,
    },
    /// Say what the agent is doing while it is doing it.
    ///
    /// Not a turn and not a transcript entry: a line the panel shows in place
    /// of its own guess about the silence, replaced by the next one and thrown
    /// away when the answer lands. It is also the only proof the window has
    /// that an agent is still there, so it is what the wait is measured from.
    Doing {
        /// A few words in the present tense — `reading the retry loop`.
        text: String,
    },
    /// Stop pointing.
    ///
    /// The frame round the pane and the lit lines inside it go out. Nothing
    /// scrolls, and nothing being said stops: this is the agent taking its hand
    /// off the page, not asking for quiet.
    Clear,
    /// Fold panes away to their spines, or open them again.
    ///
    /// Folding is how the room is made for something else. A pane folded is
    /// still there — its name on a spine at the window's edge, a click from
    /// coming back — which is the difference between this and closing it.
    Fold {
        /// The panes to fold, by the name the prose calls them.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        panes: Vec<String>,
        /// Fold every pane of the group, rather than named ones.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        group: bool,
        /// Open them instead of folding them.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        open: bool,
    },
    /// Bring a file into the room that the group never showed.
    ///
    /// The answer to a question the deck was not written for. What it displaces
    /// folds in the same movement, so the room moves once: two commands would
    /// be a fold, a frame of wrong layout, and then a pane arriving.
    ///
    /// It is temporary. Turning to another group takes it away, and so does
    /// closing it.
    Bring {
        /// The file to open, as the command named it.
        file: PathBuf,
        /// One-based inclusive source lines.
        range: LineRange,
        /// What the prose may call it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        /// A short label for the pane.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        note: Option<String>,
        /// What the range should become, drawn as a change.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        after: Option<String>,
        /// Fold the whole group behind one spine to make room for it.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        fold_group: bool,
        /// Or fold only these panes of it, by name.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        fold: Vec<String>,
    },
    /// Put a page in the room that no group in this deck carries.
    ///
    /// `Bring` for markup. The reader asks about something that only makes
    /// sense moving — *and what happens when you delete one?* — and the answer
    /// is written there and then rather than described.
    Render {
        /// The whole document, as the agent wrote it.
        html: String,
        /// What the prose may call it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        /// A short label for the pane.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        note: Option<String>,
        /// Fold the whole group behind one spine to make room for it.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        fold_group: bool,
        /// Or fold only these panes of it, by name.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        fold: Vec<String>,
    },
    /// Draw a picture that no group in this deck carries.
    ///
    /// `Bring` for diagrams. A reader who asks how a request travels is asking
    /// for something that is in no file — and answering it by naming four files
    /// in turn is the failure a picture exists to prevent.
    Draw {
        /// The whole picture, as the agent wrote it.
        diagram: deck_core::diagram::Diagram,
        /// What the prose may call it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        name: Option<String>,
        /// A short label for the pane.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        note: Option<String>,
        /// Fold the whole group behind one spine to make room for it.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        fold_group: bool,
        /// Or fold only these panes of it, by name.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        fold: Vec<String>,
    },
}

/// A request committed by a shell command for the native owner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Request {
    /// Idempotency identity for this command attempt.
    pub id: String,
    /// Ownership generation observed when the client connected.
    pub generation: Generation,
    /// Wall-clock deadline after which an unapplied request must be ignored.
    pub deadline_ms: u64,
    /// The typed operation requested.
    pub body: RequestBody,
}

impl Request {
    /// Time left before an unapplied request must be withdrawn.
    #[must_use]
    pub fn remaining(&self) -> Option<Duration> {
        let left = self.deadline_ms.saturating_sub(now_ms());
        (left > 0).then(|| Duration::from_millis(left))
    }

    /// Whether applying this later would surprise a caller that already left.
    #[must_use]
    pub fn expired(&self) -> bool {
        self.remaining().is_none()
    }
}

/// Result categories returned by live control.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ResponseStatus {
    /// The bar exists, but the reader has not opened the deck.
    Waiting,
    /// The deck is visible and can accept reader-facing commands.
    Ready,
    /// The reader put the deck back on the bar.
    Hidden,
    /// A new stage was painted and revealed.
    Applied,
    /// The requested stage was already current.
    Unchanged,
    /// The target names more than one pane and needs `--pane`.
    Ambiguous,
    /// The selected group, pane, or file is not represented.
    NotFound,
    /// The requested range is outside the displayed snapshot.
    OutOfRange,
    /// Disk no longer matches the snapshot on screen.
    SourceChanged,
    /// Reader activity currently owns movement.
    Paused,
    /// A different target moved too recently.
    Paced,
}

/// What the native view confirms about an applied spotlight.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShowReceipt {
    /// Complete resolved stage and captured evidence.
    pub stage: Stage,
    /// The stage was applied to the view model.
    pub applied: bool,
    /// A later frame has confirmed layout for the target.
    pub layout_confirmed: bool,
    /// Whether the target was known to be in the viewport.
    pub in_view: Option<bool>,
}

/// An acknowledgement committed by the owner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Response {
    /// The request this answers.
    pub request_id: String,
    /// The owner generation that answered it.
    pub generation: Generation,
    /// Digest of the request body, for idempotent retry conflict checks.
    pub request_hash: String,
    /// The typed result.
    pub status: ResponseStatus,
    /// Stable explanation for a refusal or failure.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Pacing advice for a deliberate retry.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after_ms: Option<u64>,
    /// Resolved evidence for a successful show request.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show: Option<ShowReceipt>,
}

impl Response {
    /// Build a response with no show-specific details.
    #[must_use]
    pub fn status(request: &Request, generation: Generation, status: ResponseStatus) -> Self {
        Self {
            request_id: request.id.clone(),
            generation,
            request_hash: body_hash(&request.body),
            status,
            reason: None,
            retry_after_ms: None,
            show: None,
        }
    }
}

/// Why a deck could not become the live owner.
#[derive(Debug)]
pub enum ClaimError {
    /// Another open handle already holds the advisory lock.
    AlreadyOwned,
    /// The deck path or header is not a readable deck.
    InvalidDeck(String),
    /// Runtime state could not be created or written.
    Io(std::io::Error),
}

impl std::fmt::Display for ClaimError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AlreadyOwned => write!(f, "this deck already has a live owner"),
            Self::InvalidDeck(message) => f.write_str(message),
            Self::Io(error) => write!(f, "live runtime state could not be created: {error}"),
        }
    }
}

impl std::error::Error for ClaimError {}

impl From<std::io::Error> for ClaimError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

/// Why an owner did not receive a valid request.
#[derive(Debug)]
pub enum ReceiveError {
    /// No committed request arrived before the caller's bound.
    Timeout,
    /// Runtime state could not be read or moved.
    Io(std::io::Error),
}

impl std::fmt::Display for ReceiveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Timeout => write!(f, "no live request arrived before the timeout"),
            Self::Io(error) => write!(f, "the live mailbox could not be read: {error}"),
        }
    }
}

impl std::error::Error for ReceiveError {}

impl From<std::io::Error> for ReceiveError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

/// Why a shell client could not complete an acknowledged request.
#[derive(Debug)]
pub enum ClientError {
    /// No owner metadata exists or it could not be read consistently.
    NotOpen(String),
    /// The owner did not acknowledge before the command's bound.
    Timeout,
    /// A request id was reused for different content.
    Conflict,
    /// Runtime state could not be read or written.
    Io(std::io::Error),
}

impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotOpen(message) => f.write_str(message),
            Self::Timeout => write!(f, "the live owner did not acknowledge before the timeout"),
            Self::Conflict => write!(f, "the request id was already used for different content"),
            Self::Io(error) => write!(f, "the live mailbox could not be used: {error}"),
        }
    }
}

impl std::error::Error for ClientError {}

impl From<std::io::Error> for ClientError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct OwnerMetadata {
    deck: PathBuf,
    deck_id: String,
    generation: Generation,
    control_v: u32,
    pid: u32,
}

/// The native process's exclusive ownership of one canonical deck.
///
/// Dropping this value closes the lock handle. A crash does the same in the OS,
/// so no stale-pid recovery or process-reuse guess is needed.
/// One thing the reader did, as the agent will read it.
///
/// The body is a [`Moment`] — the same type the review's transcript is made of,
/// because they are the same fact seen at two times. What the agent is told
/// while the walk is happening is exactly what the review will say afterwards,
/// so the two can never disagree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    /// Position in this generation's stream. The cursor `deck next` returns.
    pub seq: u64,
    /// The ownership this event belongs to.
    pub generation: Generation,
    /// What happened.
    #[serde(flatten)]
    pub moment: Moment,
}

/// The native window's ownership of one deck's live control mailbox.
///
/// Holds the advisory lock that makes this process the one allowed to answer,
/// and the generation every request and event is stamped with.
pub struct Owner {
    generation: Generation,
    root: PathBuf,
    requests: PathBuf,
    processing: PathBuf,
    replies: PathBuf,
    _lock: File,
    seen: std::sync::Mutex<HashSet<String>>,
    /// How many events this generation has published.
    published: std::sync::atomic::AtomicU64,
}

impl Owner {
    /// Claim `deck` and create a fresh generation mailbox below `runtime`.
    ///
    /// # Errors
    ///
    /// When the deck is invalid, runtime state cannot be created, or another
    /// handle already owns the canonical deck.
    pub fn claim(runtime: &Path, deck: &Path) -> Result<Self, ClaimError> {
        let canonical = std::fs::canonicalize(deck).map_err(|error| {
            ClaimError::InvalidDeck(format!("cannot resolve {}: {error}", deck.display()))
        })?;
        let header = read_header(&canonical).map_err(ClaimError::InvalidDeck)?;
        let root = runtime.join("live").join(deck_key(&canonical));
        private_dir(&root)?;

        let lock_path = root.join("owner.lock");
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&lock_path)?;
        private_file(&lock_path)?;
        if lock.try_lock_exclusive().is_err() {
            return Err(ClaimError::AlreadyOwned);
        }

        let generation = Generation(uuid::Uuid::new_v4().to_string());
        let session = root.join(&generation.0);
        let requests = session.join("requests");
        let processing = session.join("processing");
        let replies = session.join("replies");
        for directory in [
            &session,
            &requests,
            &processing,
            &replies,
            &session.join("events"),
        ] {
            private_dir(directory)?;
        }

        let metadata = OwnerMetadata {
            deck: canonical,
            deck_id: header.id,
            generation: generation.clone(),
            control_v: 1,
            pid: std::process::id(),
        };
        // A crashed owner can leave metadata behind, but cannot retain its OS
        // lock. Replacing it only after this owner holds the lock avoids using
        // pid liveness as authority. Windows rename does not replace a file.
        let owner_file = root.join("owner.json");
        let _ = std::fs::remove_file(&owner_file);
        write_json(&owner_file, &metadata)?;

        Ok(Self {
            published: std::sync::atomic::AtomicU64::new(0),
            generation,
            root,
            requests,
            processing,
            replies,
            _lock: lock,
            seen: std::sync::Mutex::new(HashSet::new()),
        })
    }

    /// The generation accepted by this owner.
    #[must_use]
    pub fn generation(&self) -> Generation {
        self.generation.clone()
    }

    /// The generation directory, exposed for diagnostics and integration tests.
    #[must_use]
    pub fn session_dir(&self) -> PathBuf {
        self.root.join(&self.generation.0)
    }

    /// Wait for the next committed, unexpired request for this generation.
    ///
    /// Malformed, oversized, duplicate, and expired files are quarantined by
    /// moving them out of the request directory. One bad local writer therefore
    /// cannot wedge every later command.
    ///
    /// # Errors
    ///
    /// When no request arrives before `timeout`, or the mailbox cannot be read.
    pub fn receive(&self, timeout: Duration) -> Result<Request, ReceiveError> {
        let until = Instant::now() + timeout;
        loop {
            let mut entries = std::fs::read_dir(&self.requests)?
                .filter_map(Result::ok)
                .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "json"))
                .collect::<Vec<_>>();
            entries.sort_by_key(std::fs::DirEntry::file_name);

            for entry in entries {
                let path = entry.path();
                let Some(name) = path.file_name() else {
                    continue;
                };
                let claimed = self.processing.join(name);
                if std::fs::rename(&path, &claimed).is_err() {
                    continue;
                }
                let request = match read_bounded::<Request>(&claimed) {
                    Ok(request) => request,
                    Err(_) => continue,
                };
                let expected_name = format!("{}.json", request.id);
                if uuid::Uuid::parse_str(&request.id).is_err()
                    || name != std::ffi::OsStr::new(&expected_name)
                    || request.generation != self.generation
                    || request.deadline_ms <= now_ms()
                {
                    continue;
                }
                let mut seen = self
                    .seen
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                if !seen.insert(request.id.clone()) {
                    continue;
                }
                return Ok(request);
            }

            if Instant::now() >= until {
                return Err(ReceiveError::Timeout);
            }
            std::thread::sleep(POLL_EVERY.min(until.saturating_duration_since(Instant::now())));
        }
    }

    /// Publish an acknowledgement for a received request.
    ///
    /// # Errors
    ///
    /// When the reply cannot be committed atomically.
    pub fn acknowledge(
        &self,
        request: &Request,
        status: ResponseStatus,
    ) -> Result<(), std::io::Error> {
        self.acknowledge_with(Response::status(request, self.generation.clone(), status))
    }

    /// Publish a fully detailed acknowledgement.
    ///
    /// # Errors
    ///
    /// When the reply cannot be committed atomically.
    /// Tell whoever is listening what the reader just did.
    ///
    /// Append-only, one file per event, named by its sequence so the directory
    /// itself carries the order and no index has to be kept consistent with it.
    /// Failure is swallowed by callers on purpose: a reader's reaction is
    /// recorded in the review regardless, and a full disk should not stop them
    /// walking the deck.
    ///
    /// # Errors
    ///
    /// When the event cannot be written.
    pub fn publish(&self, moment: Moment) -> Result<u64, std::io::Error> {
        let seq = self.published.fetch_add(1, AtomicOrdering::AcqRel) + 1;
        let event = Event {
            seq,
            generation: self.generation.clone(),
            moment,
        };
        write_json(
            &self.session_dir().join("events").join(name_of(seq)),
            &event,
        )?;
        Ok(seq)
    }

    /// Commit a fully formed response, including its receipt and retry advice.
    ///
    /// # Errors
    ///
    /// When the reply cannot be written.
    pub fn acknowledge_with(&self, response: Response) -> Result<(), std::io::Error> {
        write_json(
            &self.replies.join(format!("{}.json", response.request_id)),
            &response,
        )?;
        let request_id = response.request_id;
        let _ = std::fs::remove_file(self.processing.join(format!("{request_id}.json")));
        Ok(())
    }
}

impl Drop for Owner {
    fn drop(&mut self) {
        let owner_file = self.root.join("owner.json");
        if read_bounded::<OwnerMetadata>(&owner_file)
            .is_ok_and(|metadata| metadata.generation == self.generation)
        {
            let _ = std::fs::remove_file(owner_file);
        }
    }
}

/// A shell command connected to the owner generation it observed.
pub struct Client {
    generation: Generation,
    /// This generation's directory, which the event stream lives under.
    session: PathBuf,
    requests: PathBuf,
    processing: PathBuf,
    replies: PathBuf,
}

impl Client {
    /// Connect to the current owner of `deck`.
    ///
    /// # Errors
    ///
    /// When the path is not a deck or no readable owner metadata exists.
    pub fn connect(runtime: &Path, deck: &Path) -> Result<Self, ClientError> {
        let canonical = std::fs::canonicalize(deck).map_err(|error| {
            ClientError::NotOpen(format!("cannot resolve {}: {error}", deck.display()))
        })?;
        let root = runtime.join("live").join(deck_key(&canonical));
        let metadata =
            read_bounded::<OwnerMetadata>(&root.join("owner.json")).map_err(|error| {
                ClientError::NotOpen(format!("{} has no live owner: {error}", deck.display()))
            })?;
        if metadata.deck != canonical
            || metadata.control_v != 1
            || uuid::Uuid::parse_str(&metadata.generation.0).is_err()
        {
            return Err(ClientError::NotOpen(format!(
                "{} has incompatible live owner metadata",
                deck.display()
            )));
        }
        let session = root.join(&metadata.generation.0);
        Ok(Self {
            generation: metadata.generation,
            requests: session.join("requests"),
            session: session.clone(),
            processing: session.join("processing"),
            replies: session.join("replies"),
        })
    }

    /// Commit a request and wait for the owner's acknowledgement.
    ///
    /// # Errors
    ///
    /// When publication fails or no acknowledgement arrives before `timeout`.
    pub fn request(&self, body: RequestBody, timeout: Duration) -> Result<Response, ClientError> {
        self.request_with_id(uuid::Uuid::new_v4().to_string(), body, timeout)
    }

    /// Commit an idempotent request and wait for its acknowledgement.
    ///
    /// # Errors
    ///
    /// When the id is invalid, was used for different content, publication
    /// fails, or no acknowledgement arrives before `timeout`.
    pub fn request_with_id(
        &self,
        id: String,
        body: RequestBody,
        timeout: Duration,
    ) -> Result<Response, ClientError> {
        if uuid::Uuid::parse_str(&id).is_err() {
            return Err(ClientError::Conflict);
        }
        let wanted_hash = body_hash(&body);
        let reply = self.replies.join(format!("{id}.json"));
        if reply.exists() {
            let response = read_bounded::<Response>(&reply)?;
            return if response.generation == self.generation
                && response.request_id == id
                && response.request_hash == wanted_hash
            {
                Ok(response)
            } else {
                Err(ClientError::Conflict)
            };
        }

        self.deposit_with_id(id.clone(), body, timeout)?;
        let until = Instant::now() + timeout;
        loop {
            if reply.exists() {
                let response = read_bounded::<Response>(&reply)?;
                if response.generation == self.generation && response.request_id == id {
                    return if response.request_hash == wanted_hash {
                        Ok(response)
                    } else {
                        Err(ClientError::Conflict)
                    };
                }
            }
            if Instant::now() >= until {
                let _ = std::fs::rename(
                    self.requests.join(format!("{id}.json")),
                    self.requests.join(format!("{id}.withdrawn")),
                );
                return Err(ClientError::Timeout);
            }
            std::thread::sleep(POLL_EVERY.min(until.saturating_duration_since(Instant::now())));
        }
    }

    /// Commit without waiting, for asynchronous commands and transport tests.
    ///
    /// # Errors
    /// The next thing the reader wants answered, taken once.
    ///
    /// This is the author's half of a huddle. The agent that wrote the deck is
    /// already blocked in `deck wait`; making it *also* poll a second verb was
    /// the mistake — no agent naturally sits in a loop, so the author stayed
    /// asleep through the whole conversation and nothing could answer.
    ///
    /// Taken rather than read: the delivered mark advances as this returns, so
    /// the agent answering and calling `wait` again gets the *next* one instead
    /// of the same one forever. One waiter, so a file is enough.
    ///
    /// Everything the reader did arrives here. Whether they waited for a gap or
    /// took the floor decided when the window published it, not whether it was
    /// ever sent — a reaction the author never hears about is a reaction the
    /// author cannot answer.
    ///
    /// # Errors
    ///
    /// When the session directory cannot be read or the mark cannot be written.
    pub fn take_asked(&self) -> Result<Option<Event>, std::io::Error> {
        let event = self.asked()?;
        if let Some(event) = event.as_ref() {
            self.delivered(event.seq)?;
        }
        Ok(event)
    }

    /// Read without consuming, so a failed stdout write can be retried.
    ///
    /// # Errors
    /// When the session's event directory cannot be read.
    pub fn asked(&self) -> Result<Option<Event>, std::io::Error> {
        let mark = self.session.join("delivered");
        let seen = std::fs::read_to_string(&mark)
            .ok()
            .and_then(|text| text.trim().parse::<u64>().ok());

        let mut best: Option<Event> = None;
        match std::fs::read_dir(self.session.join("events")) {
            Ok(listing) => {
                for entry in listing.flatten() {
                    let Ok(event) = read_bounded::<Event>(&entry.path()) else {
                        continue;
                    };
                    if seen.is_some_and(|seen| event.seq <= seen) {
                        continue;
                    }
                    // Anything the reader did. Both kinds reach the author —
                    // `when` decided *when* the window let go of it, not
                    // whether it was ever sent.
                    if !matches!(
                        event.moment.what,
                        deck_core::What::Wrote | deck_core::What::Reacted
                    ) {
                        continue;
                    }
                    if best.as_ref().is_none_or(|held| event.seq < held.seq) {
                        best = Some(event);
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        }

        Ok(best)
    }

    /// Advance only after the caller has successfully handed the event over.
    ///
    /// # Errors
    /// When the durable cursor cannot be committed.
    pub fn delivered(&self, seq: u64) -> Result<(), std::io::Error> {
        write_json(&self.session.join("delivered"), &seq)
    }

    /// The first event after `after`, waiting up to `timeout` for one.
    ///
    /// The cursor is a sequence, so a caller that crashes and comes back reads
    /// from where it got to rather than from the start or from "now" — an agent
    /// must not lose a must-fix because it was restarting when the reader
    /// pressed the key.
    ///
    /// `None` on timeout, which is an ordinary answer rather than a failure:
    /// nothing happening is the usual case while somebody reads.
    ///
    /// # Errors
    ///
    /// When the session directory cannot be read.
    pub fn events(
        &self,
        after: Option<u64>,
        timeout: Duration,
    ) -> Result<Option<Event>, std::io::Error> {
        let events = self.session.join("events");
        let until = Instant::now() + timeout;
        loop {
            let mut best: Option<Event> = None;
            match std::fs::read_dir(&events) {
                Ok(listing) => {
                    for entry in listing.flatten() {
                        let Ok(event) = read_bounded::<Event>(&entry.path()) else {
                            continue;
                        };
                        if after.is_some_and(|seen| event.seq <= seen) {
                            continue;
                        }
                        // The lowest unseen sequence, so a reader who reacted
                        // three times while the agent was busy is replayed in
                        // the order they did it.
                        if best.as_ref().is_none_or(|held| event.seq < held.seq) {
                            best = Some(event);
                        }
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
            if best.is_some() {
                return Ok(best);
            }
            if Instant::now() >= until {
                return Ok(None);
            }
            std::thread::sleep(Duration::from_millis(40));
        }
    }

    /// Commit a request and return its id without waiting for the result.
    ///
    /// For a caller that will collect the acknowledgement later, or not at all.
    ///
    /// # Errors
    ///
    /// When the request cannot be published.
    pub fn deposit(&self, body: RequestBody, timeout: Duration) -> Result<String, ClientError> {
        let id = uuid::Uuid::new_v4().to_string();
        self.deposit_with_id(id.clone(), body, timeout)?;
        Ok(id)
    }

    fn deposit_with_id(
        &self,
        id: String,
        body: RequestBody,
        timeout: Duration,
    ) -> Result<(), ClientError> {
        let timeout_ms = u64::try_from(timeout.as_millis()).unwrap_or(u64::MAX);
        let request = Request {
            id: id.clone(),
            generation: self.generation.clone(),
            deadline_ms: now_ms().saturating_add(timeout_ms),
            body,
        };
        for existing in [
            self.requests.join(format!("{id}.json")),
            self.processing.join(format!("{id}.json")),
        ] {
            if existing.exists() {
                let had = read_bounded::<Request>(&existing)?;
                return if had.generation == request.generation && had.body == request.body {
                    Ok(())
                } else {
                    Err(ClientError::Conflict)
                };
            }
        }
        write_json(&self.requests.join(format!("{id}.json")), &request)?;
        Ok(())
    }
}

fn read_header(deck: &Path) -> Result<Header, String> {
    let path = deck.join("deck.json");
    let bytes =
        std::fs::read(&path).map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("{} is not a deck header: {error}", path.display()))
}

fn body_hash(body: &RequestBody) -> String {
    let mut digest = Sha256::new();
    // Every RequestBody serializes: failure can only come from a future custom
    // serializer, and an empty digest then makes retries fail closed.
    digest.update(serde_json::to_vec(body).unwrap_or_default());
    format!("{:x}", digest.finalize())
}

fn deck_key(path: &Path) -> String {
    let mut digest = Sha256::new();
    digest.update(path_bytes(path));
    format!("{:x}", digest.finalize())
}

#[cfg(unix)]
fn path_bytes(path: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt as _;
    path.as_os_str().as_bytes().to_vec()
}

#[cfg(windows)]
fn path_bytes(path: &Path) -> Vec<u8> {
    use std::os::windows::ffi::OsStrExt as _;
    path.as_os_str()
        .encode_wide()
        .flat_map(u16::to_le_bytes)
        .collect()
}

#[cfg(not(any(unix, windows)))]
fn path_bytes(path: &Path) -> Vec<u8> {
    path.as_os_str().to_string_lossy().into_owned().into_bytes()
}

fn now_ms() -> u64 {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO);
    u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX)
}

fn read_bounded<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, std::io::Error> {
    let mut file = File::open(path)?;
    if file.metadata()?.len() > MAX_MESSAGE_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "live control document exceeds 1 MiB",
        ));
    }
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    serde_json::from_slice(&bytes).map_err(std::io::Error::other)
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), std::io::Error> {
    let bytes = serde_json::to_vec(value).map_err(std::io::Error::other)?;
    if bytes.len() as u64 > MAX_MESSAGE_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "live control document exceeds 1 MiB",
        ));
    }
    let parent = path
        .parent()
        .ok_or_else(|| std::io::Error::other("live path has no parent"))?;
    private_dir(parent)?;
    let tmp = parent.join(format!(
        ".{}.{}.tmp",
        std::process::id(),
        uuid::Uuid::new_v4()
    ));
    std::fs::write(&tmp, bytes)?;
    private_file(&tmp)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

/// The file an event of this sequence lives in.
///
/// Zero padded so the directory listing is already in order — nothing has to
/// parse a name to sort it, and a reader with `ls` sees the walk in sequence.
fn name_of(seq: u64) -> String {
    format!("{seq:012}.json")
}

fn private_dir(path: &Path) -> Result<(), std::io::Error> {
    std::fs::create_dir_all(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

fn private_file(path: &Path) -> Result<(), std::io::Error> {
    // Nothing to set off unix, where the parameter would otherwise read as
    // unused — which is an error under `-D warnings`, and so a red Windows run
    // on a tree that builds here.
    #[cfg(not(unix))]
    let _ = path;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {

    #[test]
    fn bringing_a_file_says_what_it_displaces() {
        let plain = super::RequestBody::Bring {
            file: "crates/core/src/protocol.rs".into(),
            range: deck_core::LineRange::new(120, 160),
            name: Some("protocol".into()),
            note: None,
            after: None,
            fold_group: true,
            fold: Vec::new(),
        };
        let json = serde_json::to_value(&plain).unwrap();
        assert_eq!(json["type"], "bring");
        // Snake case, like every other field on the wire: the rename on the
        // enum names the variants, not what is inside them.
        assert_eq!(json["fold_group"], true);
        // Nothing it did not ask for travels with it.
        assert!(json.get("note").is_none(), "{json}");
        assert!(json.get("fold").is_none(), "{json}");
        assert_eq!(
            serde_json::from_value::<super::RequestBody>(json).unwrap(),
            plain
        );
    }

    #[test]
    fn folding_names_the_panes_it_means() {
        let json = serde_json::to_value(super::RequestBody::Fold {
            panes: vec!["retry".into()],
            group: false,
            open: false,
        })
        .unwrap();
        assert_eq!(
            json,
            serde_json::json!({ "type": "fold", "panes": ["retry"] })
        );
    }

    #[test]
    fn a_show_may_carry_a_replacement_and_usually_does_not() {
        // Added after the protocol was frozen, so it has to be optional both
        // ways: an older window reading a newer request ignores it, and a
        // request without one is byte for byte what it always was.
        let plain = super::RequestBody::Show {
            file: "a.rs".into(),
            range: deck_core::LineRange::new(1, 2),
            group: None,
            pane: None,
            after: None,
        };
        let json = serde_json::to_value(&plain).unwrap();
        assert!(json.get("after").is_none(), "{json}");

        let proposing = super::RequestBody::Show {
            file: "a.rs".into(),
            range: deck_core::LineRange::new(1, 2),
            group: None,
            pane: None,
            after: Some("let x = 1;".into()),
        };
        let there = serde_json::to_value(&proposing).unwrap();
        assert_eq!(there["after"], "let x = 1;");
        assert_eq!(
            serde_json::from_value::<super::RequestBody>(there).unwrap(),
            proposing
        );
    }

    #[test]
    fn clearing_is_a_request_of_its_own_on_the_wire() {
        let json = serde_json::to_value(super::RequestBody::Clear).unwrap();
        assert_eq!(json, serde_json::json!({ "type": "clear" }));
        let back: super::RequestBody = serde_json::from_value(json).unwrap();
        assert_eq!(back, super::RequestBody::Clear);
    }

    #[test]
    fn a_note_about_what_the_agent_is_doing_is_its_own_request() {
        // A new variant rather than a flag on `say`: a note is not a turn, it
        // is not recorded, and a window that treated it as one would put
        // "reading the retry loop" in somebody's review.
        let doing = super::RequestBody::Doing {
            text: "reading the retry loop".into(),
        };
        let json = serde_json::to_value(&doing).unwrap();
        assert_eq!(
            json,
            serde_json::json!({ "type": "doing", "text": "reading the retry loop" })
        );
        assert_eq!(
            serde_json::from_value::<super::RequestBody>(json).unwrap(),
            doing
        );
    }

    #[test]
    fn a_cursor_replays_every_reaction_in_order() {
        // The reason the stream takes a cursor rather than handing back
        // "whatever happened since you asked": an agent that was busy or
        // restarting must not lose a must-fix the reader pressed while it was
        // away, and must read them in the order they were pressed.
        let temp = tempfile::tempdir().unwrap();
        let runtime = temp.path().join("runtime");
        let deck = deck_dir(temp.path());
        let owner = Owner::claim(&runtime, &deck).unwrap();
        let client = Client::connect(&runtime, &deck).unwrap();

        for (at_ms, text) in [(10, "first"), (20, "second"), (30, "third")] {
            owner
                .publish(deck_core::Moment {
                    at_ms,
                    what: deck_core::What::Reacted,
                    group: Some("g1".into()),
                    ref_id: None,
                    file: None,
                    range: None,
                    text: text.into(),
                    kind: Some(deck_core::Kind::MustFix),
                    when: deck_core::When::Queue,
                })
                .unwrap();
        }

        let mut seen = Vec::new();
        let mut cursor = None;
        while let Some(event) = client.events(cursor, Duration::from_millis(50)).unwrap() {
            cursor = Some(event.seq);
            seen.push(event.moment.text);
        }
        assert_eq!(seen, vec!["first", "second", "third"]);
    }

    fn moment(text: &str, when: deck_core::When) -> deck_core::Moment {
        deck_core::Moment {
            at_ms: 0,
            what: deck_core::What::Wrote,
            group: Some("g1".into()),
            ref_id: None,
            file: None,
            range: None,
            text: text.into(),
            kind: Some(deck_core::Kind::Question),
            when,
        }
    }

    #[test]
    fn a_drawn_picture_says_what_it_displaces() {
        // `Bring` for diagrams: the same folding, so the room moves once.
        let asked = RequestBody::Draw {
            diagram: deck_core::diagram::Diagram {
                title: Some("how a re-import travels".into()),
                flow: deck_core::diagram::Direction::default(),
                nodes: Vec::new(),
                edges: Vec::new(),
                clusters: Vec::new(),
                flows: Vec::new(),
            },
            name: Some("flow".into()),
            note: None,
            fold_group: true,
            fold: Vec::new(),
        };
        let json = serde_json::to_value(&asked).unwrap();
        assert_eq!(json["type"], "draw");
        assert_eq!(json["fold_group"], true);
        assert_eq!(json["name"], "flow");
        assert!(json.get("note").is_none(), "an absent note is left out");

        let back: RequestBody = serde_json::from_value(json).unwrap();
        assert_eq!(back, asked, "and it reads back as what was sent");
    }

    #[test]
    fn both_kinds_reach_the_author() {
        // `when` is about the floor, not about being heard. Waiting for a gap
        // decides *when* the window lets go of a remark; a reaction the author
        // never hears about is a reaction the author cannot answer.
        let temp = tempfile::tempdir().unwrap();
        let runtime = temp.path().join("runtime");
        let deck = deck_dir(temp.path());
        let owner = Owner::claim(&runtime, &deck).unwrap();
        let client = Client::connect(&runtime, &deck).unwrap();

        owner
            .publish(moment("a reaction", deck_core::When::Queue))
            .unwrap();
        owner
            .publish(moment(
                "so it fires on every edit?",
                deck_core::When::Interrupt,
            ))
            .unwrap();

        assert_eq!(
            client.take_asked().unwrap().unwrap().moment.text,
            "a reaction",
            "the one that waited its turn is still heard, and heard first"
        );
        assert_eq!(
            client.take_asked().unwrap().unwrap().moment.text,
            "so it fires on every edit?"
        );
    }

    #[test]
    fn a_question_is_answered_once_and_not_asked_again() {
        // The agent answers and calls `deck wait` again. Reading rather than
        // taking would hand it the same question for ever, and it would answer
        // it for ever.
        let temp = tempfile::tempdir().unwrap();
        let runtime = temp.path().join("runtime");
        let deck = deck_dir(temp.path());
        let owner = Owner::claim(&runtime, &deck).unwrap();
        let client = Client::connect(&runtime, &deck).unwrap();

        owner
            .publish(moment("first", deck_core::When::Interrupt))
            .unwrap();
        owner
            .publish(moment("second", deck_core::When::Interrupt))
            .unwrap();

        assert_eq!(client.take_asked().unwrap().unwrap().moment.text, "first");
        assert_eq!(client.take_asked().unwrap().unwrap().moment.text, "second");
        assert!(
            client.take_asked().unwrap().is_none(),
            "and then it waits again"
        );
    }

    #[test]
    fn an_unprinted_question_is_available_to_the_next_waiter() {
        let temp = tempfile::tempdir().unwrap();
        let runtime = temp.path().join("runtime");
        let deck = deck_dir(temp.path());
        let owner = Owner::claim(&runtime, &deck).unwrap();
        owner
            .publish(moment("do not lose this", deck_core::When::Interrupt))
            .unwrap();
        let first = Client::connect(&runtime, &deck)
            .unwrap()
            .asked()
            .unwrap()
            .unwrap();
        let retry = Client::connect(&runtime, &deck).unwrap();
        assert_eq!(retry.asked().unwrap().unwrap().seq, first.seq);
        retry.delivered(first.seq).unwrap();
        assert!(retry.asked().unwrap().is_none());
    }

    #[test]
    fn a_quiet_session_answers_rather_than_hanging() {
        // Nothing happening is the usual case while somebody reads, so it is an
        // ordinary answer and not a failure.
        let temp = tempfile::tempdir().unwrap();
        let runtime = temp.path().join("runtime");
        let deck = deck_dir(temp.path());
        let _owner = Owner::claim(&runtime, &deck).unwrap();
        let client = Client::connect(&runtime, &deck).unwrap();

        assert!(
            client
                .events(None, Duration::from_millis(60))
                .unwrap()
                .is_none()
        );
    }

    fn deck_dir(root: &Path) -> PathBuf {
        let deck = root.join("d-events.deck");
        std::fs::create_dir_all(&deck).unwrap();
        std::fs::write(
            deck.join("deck.json"),
            r#"{"v":1,"id":"d-events","title":"test"}"#,
        )
        .unwrap();
        deck
    }
    use std::time::Duration;

    use super::*;

    fn deck(root: &std::path::Path, id: &str) -> std::path::PathBuf {
        let deck = root.join(format!("{id}.deck"));
        std::fs::create_dir_all(&deck).unwrap();
        std::fs::write(
            deck.join("deck.json"),
            format!(r#"{{"v":1,"id":"{id}","title":"test"}}"#),
        )
        .unwrap();
        deck
    }

    #[test]
    fn two_open_attempts_cannot_own_the_same_canonical_deck() {
        let temp = tempfile::tempdir().unwrap();
        let runtime = temp.path().join("runtime");
        let deck = deck(temp.path(), "d-one");
        let owner = Owner::claim(&runtime, &deck).expect("the first window owns it");

        let second = Owner::claim(&runtime, &deck);
        assert!(matches!(second, Err(ClaimError::AlreadyOwned)));
        drop(owner);
        Owner::claim(&runtime, &deck).expect("dropping the handle releases ownership");
    }

    #[test]
    fn copied_header_ids_do_not_share_a_mailbox() {
        let temp = tempfile::tempdir().unwrap();
        let runtime = temp.path().join("runtime");
        let first = deck(&temp.path().join("one"), "copied");
        let second = deck(&temp.path().join("two"), "copied");

        let one = Owner::claim(&runtime, &first).unwrap();
        let two = Owner::claim(&runtime, &second).unwrap();

        assert_ne!(one.session_dir(), two.session_dir());
    }

    #[cfg(unix)]
    #[test]
    fn canonical_path_aliases_name_one_owner() {
        let temp = tempfile::tempdir().unwrap();
        let runtime = temp.path().join("runtime");
        let deck = deck(temp.path(), "d-one");
        let alias = temp.path().join("alias.deck");
        std::os::unix::fs::symlink(&deck, &alias).unwrap();
        let _owner = Owner::claim(&runtime, &deck).unwrap();

        assert!(matches!(
            Owner::claim(&runtime, &alias),
            Err(ClaimError::AlreadyOwned)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn path_hashing_does_not_replace_non_utf8_bytes() {
        use std::os::unix::ffi::OsStringExt as _;

        let raw = std::path::PathBuf::from(std::ffi::OsString::from_vec(vec![b'd', 0xff]));
        let replacement = std::path::PathBuf::from("d�");

        assert_eq!(deck_key(&raw), deck_key(&raw));
        assert_ne!(deck_key(&raw), deck_key(&replacement));
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    #[test]
    fn a_non_utf8_deck_path_has_a_stable_private_key() {
        use std::os::unix::ffi::OsStringExt as _;

        let temp = tempfile::tempdir().unwrap();
        let runtime = temp.path().join("runtime");
        let root = temp
            .path()
            .join(std::ffi::OsString::from_vec(vec![b'd', 0xff]));
        let deck = deck(&root, "d-one");

        let owner = Owner::claim(&runtime, &deck).expect("path bytes do not need to be UTF-8");
        let client = Client::connect(&runtime, &deck).expect("the same bytes find the owner");
        assert_eq!(client.generation, owner.generation);
    }

    #[test]
    fn a_client_waits_for_an_acknowledgement_not_a_deposited_request() {
        let temp = tempfile::tempdir().unwrap();
        let runtime = temp.path().join("runtime");
        let deck = deck(temp.path(), "d-one");
        let owner = Owner::claim(&runtime, &deck).unwrap();
        let client = Client::connect(&runtime, &deck).unwrap();

        let worker = std::thread::spawn(move || {
            let request = owner.receive(Duration::from_secs(1)).unwrap();
            assert_eq!(request.body, RequestBody::Status);
            std::thread::sleep(Duration::from_millis(40));
            owner
                .acknowledge(&request, ResponseStatus::Ready)
                .expect("the owner publishes one committed reply");
        });

        let began = std::time::Instant::now();
        let response = client
            .request(RequestBody::Status, Duration::from_secs(1))
            .unwrap();
        assert_eq!(response.status, ResponseStatus::Ready);
        assert!(began.elapsed() >= Duration::from_millis(40));
        worker.join().unwrap();
    }

    #[test]
    fn a_request_id_replays_one_outcome_and_rejects_different_content() {
        let temp = tempfile::tempdir().unwrap();
        let runtime = temp.path().join("runtime");
        let deck = deck(temp.path(), "d-one");
        let owner = Owner::claim(&runtime, &deck).unwrap();
        let client = Client::connect(&runtime, &deck).unwrap();
        let id = uuid::Uuid::new_v4().to_string();

        std::thread::scope(|scope| {
            scope.spawn(|| {
                let request = owner.receive(Duration::from_secs(1)).unwrap();
                owner.acknowledge(&request, ResponseStatus::Ready).unwrap();
            });
            let first = client
                .request_with_id(id.clone(), RequestBody::Status, Duration::from_secs(1))
                .unwrap();
            let replay = client
                .request_with_id(id.clone(), RequestBody::Status, Duration::from_secs(1))
                .unwrap();
            assert_eq!(replay, first);
        });

        assert!(matches!(
            client.request_with_id(
                id,
                RequestBody::Show {
                    file: "src/lib.rs".into(),
                    range: LineRange::single(1),
                    group: None,
                    pane: None,
                    after: None,
                },
                Duration::from_secs(1),
            ),
            Err(ClientError::Conflict)
        ));
    }

    #[test]
    fn request_ids_cannot_choose_a_reply_path() {
        let temp = tempfile::tempdir().unwrap();
        let runtime = temp.path().join("runtime");
        let deck = deck(temp.path(), "d-one");
        let owner = Owner::claim(&runtime, &deck).unwrap();
        let malicious = Request {
            id: "../outside".into(),
            generation: owner.generation.clone(),
            deadline_ms: now_ms() + 1_000,
            body: RequestBody::Status,
        };
        write_json(
            &owner.session_dir().join("requests/malicious.json"),
            &malicious,
        )
        .unwrap();

        assert!(matches!(
            owner.receive(Duration::from_millis(80)),
            Err(ReceiveError::Timeout)
        ));
        assert!(!owner.session_dir().join("outside.json").exists());
    }

    #[test]
    fn a_stale_generation_cannot_reach_a_reopened_owner() {
        let temp = tempfile::tempdir().unwrap();
        let runtime = temp.path().join("runtime");
        let deck = deck(temp.path(), "d-one");
        let old = Owner::claim(&runtime, &deck).unwrap();
        let old_client = Client::connect(&runtime, &deck).unwrap();
        drop(old);
        let new = Owner::claim(&runtime, &deck).unwrap();

        old_client
            .deposit(RequestBody::Status, Duration::from_secs(1))
            .unwrap();
        assert!(matches!(
            new.receive(Duration::from_millis(80)),
            Err(ReceiveError::Timeout)
        ));
    }
}
