//! The decks that are waiting.
//!
//! One window at a time is on screen: either the bar, saying what has arrived,
//! or a deck being read. Never both — a bar floating over the deck it just
//! opened is the interruption the bar exists to avoid.
//!
//! So the decks that are not being read live here rather than in either window,
//! and both windows are views onto this. Putting a deck away pushes it back;
//! opening one takes it out; the bar appears whenever this is not empty and
//! nothing is being read.
//!
//! A global, because it outlives both windows and belongs to neither. Deck's
//! only one — everything else that looks like state is owned by the view that
//! draws it.

use gpui_kit::{App, Global};

use crate::view::Session;

/// Every deck waiting, oldest first.
#[derive(Default)]
struct Waiting(Vec<Session>);

impl Global for Waiting {}

/// Add decks to the back of the queue, in the order they were given.
pub fn extend(sessions: Vec<Session>, cx: &mut App) {
    with(cx, |waiting| waiting.extend(sessions));
}

/// Take the deck at `at` out of the queue.
pub fn take(at: usize, cx: &mut App) -> Option<Session> {
    with(cx, |waiting| {
        (at < waiting.len()).then(|| waiting.remove(at))
    })
}

/// How many are waiting.
#[must_use]
pub fn len(cx: &App) -> usize {
    cx.try_global::<Waiting>()
        .map_or(0, |waiting| waiting.0.len())
}

/// Read something off the deck at `at`.
///
/// A borrow rather than a clone: what is wanted from a waiting deck is a title
/// and a count, and a `Session` carries a whole deck's worth of text.
#[must_use]
pub fn about<T>(at: usize, cx: &App, read: impl FnOnce(&Session) -> T) -> Option<T> {
    cx.try_global::<Waiting>()
        .and_then(|waiting| waiting.0.get(at))
        .map(read)
}

/// Look for new groups in every deck waiting.
///
/// Returns whether anything changed, and whether anything is still being
/// written — the first says whether to redraw, the second whether to keep
/// looking.
pub fn refresh(cx: &mut App) -> (bool, bool) {
    with(cx, |waiting| {
        let mut changed = false;
        let mut writing = false;
        for session in waiting.iter_mut() {
            changed |= session.deck.refresh();
            writing |= !session.deck.sealed();
        }
        (changed, writing)
    })
}

/// Do something with the queue, making it if it is not there yet.
fn with<T>(cx: &mut App, and: impl FnOnce(&mut Vec<Session>) -> T) -> T {
    if cx.try_global::<Waiting>().is_none() {
        cx.set_global(Waiting::default());
    }
    let waiting = cx.global_mut::<Waiting>();
    and(&mut waiting.0)
}
