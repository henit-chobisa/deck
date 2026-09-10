//! Which decks already have a window.
//!
//! Every `deck open` is a process of its own, and nothing stopped two of them
//! showing the same deck — an agent that ran the command twice put a second
//! window on top of the one the reader was already reading.
//!
//! So a deck being shown is written down, and the second process finds it
//! there and leaves. Under `~/.deck` rather than inside the deck directory:
//! the protocol says what a deck holds, and this is not part of it.
//!
//! Nothing gives a claim back. A window that closes takes its process with it,
//! and the next reader of the claim checks whether that process is still
//! running — so a claim is released by the only event that can be relied on,
//! which is the process ceasing to exist. There is no tidy exit path to miss.

use std::path::PathBuf;

/// Where the claim for `id` lives.
fn claim(id: &str) -> Option<PathBuf> {
    Some(deck_core::home::deck()?.join("showing").join(id))
}

/// Whether some other living process is already showing this deck.
///
/// A claim left behind by a process that died does not count. Deck can crash,
/// and a crash that made a deck permanently unopenable would be the worse
/// failure of the two.
#[must_use]
pub fn taken(id: &str) -> bool {
    let Some(at) = claim(id) else {
        return false;
    };
    let Ok(text) = std::fs::read_to_string(&at) else {
        return false;
    };
    let Ok(pid) = text.trim().parse::<i32>() else {
        return false;
    };
    if pid == std::process::id() as i32 {
        return false;
    }
    if alive(pid) {
        return true;
    }
    // Stale, so it is ours to take.
    let _ = std::fs::remove_file(&at);
    false
}

/// Say this process is showing the deck.
pub fn take(id: &str) {
    let Some(at) = claim(id) else {
        return;
    };
    if let Some(dir) = at.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(at, std::process::id().to_string());
}

/// Whether a process is still running.
///
/// Signal zero: the kernel does every check it would do to deliver a signal and
/// then delivers nothing, which is the ordinary way to ask this.
#[cfg(unix)]
fn alive(pid: i32) -> bool {
    // SAFETY: `kill` with signal zero sends nothing and only reports whether
    // the process could be signalled.
    unsafe { libc::kill(pid, 0) == 0 }
}

/// Windows has no cheap equivalent that does not open a handle, and deck has
/// never run there. Taking the claim at face value costs a duplicate window
/// after a crash and never costs a deck that will not open.
#[cfg(not(unix))]
fn alive(_pid: i32) -> bool {
    true
}

#[cfg(test)]
mod tests {
    // Spelled out rather than `#[test]`: this crate glob-imports GPUI
    // elsewhere, which exports a `test` attribute of its own.
    use core::prelude::v1::test;

    use super::*;

    #[test]
    fn a_deck_nobody_has_claimed_is_free() {
        assert!(!taken("d-nobody-has-this-one"));
    }

    #[test]
    fn our_own_claim_does_not_keep_us_out() {
        // The process that holds a claim reads it again on every later deck in
        // the same command. Its own claim is not somebody else's window.
        let id = "d-test-our-own";
        take(id);
        assert!(!taken(id));
        if let Some(at) = claim(id) {
            let _ = std::fs::remove_file(at);
        }
    }

    #[test]
    fn a_claim_from_a_dead_process_is_cleared_rather_than_obeyed() {
        // Deck can crash, and it has. A claim that outlived its process would
        // make that deck permanently unopenable, which is the worse of the two
        // failures by a distance.
        let id = "d-test-stale";
        let at = claim(id).expect("there is a home to write under");
        if let Some(dir) = at.parent() {
            std::fs::create_dir_all(dir).expect("the directory is writable");
        }
        // Process 1 is launchd. Signalling it is not permitted, so `kill` fails
        // with EPERM rather than ESRCH — which is a process that exists, and
        // exactly what must NOT be treated as dead. So use a pid that cannot
        // be running: the kernel allocates none above the maximum.
        std::fs::write(&at, "999999999").expect("the claim is writable");

        assert!(!taken(id), "a pid nothing owns is not a live window");
        assert!(!at.exists(), "and the claim was cleared on the way past");
    }
}
