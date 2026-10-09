//! Counts of how deck is used, shared only by people who said yes.
//!
//! # What this is for
//!
//! To know what to make next: how many people use deck, on which versions, and
//! which parts of it they reach for. Nothing here is about any one person, and
//! nothing in it could tell anybody what they were working on.
//!
//! # The promise, which this file keeps
//!
//! - **Nothing before a yes.** Until somebody answers — in `deck setup`, or
//!   the one question the window asks — nothing is counted and nothing is
//!   sent. Not "on until you say no": off until you say yes.
//! - **Counts and versions, never content.** A random id made here, the
//!   version, the operating system, and how many times things happened. Never
//!   code, file paths, repository names, titles, comments or questions.
//! - **Every way of saying no works.** `deck telemetry off`, `DO_NOT_TRACK`,
//!   `DECK_TELEMETRY=0`, running under CI, or `[updates] automatic = false` —
//!   which already promises deck makes no request of its own.
//! - **Nothing hidden.** `deck telemetry show` prints exactly what the next
//!   send would carry, and `docs/telemetry.md` lists every field.
//! - **Never in the way.** Sent at most once an hour, off the main thread,
//!   with a short patience. A failure is kept for next time and never shown.

use std::collections::BTreeMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// Where the counts go: PostHog's ingestion in the EU.
const HOST: &str = "https://eu.i.posthog.com/batch/";

/// The project's key. Public by design: it can only add events, never read
/// them. A build with it emptied sends nothing.
const KEY: &str = "phc_Ck3ThjdcBuapV26zSbxsNiZBbqg52pthagE4v66hbadA";

/// How often, at most, the counts are sent.
const EVERY: Duration = Duration::from_secs(60 * 60);

/// How often a deck looks whether a send is due.
const LOOK: Duration = Duration::from_secs(5 * 60);

/// How long to wait on the network. Only a background thread is waiting.
const PATIENCE: Duration = Duration::from_secs(3);

/// What this build is.
const RUNNING: &str = env!("CARGO_PKG_VERSION");

/// Something that happened, to be counted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Count {
    /// `deck new`.
    DeckCreated,
    /// A deck window opened.
    DeckOpened,
    /// `deck group`.
    GroupWritten,
    /// A group came on screen.
    GroupViewed,
    /// A comment left on a deck.
    Comment,
    /// A review submitted.
    Review,
    /// *Ask now* pressed.
    AskNow,
    /// Suggested questions written into a group.
    QuestionsOffered,
    /// A suggested question asked straight away.
    QuestionAsked,
    /// A suggested question taken into the comment box first.
    QuestionEdited,
    /// A walk started.
    Walk,
    /// deck upgraded itself, or was upgraded.
    Upgrade,
}

impl Count {
    /// Its name in what is sent.
    fn name(self) -> &'static str {
        match self {
            Self::DeckCreated => "decks_created",
            Self::DeckOpened => "decks_opened",
            Self::GroupWritten => "groups_written",
            Self::GroupViewed => "groups_viewed",
            Self::Comment => "comments",
            Self::Review => "reviews",
            Self::AskNow => "ask_now",
            Self::QuestionsOffered => "questions_offered",
            Self::QuestionAsked => "questions_asked",
            Self::QuestionEdited => "questions_edited",
            Self::Walk => "walks",
            Self::Upgrade => "upgrades",
        }
    }
}

/// What is kept between runs, in `~/.deck/usage.json`.
#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
struct Kept {
    /// The answer: `None` until somebody gives one.
    share: Option<bool>,
    /// A random id, made the first time there is something to send.
    id: Option<String>,
    /// Counts since the last send.
    counts: BTreeMap<String, u64>,
    /// Decks created since the last send, by the agent that created them.
    agents: BTreeMap<String, u64>,
    /// When the counts were last sent, in seconds since the epoch.
    sent: u64,
}

/// Why nothing is being counted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Off {
    /// Nobody has answered yet.
    Unasked,
    /// They said no.
    Declined,
    /// `DO_NOT_TRACK` is set.
    DoNotTrack,
    /// `DECK_TELEMETRY` says off.
    Variable,
    /// Running under CI, where nobody was asked anything.
    Ci,
    /// `[updates] automatic = false`: no requests of deck's own, this included.
    NoRequests,
    /// This build has nowhere to send to.
    NoKey,
}

impl Off {
    /// The reason, as `deck telemetry status` says it.
    #[must_use]
    pub fn reason(self) -> &'static str {
        match self {
            Self::Unasked => "you haven't been asked yet — `deck telemetry on` to share",
            Self::Declined => "you said no — `deck telemetry on` to change that",
            Self::DoNotTrack => "DO_NOT_TRACK is set",
            Self::Variable => "DECK_TELEMETRY is set to off",
            Self::Ci => "this is a CI environment",
            Self::NoRequests => {
                "~/.deck/config.toml says no requests ([updates] automatic = false), or does not read"
            }
            Self::NoKey => "this build of deck sends nothing",
        }
    }
}

/// Whether anything is counted, and if not, why not.
fn off(kept: &Kept) -> Option<Off> {
    off_given(
        kept,
        &|name| std::env::var(name).ok(),
        requests_allowed(),
        KEY,
    )
}

/// Whether deck may make requests of its own at all.
fn requests_allowed() -> bool {
    // A config that will not read is not a yes: it may be the one that says no.
    crate::config::read().is_ok_and(|config| config.updates.automatic)
}

/// [`off`], from what it depends on.
fn off_given(
    kept: &Kept,
    var: &dyn Fn(&str) -> Option<String>,
    requests: bool,
    key: &str,
) -> Option<Off> {
    let set = |name: &str| var(name).is_some_and(|value| !value.trim().is_empty());
    let says_off = |name: &str| {
        var(name).is_some_and(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "0" | "off" | "false" | "no"
            )
        })
    };
    if set("DO_NOT_TRACK") && !says_off("DO_NOT_TRACK") {
        return Some(Off::DoNotTrack);
    }
    if says_off("DECK_TELEMETRY") {
        return Some(Off::Variable);
    }
    if set("CI") && !says_off("CI") {
        return Some(Off::Ci);
    }
    if !requests {
        return Some(Off::NoRequests);
    }
    if key.is_empty() {
        return Some(Off::NoKey);
    }
    match kept.share {
        None => Some(Off::Unasked),
        Some(false) => Some(Off::Declined),
        Some(true) => None,
    }
}

/// Count one of `count`.
pub fn record(count: Count) {
    record_many(count, 1);
}

/// [`record`], from the window: on a thread of its own, so a lock another
/// deck left behind can never make the window wait.
pub fn record_soon(count: Count) {
    std::thread::spawn(move || record(count));
}

/// [`choose`], from the window, for the same reason.
pub fn choose_soon(share: bool) {
    std::thread::spawn(move || choose(share));
}

/// Count `n` of `count`.
pub fn record_many(count: Count, n: u64) {
    // Asked first without the lock: for everyone who has not said yes,
    // which is the cheap and common case, nothing more is done.
    if n == 0 || off(&read()).is_some() {
        return;
    }
    change(|kept| {
        if off(kept).is_some() {
            return false;
        }
        *kept.counts.entry(count.name().to_string()).or_default() += n;
        true
    });
}

/// Count a deck created, and which agent created it.
pub fn deck_created() {
    if off(&read()).is_some() {
        return;
    }
    let agent = agent_from(std::env::vars().map(|(name, _)| name));
    change(|kept| {
        if off(kept).is_some() {
            return false;
        }
        *kept
            .counts
            .entry(Count::DeckCreated.name().to_string())
            .or_default() += 1;
        *kept.agents.entry(agent.to_string()).or_default() += 1;
        true
    });
}

/// Which agent is running deck, from the names of its environment variables —
/// names only, never their values.
fn agent_from(names: impl Iterator<Item = String>) -> &'static str {
    let names: Vec<String> = names.collect();
    let has = |name: &str| names.iter().any(|seen| seen == name);
    let starts = |prefix: &str| names.iter().any(|seen| seen.starts_with(prefix));
    if has("CLAUDECODE") || starts("CLAUDE_CODE_") {
        "claude-code"
    } else if starts("CODEX_") {
        "codex"
    } else if has("CURSOR_AGENT") || has("CURSOR_TRACE_ID") {
        "cursor"
    } else if starts("AMP_") {
        "amp"
    } else if has("GEMINI_CLI") {
        "gemini"
    } else {
        "other"
    }
}

/// The answer, given: in `deck setup`, the window's question, or
/// `deck telemetry on|off`. Saying no forgets anything counted, and the id.
pub fn choose(share: bool) {
    answer(|kept| {
        kept.share = Some(share);
        if !share {
            // Forgotten entirely: turned on again later, it is a new id.
            kept.id = None;
            kept.counts.clear();
            kept.agents.clear();
        }
        true
    });
}

/// Whether the window should ask: nobody has answered, and nothing else
/// already says no — a reader with `DO_NOT_TRACK` set is not asked at all.
#[must_use]
pub fn worth_asking() -> bool {
    off(&read()) == Some(Off::Unasked)
}

/// Whether counts are being kept and sent, and if not, why not.
/// # Errors
///
/// Why nothing is shared, when it is not.
pub fn status() -> Result<(), Off> {
    off(&read()).map_or(Ok(()), Err)
}

/// Exactly what the next send would carry, as it would be sent.
#[must_use]
pub fn show() -> String {
    let mut kept = read();
    if let Some(off) = off(&kept) {
        return format!("Nothing is sent: {}.", off.reason());
    }
    if kept.counts.is_empty() {
        return "Nothing to send: nothing has been counted since the last send.".to_string();
    }
    if kept.id.is_none() {
        kept.id = Some("(a random id, made on the first send)".to_string());
    }
    let wait = EVERY
        .as_secs()
        .saturating_sub(now().saturating_sub(kept.sent));
    let when = if wait == 0 {
        "the next time a deck window is open".to_string()
    } else {
        format!(
            "in about {} minutes, while a deck window is open",
            wait.div_ceil(60)
        )
    };
    format!(
        "Sent {when}, to {HOST}:\n{}",
        serde_json::to_string_pretty(&payload(&kept)).unwrap_or_default()
    )
}

/// The request body for `kept`.
fn payload(kept: &Kept) -> serde_json::Value {
    let mut properties = serde_json::Map::new();
    properties.insert("version".into(), RUNNING.into());
    properties.insert("os".into(), std::env::consts::OS.into());
    properties.insert("arch".into(), std::env::consts::ARCH.into());
    for (name, n) in &kept.counts {
        properties.insert(name.clone(), (*n).into());
    }
    if !kept.agents.is_empty() {
        properties.insert(
            "agents".into(),
            serde_json::to_value(&kept.agents).unwrap_or_default(),
        );
    }
    // No person behind the id, and no place behind the request.
    properties.insert("$process_person_profile".into(), false.into());
    properties.insert("$geoip_disable".into(), true.into());
    properties.insert("$ip".into(), serde_json::Value::Null);
    properties.insert("$lib".into(), "deck".into());
    serde_json::json!({
        "api_key": KEY,
        "batch": [{
            "event": "deck usage",
            "distinct_id": kept.id.clone().unwrap_or_default(),
            "properties": properties,
        }],
    })
}

/// Send what has been counted, now and then, for as long as this deck runs.
///
/// From the window's process only: it is the one that lives long enough to
/// send without anybody waiting on it.
pub fn send_now_and_then(cx: &mut gpui_kit::App) {
    let executor = cx.background_executor().clone();
    cx.background_executor()
        .spawn(async move {
            loop {
                send_if_due();
                executor.timer(LOOK).await;
            }
        })
        .detach();
}

/// Send, if there is something to send and an hour has passed.
fn send_if_due() {
    // One deck sends at a time, under a lock of its own. The counts' lock is
    // only held to read and to write, never across the network: an answer —
    // `deck telemetry off` above all — must never wait on a slow request, or
    // be lost behind one a quitting deck never finished.
    let Some(_sending) =
        store().and_then(|path| crate::update::Held::take(&path.with_extension("send.lock")))
    else {
        return;
    };
    let kept = read();
    if off(&kept).is_some() || kept.counts.is_empty() {
        return;
    }
    if now().saturating_sub(kept.sent) < EVERY.as_secs() {
        return;
    }
    if kept.id.is_none() {
        change(|kept| {
            // Only for somebody still saying yes: an id written after a no
            // would outlive it.
            if kept.id.is_some() || off(kept).is_some() {
                return false;
            }
            kept.id = Some(fresh_id());
            true
        });
    }
    // Read once more, last thing: a no given a moment ago still wins.
    let kept = read();
    if off(&kept).is_some() || kept.id.is_none() {
        return;
    }
    let body = payload(&kept);
    let sent = ureq::post(HOST)
        .config()
        .timeout_global(Some(PATIENCE))
        .build()
        .header("User-Agent", concat!("deck/", env!("CARGO_PKG_VERSION")))
        .send_json(&body)
        .is_ok();
    if sent {
        // What was sent comes off what is there now: counts made while the
        // request was out are kept for next time. Waited for as long as an
        // answer is, so a sent batch is not sent again.
        let Some(_held) = wait_for_lock(Duration::from_secs(2)) else {
            return;
        };
        let mut after = read();
        for (name, n) in &kept.counts {
            if let Some(now) = after.counts.get_mut(name) {
                *now = now.saturating_sub(*n);
            }
        }
        for (name, n) in &kept.agents {
            if let Some(now) = after.agents.get_mut(name) {
                *now = now.saturating_sub(*n);
            }
        }
        after.counts.retain(|_, n| *n > 0);
        after.agents.retain(|_, n| *n > 0);
        after.sent = now();
        write(&after);
    }
}

/// A random id: nothing about the machine or the person goes into it.
fn fresh_id() -> String {
    use std::hash::{BuildHasher as _, Hasher as _};
    let half = |salt: u64| {
        let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
        hasher.write_u128(now_nanos());
        hasher.write_u64(salt);
        hasher.finish()
    };
    format!("{:016x}{:016x}", half(1), half(2))
}

/// Read, change and write the kept file under its lock. `change` says whether
/// it changed anything. The lock is only ever held for a read and a write, so
/// a short wait is enough; past it, the count is dropped rather than waited
/// for.
fn change(change: impl FnOnce(&mut Kept) -> bool) {
    if let Some(_held) = wait_for_lock(Duration::from_millis(100)) {
        let mut kept = read();
        if change(&mut kept) {
            write(&kept);
        }
    }
}

/// [`change`], for an answer: it waits longer, and is written even if the
/// lock never comes. A count can be lost; a no cannot.
fn answer(change: impl FnOnce(&mut Kept) -> bool) {
    let _held = wait_for_lock(Duration::from_secs(2));
    let mut kept = read();
    if change(&mut kept) {
        write(&kept);
    }
}

/// The counts' lock, waited for up to `patience`.
fn wait_for_lock(patience: Duration) -> Option<crate::update::Held> {
    let until = std::time::Instant::now() + patience;
    loop {
        if let Some(held) = lock() {
            return Some(held);
        }
        if std::time::Instant::now() >= until {
            return None;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn store() -> Option<std::path::PathBuf> {
    deck_core::home::deck().map(|deck| deck.join("usage.json"))
}

fn lock() -> Option<crate::update::Held> {
    crate::update::Held::take(&store()?.with_extension("lock"))
}

fn read() -> Kept {
    store()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn write(kept: &Kept) {
    let Some(path) = store() else {
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(text) = serde_json::to_string_pretty(kept) {
        let partial = path.with_extension("json.partial");
        if std::fs::write(&partial, text).is_ok() {
            let _ = std::fs::rename(partial, path);
        }
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

fn now_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_nanos())
}

#[cfg(test)]
mod tests {
    // Spelled out: the gpui glob elsewhere exports a `test` attribute.
    use core::prelude::v1::test;

    use super::*;

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let pairs: Vec<(String, String)> = pairs
            .iter()
            .map(|(name, value)| ((*name).to_string(), (*value).to_string()))
            .collect();
        move |name| {
            pairs
                .iter()
                .find(|(seen, _)| seen == name)
                .map(|(_, value)| value.clone())
        }
    }

    fn yes() -> Kept {
        Kept {
            share: Some(true),
            ..Kept::default()
        }
    }

    #[test]
    fn nothing_is_counted_before_a_yes() {
        let none = env(&[]);
        assert_eq!(
            off_given(&Kept::default(), &none, true, "k"),
            Some(Off::Unasked)
        );
        let no = Kept {
            share: Some(false),
            ..Kept::default()
        };
        assert_eq!(off_given(&no, &none, true, "k"), Some(Off::Declined));
        assert_eq!(off_given(&yes(), &none, true, "k"), None);
    }

    #[test]
    fn every_way_of_saying_no_outranks_a_yes() {
        let k = "k";
        assert_eq!(
            off_given(&yes(), &env(&[("DO_NOT_TRACK", "1")]), true, k),
            Some(Off::DoNotTrack)
        );
        assert_eq!(
            off_given(&yes(), &env(&[("DECK_TELEMETRY", "off")]), true, k),
            Some(Off::Variable)
        );
        assert_eq!(
            off_given(&yes(), &env(&[("CI", "true")]), true, k),
            Some(Off::Ci)
        );
        assert_eq!(
            off_given(&yes(), &env(&[]), false, k),
            Some(Off::NoRequests)
        );
        assert_eq!(off_given(&yes(), &env(&[]), true, ""), Some(Off::NoKey));
        // A variable set to say "not off" is not a no.
        assert_eq!(
            off_given(&yes(), &env(&[("DO_NOT_TRACK", "0")]), true, k),
            None
        );
    }

    #[test]
    fn what_is_sent_is_counts_and_versions_only() {
        let mut kept = yes();
        kept.id = Some("abc".into());
        kept.counts.insert("comments".into(), 3);
        kept.agents.insert("claude-code".into(), 2);
        let body = payload(&kept);
        let event = &body["batch"][0];
        assert_eq!(event["distinct_id"], "abc");
        let properties = event["properties"].as_object().expect("properties");
        let mut names: Vec<&str> = properties.keys().map(String::as_str).collect();
        names.sort_unstable();
        assert_eq!(
            names,
            [
                "$geoip_disable",
                "$ip",
                "$lib",
                "$process_person_profile",
                "agents",
                "arch",
                "comments",
                "os",
                "version"
            ],
            "nothing else rides along"
        );
        assert_eq!(properties["$process_person_profile"], false);
        assert_eq!(properties["$geoip_disable"], true);
    }

    #[test]
    fn the_agent_is_told_by_variable_names_alone() {
        let names = |list: &[&str]| {
            list.iter()
                .map(|name| (*name).to_string())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            agent_from(names(&["PATH", "CLAUDECODE"]).into_iter()),
            "claude-code"
        );
        assert_eq!(agent_from(names(&["CODEX_SANDBOX"]).into_iter()), "codex");
        assert_eq!(agent_from(names(&["CURSOR_AGENT"]).into_iter()), "cursor");
        assert_eq!(agent_from(names(&["HOME"]).into_iter()), "other");
    }

    #[test]
    fn ids_are_random() {
        assert_ne!(fresh_id(), fresh_id());
        assert_eq!(fresh_id().len(), 32);
    }
}
