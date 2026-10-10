//! Whether a newer deck exists, found out quietly.
//!
//! # Nothing here may be noticed when it fails
//!
//! deck runs on laptops on planes, behind proxies, and on machines that should
//! make no requests at all. So the question is asked at most once an hour, off
//! the main thread, with a short patience, and every way it can fail — no
//! network, a proxy that eats the request, GitHub answering with something
//! odd — ends the same way: nothing is shown and nothing is said. A deck that
//! paused before opening, or put an error in the corner, because it could not
//! reach a server it did not need would be broken for exactly the people who
//! can least do anything about it.
//!
//! # What is remembered
//!
//! The last answer is kept in `~/.deck/update.json`, and read at startup before
//! any request is made. So a newer release that was already known shows the
//! moment a deck opens, offline or not, and the network is only asked again
//! once that answer is an hour old.
//!
//! # Why an hour
//!
//! It was a day, and a fix released in the morning reached somebody who had
//! opened a deck the night before only the next night. An hour is soon enough
//! that a deck opened after a release finds it, and still far under GitHub's
//! sixty unauthenticated requests an hour from one address — which an office
//! sharing an address, or an agent opening twenty decks in an afternoon,
//! would reach if every launch asked. It also leaves the hour in which a
//! broken release can be noticed, by us first, and taken down.
//!
//! Stable releases only. A prerelease is something somebody asks for with
//! `deck upgrade --prerelease`; it is never offered to them unasked.
//!
//! # And then installed
//!
//! When the ask finds a newer release, it is installed in the same
//! background task, the way `deck upgrade` would — checked, signed, tried, and
//! renamed over the running binary, which carries on untouched. The next deck
//! opened is the new one, and it opens on its notes. A copy Homebrew owns, or
//! one somebody is building, is told about and left alone.

use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// How long an answer is good for.
const ONCE_AN_HOUR: Duration = Duration::from_secs(60 * 60);

/// How long to wait before deciding there is no network. Short, because the
/// only thing waiting is a background thread nobody is watching.
const PATIENCE: Duration = Duration::from_secs(3);

/// The newest stable release. GitHub leaves prereleases out of this one by
/// itself, which is the whole of the stable-only rule.
const LATEST: &str = "https://api.github.com/repos/henit-chobisa/deck/releases/latest";

/// What this build is.
const RUNNING: &str = env!("CARGO_PKG_VERSION");

/// The newer version, once one is known. Read by the window on every frame, so
/// it is a lock around a short string rather than anything that does work.
static FOUND: Mutex<Option<String>> = Mutex::new(None);

/// What was learned last time, kept between runs.
#[derive(Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
struct Known {
    /// The newest stable release that has a binary attached, without its `v`.
    latest: Option<String>,
    /// When that was asked, in seconds since the epoch.
    checked: u64,
}

/// A newer stable deck that has been installed, and is what opens next.
static INSTALLED: Mutex<Option<String>> = Mutex::new(None);

/// A newer stable deck, if one is known and not already installed.
#[must_use]
pub fn available() -> Option<String> {
    if installed().is_some() {
        return None;
    }
    FOUND.lock().ok().and_then(|found| found.clone())
}

/// The newer deck installed while this one was running, if one was.
#[must_use]
pub fn installed() -> Option<String> {
    INSTALLED
        .lock()
        .ok()
        .and_then(|installed| installed.clone())
}

/// Show what is already known, and ask again if it is an hour old.
///
/// Returns at once. The asking happens on the background executor, and the
/// answer is brought back to the main thread so the windows draw it straight
/// away rather than whenever something else next makes them paint.
pub fn look(cx: &mut gpui_kit::App) {
    let automatic = crate::config::read().map_or(true, |config| config.updates.automatic);
    if !automatic {
        return;
    }

    let known = read();
    remember(&known);
    if !due(known.checked, now()) {
        return;
    }

    let asking = cx.background_executor().spawn(async move {
        // Another deck is asking. What it learns lands in the file, and this
        // one reads it next launch rather than asking the same question twice.
        let Some(_held) = store().and_then(|path| Held::take(&path.with_extension("lock"))) else {
            return (read(), None);
        };
        // Read again now the lock is held: a deck that held it a moment ago
        // may have asked already, and the answer it wrote is good for an hour.
        let before = read();
        if !due(before.checked, now()) {
            return (before, None);
        }
        let known = settle(latest(), before, now());
        write(&known);
        // Still under the lock, so two decks never install at once. Any
        // failure leaves the deck that was there, and the foot still says a
        // newer one is available for somebody to run `deck upgrade` by hand.
        let installed = worth_showing(known.latest.as_deref(), RUNNING)
            .and_then(|_| crate::upgrade::quietly().ok().flatten());
        (known, installed)
    });
    cx.spawn(async move |cx| {
        let (known, installed) = asking.await;
        remember(&known);
        if let (Some(version), Ok(mut slot)) = (installed, INSTALLED.lock()) {
            *slot = Some(version);
        }
        cx.update(gpui_kit::App::refresh_windows);
    })
    .detach();
}

/// What to keep, given what the network said and what is on disk.
///
/// `on_disk` is read under the lock, not carried from startup, so a deck that
/// asked in the meantime and got an answer is not overwritten by this one
/// failing. A failure keeps whatever is known and moves only the clock: a
/// machine with no route out would otherwise ask on every launch, each a wait
/// of three seconds on nothing.
fn settle(answer: Option<String>, on_disk: Known, now: u64) -> Known {
    Known {
        latest: answer.or(on_disk.latest),
        checked: now,
    }
}

/// Whether an answer from `checked` is old enough to ask again.
fn due(checked: u64, now: u64) -> bool {
    now.saturating_sub(checked) >= ONCE_AN_HOUR.as_secs()
}

/// What the window should show, given what is known and what is running.
///
/// Only a release strictly newer than this one. An older one is not an update,
/// and neither is the same one.
fn worth_showing(latest: Option<&str>, running: &str) -> Option<String> {
    latest
        .filter(|latest| crate::upgrade::newer(latest, running))
        .map(ToString::to_string)
}

/// Make what is known what the window shows.
fn remember(known: &Known) {
    if let Ok(mut found) = FOUND.lock() {
        *found = worth_showing(known.latest.as_deref(), RUNNING);
    }
}

/// Ask GitHub for the newest stable release with a binary on it.
///
/// `None` for every failure, because every failure means the same thing here.
fn latest() -> Option<String> {
    let reply: serde_json::Value = ureq::get(LATEST)
        .config()
        .timeout_global(Some(PATIENCE))
        .build()
        .header("User-Agent", concat!("deck/", env!("CARGO_PKG_VERSION")))
        .call()
        .ok()?
        .body_mut()
        .read_json()
        .ok()?;
    installable(&reply)
}

/// The version a release offers, if it offers anything to install.
///
/// A release `deck upgrade` would refuse is not worth telling anybody about.
/// It wants this platform's tarball and the checksum beside it, so this wants
/// both — and no release before the workflow that builds them has either.
fn installable(release: &serde_json::Value) -> Option<String> {
    let tag = release["tag_name"].as_str()?;
    let names: Vec<&str> = release["assets"]
        .as_array()?
        .iter()
        .filter_map(|asset| asset["name"].as_str())
        .collect();
    let has = |suffix: &str| names.iter().any(|name| crate::upgrade::ours(name, suffix));
    (has(".tar.gz") && has(".tar.gz.sha256")).then(|| tag.trim_start_matches('v').to_string())
}

/// Asking the network, held by one deck at a time across every process.
///
/// A file created only if it does not exist, which the filesystem decides
/// atomically, and removed when this is dropped. A deck killed while holding
/// it leaves it behind, so one older than any ask and install could take is
/// taken over.
///
/// The file holds who took it, and is only removed by them. Otherwise a deck
/// that ran past the limit and had its lock taken over would, on finishing,
/// delete the lock of the deck that took it — and let a third one in.
pub(crate) struct Held(std::path::PathBuf, String);

impl Held {
    /// As long as an abandoned upgrade, which this covers too.
    const STALE: Duration = crate::upgrade::ABANDONED;

    pub(crate) fn take(path: &std::path::Path) -> Option<Self> {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let mine = format!("{} {}", std::process::id(), now_nanos());
        let create = || {
            use std::io::Write as _;
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
                .and_then(|mut file| file.write_all(mine.as_bytes()))
        };
        if create().is_ok() {
            return Some(Self(path.to_path_buf(), mine));
        }
        let abandoned = std::fs::metadata(path)
            .and_then(|meta| meta.modified())
            .ok()
            .and_then(|at| at.elapsed().ok())
            .is_some_and(|age| age >= Self::STALE);
        if abandoned && std::fs::remove_file(path).is_ok() && create().is_ok() {
            return Some(Self(path.to_path_buf(), mine));
        }
        None
    }
}

impl Drop for Held {
    fn drop(&mut self) {
        if std::fs::read_to_string(&self.0).is_ok_and(|held| held == self.1) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
}

/// A moment, finely enough that two decks taking the lock never write the
/// same thing.
fn now_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_nanos())
}

fn store() -> Option<std::path::PathBuf> {
    deck_core::home::deck().map(|deck| deck.join("update.json"))
}

fn read() -> Known {
    store()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn write(known: &Known) {
    let Some(path) = store() else {
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    // Written beside and renamed over, so a deck reading at the same moment
    // sees the old answer or the new one and never half of either.
    let Ok(text) = serde_json::to_string(known) else {
        return;
    };
    let partial = path.with_extension(format!("json.{}", std::process::id()));
    if std::fs::write(&partial, text).is_ok() && std::fs::rename(&partial, &path).is_err() {
        let _ = std::fs::remove_file(&partial);
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asked_at_most_once_an_hour() {
        let hour = ONCE_AN_HOUR.as_secs();
        assert_eq!(hour, 3_600);
        assert!(due(0, hour), "never asked is due");
        assert!(
            !due(1_000, 1_000 + hour - 1),
            "a second short of an hour is not"
        );
        assert!(due(1_000, 1_000 + hour), "an hour is");

        // A clock set backwards must not make it ask on every launch forever,
        // or never again. Saturating keeps it from wrapping round to huge.
        assert!(
            !due(5_000, 1_000),
            "a checked time in the future is not due"
        );
    }

    #[test]
    fn only_a_strictly_newer_release_is_shown() {
        assert_eq!(worth_showing(Some("0.1.4"), "0.1.3"), Some("0.1.4".into()));
        assert_eq!(worth_showing(Some("0.1.3"), "0.1.3"), None, "the same");
        assert_eq!(worth_showing(Some("0.1.2"), "0.1.3"), None, "an older one");
        assert_eq!(worth_showing(None, "0.1.3"), None, "nothing known");

        // Somebody running an rc of a release sees the release when it ships.
        assert_eq!(
            worth_showing(Some("0.1.3"), "0.1.3-rc.2"),
            Some("0.1.3".into())
        );
    }

    #[test]
    fn a_release_with_no_binary_is_not_offered() {
        let media_only = serde_json::json!({
            "tag_name": "v0.1.2",
            "assets": [{ "name": "pages-demo.gif" }, { "name": "4.26.13.mp4" }]
        });
        assert_eq!(installable(&media_only), None);

        let tarball = |platform: &str| format!("deck-v0.1.4-{platform}.tar.gz");
        let ours = tarball(crate::upgrade::PLATFORM);
        let real = serde_json::json!({
            "tag_name": "v0.1.4",
            "assets": [{ "name": ours }, { "name": format!("{ours}.sha256") }]
        });
        assert_eq!(installable(&real), Some("0.1.4".into()));

        // `deck upgrade` will not install a tarball it cannot check.
        let unchecked = serde_json::json!({
            "tag_name": "v0.1.4",
            "assets": [{ "name": ours }]
        });
        assert_eq!(installable(&unchecked), None);

        // Nor one built for somewhere else — another system, or the same
        // system on the other chip.
        for elsewhere in [
            "macos-universal",
            "windows-x86_64",
            "windows-aarch64",
            "linux-x86_64",
            "linux-aarch64",
        ]
        .into_iter()
        .filter(|platform| *platform != crate::upgrade::PLATFORM)
        {
            let elsewhere = tarball(elsewhere);
            let theirs = serde_json::json!({
                "tag_name": "v0.1.4",
                "assets": [{ "name": elsewhere }, { "name": format!("{elsewhere}.sha256") }]
            });
            assert_eq!(installable(&theirs), None, "{elsewhere}");
        }
    }

    #[test]
    fn only_one_deck_asks_at_a_time() {
        let path = std::env::temp_dir().join(format!("deck-update-{}.lock", std::process::id()));
        let _ = std::fs::remove_file(&path);

        let first = Held::take(&path).expect("nobody holds it");
        assert!(Held::take(&path).is_none(), "a second deck waits its turn");
        drop(first);
        assert!(!path.exists(), "letting go removes it");
        assert!(Held::take(&path).is_some(), "and the next deck can take it");
        assert!(!path.exists());

        // A deck whose lock was taken over leaves the new holder's alone.
        let late = Held::take(&path).expect("taken");
        std::fs::write(&path, "somebody else").expect("taken over");
        drop(late);
        assert!(path.exists(), "the deck that took it over still holds it");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_failed_ask_keeps_what_another_deck_found() {
        let found_meanwhile = Known {
            latest: Some("0.1.4".into()),
            checked: 100,
        };
        assert_eq!(
            settle(None, found_meanwhile, 200),
            Known {
                latest: Some("0.1.4".into()),
                checked: 200
            },
            "no answer moves the clock and nothing else"
        );
        assert_eq!(
            settle(Some("0.1.5".into()), Known::default(), 200).latest,
            Some("0.1.5".into()),
            "an answer wins"
        );
    }

    #[test]
    fn an_odd_answer_is_no_answer() {
        assert_eq!(installable(&serde_json::json!({})), None);
        assert_eq!(
            installable(&serde_json::json!({ "message": "rate limited" })),
            None
        );
    }

    #[test]
    fn what_is_known_survives_a_restart() {
        let known = Known {
            latest: Some("0.1.4".into()),
            checked: 1_790_000_000,
        };
        let text = serde_json::to_string(&known).expect("it writes");
        assert_eq!(
            serde_json::from_str::<Known>(&text).expect("it reads"),
            known
        );

        // A file from some future deck with more in it, or a broken one, reads
        // as knowing nothing rather than failing — which means asking again.
        let unknown: Known = serde_json::from_str("not json").unwrap_or_default();
        assert_eq!(unknown, Known::default());
    }
}
