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

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use deck_core::config::{Engine, Speech};
use gpui_kit::{App, Global};

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

/// A voice, what it is saying, and what it has still to say.
///
/// Utterances queue. They used to replace: saying a second thing killed the
/// first mid-word, so an agent that answered in three sentences was heard
/// saying only the last one. A reply is not an interruption of itself.
#[derive(Debug, Default)]
pub struct Voice {
    said: Option<Child>,
    /// Waiting their turn, in the order they were given.
    next: std::collections::VecDeque<String>,
    /// A cloud utterance being fetched on a worker thread.
    ///
    /// Held so the paint thread never blocks on a network round trip. The
    /// window asks each frame whether it has arrived; until it has, the voice
    /// is simply not talking yet.
    fetching: Option<std::sync::mpsc::Receiver<anyhow::Result<PathBuf>>>,
}

impl Voice {
    /// Whether something is being said right now.
    ///
    /// Asks the process rather than remembering, because the interesting case
    /// is the one where it finished on its own — a reader who listened to the
    /// whole group and presses the key again means *say it again*, not *stop*.
    pub fn talking(&mut self) -> bool {
        if self.fetching.is_some() {
            return true;
        }
        match self.said.as_mut() {
            Some(child) => match child.try_wait() {
                Ok(None) => true,
                _ => {
                    self.said = None;
                    false
                }
            },
            None => false,
        }
    }

    /// Whether anything is queued or on its way.
    #[must_use]
    pub fn waiting(&self) -> bool {
        !self.next.is_empty() || self.fetching.is_some()
    }

    /// Say this after whatever is already being said.
    ///
    /// Queued rather than substituted. The reader hears a reply in the order it
    /// was given, and an agent that answers in three sentences is heard saying
    /// all three.
    pub fn say(&mut self, text: &str, speech: &Speech) {
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        self.next.push_back(text.to_string());
        self.pump(speech);
    }

    /// Start the next utterance if nothing is being said.
    ///
    /// Called as the window paints, which is how the queue advances: the child
    /// exits, the next render notices, and the following sentence begins.
    pub fn pump(&mut self, speech: &Speech) {
        // Collect a finished fetch *first*. `talking` counts a fetch in flight
        // as talking — correctly, since something is on its way — so checking
        // it before looking in the channel meant this returned early for ever
        // and the audio was never collected. The panel said "speaking" the
        // whole time, which was true and useless.
        if let Some(waiting) = self.fetching.as_ref() {
            match waiting.try_recv() {
                Err(std::sync::mpsc::TryRecvError::Empty) => return,
                Ok(Ok(at)) => {
                    self.fetching = None;
                    self.said = play(&at);
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
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.fetching = None;
                }
            }
        }

        if self.talking() {
            return;
        }
        let Some(text) = self.next.pop_front() else {
            return;
        };

        if speech.engine == Engine::Google {
            let (send, receive) = std::sync::mpsc::channel();
            let speech = speech.clone();
            std::thread::Builder::new()
                .name("deck-speech".into())
                .spawn(move || {
                    let _ = send.send(fetch(&text, &speech));
                })
                .ok();
            self.fetching = Some(receive);
            return;
        }
        let Some(mut spoken) = start(&text, speech) else {
            // Nothing can speak it, so draining the rest would only stall.
            self.next.clear();
            return;
        };
        let text = text.as_str();
        // Written to stdin rather than passed as an argument, because a
        // narration is prose: it has quotes and dashes in it, and it can be
        // longer than a command line is allowed to be.
        if let Some(stdin) = spoken.stdin.as_mut() {
            let _ = stdin.write_all(prepared(text, speech).as_bytes());
        }
        // Dropped so the child sees the end of its input and starts speaking.
        drop(spoken.stdin.take());
        self.said = Some(spoken);
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
        if let Some(mut child) = self.said.take() {
            let _ = child.kill();
            let _ = child.wait();
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
/// # Errors
///
/// When the engine cannot speak, which is the whole point of calling it.
pub fn test(speech: &Speech) -> anyhow::Result<()> {
    let line = "Deck will read your decks in this voice.";
    let mut child = match speech.engine {
        Engine::Google => play(&fetch(line, speech)?)
            .ok_or_else(|| anyhow::anyhow!("nothing on this machine plays audio"))?,
        _ => {
            let mut child =
                start(line, speech).ok_or_else(|| anyhow::anyhow!("no voice to speak with"))?;
            if let Some(stdin) = child.stdin.as_mut() {
                let _ = stdin.write_all(prepared(line, speech).as_bytes());
            }
            drop(child.stdin.take());
            child
        }
    };
    child.wait()?;
    Ok(())
}

/// Fetch spoken audio from Google, and say where it landed.
///
/// Blocking, and called from a worker thread for that reason: a slow network
/// must never hold up the paint. The audio is written to a temporary file and
/// played by whatever this platform plays files with, which keeps the rest of
/// the voice exactly as it is — something to start, and something to kill.
///
/// # Errors
///
/// When the key is missing, the request fails, or the reply is not audio.
fn fetch(text: &str, speech: &Speech) -> anyhow::Result<PathBuf> {
    use base64::Engine as _;

    let key = speech
        .secret()
        .ok_or_else(|| anyhow::anyhow!("no key: set DECK_SPEECH_KEY or run `deck live`"))?;
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
                "input": { "ssml": ssml(text, speech.pause) },
                "voice": { "languageCode": language, "name": voice },
                "audioConfig": {
                    "audioEncoding": "MP3",
                    // The reader's own pace, expressed the way this API takes it:
                    // a multiple of its own normal speed rather than words a minute.
                    "speakingRate": f64::from(speech.words_a_minute()) / 175.0,
                },
            }))
            .map_err(|why| anyhow::anyhow!("google would not speak: {why}"))?
            .body_mut()
            .read_json()
            .map_err(|why| anyhow::anyhow!("google sent something that is not audio: {why}"))?;

    let encoded = reply
        .get("audioContent")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("google sent no audio"))?;
    let audio = base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|why| anyhow::anyhow!("google sent audio that will not decode: {why}"))?;

    // A name per utterance. They shared one, so a second fetch overwrote the
    // file the player still had open.
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_nanos())
        .unwrap_or_default();
    let at = std::env::temp_dir().join(format!("deck-said-{}-{now}.mp3", std::process::id()));
    std::fs::write(&at, audio)?;
    Ok(at)
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
        voice.fetching = Some(receive);
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
        voice.say("first", &speech);
        voice.say("second", &speech);
        voice.say("third", &speech);

        // With no synthesiser the queue drains rather than stalling, which is
        // the other half: a machine that cannot speak must not silently hold a
        // backlog for ever.
        assert!(voice.next.is_empty());
    }

    #[test]
    fn what_is_queued_is_kept_in_order() {
        let mut voice = Voice::default();
        voice.next.push_back("first".into());
        voice.next.push_back("second".into());
        assert_eq!(voice.next.front().map(String::as_str), Some("first"));
    }

    #[test]
    fn hushing_forgets_what_was_still_to_come() {
        // The reader asked for quiet. Finishing the backlog first would be
        // ignoring them politely.
        let mut voice = Voice::default();
        voice.next.push_back("one".into());
        voice.next.push_back("two".into());
        voice.hush();
        assert!(voice.next.is_empty());
    }

    #[test]
    fn nothing_is_said_about_nothing() {
        // A group with an empty narration should not start a process, and
        // should certainly not stop one that is mid-sentence for it.
        let mut voice = Voice::default();
        voice.say("   ", &Speech::default());
        assert!(!voice.talking());
    }
}
