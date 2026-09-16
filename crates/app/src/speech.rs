//! Reading the narration out loud.
//!
//! A deck is already written to be heard. The narration is ordered, it is one
//! claim at a time, and it was composed to be read in sequence rather than
//! skimmed — which is most of what a thing needs before a voice can carry it.
//! What a reader gets from this is two channels that do not compete: the code
//! in front of their eyes, the argument in their ears.
//!
//! # Why the system voice and not a good one
//!
//! The genuinely natural voices are all services, and a narration is a
//! description of somebody's unreleased code. Turning it into a sound file
//! means uploading it, and that is not a trade deck can make on a reader's
//! behalf for a nicer timbre. So: whatever the machine already has.
//!
//! That is not the compromise it sounds like. The neural voices Apple ships are
//! a free download and are very good; the ones installed by default are the
//! compact ones, and the difference between the two is most of the difference
//! between listening to a deck and enduring one. Deck says so in setup rather
//! than quietly sounding bad.
//!
//! # Why a process and not a library
//!
//! Speech synthesis in-process means binding a platform framework, keeping a
//! synthesiser alive across frames, and owning an audio session. A child
//! process that reads stdin and exits is a thing that can be started and killed,
//! which is the entire interface needed here.

use std::collections::VecDeque;
use std::io::{Seek as _, Write as _};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::{Duration, Instant};

use deck_core::LineRange;
use deck_core::config::{Engine, Speech};
use gpui_kit::{App, Global};

use crate::prose::Said;

/// What the config said about the voice.
///
/// A global for the same reason zen's is: read once at startup, never changed,
/// and carrying it through `Session` would mean the model knowing how somebody
/// likes to be read to.
#[derive(Default)]
pub struct Asked(pub Speech);

impl Global for Asked {}

/// Remember what the config said, before any window opens.
pub fn remember(speech: Speech, cx: &mut App) {
    cx.set_global(Asked(speech));
}

/// What the reader asked for, or the design's answer if they said nothing.
#[must_use]
pub fn asked(cx: &App) -> Speech {
    cx.try_global::<Asked>()
        .map_or_else(Speech::default, |asked| asked.0.clone())
}

/// How good a voice is, which the system says in its name.
///
/// Three tiers and they are genuinely different models, not marketing. Compact
/// is what ships by default and is what people mean when they say a computer
/// voice. Enhanced is a real jump. Premium is a bigger model again, and is the
/// one worth the download.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Grade {
    /// The best the system offers.
    Premium,
    /// Neural, and a long way past compact.
    Enhanced,
    /// Installed by default, and it sounds like it.
    Compact,
}

/// A voice the machine has, and how good it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Installed {
    /// Its name, as the synthesiser wants it back.
    pub name: String,
    /// Which model it is.
    pub grade: Grade,
}

impl Installed {
    /// Whether this is one of the neural voices.
    ///
    /// The line deck actually cares about: above it there is a voice worth
    /// offering, below it there is only a reason to send somebody to the
    /// download page.
    #[must_use]
    pub fn natural(&self) -> bool {
        self.grade != Grade::Compact
    }
}

/// Every English voice installed, best first.
///
/// English only, because that is the language the narration is in. A machine
/// set up in another language still has its English voices; picking a voice by
/// the system locale would read the prose with the wrong phonemes.
#[must_use]
pub fn voices() -> Vec<Installed> {
    let Ok(out) = Command::new("say").arg("-v").arg("?").output() else {
        return Vec::new();
    };
    listed(&String::from_utf8_lossy(&out.stdout))
}

/// [`voices`], against the text the synthesiser printed.
///
/// Split out because this is the part that can rot. The listing is a column
/// layout meant for a person, its quality suffix is the only thing marking a
/// voice as worth using, and both are Apple's to change — which they have.
fn listed(out: &str) -> Vec<Installed> {
    let mut found: Vec<Installed> = out
        .lines()
        .filter_map(|line| {
            // `Ava (Enhanced)      en_US    # Hello! My name is Ava.`
            let said = line.split('#').next()?.trim_end();
            let (name, locale) = said.rsplit_once(char::is_whitespace)?;
            locale.starts_with("en").then(|| Installed {
                grade: if name.contains("(Premium)") {
                    Grade::Premium
                } else if name.contains("(Enhanced)") {
                    Grade::Enhanced
                } else {
                    Grade::Compact
                },
                name: name.trim().to_string(),
            })
        })
        .collect();

    // Best model first, and within a tier the order the system gave them. Sorted
    // by grade rather than by whether it is neural at all, because a machine
    // with both an Enhanced and a Premium voice installed should be offered the
    // Premium one — and alphabetical order would hand it Ava over Zoe.
    found.sort_by_key(|voice| voice.grade);
    found
}

/// Where a reader goes to get a voice worth listening to.
///
/// Worth printing in full. Nobody finds this by looking, and the difference it
/// makes is the difference between the feature working and the feature being
/// switched off after one paragraph.
///
/// Both names, because Apple moved it. macOS 26 calls the pane **Read & Speak**
/// and files it under Vision; every earlier version calls it Spoken Content. A
/// reader following a path that is not on their screen concludes the
/// instructions are stale and stops, so both are named rather than the newer
/// one guessed at.
pub const WHERE: &str = "System Settings → Accessibility → Read & Speak\n    → System Voice → Manage Voices… → English → anything marked Premium\n\n    (macOS 15 and earlier call that pane Spoken Content)";

/// Whose words a passage is, so they can be lit on the page as they are heard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Narration {
    /// A group's own narration, by index.
    Group(usize),
    /// An answer the agent gave during the walk, by its place in the transcript.
    Answer(usize),
}

/// A voice, what it is saying, and what it has still to say.
///
/// A passage is one `say`: everything the agent sent in one breath, cut where
/// it pointed. Passages queue. They used to replace: saying a second thing
/// killed the first mid-word, so an agent that answered in three sentences was
/// heard saying only the last one. A reply is not an interruption of itself.
///
/// The pieces *inside* a passage do not queue, and that is what stopped the
/// voice stalling every time it pointed. Each piece used to be an utterance of
/// its own — its own request to the engine, its own file, its own player — and
/// the voice went quiet for as long as all of that took, every time the finger
/// moved. It sounded like somebody reading a sentence, stopping to point, and
/// starting again. Now the pieces are rendered together and played as one
/// sound, and the points ride on that sound's clock.
#[derive(Debug, Default)]
pub struct Voice {
    /// Passages waiting their turn, in the order they were given.
    next: VecDeque<(Vec<Said>, Option<Narration>)>,
    /// A passage being rendered on a worker thread.
    ///
    /// Held so the paint thread never blocks on a network round trip. Until it
    /// arrives the voice counts as talking, because something is on its way.
    fetching: Option<(Receiver<anyhow::Result<Ready>>, Option<Narration>)>,
    /// The passage being heard, or walked in silence.
    playing: Option<Playing>,
    /// Where the last passage left the finger.
    ///
    /// Kept when the passage ends: an argument that finished on line 118 did
    /// not stop being about line 118 because the voice went quiet.
    now: Option<LineRange>,
}

impl Voice {
    /// Whether something is being said right now.
    ///
    /// Asks the process rather than remembering, because the interesting case
    /// is the one where it finished on its own — a reader who listened to the
    /// whole group and presses the key again means *say it again*, not *stop*.
    ///
    /// A passage walked in silence is not talking. Nothing is, and a queued
    /// remark still goes through at once.
    pub fn talking(&mut self) -> bool {
        if self.fetching.is_some() {
            return true;
        }
        self.playing
            .as_mut()
            .is_some_and(|playing| !playing.quiet && !playing.over())
    }

    /// Whether anything is queued, on its way, or still being walked.
    #[must_use]
    pub fn waiting(&self) -> bool {
        !self.next.is_empty() || self.fetching.is_some() || self.playing.is_some()
    }

    /// Whether a passage is being made right now.
    ///
    /// Worth saying out loud in the panel. A cloud voice takes a second or two
    /// to answer, and silence with nothing on screen to explain it reads as a
    /// reply that never came.
    #[must_use]
    pub fn making(&self) -> bool {
        self.fetching.is_some()
    }

    /// The lines the narration is pointing at, if it is pointing anywhere.
    #[must_use]
    pub fn pointing(&self) -> Option<LineRange> {
        self.playing.as_ref().map_or(self.now, Playing::pointing)
    }

    /// Whose words are being heard, and which word of them, counted as the
    /// band counts.
    #[must_use]
    pub fn hearing(&self) -> Option<(Narration, usize)> {
        self.playing.as_ref().and_then(Playing::hearing)
    }

    /// Take the finger off the page, even mid-passage.
    ///
    /// What is being said goes on. The rest of this passage points nowhere,
    /// because the agent asked to stop pointing, not to stop talking.
    pub fn forget_point(&mut self) {
        self.now = None;
        if let Some(playing) = self.playing.as_mut() {
            for (_, point) in &mut playing.marks {
                *point = None;
            }
        }
    }

    /// Whether the speech pump still has work to do.
    ///
    /// Keeping this decision with the queue prevents a view from inverting
    /// `waiting` and polling forever once the last utterance has finished.
    pub fn has_work(&mut self) -> bool {
        self.talking() || self.waiting()
    }

    /// Say this after whatever is already being said.
    ///
    /// One passage, heard as one sound. The point on each piece arrives when
    /// that piece is heard rather than when the command did, which is the only
    /// way the light can mean anything: an agent sends its sentences as fast as
    /// it can write them, and a reader hears them one at a time.
    pub fn say(&mut self, said: Vec<Said>, of: Option<Narration>, speech: &Speech) {
        let said = passage(said);
        if said.is_empty() {
            return;
        }
        self.next.push_back((said, of));
        self.pump(speech);
    }

    /// Start the next passage if nothing is being said.
    ///
    /// Called on a timer while there is work, which is how the queue advances:
    /// the sound ends, the next tick notices, and the following passage begins.
    pub fn pump(&mut self, speech: &Speech) {
        // Collect a finished render *first*. `talking` counts one in flight as
        // talking — correctly, since something is on its way — so checking it
        // before looking in the channel meant this returned early for ever and
        // the audio was never collected. The panel said "speaking" the whole
        // time, which was true and useless.
        if let Some((waiting, of)) = self.fetching.as_ref() {
            let of = *of;
            match waiting.try_recv() {
                Err(TryRecvError::Empty) => return,
                Ok(Ok(ready)) => {
                    self.fetching = None;
                    self.begin(ready, of, speech);
                    return;
                }
                Ok(Err(why)) => {
                    // Said once, to the terminal. A reader whose key is wrong
                    // is in a window with nowhere to show it, so the queue is
                    // dropped rather than left stuck behind a voice that will
                    // never arrive.
                    eprintln!("deck: {why}");
                    self.fetching = None;
                    self.next.clear();
                    return;
                }
                Err(TryRecvError::Disconnected) => self.fetching = None,
            }
        }

        if let Some(playing) = self.playing.as_mut() {
            if !playing.over() {
                return;
            }
            self.now = playing.pointing();
            self.playing = None;
        }

        let Some((said, of)) = self.next.pop_front() else {
            return;
        };
        if !renders(speech) {
            self.begin(timed(&said, speech), of, speech);
            return;
        }
        let (send, receive) = std::sync::mpsc::channel();
        let speech = speech.clone();
        let spawned = std::thread::Builder::new()
            .name("deck-speech".into())
            .spawn(move || {
                let _ = send.send(rendered(&said, &speech));
            });
        if spawned.is_ok() {
            self.fetching = Some((receive, of));
        }
    }

    /// Start hearing a passage that is ready.
    fn begin(&mut self, ready: Ready, of: Option<Narration>, speech: &Speech) {
        match Playing::start(ready, of, speech) {
            Some(playing) => {
                self.now = playing.pointing();
                self.playing = Some(playing);
            }
            // Nothing can speak it, so draining the rest would only stall.
            None => self.next.clear(),
        }
    }

    /// Stop, now.
    ///
    /// Killed rather than asked politely. The reader pressed a key because they
    /// want the room quiet, and a voice that finishes its sentence first is a
    /// voice that ignored them.
    pub fn hush(&mut self) {
        self.next.clear();
        // Whatever is in flight is abandoned. Its thread will finish and find
        // nobody listening, which is cheaper than making it cancellable.
        self.fetching = None;
        if let Some(playing) = self.playing.take() {
            // The finger stays where the voice was cut off. That is the line
            // the reader stopped it to look at.
            self.now = playing.pointing();
        }
    }
}

/// A voice stops when the window holding it goes.
///
/// Without this a reader who closes a deck mid-sentence keeps hearing it,
/// from a process with nothing left on screen to explain itself.
impl Drop for Voice {
    fn drop(&mut self) {
        self.hush();
    }
}

/// A passage, ready to be heard.
#[derive(Debug)]
struct Ready {
    sound: Sound,
    /// When each piece begins, from the start of the sound, and where it points.
    marks: Vec<(Duration, Option<LineRange>)>,
    /// How long the whole passage lasts.
    length: Duration,
    /// How many words of the page each piece is, in the order of `marks`.
    words: Vec<usize>,
}

/// What carries a passage to the reader.
#[derive(Debug)]
enum Sound {
    /// Rendered ahead of time and joined, so every point is timed exactly.
    File {
        at: PathBuf,
        /// Kept for next time, rather than removed once heard.
        keep: bool,
    },
    /// Handed to a program that speaks as it reads. Deck cannot see inside
    /// that, so the points are timed from the words.
    Piped(String),
    /// Nothing is heard. The points still walk, at reading pace.
    Quiet,
}

/// A passage being heard, or walked in silence.
#[derive(Debug)]
struct Playing {
    child: Option<Child>,
    /// The rendered sound, removed once it has been heard.
    file: Option<PathBuf>,
    quiet: bool,
    since: Instant,
    marks: Vec<(Duration, Option<LineRange>)>,
    length: Duration,
    words: Vec<usize>,
    of: Option<Narration>,
}

/// How far ahead of its words a point starts to rise.
///
/// The light fades in. Started on the first word, it would still be arriving
/// halfway through the sentence; started in the breath before it, it is there
/// when the sentence is.
const LEAD: Duration = Duration::from_millis(220);

impl Playing {
    /// Start a ready passage, or say that nothing here can.
    fn start(ready: Ready, of: Option<Narration>, speech: &Speech) -> Option<Self> {
        let (child, file, quiet) = match ready.sound {
            Sound::File { at, keep } => (Some(play(&at)?), (!keep).then_some(at), false),
            Sound::Piped(text) => {
                let mut child = start(&text, speech)?;
                // Written to stdin rather than passed as an argument, because a
                // narration is prose: it has quotes and dashes in it, and it can
                // be longer than a command line is allowed to be.
                if let Some(stdin) = child.stdin.as_mut() {
                    let _ = stdin.write_all(prepared(&text, speech).as_bytes());
                }
                // Dropped so the child sees the end of its input and starts.
                drop(child.stdin.take());
                (Some(child), None, false)
            }
            Sound::Quiet => (None, None, true),
        };
        Some(Self {
            child,
            file,
            quiet,
            since: Instant::now(),
            marks: ready.marks,
            length: ready.length,
            words: ready.words,
            of,
        })
    }

    /// Whether the passage has been heard to its end.
    fn over(&mut self) -> bool {
        match self.child.as_mut() {
            Some(child) => !matches!(child.try_wait(), Ok(None)),
            None => self.since.elapsed() >= self.length,
        }
    }

    /// Where the passage points at this moment.
    fn pointing(&self) -> Option<LineRange> {
        let heard = self.since.elapsed() + LEAD;
        self.marks
            .iter()
            .take_while(|(from, _)| *from <= heard)
            .last()
            .and_then(|(_, point)| *point)
    }

    /// Which word of the page is being heard.
    ///
    /// Exact to the piece, and an estimate inside it: a piece's words are
    /// spread evenly over its length. Lit a sentence at a time, the difference
    /// does not show.
    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
    fn hearing(&self) -> Option<(Narration, usize)> {
        let of = self.of?;
        let heard = self.since.elapsed() + LEAD;
        let piece = self.marks.iter().rposition(|(from, _)| *from <= heard)?;
        let start = self.marks[piece].0;
        let end = self
            .marks
            .get(piece + 1)
            .map_or(self.length, |(from, _)| *from);
        let span = end.saturating_sub(start).as_secs_f32().max(0.001);
        let along = (heard.saturating_sub(start).as_secs_f32() / span).clamp(0., 1.);
        let count = self.words.get(piece).copied().unwrap_or(0);
        let before: usize = self.words.iter().take(piece).sum();
        let within = ((along * count as f32) as usize).min(count.saturating_sub(1));
        Some((of, before + within))
    }
}

impl Drop for Playing {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        if let Some(at) = self.file.take() {
            let _ = std::fs::remove_file(at);
        }
    }
}

/// Whether this engine can render a passage before it is heard.
///
/// Only a rendered passage can be timed exactly. A program that speaks as it
/// reads is a closed box: deck hears when it ends, and nothing about where it
/// has got to.
fn renders(speech: &Speech) -> bool {
    speech.aloud
        && match speech.engine {
            Engine::Google => true,
            Engine::System => cfg!(target_os = "macos"),
            Engine::Command => false,
        }
}

/// A passage timed from its words, for a sound deck cannot see inside.
///
/// The same words-a-minute the synthesiser is given, so the light walks at the
/// pace the reader chose rather than at a second pace invented for silence. It
/// is a guess at where somebody has got to, and only a guess.
fn timed(said: &[Said], speech: &Speech) -> Ready {
    let mut marks = Vec::with_capacity(said.len());
    let mut length = Duration::ZERO;
    for piece in said {
        marks.push((length, piece.point));
        length += reading(&piece.text, speech.words_a_minute());
    }
    let sound = if speech.aloud {
        let text: Vec<&str> = said.iter().map(|piece| piece.text.as_str()).collect();
        Sound::Piped(text.join(" "))
    } else {
        Sound::Quiet
    };
    Ready {
        sound,
        marks,
        length,
        words: said.iter().map(|piece| piece.words).collect(),
    }
}

/// How long this takes to read at `rate` words a minute.
fn reading(text: &str, rate: u16) -> Duration {
    #[allow(clippy::cast_precision_loss)]
    let words = crate::prose::unbeat(text).split_whitespace().count().max(1) as f32;
    Duration::from_secs_f32(words / f32::from(rate.max(1)) * 60.)
}

/// Samples a second in every rendered piece.
///
/// Both engines are asked for exactly this, because pieces can only be laid end
/// to end if they agree — and then the length of each one is a matter of
/// counting bytes.
const RATE: u32 = 24_000;

/// Bytes of sound a second: one channel, sixteen bits.
const BYTES_A_SECOND: u64 = RATE as u64 * 2;

fn lasting(bytes: usize) -> Duration {
    let bytes = u64::try_from(bytes).unwrap_or(u64::MAX);
    Duration::from_micros(bytes.saturating_mul(1_000_000) / BYTES_A_SECOND)
}

/// The pieces of a passage as the voice takes them: trimmed, and none empty.
///
/// Shared by the walk and the look-ahead, because the cache is keyed by the
/// pieces. Two spellings of the same passage would render it twice.
fn passage(said: Vec<Said>) -> Vec<Said> {
    said.into_iter()
        .map(|piece| Said {
            text: piece.text.trim().to_string(),
            ..piece
        })
        .filter(|piece| !piece.text.is_empty())
        .collect()
}

/// Render these passages before anybody asks for them.
///
/// Turning to a group used to wait for its voice to be made, which for a cloud
/// voice is seconds of silence at exactly the moment the reader acted. Made in
/// the background as soon as the deck is open, one passage after another in the
/// order given, the sound is already on disk when the reader gets there.
///
/// Failures are ignored here. The walk asks again, and that is the attempt
/// that says what went wrong.
pub fn preload(passages: Vec<Vec<Said>>, speech: &Speech) {
    if !renders(speech) {
        return;
    }
    let speech = speech.clone();
    let _ = std::thread::Builder::new()
        .name("deck-voice-ahead".into())
        .spawn(move || {
            for said in passages {
                let said = passage(said);
                if !said.is_empty() {
                    let _ = rendered(&said, &speech);
                }
            }
        });
}

/// A passage, from the cache if it has been made before.
///
/// One render per passage however many ask at once: the walk and the
/// look-ahead both want the group on screen, and the second waits for the
/// first and then finds it on disk.
fn rendered(said: &[Said], speech: &Speech) -> anyhow::Result<Ready> {
    let Some(at) = kept(said, speech) else {
        let at = scratch("wav");
        let (length, starts) = joined(said, speech, &at)?;
        return Ok(ready(at, false, said, &starts, length));
    };
    let turn = turn_for(&at);
    let _mine = turn
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(found) = recalled(&at, said) {
        return Ok(found);
    }
    if let Some(dir) = at.parent() {
        std::fs::create_dir_all(dir)?;
        forget_old(dir);
    }
    // Written beside and moved into place, so a player never opens half a file.
    // A failed render leaves the part behind, which is what `forget_old` sweeps.
    let partial = at.with_extension(format!("{}.part", std::process::id()));
    let (length, starts) = joined(said, speech, &partial)?;
    std::fs::rename(&partial, &at)?;
    Ok(ready(at, true, said, &starts, length))
}

/// Where a passage is kept between walks.
///
/// Named by everything that changes the sound — the engine, the voice, the pace,
/// the pause and the words of every piece — so a deck opened twice is rendered
/// once, and a changed voice is never answered with the old one.
fn kept(said: &[Said], speech: &Speech) -> Option<PathBuf> {
    use sha2::{Digest as _, Sha256};

    let mut key = Sha256::new();
    key.update(b"deck voice 1\0");
    key.update(format!(
        "{:?}\0{}\0{}\0{}\0",
        speech.engine,
        speech.voice.as_deref().unwrap_or_default(),
        speech.words_a_minute(),
        speech.pause
    ));
    for piece in said {
        key.update(piece.text.as_bytes());
        key.update(b"\0");
    }
    let name: String = key
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    Some(
        deck_core::home::deck()?
            .join("voice")
            .join(format!("{name}.wav")),
    )
}

/// The lock that lets only one thread make a given file.
fn turn_for(at: &Path) -> std::sync::Arc<std::sync::Mutex<()>> {
    type Turns =
        std::sync::Mutex<std::collections::HashMap<PathBuf, std::sync::Arc<std::sync::Mutex<()>>>>;
    static TURNS: std::sync::OnceLock<Turns> = std::sync::OnceLock::new();
    TURNS
        .get_or_init(Turns::default)
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .entry(at.to_path_buf())
        .or_default()
        .clone()
}

/// A passage made on an earlier walk, if it is on disk and still fits.
fn recalled(at: &Path, said: &[Said]) -> Option<Ready> {
    let bytes = std::fs::read(at).ok()?;
    let (starts, sound) = layout(&bytes)?;
    if starts.len() != said.len() {
        return None;
    }
    // Touched, so clearing out goes by when a sound was last wanted rather than
    // when it was first made.
    if let Ok(file) = std::fs::File::options().append(true).open(at) {
        let _ = file.set_modified(std::time::SystemTime::now());
    }
    Some(ready(at.to_path_buf(), true, said, &starts, sound))
}

fn ready(at: PathBuf, keep: bool, said: &[Said], starts: &[usize], bytes: usize) -> Ready {
    Ready {
        sound: Sound::File { at, keep },
        marks: said
            .iter()
            .zip(starts)
            .map(|(piece, start)| (lasting(*start), piece.point))
            .collect(),
        length: lasting(bytes),
        words: said.iter().map(|piece| piece.words).collect(),
    }
}

/// Clear out sounds nobody has wanted for a month, once a run.
///
/// Without it the cache is every sentence ever narrated to this machine.
fn forget_old(dir: &Path) {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        let now = std::time::SystemTime::now();
        for entry in entries.flatten() {
            let path = entry.path();
            let partial = path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(".part"));
            let limit = if partial {
                Duration::from_secs(60 * 60 * 24)
            } else {
                Duration::from_secs(60 * 60 * 24 * 30)
            };
            let old = entry
                .metadata()
                .and_then(|meta| meta.modified())
                .ok()
                .and_then(|when| now.duration_since(when).ok())
                .is_some_and(|age| age > limit);
            if old {
                let _ = std::fs::remove_file(path);
            }
        }
    });
}

/// Every piece rendered at once, trimmed, and laid end to end.
///
/// All at once, so the wait before the voice starts is the slowest piece rather
/// than all of them added up. Returns the sound, and the byte each piece starts
/// at in it.
fn joined(said: &[Said], speech: &Speech, into: &Path) -> anyhow::Result<(usize, Vec<usize>)> {
    let pieces: Vec<anyhow::Result<Vec<u8>>> = std::thread::scope(|scope| {
        let working: Vec<_> = said
            .iter()
            .map(|piece| scope.spawn(move || samples(&synthesised(&piece.text, speech)?)))
            .collect();
        working
            .into_iter()
            .map(|work| {
                work.join()
                    .unwrap_or_else(|_| Err(anyhow::anyhow!("a voice thread stopped")))
            })
            .collect()
    });

    // Straight onto the disk, not into a buffer. The tape used to be built in
    // memory and then copied a second time to put a header on it, so a long
    // answer was held three times over at the peak: the pieces, the tape, and
    // the file about to be written. Now one piece is in memory at a time.
    //
    // The header goes down first with its sizes left blank — they are only
    // known once the last piece has landed — and is written again at the end,
    // over itself, with the real length and the real starts in it. Both
    // writes are the same size because the number of pieces is known from
    // the start, which is the whole reason this works.
    let mut tape = std::fs::File::create(into)?;
    tape.write_all(&header(0, &vec![0; said.len()]))?;
    let mut starts = Vec::with_capacity(said.len());
    let mut length = 0;
    for audio in pieces {
        let audio = audio?;
        let sound = trimmed(&audio);
        starts.push(length);
        tape.write_all(sound)?;
        length += sound.len();
    }
    tape.seek(std::io::SeekFrom::Start(0))?;
    tape.write_all(&header(length, &starts))?;
    Ok((length, starts))
}

/// One piece as a WAV file's bytes, from whichever engine renders.
fn synthesised(text: &str, speech: &Speech) -> anyhow::Result<Vec<u8>> {
    match speech.engine {
        Engine::Google => fetch(text, speech),
        _ => said_by_system(text, speech),
    }
}

/// A piece with the engine's run-up silence taken off the front.
///
/// Google opens every clip with nearly half a second of nothing. Harmless on
/// one sentence; laid end to end it put that half second in front of every
/// point, which is the stall this exists to remove. A little is kept, so a
/// soft first consonant is not clipped.
///
/// Only the front. The end of a piece is where the agent's own beat before a
/// point is, and that silence is meant.
fn trimmed(pcm: &[u8]) -> &[u8] {
    const LOUD: u16 = 300;
    // Eighty milliseconds: one channel, sixteen bits, at the agreed rate.
    const KEEP: usize = 24_000 * 2 * 80 / 1000;
    let first = pcm
        .as_chunks::<2>()
        .0
        .iter()
        .position(|sample| i16::from_le_bytes(*sample).unsigned_abs() > LOUD);
    first.map_or(pcm, |ix| &pcm[(ix * 2).saturating_sub(KEEP)..])
}

/// The sound in a WAV file, checked to be the one shape deck joins.
fn samples(wav: &[u8]) -> anyhow::Result<Vec<u8>> {
    anyhow::ensure!(
        wav.len() >= 12 && &wav[..4] == b"RIFF" && &wav[8..12] == b"WAVE",
        "the voice sent something that is not a WAV file"
    );
    let mut at = 12;
    let mut joinable = false;
    while at + 8 <= wav.len() {
        let id = &wav[at..at + 4];
        let size = usize::try_from(u32::from_le_bytes([
            wav[at + 4],
            wav[at + 5],
            wav[at + 6],
            wav[at + 7],
        ]))?;
        let body = &wav[at + 8..(at + 8).saturating_add(size).min(wav.len())];
        if id == b"fmt " && body.len() >= 16 {
            let word = |ix: usize| u16::from_le_bytes([body[ix], body[ix + 1]]);
            let rate = u32::from_le_bytes([body[4], body[5], body[6], body[7]]);
            // Plain samples, one channel, the agreed rate, sixteen bits.
            joinable = word(0) == 1 && word(2) == 1 && rate == RATE && word(14) == 16;
        }
        if id == b"data" {
            anyhow::ensure!(joinable, "the voice sent sound in a shape deck cannot join");
            // An odd byte would put every later sample out of step by half.
            return Ok(body[..body.len() & !1].to_vec());
        }
        at = at.saturating_add(8 + size + (size & 1));
    }
    anyhow::bail!("the voice sent a WAV file with no sound in it")
}

/// Where each piece starts in a joined sound, as written into the file.
///
/// A chunk of deck's own. Players skip chunks they do not know, so the file
/// stays an ordinary WAV, and the timing travels with the sound it describes
/// rather than in a second file that could go missing.
const STARTS: &[u8; 4] = b"dkpc";

/// Samples wrapped in the header a player needs, with the piece starts.
///
/// Only the tests build a whole file in memory now. The voice writes the
/// header, then the sound, then the header again over itself.
#[cfg(test)]
fn wav(pcm: &[u8], starts: &[usize]) -> Vec<u8> {
    [header(pcm.len(), starts), pcm.to_vec()].concat()
}

/// The bytes in front of the sound: what a player needs, and where each piece
/// of the passage starts.
///
/// Its size depends on how many pieces there are and not on how long they are,
/// so it can be written before a single sample exists and written again,
/// exactly over itself, once they are all down.
fn header(length: usize, starts: &[usize]) -> Vec<u8> {
    let length = u32::try_from(length).unwrap_or(u32::MAX);
    let marks: Vec<u8> = starts
        .iter()
        .flat_map(|start| u32::try_from(*start).unwrap_or(u32::MAX).to_le_bytes())
        .collect();
    let marks_length = u32::try_from(marks.len()).unwrap_or(u32::MAX);
    let mut out = Vec::with_capacity(marks.len() + 52);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(
        &length
            .saturating_add(marks_length)
            .saturating_add(44)
            .to_le_bytes(),
    );
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&RATE.to_le_bytes());
    out.extend_from_slice(&(RATE * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(STARTS);
    out.extend_from_slice(&marks_length.to_le_bytes());
    out.extend_from_slice(&marks);
    out.extend_from_slice(b"data");
    out.extend_from_slice(&length.to_le_bytes());
    out
}

/// The piece starts and the length of the sound, from a file deck wrote.
fn layout(wav: &[u8]) -> Option<(Vec<usize>, usize)> {
    let mut at = 12;
    let mut starts = None;
    while at + 8 <= wav.len() {
        let id = &wav[at..at + 4];
        let size =
            usize::try_from(u32::from_le_bytes(wav[at + 4..at + 8].try_into().ok()?)).ok()?;
        let body = wav.get(at + 8..at + 8 + size)?;
        if id == STARTS {
            starts = Some(
                body.as_chunks::<4>()
                    .0
                    .iter()
                    .filter_map(|word| usize::try_from(u32::from_le_bytes(*word)).ok())
                    .collect(),
            );
        }
        if id == b"data" {
            return Some((starts?, size));
        }
        at += 8 + size + (size & 1);
    }
    None
}

/// A fresh file name in the temporary directory.
///
/// A name per use. A shared name let a second render overwrite a file the
/// player still had open, and pieces now render side by side.
fn scratch(extension: &str) -> PathBuf {
    static MADE: AtomicU64 = AtomicU64::new(0);
    let made = MADE.fetch_add(1, AtomicOrdering::Relaxed);
    std::env::temp_dir().join(format!(
        "deck-said-{}-{made}.{extension}",
        std::process::id()
    ))
}

/// The text as this platform's synthesiser wants it.
///
/// `[[slnc n]]` is macOS's own instruction for a pause and is what gives the
/// paragraph gaps. Anywhere else it is four brackets and a number that would be
/// read out, so it comes back off.
fn prepared(text: &str, speech: &Speech) -> String {
    // Beats are written in Google's spelling, because one of the engines has to
    // win and that is the one that understands them natively.
    //
    // macOS `say` has its own instruction for silence and can be told exactly.
    // Everything else would read the brackets out loud, so they come off — the
    // gap is deck's to keep, not the engine's to understand.
    if speech.engine == Engine::System && cfg!(target_os = "macos") {
        let long = format!("[[slnc {}]]", speech.pause);
        let short = format!("[[slnc {}]]", speech.pause / 2);
        return text
            .replace("[pause long]", &long)
            .replace("[pause short]", &short)
            .replace("[pause]", &short);
    }
    let text = crate::prose::unbeat(text);
    let mut out = String::with_capacity(text.len());
    let mut rest = text.as_str();
    while let Some(at) = rest.find("[[") {
        out.push_str(&rest[..at]);
        match rest[at..].find("]]") {
            Some(end) => rest = &rest[at + end + 2..],
            None => return out,
        }
    }
    out.push_str(rest);
    out
}

/// Say one line now, and wait for it, so setup can prove a key works.
///
/// Through the same path a deck uses, so a voice that passes here is a voice
/// that will work in a walk.
///
/// # Errors
///
/// When the engine cannot speak, which is the whole point of calling it.
pub fn test(speech: &Speech) -> anyhow::Result<()> {
    let speech = Speech {
        aloud: true,
        ..speech.clone()
    };
    let said = [Said {
        point: None,
        text: "Deck will read your decks in this voice.".to_string(),
        words: 8,
    }];
    let ready = if renders(&speech) {
        rendered(&said, &speech)?
    } else {
        timed(&said, &speech)
    };
    let mut playing = Playing::start(ready, None, &speech)
        .ok_or_else(|| anyhow::anyhow!("no voice to speak with"))?;
    while !playing.over() {
        std::thread::sleep(Duration::from_millis(40));
    }
    Ok(())
}

/// One piece, rendered by Google.
///
/// Blocking, and only ever called from a worker thread: a slow network must
/// never hold up the paint.
///
/// # Errors
///
/// When the key is missing, the request fails, or the reply is not audio.
fn fetch(text: &str, speech: &Speech) -> anyhow::Result<Vec<u8>> {
    use base64::Engine as _;

    let key = speech
        .secret()
        .ok_or_else(|| anyhow::anyhow!("no key: set DECK_SPEECH_KEY or run `deck walk`"))?;
    let voice = speech
        .voice
        .clone()
        .unwrap_or_else(|| "en-US-Chirp3-HD-Charon".to_string());
    // The locale is the part of the name before the third dash, and the API
    // wants it separately from the voice it already identifies.
    let language = voice.splitn(3, '-').take(2).collect::<Vec<_>>().join("-");

    let reply: serde_json::Value =
        ureq::post("https://texttospeech.googleapis.com/v1/text:synthesize")
            .header("X-Goog-Api-Key", &key)
            .send_json(serde_json::json!({
                // SSML, not text. The documented `markup` field is accepted,
                // returns audio, and reads the tags out as words — `markup:
                // "one [pause long] two"` came back the same length as `text:
                // "one pause long two"`, to the millisecond. `<break>` is the
                // one that is actually silence.
                //
                // And no `<mark>`. Chirp 3 takes the tag and returns no time
                // for it, which is why a passage is timed by joining pieces
                // rather than by asking where a mark landed.
                "input": { "ssml": ssml(text, speech.pause) },
                "voice": { "languageCode": language, "name": voice },
                "audioConfig": {
                    // Raw samples at an agreed rate, not MP3, so pieces can be
                    // laid end to end and measured by counting.
                    "audioEncoding": "LINEAR16",
                    "sampleRateHertz": RATE,
                    // The reader's own pace, expressed the way this API takes it:
                    // a multiple of its own normal speed rather than words a minute.
                    "speakingRate": f64::from(speech.words_a_minute()) / 175.0,
                },
            }))
            .map_err(|why| anyhow::anyhow!("google would not speak: {why}"))?
            .body_mut()
            .with_config()
            // A long answer in raw samples is a few megabytes, past the default.
            .limit(64 * 1024 * 1024)
            .read_json()
            .map_err(|why| anyhow::anyhow!("google sent something that is not audio: {why}"))?;

    let encoded = reply
        .get("audioContent")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("google sent no audio"))?;
    base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|why| anyhow::anyhow!("google sent audio that will not decode: {why}"))
}

/// One piece, rendered by the system voice into a file instead of the speakers.
///
/// macOS only, where `say` can write the agreed shape of sound directly.
fn said_by_system(text: &str, speech: &Speech) -> anyhow::Result<Vec<u8>> {
    let at = scratch("wav");
    let mut say = Command::new("say");
    say.arg("-r")
        .arg(speech.words_a_minute().to_string())
        .arg("-o")
        .arg(&at)
        .arg("--file-format=WAVE")
        .arg(format!("--data-format=LEI16@{RATE}"));
    if let Some(voice) = speech.voice.as_deref().filter(|name| !name.is_empty()) {
        say.arg("-v").arg(voice);
    }
    let mut child = say.stdin(Stdio::piped()).spawn()?;
    if let Some(stdin) = child.stdin.as_mut() {
        stdin.write_all(prepared(text, speech).as_bytes())?;
    }
    drop(child.stdin.take());
    let finished = child.wait()?;
    let audio = std::fs::read(&at);
    let _ = std::fs::remove_file(&at);
    anyhow::ensure!(finished.success(), "the system voice would not speak");
    Ok(audio?)
}

/// The narration as SSML, with the beats turned into real silence.
///
/// Escaped first and marked up second, so a narration full of `&&`, `<` and `>`
/// — which prose about code always is — cannot close a tag it did not open.
fn ssml(text: &str, pause: u16) -> String {
    let escaped = text
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    let long = format!("<break time=\"{pause}ms\"/>");
    let short = format!("<break time=\"{}ms\"/>", pause / 2);
    let body = escaped
        .replace("[pause long]", &long)
        .replace("[pause short]", &short)
        .replace("[pause]", &format!("<break time=\"{}ms\"/>", pause * 3 / 4));
    format!("<speak>{body}</speak>")
}

/// Play a file, with whatever this machine plays files with.
fn play(at: &Path) -> Option<Child> {
    let (program, args): (&str, &[&str]) = if cfg!(target_os = "macos") {
        ("afplay", &[])
    } else if cfg!(target_os = "windows") {
        ("powershell", &["-NoProfile", "-Command"])
    } else {
        ("ffplay", &["-nodisp", "-autoexit", "-loglevel", "quiet"])
    };

    if cfg!(target_os = "windows") {
        return Command::new(program)
            .args(args)
            .arg(format!(
                "(New-Object Media.SoundPlayer '{}').PlaySync()",
                at.display()
            ))
            .spawn()
            .ok();
    }
    Command::new(program).args(args).arg(at).spawn().ok()
}

/// Start a synthesiser reading stdin, if there is one to start.
///
/// The reader's own command first. That is the whole provider story: deck pipes
/// text to a program and plays nothing itself, so Kokoro, Piper, Fish Audio,
/// ElevenLabs and whatever ships next are all reachable without deck learning
/// any of them — and the reader decides what leaves their machine.
fn start(_text: &str, speech: &Speech) -> Option<Child> {
    if speech.engine == Engine::Command {
        return spoken_by(speech.command.as_deref()?);
    }
    system(speech)
}

/// Run the reader's own program, reading text on stdin.
///
/// Split on whitespace rather than shelled out. A shell would mean quoting
/// rules, an extra process, and a config field that can run arbitrary pipelines
/// — and the thing on the other end only ever needs a program and its flags.
fn spoken_by(command: &str) -> Option<Child> {
    let mut words = command.split_whitespace();
    let program = words.next()?;
    Command::new(program)
        .args(words)
        .stdin(Stdio::piped())
        .spawn()
        .ok()
}

/// Whatever this machine already has.
fn system(speech: &Speech) -> Option<Child> {
    let rate = speech.words_a_minute();
    let voice = speech.voice.as_deref().filter(|name| !name.is_empty());

    #[cfg(target_os = "macos")]
    {
        let mut say = Command::new("say");
        say.arg("-r").arg(rate.to_string());
        if let Some(voice) = voice {
            say.arg("-v").arg(voice);
        }
        say.stdin(Stdio::piped()).spawn().ok()
    }

    #[cfg(target_os = "linux")]
    {
        // speech-dispatcher takes a rate from -100 to 100 rather than words a
        // minute, with 0 sitting around the 175 the other platforms default to.
        let scaled = (i32::from(rate) - 175) / 2;
        let mut spd = Command::new("spd-say");
        spd.arg("-e")
            .arg("-r")
            .arg(scaled.clamp(-100, 100).to_string());
        if let Some(voice) = voice {
            spd.arg("-y").arg(voice);
        }
        spd.stdin(Stdio::piped()).spawn().ok()
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        // Windows has no system synthesiser deck can pipe into. `engine =
        // "command"` is the answer there, and setup says so.
        let _ = (rate, voice);
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A passage of one piece, pointing nowhere.
    fn one(text: &str) -> Vec<Said> {
        vec![Said {
            point: None,
            text: text.to_string(),
            words: text.split_whitespace().count(),
        }]
    }

    #[test]
    fn the_speech_pump_retires_only_when_playback_and_the_queue_are_empty() {
        let mut voice = Voice::default();
        assert!(
            !voice.has_work(),
            "an idle voice must not keep waking the window"
        );
        voice.next.push_back((one("another turn"), None));
        assert!(
            voice.has_work(),
            "a gap before queued speech is not completion"
        );
        voice.hush();
        assert!(!voice.has_work(), "interrupting also retires queued work");
    }

    #[test]
    fn the_system_voice_keeps_the_pause_it_understands() {
        // On macOS `say` reads `[[slnc n]]` as an instruction, which is the
        // whole reason it is in the text.
        let speech = Speech::default();
        if cfg!(target_os = "macos") {
            assert_eq!(
                prepared("one [[slnc 400]] two", &speech),
                "one [[slnc 400]] two"
            );
        }
    }

    #[test]
    fn another_engine_never_hears_a_pause_marker() {
        // Piped to Kokoro or an ElevenLabs script those brackets are four
        // characters and a number a listener would hear. The gap is deck's to
        // keep, not the engine's to understand.
        let speech = Speech {
            engine: Engine::Command,
            command: Some("some-voice -".into()),
            ..Speech::default()
        };
        assert_eq!(prepared("one [[slnc 400]] two", &speech), "one  two");
    }

    #[test]
    fn a_command_engine_with_nothing_to_run_stays_quiet() {
        // Rather than falling back to the system voice, which would be deck
        // quietly ignoring what the reader configured.
        let speech = Speech {
            engine: Engine::Command,
            command: None,
            ..Speech::default()
        };
        assert!(start("anything", &speech).is_none());
    }

    #[test]
    fn a_beat_becomes_real_silence() {
        // The documented `markup` field is accepted, returns audio, and reads
        // the tags out as words — `markup: "one [pause long] two"` came back
        // the same length as `text: "one pause long two"`, to the millisecond.
        // `<break>` is the one that is actually silence.
        let said = ssml("one [pause long] two [pause short] three", 420);
        assert!(said.starts_with("<speak>") && said.ends_with("</speak>"));
        assert!(said.contains(r#"<break time="420ms"/>"#), "{said}");
        assert!(said.contains(r#"<break time="210ms"/>"#), "{said}");
        assert!(
            !said.contains("[pause"),
            "no tag survives to be read: {said}"
        );
    }

    #[test]
    fn code_in_the_narration_cannot_break_the_markup() {
        // Prose about code is full of `&&`, `<` and `>`. Unescaped, the first
        // one closes a tag nobody opened and the whole request is rejected —
        // or worse, silently mangled.
        let said = ssml("if a && b < c > d", 420);
        assert!(said.contains("&amp;&amp;"), "{said}");
        assert!(said.contains("&lt;") && said.contains("&gt;"), "{said}");
        assert_eq!(
            said.matches("<break").count() + said.matches("<speak").count(),
            1,
            "the only tags are deck's own"
        );
    }

    #[test]
    fn a_fetch_in_flight_is_still_collected() {
        // The deadlock this exists for: `talking` counts a fetch in flight as
        // talking, and `pump` used to check that before looking in the channel
        // — so once a cloud utterance started, the audio was never collected
        // and the panel said "speaking" for ever.
        let (send, receive) = std::sync::mpsc::channel();
        let mut voice = Voice::default();
        voice.fetching = Some((receive, None));
        assert!(voice.talking(), "something is on its way");

        // The worker finishes and finds nobody listening if pump returns early.
        drop(send);
        voice.pump(&Speech::default());
        assert!(
            voice.fetching.is_none(),
            "pump reached the channel rather than returning at the door"
        );
    }

    #[test]
    fn a_reply_of_three_sentences_is_heard_as_three() {
        // Saying a second thing used to kill the first mid-word, so an agent
        // that answered in three sentences was heard saying only the last one.
        // A reply is not an interruption of itself.
        let mut voice = Voice::default();
        let speech = Speech {
            engine: Engine::Command,
            // Nothing to run, so nothing is spawned and nothing is spoken —
            // but the queue is the thing under test, not the sound.
            command: None,
            ..Speech::default()
        };
        voice.say(one("first"), None, &speech);
        voice.say(one("second"), None, &speech);
        voice.say(one("third"), None, &speech);

        // With no synthesiser the queue drains rather than stalling, which is
        // the other half: a machine that cannot speak must not silently hold a
        // backlog for ever.
        assert!(voice.next.is_empty());
    }

    #[test]
    fn what_is_queued_is_kept_in_order() {
        let mut voice = Voice::default();
        voice.next.push_back((one("first"), None));
        voice.next.push_back((one("second"), None));
        assert_eq!(
            voice.next.front().map(|(said, _)| said),
            Some(&one("first"))
        );
    }

    #[test]
    fn hushing_forgets_what_was_still_to_come() {
        // The reader asked for quiet. Finishing the backlog first would be
        // ignoring them politely.
        let mut voice = Voice::default();
        voice.next.push_back((one("one"), None));
        voice.next.push_back((one("two"), None));
        voice.hush();
        assert!(voice.next.is_empty());
    }

    #[test]
    fn the_point_waits_for_the_words_it_belongs_to() {
        // The bug this exists for: an agent writes four sentences in one
        // breath, so applying each point as its command arrived put the light
        // on the last line before the first sentence had been heard.
        //
        // Walked silently, so the timing is the words' and not a player's.
        let speech = Speech {
            aloud: false,
            rate: 80,
            ..Speech::default()
        };
        let mut voice = Voice::default();
        let (guard, thrown) = (LineRange::new(106, 110), LineRange::new(140, 140));

        voice.say(
            vec![
                Said {
                    point: Some(guard),
                    text: "the guard is here and it asks whether anything changed".into(),
                    words: 10,
                },
                Said {
                    point: Some(thrown),
                    text: "and the answer is thrown away".into(),
                    words: 6,
                },
            ],
            None,
            &speech,
        );
        assert_eq!(
            voice.pointing(),
            Some(guard),
            "the first one starts at once"
        );
        assert!(!voice.talking(), "and nothing is being said out loud");
        assert!(voice.has_work(), "but the passage is still being walked");
    }

    #[test]
    fn a_passage_points_on_its_own_clock() {
        // The pieces of one passage are one sound. Each point arrives at the
        // moment its words start inside it — never at the moment a separate
        // utterance happened to be fetched.
        let (guard, thrown) = (LineRange::new(106, 110), LineRange::new(140, 140));
        let playing = Playing {
            child: None,
            file: None,
            quiet: true,
            since: Instant::now() - Duration::from_millis(2_000),
            marks: vec![
                (Duration::ZERO, Some(guard)),
                (Duration::from_millis(1_900), Some(thrown)),
                (Duration::from_millis(9_000), None),
            ],
            length: Duration::from_millis(12_000),
            words: Vec::new(),
            of: None,
        };
        assert_eq!(playing.pointing(), Some(thrown));
    }

    #[test]
    fn the_word_being_heard_is_found_inside_its_piece() {
        // Two pieces of four words each; halfway through the second is the
        // seventh word, counted from nought.
        let playing = Playing {
            child: None,
            file: None,
            quiet: true,
            since: Instant::now() - Duration::from_millis(3_000) + LEAD,
            marks: vec![(Duration::ZERO, None), (Duration::from_millis(2_000), None)],
            length: Duration::from_millis(4_000),
            words: vec![4, 4],
            of: Some(Narration::Group(0)),
        };
        assert_eq!(playing.hearing(), Some((Narration::Group(0), 6)));
    }

    #[test]
    fn a_point_rises_in_the_breath_before_its_words() {
        // The light takes a moment to come up, so it starts a little early.
        // Here the second piece is 100ms away and already pointed at.
        let thrown = LineRange::new(140, 140);
        let playing = Playing {
            child: None,
            file: None,
            quiet: true,
            since: Instant::now() - Duration::from_millis(1_000),
            marks: vec![
                (Duration::ZERO, None),
                (Duration::from_millis(1_100), Some(thrown)),
            ],
            length: Duration::from_millis(3_000),
            words: Vec::new(),
            of: None,
        };
        assert_eq!(playing.pointing(), Some(thrown));
    }

    #[test]
    fn a_finished_argument_keeps_its_finger_where_it_ended() {
        // Silence is not a reason to stop pointing. The lines the narration
        // ended on are the lines the reader is looking at, and taking the
        // light off them the moment the voice stops would leave them reading
        // the whole range again to find what was being talked about.
        let speech = Speech {
            aloud: false,
            ..Speech::default()
        };
        let mut voice = Voice::default();
        voice.say(
            vec![Said {
                point: Some(LineRange::new(12, 14)),
                text: "done".into(),
                words: 1,
            }],
            None,
            &speech,
        );
        voice.hush();
        assert_eq!(voice.pointing(), Some(LineRange::new(12, 14)));
        assert!(!voice.has_work(), "and the silent walk is over");
    }

    #[test]
    #[cfg(target_os = "macos")]
    fn a_tape_written_to_disk_reads_back_as_one_sound() {
        // The whole path, with the voice this machine already has: two pieces
        // rendered, trimmed, streamed into a file, and the header written twice
        // over the same bytes. A seek off by one would leave a file no player
        // opens, and nothing else here would notice.
        let speech = Speech {
            aloud: true,
            engine: Engine::System,
            voice: None,
            ..Speech::default()
        };
        let said = vec![
            Said {
                point: None,
                text: "one two three".into(),
                words: 3,
            },
            Said {
                point: None,
                text: "four five six".into(),
                words: 3,
            },
        ];

        let at = scratch("wav");
        let (length, starts) = joined(&said, &speech, &at).expect("the system voice renders");
        let bytes = std::fs::read(&at).expect("a tape on disk");
        let _ = std::fs::remove_file(&at);

        assert_eq!(layout(&bytes), Some((starts.clone(), length)));
        assert_eq!(samples(&bytes).map(|sound| sound.len()).ok(), Some(length));
        assert_eq!(starts.first(), Some(&0));
        assert!(
            starts[1] > 0 && starts[1] < length,
            "the second piece starts inside the tape: {starts:?} of {length}"
        );
    }

    #[test]
    fn the_header_is_the_same_size_before_and_after_the_sound() {
        // The tape is written by putting a blank header down, streaming the
        // pieces after it, and writing the header again over itself. That only
        // works while its size depends on how many pieces there are and not on
        // how long they turned out to be.
        let blank = header(0, &[0, 0, 0]);
        let filled = header(4_800, &[0, 1_200, 3_600]);
        assert_eq!(blank.len(), filled.len());
        assert_ne!(
            blank, filled,
            "and the second write has to say something new"
        );
    }

    #[test]
    fn joined_sound_says_where_every_piece_starts() {
        // The timing is written into the file beside the sound, so a passage
        // made on an earlier walk is pointed exactly as the first time.
        let pcm = vec![0u8; 4_800];
        let file = wav(&pcm, &[0, 2_400]);
        assert_eq!(layout(&file), Some((vec![0, 2_400], 4_800)));
        assert_eq!(samples(&file).map(|sound| sound.len()).ok(), Some(4_800));
    }

    #[test]
    fn only_the_silence_in_front_is_taken() {
        // Google opens every clip with nearly half a second of nothing, and
        // joined end to end that was a stall at every point. The end is left
        // alone: that is where the agent's own beat before a point lives.
        let quiet = [0u8, 0].repeat(24_000);
        let loud = 4_000i16.to_le_bytes().repeat(100);
        let clip = [quiet.clone(), loud, quiet.clone()].concat();

        let kept = trimmed(&clip);
        let run_up = kept.len() - 200 - quiet.len();
        assert!(
            run_up <= 4_000,
            "about 80ms kept in front, got {run_up} bytes"
        );
        assert!(kept.ends_with(&quiet), "the tail is untouched");
    }

    #[test]
    fn a_changed_voice_is_never_answered_from_the_cache() {
        let said = one("the guard waves it through");
        let charon = Speech {
            voice: Some("en-US-Chirp3-HD-Charon".into()),
            ..Speech::default()
        };
        let other = Speech {
            voice: Some("en-US-Chirp3-HD-Kore".into()),
            ..Speech::default()
        };
        let quicker = Speech {
            rate: 210,
            ..charon.clone()
        };
        assert_eq!(kept(&said, &charon), kept(&said, &charon));
        assert_ne!(kept(&said, &charon), kept(&said, &other));
        assert_ne!(kept(&said, &charon), kept(&said, &quicker));
    }

    #[test]
    fn a_sound_in_another_shape_is_refused_rather_than_joined() {
        // Laid end to end, a clip at another rate plays at the wrong speed and
        // throws every point after it out of time.
        let mut file = wav(&[0u8; 8], &[0]);
        let rate_at = 12 + 8 + 4;
        file[rate_at..rate_at + 4].copy_from_slice(&22_050u32.to_le_bytes());
        assert!(samples(&file).is_err());
    }

    #[test]
    fn nothing_is_said_about_nothing() {
        // A group with an empty narration should not start a process, and
        // should certainly not stop one that is mid-sentence for it.
        let mut voice = Voice::default();
        voice.say(one("   "), None, &Speech::default());
        assert!(!voice.talking());
    }
}
