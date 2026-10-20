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

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use deck_core::LineRange;
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
    /// When each group's questions were last renewed, as the file says: a
    /// renewal is noticed by its time changing, not by reading it every poll.
    renewed: HashMap<String, (std::time::SystemTime, u64)>,
    /// What came with the deck when it was opened from a `.deck` file, and
    /// `None` for a deck folder an agent wrote here.
    pub packed: Option<Packed>,
}

/// A deck opened from a `.deck` file: the code it carries, where it came
/// from, and whether the reader is looking at its copy or their own.
#[derive(Clone)]
pub struct Packed {
    /// Which file, and where its review goes.
    pub origin: deck_cli::opened::Origin,
    /// Every source file the deck carries, by the path its refs name.
    snapshots: HashMap<String, std::sync::Arc<deck_core::bundle::snap::Snapshot>>,
    /// How the repo the reader is in stands against the deck's.
    pub repo: deck_cli::opened::Match,
    /// Whether the panes show the reader's working copy rather than the
    /// deck's. Off until they ask: the deck's copy is what they were sent,
    /// and the code the claims were written about.
    pub live: bool,
}

impl Packed {
    /// What `deck open` found when it opened the deck folder at `root`, or
    /// `None` for a folder that did not come from a file.
    ///
    /// The repo is asked about the directory the reader is in, once, here: a
    /// reader does not change repo under an open window, and asking git on
    /// every pane would be a process per file for an answer that does not
    /// change.
    #[must_use]
    pub fn of(root: &Path) -> Option<Self> {
        let origin = deck_cli::opened::origin(root)?;
        let manifest = deck_cli::opened::manifest(root);
        let repo = std::env::current_dir().map_or(deck_cli::opened::Match::Elsewhere, |cwd| {
            deck_cli::opened::matching(
                &cwd,
                manifest
                    .as_ref()
                    .and_then(|manifest| manifest.repo.as_ref()),
                None,
            )
        });
        let snapshots = deck_cli::opened::snapshots(root)
            .into_iter()
            .map(|snapshot| (snapshot.file.path.clone(), std::sync::Arc::new(snapshot)))
            .collect();
        Some(Self {
            origin,
            snapshots,
            repo,
            live: false,
        })
    }

    /// The top of the reader's checkout, when it is the repo the deck came
    /// from — at any commit — and so has the deck's files to offer.
    #[must_use]
    pub fn checkout(&self) -> Option<&Path> {
        match &self.repo {
            deck_cli::opened::Match::Same { root, .. }
            | deck_cli::opened::Match::Moved { root, .. } => Some(root),
            deck_cli::opened::Match::Elsewhere => None,
        }
    }

    /// The deck's own copy of `file`, whichever side the panes are showing.
    #[must_use]
    pub fn frozen(&self, file: &Path) -> Option<Source> {
        self.snapshots
            .get(&file.to_string_lossy().replace('\\', "/"))
            .map(|snapshot| Source::frozen(snapshot))
    }

    /// The file's name, as the strip shows it.
    #[must_use]
    pub fn name(&self) -> String {
        self.origin
            .file
            .file_name()
            .map_or_else(String::new, |name| name.to_string_lossy().into_owned())
    }
}

/// Where a pane's code comes from, and what can honestly be shown of it.
///
/// It used to be a string, and when a file could not be read the string was
/// the error — so the reason a file was missing was drawn as if it were the
/// file, numbered, highlighted and selectable. A pane has three different
/// things to draw, and each is its own case here.
#[derive(Clone)]
pub enum Source {
    /// The deck's own copy, from a `.deck` file: only the spans it kept.
    Snapshot {
        /// The file numbered as it was whole, up to the last line kept, with
        /// the lines it did not keep left empty — so line `n` is still line
        /// `n`, and everything that counts lines counts the real ones.
        text: std::sync::Arc<str>,
        /// The spans kept, in order and apart.
        kept: std::sync::Arc<[LineRange]>,
        /// How many lines the whole file had, when the deck says.
        lines: Option<u32>,
    },
    /// The file as it is on disk now.
    Live(std::sync::Arc<str>),
    /// Nothing to show.
    Missing {
        /// The file, as the ref names it.
        file: PathBuf,
        /// The rest of the sentence after its name: ` is not in this deck.`
        said: std::sync::Arc<str>,
        /// Whether the reader's checkout has the file to offer instead.
        live: bool,
    },
}

impl Source {
    /// The text a pane is built from: empty for a file that is missing.
    #[must_use]
    pub fn text(&self) -> &str {
        match self {
            Self::Snapshot { text, .. } | Self::Live(text) => text,
            Self::Missing { .. } => "",
        }
    }

    /// The deck's copy of `snapshot`, numbered as the whole file was.
    #[must_use]
    pub fn frozen(snapshot: &deck_core::bundle::snap::Snapshot) -> Self {
        let last = snapshot.file.kept.last().map_or(0, |span| span.last);
        let mut text = String::new();
        for n in 1..=last {
            text.push_str(snapshot.line(n).unwrap_or_default());
            text.push('\n');
        }
        Self::Snapshot {
            text: text.into(),
            kept: snapshot.file.kept.clone().into(),
            lines: snapshot.file.lines,
        }
    }
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
            renewed: HashMap::new(),
            packed: Packed::of(path),
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
        heading(&self.header.title)
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

        changed |= self.refresh_asks();

        if changed {
            // The story's order is what `ord` says, not what the filesystem
            // happened to hand back.
            self.arrived
                .sort_by_key(|group| group.ord.unwrap_or(u32::MAX));
            self.ready = readable(&self.arrived);
        }
        changed
    }

    /// Look for questions renewed after an answer, and say whether any were.
    ///
    /// Separate from [`Self::refresh`] because it goes on after the seal: a
    /// finished deck still answers questions, and its rows renew.
    pub fn refresh_asks(&mut self) -> bool {
        let mut changed = false;
        for group in &mut self.arrived {
            let at = deck_cli::renewed_path(&self.root, &group.id);
            // The size too: two renewals inside one tick of a coarse clock
            // share a time, and seldom a length.
            let Ok(when) = std::fs::metadata(&at)
                .and_then(|meta| meta.modified().map(|time| (time, meta.len())))
            else {
                continue;
            };
            if self.renewed.get(&group.id) == Some(&when) {
                continue;
            }
            let Some(renewed) = std::fs::read_to_string(&at)
                .ok()
                .and_then(|text| serde_json::from_str::<deck_cli::Renewed>(&text).ok())
            else {
                // Half written: the next poll will find it whole.
                continue;
            };
            self.renewed.insert(group.id.clone(), when);
            if renewed.asks != group.asks {
                group.asks = renewed.asks;
                changed = true;
            }
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

impl Deck {
    /// The code a ref names, from wherever this deck reads it.
    ///
    /// A deck folder reads the file on disk. A deck from a `.deck` file reads
    /// its own copy first — what the reader was sent, and the code the claims
    /// are about — and the reader's checkout only once they have asked for
    /// it. A file it does not carry is said to be missing rather than looked
    /// for anywhere else: the same path in some other directory is somebody
    /// else's file.
    #[must_use]
    pub fn read_source(&self, file: &Path) -> Source {
        let Some(packed) = &self.packed else {
            return read_source(&self.base(), file);
        };
        let checkout = packed.checkout();
        if packed.live
            && let Some(checkout) = checkout
        {
            return match read_source(checkout, file) {
                Source::Missing { file, .. } => Source::Missing {
                    file,
                    said: " is not in your checkout.".into(),
                    live: false,
                },
                found => found,
            };
        }
        let named = file.to_string_lossy().replace('\\', "/");
        match packed.snapshots.get(&named) {
            Some(snapshot) => Source::frozen(snapshot),
            None => Source::Missing {
                file: file.to_path_buf(),
                said: " is not in this deck.".into(),
                live: checkout.is_some(),
            },
        }
    }
}

/// The text of a code ref, and whether it could be read.
///
/// A missing file is shown as a pane saying so. A deck that silently drops the
/// evidence for its own claim is worse than one that admits the file has moved.
#[must_use]
pub fn read_source(base: &Path, file: &Path) -> Source {
    let full = if file.is_absolute() {
        file.to_path_buf()
    } else {
        base.join(file)
    };

    match std::fs::read_to_string(&full) {
        Ok(text) => Source::Live(text.into()),
        Err(err) => Source::Missing {
            file: file.to_path_buf(),
            said: format!(" could not be read: {err}.").into(),
            live: false,
        },
    }
}

/// A title as it is shown: capitalised at the front, as [`Deck::title`] says.
#[must_use]
pub fn heading(title: &str) -> String {
    let mut letters = title.chars();
    match letters.next() {
        Some(first) => first.to_uppercase().chain(letters).collect(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use deck_core::bundle::snap;

    /// The deck's copy of a 60-line file, kept around lines 30–31.
    fn frozen() -> snap::Snapshot {
        let source: String = (1..=60).map(|n| format!("line {n}\n")).collect();
        let (kept, text) = snap::keep(&source, &[LineRange::new(30, 31)], 5);
        snap::Snapshot {
            file: snap::File {
                path: "src/batch.ts".to_string(),
                entry: "snap/1.txt".to_string(),
                sha256: String::new(),
                commit: None,
                state: snap::State::Untracked,
                kept,
                lines: Some(60),
            },
            text,
        }
    }

    /// A deck from a file carrying [`frozen`], read from `cwd`'s point of view.
    fn packed(checkout: Option<&Path>, live: bool) -> Deck {
        let snapshot = frozen();
        let repo = checkout.map_or(deck_cli::opened::Match::Elsewhere, |root| {
            deck_cli::opened::Match::Moved {
                packed: "a1b2c3d".to_string(),
                head: "9f8e7d6".to_string(),
                root: root.to_path_buf(),
            }
        });
        Deck {
            root: PathBuf::from("/nowhere/deck"),
            header: serde_json::from_str(r#"{"v":1,"id":"d-1","title":"t"}"#).expect("a header"),
            arrived: Vec::new(),
            ready: 0,
            seen: HashSet::new(),
            sealed: true,
            renewed: HashMap::new(),
            packed: Some(Packed {
                origin: deck_cli::opened::Origin {
                    file: PathBuf::from("/shelf/batch.deck"),
                    review: PathBuf::from("/shelf/batch.review"),
                },
                snapshots: [(snapshot.file.path.clone(), std::sync::Arc::new(snapshot))]
                    .into_iter()
                    .collect(),
                repo,
                live,
            }),
        }
    }

    #[test]
    fn a_packed_deck_serves_its_own_copy_first() {
        let dir = tempfile::tempdir().expect("a temp dir");
        std::fs::create_dir_all(dir.path().join("src")).expect("a folder");
        std::fs::write(dir.path().join("src/batch.ts"), "the checkout\n").expect("a file");
        // Inside the repo, and the checkout has the file — and still the
        // deck's copy is what comes back until the reader asks.
        let deck = packed(Some(dir.path()), false);

        let Source::Snapshot { text, kept, lines } = deck.read_source(Path::new("src/batch.ts"))
        else {
            panic!("the deck's copy");
        };
        assert_eq!(&*kept, &[LineRange::new(25, 36)]);
        assert_eq!(lines, Some(60));
        let numbered: Vec<&str> = text.lines().collect();
        assert_eq!(numbered.len(), 36, "numbered up to the last line kept");
        assert_eq!(numbered[29], "line 30", "line 30 is line 30");
        assert_eq!(numbered[0], "", "a line it did not keep is empty");
    }

    #[test]
    fn shown_live_it_reads_the_checkout() {
        let dir = tempfile::tempdir().expect("a temp dir");
        std::fs::create_dir_all(dir.path().join("src")).expect("a folder");
        std::fs::write(dir.path().join("src/batch.ts"), "the checkout\n").expect("a file");
        let deck = packed(Some(dir.path()), true);

        let Source::Live(text) = deck.read_source(Path::new("src/batch.ts")) else {
            panic!("the checkout");
        };
        assert_eq!(&*text, "the checkout\n");
    }

    #[test]
    fn a_file_the_deck_does_not_carry_is_missing_and_offered_only_inside_the_repo() {
        let outside = packed(None, false);
        let Source::Missing { file, said, live } = outside.read_source(Path::new("src/flush.ts"))
        else {
            panic!("missing");
        };
        assert_eq!(file, Path::new("src/flush.ts"));
        assert_eq!(&*said, " is not in this deck.");
        assert!(!live, "no checkout, nothing to offer");

        let dir = tempfile::tempdir().expect("a temp dir");
        let inside = packed(Some(dir.path()), false);
        assert!(matches!(
            inside.read_source(Path::new("src/flush.ts")),
            Source::Missing { live: true, .. }
        ));
        assert_eq!(inside.read_source(Path::new("src/flush.ts")).text(), "");
    }

    #[test]
    fn a_deck_folder_reads_the_disk_and_says_when_it_cannot() {
        let dir = tempfile::tempdir().expect("a temp dir");
        std::fs::write(dir.path().join("a.rs"), "fn a() {}\n").expect("a file");
        assert!(matches!(
            read_source(dir.path(), Path::new("a.rs")),
            Source::Live(text) if &*text == "fn a() {}\n"
        ));
        // Said, not shown as if it were the file.
        let Source::Missing { said, live, .. } = read_source(dir.path(), Path::new("gone.rs"))
        else {
            panic!("missing");
        };
        assert!(said.starts_with(" could not be read"), "{said}");
        assert!(!live);
    }
    use deck_core::protocol::Group;

    fn group(ord: Option<u32>) -> Group {
        Group {
            asks: Vec::new(),
            id: format!("g{}", ord.unwrap_or(0)),
            ord,
            title: None,
            confidence: None,
            say: String::new(),
            refs: Vec::new(),
        }
    }

    #[test]
    fn a_full_run_is_all_readable() {
        let deck = [group(Some(1)), group(Some(2)), group(Some(3))];
        assert_eq!(readable(&deck), 3);
    }

    #[test]
    fn a_group_that_arrives_early_waits_for_the_gap_in_front_of_it() {
        // Group 3 is here and group 2 is not. Showing it would put the reader
        // in the middle of an argument whose previous step has not been made.
        let deck = [group(Some(1)), group(Some(3))];
        assert_eq!(readable(&deck), 1);
    }

    #[test]
    fn nothing_is_readable_until_the_first_group_lands() {
        let deck = [group(Some(2)), group(Some(3))];
        assert_eq!(readable(&deck), 0);
    }

    #[test]
    fn a_deck_that_numbers_nothing_is_not_claiming_an_order() {
        let deck = [group(None), group(None)];
        assert_eq!(readable(&deck), 2);
    }

    #[test]
    fn an_empty_deck_is_readable_and_empty() {
        assert_eq!(readable(&[]), 0);
    }
}
