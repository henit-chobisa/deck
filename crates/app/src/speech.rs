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
use std::process::{Child, Command, Stdio};

use deck_core::config::Speech;
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

/// A voice the machine has, and how good it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Installed {
    /// Its name, as the synthesiser wants it back.
    pub name: String,
    /// Whether it is one of the neural ones.
    ///
    /// This is the whole quality question. The compact voices shipped by
    /// default are the ones people mean when they say a computer voice; the
    /// neural ones are a free download and are close to a recording. Nothing
    /// deck can do to the text closes that gap.
    pub natural: bool,
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
    let listed = String::from_utf8_lossy(&out.stdout);

    let mut found: Vec<Installed> = listed
        .lines()
        .filter_map(|line| {
            // `Ava (Premium)        en_US    # Hello! My name is Ava.`
            let said = line.split('#').next()?.trim_end();
            let (name, locale) = said.rsplit_once(char::is_whitespace)?;
            locale.starts_with("en").then(|| Installed {
                name: name.trim().to_string(),
                natural: name.contains("(Premium)") || name.contains("(Enhanced)"),
            })
        })
        .collect();

    // Premium and Enhanced first; within each, the order the system gave them.
    found.sort_by_key(|voice| !voice.natural);
    found
}

/// The best voice on this machine, if it has one worth naming.
///
/// `None` means every installed voice is a compact one, and deck would rather
/// leave the system default in place and say so than pick a robot on the
/// reader's behalf and let them think that is the best it does.
#[must_use]
pub fn best() -> Option<String> {
    voices()
        .into_iter()
        .find(|voice| voice.natural)
        .map(|voice| voice.name)
}

/// Where a reader goes to get a voice worth listening to.
///
/// Worth printing in full. Nobody finds this by looking, and the difference it
/// makes is the difference between the feature working and the feature being
/// switched off after one paragraph.
pub const WHERE: &str = "System Settings → Accessibility → Spoken Content → \n    System Voice → Manage Voices… → English, and pick any marked Premium";

/// A voice, and whatever it is currently saying.
#[derive(Debug, Default)]
pub struct Voice {
    said: Option<Child>,
}

impl Voice {
    /// Whether something is being said right now.
    ///
    /// Asks the process rather than remembering, because the interesting case
    /// is the one where it finished on its own — a reader who listened to the
    /// whole group and presses the key again means *say it again*, not *stop*.
    pub fn talking(&mut self) -> bool {
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

    /// Say this, stopping whatever was being said before.
    ///
    /// Silent when there is no synthesiser to run: a machine without one is not
    /// a machine deck should refuse to open a deck on.
    pub fn say(&mut self, text: &str, speech: &Speech) {
        self.hush();
        let text = text.trim();
        if text.is_empty() {
            return;
        }

        let Some(mut spoken) = start(text, speech) else {
            return;
        };
        // Written to stdin rather than passed as an argument, because a
        // narration is prose: it has quotes and dashes in it, and it can be
        // longer than a command line is allowed to be.
        if let Some(stdin) = spoken.stdin.as_mut() {
            let _ = stdin.write_all(prepared(text).as_bytes());
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
fn prepared(text: &str) -> String {
    if cfg!(target_os = "macos") {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
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

/// Start a synthesiser reading stdin, if this machine has one.
fn start(_text: &str, speech: &Speech) -> Option<Child> {
    let rate = speech.words_a_minute();

    #[cfg(target_os = "macos")]
    {
        let mut say = Command::new("say");
        say.arg("-r").arg(rate.to_string());
        if let Some(voice) = speech.voice.as_deref().filter(|name| !name.is_empty()) {
            say.arg("-v").arg(voice);
        }
        return say.stdin(Stdio::piped()).spawn().ok();
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
        if let Some(voice) = speech.voice.as_deref().filter(|name| !name.is_empty()) {
            spd.arg("-y").arg(voice);
        }
        return spd.stdin(Stdio::piped()).spawn().ok();
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = rate;
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pause_is_left_alone_where_it_means_something() {
        // On macOS the synthesiser reads it as an instruction, which is the
        // whole reason it is in the text.
        if cfg!(target_os = "macos") {
            assert_eq!(prepared("one [[slnc 400]] two"), "one [[slnc 400]] two");
        }
    }

    #[test]
    fn a_pause_is_taken_out_where_it_would_be_read_aloud() {
        // Everywhere else those brackets are four characters and a number that
        // a listener would hear.
        if !cfg!(target_os = "macos") {
            assert_eq!(prepared("one [[slnc 400]] two"), "one  two");
        }
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
