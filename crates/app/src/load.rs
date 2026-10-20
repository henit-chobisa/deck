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
    /// The deck folder at `root`, opened from the file `origin` names.
    ///
    /// The repo is asked about the directory the reader is in, once, here: a
    /// reader does not change repo under an open window, and asking git on
    /// every pane would be a process per file for an answer that does not
    /// change.
    #[must_use]
    pub fn of(root: &Path, origin: deck_cli::opened::Origin) -> Self {
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
        Self {
            origin,
            snapshots,
            repo,
            live: false,
        }
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
    pub fn frozen(
        &self,
        file: &Path,
    ) -> Option<&std::sync::Arc<deck_core::bundle::snap::Snapshot>> {
        self.snapshots.get(&named(file))
    }

    /// Whether the reader's checkout, at the very commit the deck was packed
    /// at, still holds something other than the deck's copy of `file`.
    ///
    /// The commit alone does not say. A deck packed from a file with changes
    /// nobody had committed — or one git was not tracking — shows code no
    /// commit has, and a checkout can have edits of its own on top of the
    /// same commit. So the file's own hash is compared with the one the deck
    /// recorded, whenever it recorded one. Only a deck that recorded no hash
    /// — an export that froze only part of a file cannot vouch for the rest
    /// of it — falls back to the deck's word on how the file stood: unknown
    /// is not the same as different.
    #[must_use]
    pub fn differs(&self, file: &Path) -> bool {
        let deck_cli::opened::Match::Same { root, .. } = &self.repo else {
            return false;
        };
        let Some(snapshot) = self.frozen(file) else {
            return false;
        };
        differs(
            &snapshot.file,
            std::fs::read(root.join(file)).ok().as_deref(),
        )
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

/// A ref's path as a snapshot names it: forward slashes, whatever the
/// machine.
fn named(file: &Path) -> String {
    file.to_string_lossy().replace('\\', "/")
}

/// Whether the checkout's `bytes` for a file — `None` when it has none — are
/// other than the deck's copy described by `file`. See [`Packed::differs`].
#[must_use]
pub fn differs(file: &deck_core::bundle::snap::File, bytes: Option<&[u8]>) -> bool {
    use deck_core::bundle::snap::State;
    // The hash, whenever there is one, whatever the state: a file packed
    // with changes the reader has since committed is the same file.
    if !file.sha256.is_empty() {
        return bytes.is_none_or(|bytes| deck_core::bundle::sha256(bytes) != file.sha256);
    }
    file.state != State::Clean
}

/// What a pane with no code offers instead.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Offer {
    /// The reader's checkout has the file: `· Show live`.
    Live,
    /// The panes are showing the checkout, and it lacks the file: `· Show the
    /// deck's copy`, the way back.
    Deck,
}

/// Where a pane's code comes from, and what can honestly be shown of it.
///
/// It used to be a string, and when a file could not be read the string was
/// the error — so the reason a file was missing was drawn as if it were the
/// file, numbered, highlighted and selectable. A pane has three different
/// things to draw, and each is its own case here.
#[derive(Clone)]
pub enum Source {
    /// The deck's own copy, from a `.deck` file: only the spans it kept, as
    /// spans. Never padded out to the file it came from — a span is two
    /// numbers, and a hostile one is a long way from line one.
    Snapshot(std::sync::Arc<deck_core::bundle::snap::Snapshot>),
    /// The file as it is on disk now.
    Live(std::sync::Arc<str>),
    /// Nothing to show.
    Missing {
        /// The file, as the ref names it.
        file: PathBuf,
        /// The rest of the sentence after its name: ` is not in this deck.`
        said: std::sync::Arc<str>,
        /// What the pane offers instead, if anything.
        offer: Option<Offer>,
    },
}

impl Source {
    /// The whole file, when this is one — the text everything that diffs or
    /// quotes a file reads. The deck's copy is not a whole file, and a file
    /// that is missing has no text.
    #[must_use]
    pub fn whole(&self) -> Option<&str> {
        match self {
            Self::Live(text) => Some(text),
            Self::Snapshot(_) | Self::Missing { .. } => None,
        }
    }
}

/// Where the lines `range` of the deck's copy are in `current`, the checkout's
/// file, or `None` when they cannot be followed there with confidence.
///
/// The copy's kept lines are compared with the checkout's whole file — the
/// lines the deck left out are not anybody's to compare — so the range is
/// taken into the copy's own count first and brought back out of the
/// checkout's. A range it could not place at all — stale — comes back as
/// `None`: lighting the old numbers on a file that has moved would light code
/// the claim was never about.
#[must_use]
pub fn followed(
    snapshot: &deck_core::bundle::snap::Snapshot,
    current: &str,
    range: LineRange,
) -> Option<LineRange> {
    let mut before = 0u32;
    let mut inside = None;
    for span in &snapshot.file.kept {
        if span.first <= range.first && range.last <= span.last {
            inside = Some(before + (range.first - span.first) + 1);
            break;
        }
        before += span.len();
    }
    let first = inside?;
    let moved = deck_core::relocate(
        &snapshot.text,
        current,
        &[LineRange::new(first, first + range.len() - 1)],
    );
    moved
        .first()
        .filter(|moved| moved.source != deck_core::protocol::Source::Stale)
        .map(|moved| moved.range)
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
            // Set by whoever opened it from a file: see `Packed::of`.
            packed: None,
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
                    offer: Some(Offer::Deck),
                },
                found => found,
            };
        }
        match packed.frozen(file) {
            Some(snapshot) => Source::Snapshot(snapshot.clone()),
            None => Source::Missing {
                file: file.to_path_buf(),
                said: " is not in this deck.".into(),
                offer: checkout.map(|_| Offer::Live),
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
            offer: None,
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
                    sha256: "f".repeat(64),
                    review: PathBuf::from("/shelf/batch.review"),
                    beside: None,
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

        let Source::Snapshot(snapshot) = deck.read_source(Path::new("src/batch.ts")) else {
            panic!("the deck's copy");
        };
        assert_eq!(snapshot.file.kept, [LineRange::new(25, 36)]);
        assert_eq!(snapshot.line(30), Some("line 30"), "line 30 is line 30");
        assert_eq!(
            snapshot.line(24),
            None,
            "a line it did not keep is not there"
        );
        // Spans, not a file padded out to its length: what is held is what
        // was kept and nothing more.
        assert_eq!(snapshot.text.lines().count(), 12);
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
        let Source::Missing { file, said, offer } = outside.read_source(Path::new("src/flush.ts"))
        else {
            panic!("missing");
        };
        assert_eq!(file, Path::new("src/flush.ts"));
        assert_eq!(&*said, " is not in this deck.");
        assert_eq!(offer, None, "no checkout, nothing to offer");

        let dir = tempfile::tempdir().expect("a temp dir");
        let inside = packed(Some(dir.path()), false);
        assert!(matches!(
            inside.read_source(Path::new("src/flush.ts")),
            Source::Missing {
                offer: Some(Offer::Live),
                ..
            }
        ));
        assert!(
            inside
                .read_source(Path::new("src/flush.ts"))
                .whole()
                .is_none()
        );
    }

    #[test]
    fn shown_live_a_file_the_checkout_lacks_offers_the_way_back() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let deck = packed(Some(dir.path()), true);
        let Source::Missing { said, offer, .. } = deck.read_source(Path::new("src/batch.ts"))
        else {
            panic!("missing from the checkout");
        };
        assert_eq!(&*said, " is not in your checkout.");
        assert_eq!(offer, Some(Offer::Deck));
    }

    #[test]
    fn lines_are_followed_into_the_checkout_or_not_lit_at_all() {
        let snapshot = frozen();
        // Twelve lines added above: the range moves down twelve.
        let moved: String = (1..=12)
            .map(|n| format!("added {n}\n"))
            .chain((1..=60).map(|n| format!("line {n}\n")))
            .collect();
        assert_eq!(
            followed(&snapshot, &moved, LineRange::new(30, 31)),
            Some(LineRange::new(42, 43))
        );
        // Nothing of the copy left in the checkout: stale, and not lit.
        let gone: String = (1..=60).map(|n| format!("other {n}\n")).collect();
        assert_eq!(followed(&snapshot, &gone, LineRange::new(30, 31)), None);
        // A range the copy does not hold cannot be followed from it.
        assert_eq!(followed(&snapshot, &moved, LineRange::new(2, 3)), None);
    }

    #[test]
    fn the_same_commit_can_still_hold_other_code() {
        let mut file = frozen().file;
        let bytes: &[u8] = b"line 1\n";
        file.state = snap::State::Clean;
        file.sha256 = deck_core::bundle::sha256(bytes);
        assert!(!differs(&file, Some(bytes)), "the very file");
        assert!(differs(&file, Some(b"edited\n")), "edited since");
        assert!(differs(&file, None), "not in the checkout");
        // Packed from changes nobody committed, which the reader now has
        // byte for byte: the hash says it is the same file, whatever the
        // state said.
        file.state = snap::State::Modified;
        assert!(!differs(&file, Some(bytes)));
        assert!(differs(&file, Some(b"edited\n")));
    }

    #[test]
    fn a_hash_the_deck_did_not_record_is_unknown_not_different() {
        // An export that froze only part of the file leaves the hash empty:
        // the state and the commit are all there is to go on.
        let mut file = frozen().file;
        file.sha256 = String::new();
        file.state = snap::State::Clean;
        assert!(!differs(&file, Some(b"anything at all\n")));
        assert!(!differs(&file, None));
        file.state = snap::State::Modified;
        assert!(differs(&file, Some(b"anything at all\n")));
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
        let Source::Missing { said, offer, .. } = read_source(dir.path(), Path::new("gone.rs"))
        else {
            panic!("missing");
        };
        assert!(said.starts_with(" could not be read"), "{said}");
        assert_eq!(offer, None);
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
