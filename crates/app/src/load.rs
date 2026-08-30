//! Reading a deck off disk, and going on reading it.
//!
//! I/O lives here, not in `deck-core`. The model has no opinion about files;
//! this module turns a directory into one.
//!
//! A deck is a directory because writing one takes an agent tens of seconds and
//! there is no reason for the reader to wait on the last group before seeing
//! the first. So this does not read a deck, it *tails* one: the header once,
//! then whatever groups have landed, and again whenever asked until the agent
//! says there is no more.
//!
//! # Arrived is not the same as ready
//!
//! Group files land in whatever order the agent finishes them, and the story
//! does not: the prose of group 3 assumes group 2 has been read. So a group
//! that arrives with a gap in front of it is held — kept, counted, and not
//! shown — until the gap fills. [`Deck::groups`] is the unbroken run from the
//! beginning, which is the most of the story that can honestly be read.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use deck_core::protocol::{Group, Header, VERSION};

/// The file the agent writes when the deck is finished.
///
/// Its presence is the only difference between "there is more coming" and
/// "that is all there is", and the reader is told which.
const SEAL: &str = "done";

/// A deck, assembled and ready to show.
#[derive(Clone)]
pub struct Deck {
    /// Where it came from, for the error message when a file has moved.
    pub root: PathBuf,
    /// The header.
    pub header: Header,
    /// Every group that has landed, in `ord` order — including any held back
    /// behind a gap.
    arrived: Vec<Group>,
    /// How many of `arrived` form an unbroken run from the start.
    ready: usize,
    /// Group files already read, so a poll only pays for what is new.
    seen: HashSet<std::ffi::OsString>,
    /// Whether the agent has said there is nothing more to come.
    sealed: bool,
}

impl Deck {
    /// Open the deck at `path`.
    ///
    /// `path` is a `<id>.deck` directory: `deck.json`, one `g<n>.json` per
    /// group, and `done` once the agent has finished.
    ///
    /// A deck with no groups yet is not an error. The agent opens the window
    /// and then writes, so an empty one is the normal first moment of a deck's
    /// life rather than a failure.
    ///
    /// # Errors
    ///
    /// When the header is missing, unreadable, or written to a version of the
    /// protocol this build does not know. A *group* that will not parse is
    /// skipped rather than fatal — losing one group of a story beats refusing
    /// to show any of it, and a group is one claim. A header is the whole deck.
    pub fn read(path: &Path) -> anyhow::Result<Self> {
        let header: Header =
            serde_json::from_str(&std::fs::read_to_string(path.join("deck.json"))?)?;

        // Refused, not guessed at. A later protocol may mean something
        // different by the same field names, and a window that renders it
        // anyway shows the reader something the agent did not say.
        anyhow::ensure!(
            header.v == VERSION,
            "deck is protocol v{}, this build reads v{VERSION}",
            header.v
        );

        let mut deck = Self {
            root: path.to_path_buf(),
            header,
            arrived: Vec::new(),
            ready: 0,
            seen: HashSet::new(),
            sealed: false,
        };
        deck.refresh();
        Ok(deck)
    }

    /// The deck's title, as a heading.
    ///
    /// Capitalised at the front and nowhere else. Title Case would mangle the
    /// identifiers a title is allowed to name — `relocate` is not `Relocate` —
    /// and lower-case reads as a fragment on the bar, where the title is the
    /// only thing there is to go on.
    #[must_use]
    pub fn title(&self) -> String {
        let mut letters = self.header.title.chars();
        match letters.next() {
            Some(first) => first.to_uppercase().chain(letters).collect(),
            None => String::new(),
        }
    }

    /// The story so far: every group that can be read in order.
    #[must_use]
    pub fn groups(&self) -> &[Group] {
        &self.arrived[..self.ready]
    }

    /// Whether the agent has finished writing.
    #[must_use]
    pub fn sealed(&self) -> bool {
        self.sealed
    }

    /// Look for anything that has landed since last time.
    ///
    /// Returns whether the deck changed, which is what tells the window
    /// whether to redraw. Cheap when nothing has: a directory listing, and a
    /// name check per entry.
    pub fn refresh(&mut self) -> bool {
        let sealed = self.root.join(SEAL).exists();
        let mut changed = sealed != self.sealed;
        self.sealed = sealed;

        let Ok(entries) = std::fs::read_dir(&self.root) else {
            // The directory going away is not this function's problem: the
            // reader keeps whatever they were reading.
            return changed;
        };

        for entry in entries.flatten() {
            let name = entry.file_name();
            if !is_group_file(&name) || self.seen.contains(&name) {
                continue;
            }
            // Marked seen whether or not it parses. A half-written file will
            // be complete by the next poll, but a file that is simply wrong
            // must not be re-read four times a second forever.
            let Some(group) = std::fs::read_to_string(entry.path())
                .ok()
                .and_then(|text| serde_json::from_str::<Group>(&text).ok())
            else {
                continue;
            };
            self.seen.insert(name);
            self.arrived.push(group);
            changed = true;
        }

        if changed {
            // The story's order is what `ord` says, not what the filesystem
            // happened to hand back.
            self.arrived
                .sort_by_key(|group| group.ord.unwrap_or(u32::MAX));
            self.ready = readable(&self.arrived);
        }
        changed
    }

    /// The deck's `cwd`, or the deck's own directory when it is unscoped.
    ///
    /// Every relative path in a ref is resolved against this.
    #[must_use]
    pub fn base(&self) -> PathBuf {
        self.header.cwd.clone().unwrap_or_else(|| self.root.clone())
    }
}

/// Whether `name` is one of a deck's group files.
fn is_group_file(name: &std::ffi::OsStr) -> bool {
    name.to_str()
        .is_some_and(|name| name.starts_with('g') && name.ends_with(".json"))
}

/// How many of `sorted` can be read, counting from the first.
///
/// A deck that numbers its groups is read in that order and stops at the first
/// gap: group 3 arriving before group 2 is kept, not shown, because its prose
/// was written on the assumption that group 2 had been. A deck that numbers
/// nothing is not claiming an order, so all of it is readable.
fn readable(sorted: &[Group]) -> usize {
    if sorted.iter().any(|group| group.ord.is_none()) {
        return sorted.len();
    }
    sorted
        .iter()
        .enumerate()
        .take_while(|(ix, group)| group.ord == u32::try_from(*ix).ok().map(|ix| ix + 1))
        .count()
}

/// The text of a code ref, and whether it could be read.
///
/// A missing file is shown as a pane saying so. A deck that silently drops the
/// evidence for its own claim is worse than one that admits the file has moved.
#[must_use]
pub fn read_source(base: &Path, file: &Path) -> String {
    let full = if file.is_absolute() {
        file.to_path_buf()
    } else {
        base.join(file)
    };

    std::fs::read_to_string(&full)
        .unwrap_or_else(|err| format!("could not read {}\n\n{err}", full.display()))
}
