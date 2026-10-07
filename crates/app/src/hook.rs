//! Catching a reply that should have been a deck.
//!
//! The skill in [`crate::skill`] is a suggestion, and the line it puts in front
//! of an agent is a good one. Neither binds. An agent that has just finished an
//! investigation has enormous momentum toward typing out what it found, and a
//! suggestion loses to that momentum often enough to make deck feel optional —
//! which is the same as not being installed.
//!
//! This does not lose. It reads the reply that was about to be sent, counts the
//! `file:line` locations in it, and if there are two or more it refuses the stop
//! and says to build the deck instead. The agent is still mid-turn at that
//! point, still holding everything it just learned, and can spend it on a deck
//! rather than on prose.
//!
//! # Why a subcommand and not a script
//!
//! The obvious shape is a small Python file dropped next to the settings that
//! name it. It costs an interpreter deck does not otherwise need, a second
//! thing on disk that can drift from the binary that wrote it, and every
//! Windows machine. `deck hook` is already on `PATH` — the same reason the
//! skill is carried in the binary rather than installed beside it.
//!
//! # Why `Stop` and not something earlier
//!
//! Every earlier hook fires while the agent is still deciding, when there is no
//! reply to judge. `Stop` is the last moment the turn is still open: the answer
//! exists, and it has not been sent.
//!
//! # Why it says nothing in every doubtful case
//!
//! A hook that fires when it should not is worse than one that never fires,
//! because it trains the reader to turn it off. So: silent when a deck was
//! already built this turn, silent when the reply is about deck itself, silent
//! on a second firing, and silent when it cannot read the transcript. It speaks
//! only when it is sure, and even then it says what to do rather than refusing
//! flatly.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

/// How many locations make a reply a deck.
///
/// One is a pointer — "the bug is at `view.rs:1109`" is a sentence, and a
/// window would be ceremony. Two is an argument: the reader has to open both,
/// hold the first in their head while they find the second, and assemble the
/// point themselves. That assembly is the work deck exists to do.
const ENOUGH: usize = 2;

/// Read a `Stop` event on stdin and decide whether to let the turn end.
///
/// Always exits zero. A hook that fails loudly on a malformed event turns every
/// bug in this file into a wedged agent, so anything unreadable is a silence.
///
/// # Errors
///
/// Never. The signature matches the other verbs so [`crate::cli`] can treat it
/// the same way.
pub fn run() -> anyhow::Result<()> {
    let Ok(event) = serde_json::from_reader::<_, Value>(std::io::stdin()) else {
        return Ok(());
    };

    // The other event this answers: a session picking up after a compaction.
    if event.get("hook_event_name").and_then(Value::as_str) == Some(AFTER) {
        let used = event
            .get("transcript_path")
            .and_then(Value::as_str)
            .is_some_and(|at| used_deck(Path::new(at)));
        if let Some(again) = reload(&event, used) {
            println!(
                "{}",
                json!({ "hookSpecificOutput": {
                    "hookEventName": AFTER,
                    "additionalContext": again,
                } })
            );
        }
        return Ok(());
    }

    // Already continuing from a stop hook. Saying it twice is nagging, and the
    // agent has already been told once this turn.
    if event.get("stop_hook_active").and_then(Value::as_bool) == Some(true) {
        return Ok(());
    }

    let Some(at) = event.get("transcript_path").and_then(Value::as_str) else {
        return Ok(());
    };
    let said = last_reply(Path::new(at));
    let Some(reason) = judge(&said) else {
        return Ok(());
    };

    println!("{}", json!({ "decision": "block", "reason": reason }));
    Ok(())
}

/// What to tell an agent whose conversation was just compacted, if anything.
///
/// A skill is loaded once and a compaction keeps only the start of it. Measured
/// on a real session of 168 groups: with the skill fresh, 98% of groups pointed
/// at their lines and the agent drew and brought files in; after one
/// compaction 66% pointed; after two, 48% did, nothing was drawn, and no
/// answer showed a file. The rules had not changed. The agent had lost them.
///
/// So it is told to load the skill again, and given the four things that go
/// first in the meantime. Only for a compaction — a fresh session has nothing
/// to have lost — and only when the session had used deck, because a reminder
/// about a tool nobody is using is noise in every other conversation.
///
/// Split from [`run`] for the same reason [`judge`] is.
#[must_use]
fn reload(event: &Value, used: bool) -> Option<String> {
    if event.get("source").and_then(Value::as_str) != Some("compact") || !used {
        return None;
    }
    Some(
        "This conversation was just compacted, and the deck skill you loaded \
         earlier was cut off with it. Before you write another `deck group`, or \
         answer in a deck, load the deck skill again with the Skill tool. Until \
         you have: every sentence points at the lines it is about with \
         `[point 12-14]`; a group has four panes at most; draw what moves, as a \
         diagram or a page; and answer a question by showing it — `deck show` \
         or `deck bring`, then point — never by naming files in prose."
            .to_string(),
    )
}

/// Whether this session has built a deck, or loaded the skill to.
fn used_deck(transcript: &Path) -> bool {
    std::fs::read_to_string(transcript).is_ok_and(|text| {
        text.contains("deck group ")
            || text.contains("deck new ")
            || text.contains("\"skill\":\"deck\"")
    })
}

/// Whether this reply should have been a deck, and what to say if so.
///
/// Split from [`run`] so the decision can be tested without a transcript on
/// disk or a process to pipe into.
#[must_use]
fn judge(said: &str) -> Option<String> {
    if said.trim().is_empty() || already(said) {
        return None;
    }
    let mut cites = citations(said);
    cites.sort_unstable();
    cites.dedup();
    if cites.len() < ENOUGH {
        return None;
    }

    let named = cites
        .iter()
        .take(4)
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(", ");
    Some(format!(
        "That reply names {} code locations ({named}). Locations in prose are \
         the thing deck exists to replace — the reader has to go and open each \
         one and hold the argument together themselves.\n\n\
         Build a deck instead: `deck new`, then `deck open` as a background \
         command, then one `deck group` per claim with the lines as `--ref`, \
         then `deck seal` and `deck wait` in the background. Read the `deck` \
         skill for how to write the groups.\n\n\
         Then say one line — that a deck is on the bar — and end the turn. Do \
         not send the prose version as well.\n\n\
         If a deck is genuinely wrong here (the reply is about deck itself, the \
         user asked for prose outright, or it is one location repeated), say so \
         in one sentence and carry on.",
        cites.len()
    ))
}

/// Whether the reply is already about a deck, or about deck.
///
/// Two cases, and they want the same silence. An agent reporting that it built
/// a deck has done the right thing and is being told off for saying so; an
/// agent explaining deck's own internals is citing code because that is the
/// subject, and a deck about deck would be a hall of mirrors.
fn already(said: &str) -> bool {
    let lower = said.to_lowercase();
    lower.contains("on the bar")
        || ["new", "group", "seal", "open", "wait"]
            .iter()
            .any(|verb| lower.contains(&format!("deck {verb}")))
}

/// The text of the final assistant message in a transcript.
///
/// The transcript is JSONL and the last text-bearing assistant row is the reply
/// about to be sent. A line that will not parse is skipped rather than fatal:
/// transcripts are written by something else, and a format that grows a field
/// should not wedge the agent.
fn last_reply(at: &Path) -> String {
    let Ok(text) = std::fs::read_to_string(at) else {
        return String::new();
    };
    let mut said = String::new();
    for line in text.lines() {
        let Ok(row) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if row.get("type").and_then(Value::as_str) != Some("assistant") {
            continue;
        }
        let parts = row
            .get("message")
            .and_then(|message| message.get("content"))
            .and_then(Value::as_array);
        let Some(parts) = parts else { continue };

        let mut whole = String::new();
        for part in parts {
            if part.get("type").and_then(Value::as_str) == Some("text")
                && let Some(one) = part.get("text").and_then(Value::as_str)
            {
                whole.push_str(one);
            }
        }
        if !whole.trim().is_empty() {
            said = whole;
        }
    }
    said
}

/// Every `file.ext:line` in a piece of prose.
///
/// Deliberately narrow. A version number, a time of day and a ratio all look a
/// little like a citation, and each false one spends a turn the reader did not
/// want spent — so this insists on a short extension immediately before the
/// colon, which `1.2.3:45` and `12:30` do not have.
///
/// Hand-written rather than a regex because it is a dozen lines and a
/// dependency is forever.
fn citations(said: &str) -> Vec<String> {
    let bytes = said.as_bytes();
    let mut found = Vec::new();
    let mut at = 0;

    while let Some(offset) = bytes[at..].iter().position(|&b| b == b':') {
        let colon = at + offset;
        at = colon + 1;

        // A line number has to follow, or this is prose punctuation.
        let Some(after) = line_number(bytes, colon + 1) else {
            continue;
        };
        // …and a short extension has to come immediately before.
        let Some(start) = path_before(bytes, colon) else {
            continue;
        };

        // The character after the number ends the word, so `main.rs:12x` — a
        // label, not a location — does not count.
        if bytes.get(after).is_some_and(|&b| word(b)) {
            continue;
        }
        found.push(said[start..after].to_string());
        at = after;
    }
    found
}

/// The end of a line number starting at `from`: digits, then optionally a dash
/// and more digits.
///
/// Both dashes, because a range typed by a model is as likely to carry an en
/// dash as a hyphen.
fn line_number(bytes: &[u8], from: usize) -> Option<usize> {
    let mut at = digits(bytes, from)?;

    let dash = if bytes.get(at) == Some(&b'-') {
        Some(at + 1)
    } else if bytes[at..].starts_with("–".as_bytes()) {
        Some(at + "–".len())
    } else {
        None
    };
    if let Some(next) = dash
        && let Some(end) = digits(bytes, next)
    {
        at = end;
    }
    Some(at)
}

/// The end of a run of one or more ASCII digits, or `None` if there are none.
fn digits(bytes: &[u8], from: usize) -> Option<usize> {
    let mut at = from;
    while bytes.get(at).is_some_and(u8::is_ascii_digit) {
        at += 1;
    }
    (at > from).then_some(at)
}

/// The start of the path ending at `colon`, if it ends in a real extension.
///
/// Walks back over a 1–6 letter extension, the dot before it, and then the rest
/// of the path — and refuses anything that runs straight into a word, so that
/// `see line foo.rs:4` is a citation and `ratio3.5:1` is not.
fn path_before(bytes: &[u8], colon: usize) -> Option<usize> {
    let mut at = colon;
    let mut ext = 0;
    while at > 0 && bytes[at - 1].is_ascii_alphabetic() && ext < 6 {
        at -= 1;
        ext += 1;
    }
    if ext == 0 || at == 0 || bytes[at - 1] != b'.' {
        return None;
    }
    at -= 1; // the dot

    let dot = at;
    while at > 0 && (word(bytes[at - 1]) || matches!(bytes[at - 1], b'.' | b'/' | b'-')) {
        at -= 1;
    }
    // Something has to precede the extension, or this is a bare `.rs:4`.
    (at < dot).then_some(at)
}

/// Whether a byte is one a word can be made of.
const fn word(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

// ─── installing ─────────────────────────────────────────────────────────────

/// Where the hook is registered, under `$HOME`.
///
/// Claude Code is the only agent deck knows of with a hook that fires on the
/// reply itself. The others are told by the skill alone until they grow one.
const SETTINGS: &str = ".claude/settings.json";

/// The event the catch answers.
const WHEN: &str = "Stop";

/// The event the reload answers, and the one cause of it that matters.
const AFTER: &str = "SessionStart";
const COMPACT: &str = "compact";

/// What is written into the settings, and what an already-installed hook is
/// recognised by.
const COMMAND: &str = "deck hook";

/// What happened when the hook was offered a home.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Put {
    /// Written into the settings at this path.
    Written(PathBuf),
    /// Already there. Running setup twice should not say anything happened.
    Already,
    /// Claude Code is not on this machine.
    Absent,
}

/// Whether the agent that can run this is installed at all.
#[must_use]
pub fn possible(home: &Path) -> bool {
    home.join(".claude").exists()
}

/// Whether the hook is already registered.
#[must_use]
pub fn installed(home: &Path) -> bool {
    let Ok(text) = std::fs::read_to_string(home.join(SETTINGS)) else {
        return false;
    };
    serde_json::from_str::<Value>(&text)
        .is_ok_and(|settings| registered(&settings) && reloaded(&settings))
}

/// Register the hook, leaving every other setting exactly as it was.
///
/// The file belongs to the reader and has their own choices in it, so it is
/// parsed, added to and written back rather than generated. A settings file
/// that will not parse is left alone entirely: a hook is not worth losing
/// somebody's configuration over.
///
/// # Errors
///
/// When the settings file exists but cannot be read, parsed or written.
pub fn install(home: &Path) -> anyhow::Result<Put> {
    if !possible(home) {
        return Ok(Put::Absent);
    }
    let at = home.join(SETTINGS);

    let mut settings = match std::fs::read_to_string(&at) {
        Ok(text) if text.trim().is_empty() => json!({}),
        Ok(text) => serde_json::from_str(&text).map_err(|why| {
            anyhow::anyhow!(
                "{} is not valid JSON ({why}), so deck has not touched it. \
                 Fix it and run `deck setup` again.",
                at.display()
            )
        })?,
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => json!({}),
        Err(why) => return Err(why.into()),
    };

    // Two entries, and either may already be there: a machine set up before
    // the reload existed has the catch and not the other.
    let (caught, reloads) = (registered(&settings), reloaded(&settings));
    if caught && reloads {
        return Ok(Put::Already);
    }

    let hooks = settings
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("{} is not a JSON object", at.display()))?
        .entry("hooks")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("`hooks` in {} is not an object", at.display()))?;
    let run = json!([{ "type": "command", "command": COMMAND }]);
    for (missing, event, entry) in [
        (!caught, WHEN, json!({ "hooks": run })),
        (!reloads, AFTER, json!({ "matcher": COMPACT, "hooks": run })),
    ] {
        if !missing {
            continue;
        }
        hooks
            .entry(event)
            .or_insert_with(|| json!([]))
            .as_array_mut()
            .ok_or_else(|| anyhow::anyhow!("`hooks.{event}` in {} is not a list", at.display()))?
            .push(entry);
    }

    if let Some(dir) = at.parent() {
        std::fs::create_dir_all(dir)?;
    }
    // Pretty, because a person opens this file and deck should not be the
    // reason it became one long line.
    std::fs::write(
        &at,
        format!("{}\n", serde_json::to_string_pretty(&settings)?),
    )?;
    Ok(Put::Written(at))
}

/// Whether these settings already name the catch, wherever it sits.
///
/// Looks at every event but the reload's rather than the one place deck
/// writes, because somebody may have moved it, and offering to install a hook
/// that is already running would be deck failing to recognise its own work.
fn registered(settings: &Value) -> bool {
    settings
        .get("hooks")
        .and_then(Value::as_object)
        .is_some_and(|hooks| {
            hooks
                .iter()
                .any(|(event, entries)| event != AFTER && names_deck(entries))
        })
}

/// Whether these settings already ask for the reload after a compaction.
fn reloaded(settings: &Value) -> bool {
    settings
        .get("hooks")
        .and_then(|hooks| hooks.get(AFTER))
        .is_some_and(names_deck)
}

/// Whether deck's command is named anywhere in this part of the settings.
fn names_deck(value: &Value) -> bool {
    match value {
        Value::String(text) => text.contains(COMMAND) || text.contains("deck-check"),
        Value::Array(list) => list.iter().any(names_deck),
        Value::Object(map) => map.values().any(names_deck),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let at = std::env::temp_dir().join(format!("deck-hook-{name}"));
        let _ = std::fs::remove_dir_all(&at);
        std::fs::create_dir_all(&at).unwrap();
        at
    }

    #[test]
    fn two_locations_is_a_deck() {
        let said = "The bug is in crates/app/src/view.rs:1109, and the enum it \
                    ignores is at protocol.rs:253.";
        let why = judge(said).expect("two locations is a deck");
        assert!(why.contains("names 2 code locations"));
        assert!(why.contains("view.rs:1109"), "and it names them back");
    }

    #[test]
    fn one_location_is_a_sentence() {
        // A single pointer is prose. Opening a window for it would be ceremony,
        // and a hook that fires on ceremony gets turned off.
        assert!(judge("The check is at view.rs:1109, right where you left it.").is_none());
    }

    #[test]
    fn the_same_location_twice_is_one_location() {
        let said = "view.rs:1109 hardcodes it, which is why view.rs:1109 is wrong.";
        assert!(judge(said).is_none(), "the reader has one place to open");
    }

    #[test]
    fn a_reply_about_a_deck_is_left_alone() {
        // The agent did the right thing and is reporting it. Blocking here
        // would punish the behaviour the hook exists to produce.
        let said = "Built it: `deck group` for each claim, covering view.rs:1109 \
                    and protocol.rs:253. A deck is on the bar.";
        assert!(judge(said).is_none());
    }

    #[test]
    fn numbers_that_only_look_like_locations_are_ignored() {
        // Every one of these would be a false firing, and a false firing is
        // how a hook gets uninstalled.
        for said in [
            "Released 1.2.3:45 and 2.0.1:9 today.",
            "Between 12:30 and 14:00, twice.",
            "A ratio of 3.5:1 against 4.5:1.",
            "See main.rs:12x and other.rs:9y for labels.",
        ] {
            assert!(judge(said).is_none(), "{said}");
        }
    }

    #[test]
    fn a_range_counts_however_the_dash_was_typed() {
        let said = "Look at view.rs:1100-1120 and then at pane.rs:40–52.";
        let why = judge(said).expect("both are citations");
        assert!(why.contains("names 2 code locations"));
    }

    #[test]
    fn a_settings_file_keeps_everything_it_had() {
        // The file is the reader's. Deck adds one entry to it and touches
        // nothing else — including keys this version has never heard of.
        let home = scratch("keeps");
        std::fs::create_dir_all(home.join(".claude")).unwrap();
        std::fs::write(
            home.join(SETTINGS),
            r#"{ "model": "opus", "hooks": { "PreToolUse": [{"mine": true}] } }"#,
        )
        .unwrap();

        assert!(matches!(install(&home).unwrap(), Put::Written(_)));

        let text = std::fs::read_to_string(home.join(SETTINGS)).unwrap();
        let after: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(after["model"], "opus", "an unrelated setting survives");
        assert!(after["hooks"]["PreToolUse"][0]["mine"].as_bool().unwrap());
        assert!(registered(&after), "and the hook is there now");
    }

    #[test]
    fn installing_twice_writes_one_hook() {
        let home = scratch("twice");
        std::fs::create_dir_all(home.join(".claude")).unwrap();

        assert!(matches!(install(&home).unwrap(), Put::Written(_)));
        assert_eq!(install(&home).unwrap(), Put::Already);

        let text = std::fs::read_to_string(home.join(SETTINGS)).unwrap();
        let after: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(
            after["hooks"][WHEN].as_array().unwrap().len(),
            1,
            "a second setup run is not a second hook"
        );
    }

    #[test]
    fn settings_that_will_not_parse_are_not_overwritten() {
        // Somebody's half-edited file is worth more than deck's hook.
        let home = scratch("broken");
        std::fs::create_dir_all(home.join(".claude")).unwrap();
        let broken = r#"{ "model": "opus", "#;
        std::fs::write(home.join(SETTINGS), broken).unwrap();

        assert!(install(&home).is_err());
        assert_eq!(
            std::fs::read_to_string(home.join(SETTINGS)).unwrap(),
            broken,
            "left exactly as it was found"
        );
    }

    #[test]
    fn an_agent_that_is_not_here_is_not_written_to() {
        let home = scratch("absent");
        assert_eq!(install(&home).unwrap(), Put::Absent);
        assert!(!home.join(".claude").exists(), "and nothing was created");
    }

    #[test]
    fn after_a_compaction_a_session_that_used_deck_is_told_to_reload() {
        let compacted = json!({ "hook_event_name": "SessionStart", "source": "compact" });
        let again = reload(&compacted, true).expect("told to reload");
        assert!(again.contains("load the deck skill again"), "{again}");
        assert!(
            again.contains("[point 12-14]"),
            "and the essentials meanwhile"
        );
    }

    #[test]
    fn a_fresh_session_or_one_without_a_deck_is_told_nothing() {
        // Nothing was lost in a session that has just started, and a reminder
        // about deck in a conversation that never used it is noise.
        let compacted = json!({ "hook_event_name": "SessionStart", "source": "compact" });
        assert!(reload(&compacted, false).is_none());
        for source in ["startup", "resume", "clear"] {
            let fresh = json!({ "hook_event_name": "SessionStart", "source": source });
            assert!(reload(&fresh, true).is_none(), "{source}");
        }
    }

    #[test]
    fn a_session_is_known_to_have_used_deck_from_its_transcript() {
        let home = scratch("used");
        let built = home.join("built.jsonl");
        std::fs::write(&built, r#"{"command":"deck group /tmp/d.deck --say x"}"#).unwrap();
        let loaded = home.join("loaded.jsonl");
        std::fs::write(&loaded, r#"{"name":"Skill","input":{"skill":"deck"}}"#).unwrap();
        let other = home.join("other.jsonl");
        std::fs::write(&other, r#"{"command":"cargo test"}"#).unwrap();
        assert!(used_deck(&built) && used_deck(&loaded));
        assert!(!used_deck(&other) && !used_deck(&home.join("missing.jsonl")));
    }

    #[test]
    fn setup_registers_the_catch_and_the_reload() {
        let home = scratch("both");
        std::fs::create_dir_all(home.join(".claude")).unwrap();
        assert!(matches!(install(&home).unwrap(), Put::Written(_)));
        let after: Value =
            serde_json::from_str(&std::fs::read_to_string(home.join(SETTINGS)).unwrap()).unwrap();
        assert_eq!(after["hooks"]["Stop"][0]["hooks"][0]["command"], COMMAND);
        assert_eq!(after["hooks"]["SessionStart"][0]["matcher"], "compact");
        assert_eq!(
            after["hooks"]["SessionStart"][0]["hooks"][0]["command"],
            COMMAND
        );
        assert!(installed(&home));
        assert_eq!(
            install(&home).unwrap(),
            Put::Already,
            "twice changes nothing"
        );
    }

    #[test]
    fn a_machine_with_only_the_catch_gains_the_reload() {
        // Set up before the reload existed: the catch is there, and must not
        // be written a second time.
        let home = scratch("older");
        std::fs::create_dir_all(home.join(".claude")).unwrap();
        std::fs::write(
            home.join(SETTINGS),
            r#"{ "hooks": { "Stop": [{ "hooks": [{ "type": "command", "command": "deck hook" }] }] } }"#,
        )
        .unwrap();
        assert!(!installed(&home), "half of it is missing");
        assert!(matches!(install(&home).unwrap(), Put::Written(_)));
        let after: Value =
            serde_json::from_str(&std::fs::read_to_string(home.join(SETTINGS)).unwrap()).unwrap();
        assert_eq!(after["hooks"]["Stop"].as_array().unwrap().len(), 1);
        assert_eq!(after["hooks"]["SessionStart"][0]["matcher"], "compact");
        assert!(installed(&home));
    }
}
