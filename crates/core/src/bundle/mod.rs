//! A `.deck` file: a deck that carries its own evidence.
//!
//! A deck on disk is a folder whose refs point into the repo it was written
//! in. Sent to anybody else it opens to nothing. A `.deck` file is the same
//! deck as a document: the protocol files, the code they point at frozen into
//! snapshots ([`snap`]), and the review when there is one, in one zip
//! ([`zip`]) that anybody can open.
//!
//! This is the format and only the format: bytes in, bytes out. Gathering the
//! files, scrubbing them and scanning them for secrets happen where there is a
//! disk and a repo to read, and they hand this module what to pack.
//!
//! # What is in one
//!
//! | entry              | what it is                                         |
//! |--------------------|----------------------------------------------------|
//! | `mimetype`         | `application/vnd.deck+zip`, first and stored, so a file can be recognised from its first bytes |
//! | `manifest.json`    | [`Manifest`]: versions, when and by what, the repo, and every other entry's sha256 |
//! | `deck/deck.json`   | the deck's header, as the protocol defines it      |
//! | `deck/g<N>.json`   | one per group                                      |
//! | `deck/done`        | present when the deck was sealed                   |
//! | `snap/index.json`  | [`snap::Index`]: every source file frozen          |
//! | `snap/<N>.txt`     | one source file's kept lines                       |
//! | `review.json`      | the review, when the deck has been read            |
//!
//! The protocol files are the ones the folder holds, after scrubbing: a deck
//! folder also keeps a reader's unsent draft and the markers a live session
//! leaves, and none of those leave the machine. `draw/` is reserved for
//! diagrams drawn ahead of time; nothing writes it yet, and a reader ignores
//! it.
//!
//! # Versions
//!
//! The container has a version of its own, apart from the protocol's. A
//! reader opens any file with its major version, ignoring entries and fields
//! it does not know — a newer deck's additions — and refuses a newer major
//! version with the name of the deck that made it, so the reader knows what
//! to upgrade to.

pub mod snap;
pub mod zip;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::protocol::{Group, Header, Review};

/// What the first entry of every `.deck` file says.
pub const MIMETYPE: &str = "application/vnd.deck+zip";

/// The container version this build writes.
pub const CONTAINER: Container = Container { major: 1, minor: 0 };

/// A container version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Container {
    /// Changes when an older reader could not read the file correctly.
    pub major: u32,
    /// Changes when something is added that an older reader can ignore.
    pub minor: u32,
}

/// What a `.deck` file says about itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    /// The container version it was written as.
    pub container: Container,
    /// The protocol version of the deck inside.
    pub protocol: u32,
    /// When it was packed, in UTC, as RFC 3339: `2026-10-20T09:30:00Z`.
    pub created: String,
    /// What packed it, as `deck 0.1.7`.
    pub generator: String,
    /// The repo the deck was written in, when there was one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo: Option<Repo>,
    /// Every entry but `mimetype` and this one, by name, as sha256 in hex.
    ///
    /// The manifest cannot carry its own hash. The zip's checksum covers it,
    /// and it covers everything else.
    #[serde(default)]
    pub entries: BTreeMap<String, String>,
    /// Secret-scanner findings the author looked at and let through, by
    /// fingerprint, so a reader can see that somebody chose to.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed: Vec<String>,
}

/// Where a deck was written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Repo {
    /// The remote it pushes to, with any credentials taken out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remote: Option<String>,
    /// The commit it was at.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
}

/// A deck's files, ready to pack or just opened.
///
/// Every file is held as the bytes it is stored as, so packing what was
/// opened gives back the same file. The typed views — [`Self::header`],
/// [`Self::groups`] and the rest — read from those bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bundle {
    /// What the file says about itself. Its `entries` are worked out when the
    /// bundle is packed, so a bundle about to be packed may leave them empty.
    pub manifest: Manifest,
    /// Every entry but `mimetype` and `manifest.json`, by name.
    pub files: BTreeMap<String, Vec<u8>>,
}

/// Why bytes could not be opened as a `.deck` file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unreadable {
    /// Not a zip deck could have written. See [`zip::Refusal`].
    Zip(zip::Refusal),
    /// A zip, but not a deck: its first entry is not the deck mimetype.
    NotADeck,
    /// Written by a deck newer than this one, in a way this one cannot read.
    Newer {
        /// The container version it was written as.
        container: Container,
        /// What wrote it.
        generator: String,
    },
    /// A file it needs, or one its manifest lists, is not there.
    Missing(String),
    /// A file is not what its manifest says it is.
    Changed(String),
    /// A file is there but does not parse as what it should be.
    Malformed(String),
}

impl std::fmt::Display for Unreadable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Zip(refusal) => write!(f, "{refusal}"),
            Self::NotADeck => write!(f, "this is a zip, but not a deck"),
            Self::Newer { generator, .. } => {
                write!(
                    f,
                    "made by {generator}, which is newer than this deck: upgrade to open it"
                )
            }
            Self::Missing(name) => write!(f, "{name} is missing"),
            Self::Changed(name) => write!(f, "{name} has been changed since it was packed"),
            Self::Malformed(why) => write!(f, "{why}"),
        }
    }
}

impl std::error::Error for Unreadable {}

impl From<zip::Refusal> for Unreadable {
    fn from(refusal: zip::Refusal) -> Self {
        Self::Zip(refusal)
    }
}

/// Where an entry goes in the file, by what kind of entry it is and then its
/// number, or `None` when it is not a name a deck uses.
///
/// The order is fixed so the same deck always packs to the same bytes, and
/// it reads like the deck: what it is, then its groups in order, then the
/// code they show, then what the reader said.
fn rank(name: &str) -> Option<(u8, u32)> {
    let numbered = |prefix: &str, suffix: &str| {
        name.strip_prefix(prefix)
            .and_then(|rest| rest.strip_suffix(suffix))
            .filter(|n| {
                !n.is_empty() && !n.starts_with('0') && n.bytes().all(|b| b.is_ascii_digit())
            })
            .and_then(|n| n.parse::<u32>().ok())
    };
    match name {
        "mimetype" => Some((0, 0)),
        "manifest.json" => Some((1, 0)),
        "deck/deck.json" => Some((2, 0)),
        "deck/done" => Some((4, 0)),
        "snap/index.json" => Some((5, 0)),
        "review.json" => Some((7, 0)),
        _ => numbered("deck/g", ".json")
            .map(|n| (3, n))
            .or_else(|| numbered("snap/", ".txt").map(|n| (6, n))),
    }
}

/// Whether `name` is one of the entries this build of deck reads.
#[must_use]
pub fn known(name: &str) -> bool {
    rank(name).is_some()
}

/// sha256 of `bytes`, in lowercase hex.
#[must_use]
pub fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .fold(String::with_capacity(64), |mut hex, b| {
            use std::fmt::Write as _;
            let _ = write!(hex, "{b:02x}");
            hex
        })
}

impl Bundle {
    /// The bundle as a `.deck` file, stamped with this build's container
    /// version.
    ///
    /// The same bundle always gives the same bytes: entries in a fixed
    /// order, every time and date fixed, the manifest's own `created` and
    /// `generator` whatever the caller put there. Packing what was opened
    /// gives back the same file when this build wrote it. A file from a newer
    /// minor version loses the additions this build cannot read, so it is
    /// stamped as this version rather than claim what it no longer holds.
    ///
    /// What comes back has been opened again before it is returned: a file
    /// `pack` makes is a file [`Self::open`] accepts.
    ///
    /// # Errors
    ///
    /// When a file's name is not one a deck uses, `deck/deck.json` is
    /// missing, or the result would not open — too many files, too large,
    /// or a part that does not parse.
    pub fn pack(&self) -> Result<Vec<u8>, String> {
        let mut stamped = self.clone();
        stamped.manifest.container = CONTAINER;
        let bytes = stamped.write()?;
        Self::open(&bytes).map_err(|err| err.to_string())?;
        Ok(bytes)
    }

    /// The bundle as a `.deck` file, as it is, unchecked.
    fn write(&self) -> Result<Vec<u8>, String> {
        let limits = zip::Limits::default();
        // Two for the mimetype and the manifest.
        if self.files.len() + 2 > limits.entries
            || self.files.values().map(Vec::len).sum::<usize>() > limits.bytes
        {
            return Err("more than a deck can hold".to_string());
        }
        if !self.files.contains_key("deck/deck.json") {
            return Err("a deck needs deck/deck.json".to_string());
        }
        if let Some(name) = self
            .files
            .keys()
            .find(|name| !known(name) || matches!(name.as_str(), "mimetype" | "manifest.json"))
        {
            return Err(format!("{name} is not a file a deck holds"));
        }
        let mut manifest = self.manifest.clone();
        manifest.entries = self
            .files
            .iter()
            .map(|(name, bytes)| (name.clone(), sha256(bytes)))
            .collect();
        let mut manifest_json =
            serde_json::to_vec_pretty(&manifest).map_err(|err| err.to_string())?;
        manifest_json.push(b'\n');

        let mut entries: Vec<(String, Vec<u8>)> = vec![
            ("mimetype".to_string(), MIMETYPE.as_bytes().to_vec()),
            ("manifest.json".to_string(), manifest_json),
        ];
        let mut rest: Vec<(&String, &Vec<u8>)> = self.files.iter().collect();
        rest.sort_by_key(|(name, _)| rank(name));
        entries.extend(rest.into_iter().map(|(n, b)| (n.clone(), b.clone())));
        Ok(zip::write(&entries))
    }

    /// The bundle in `bytes`, every entry checked against the manifest.
    ///
    /// Entries this build does not know are left out, and named in the
    /// second half of what comes back: a newer deck's additions, which an
    /// older one shows what it can of.
    ///
    /// # Errors
    ///
    /// Any [`Unreadable`].
    pub fn open(bytes: &[u8]) -> Result<(Self, Vec<String>), Unreadable> {
        let entries = zip::read(bytes, zip::Limits::default())?;
        let mut entries = entries.into_iter();
        match entries.next() {
            Some((name, content)) if name == "mimetype" && content == MIMETYPE.as_bytes() => {}
            _ => return Err(Unreadable::NotADeck),
        }
        let entries: Vec<(String, Vec<u8>)> = entries.collect();
        // In the order deck writes them, so a file that opens packs back to
        // the same bytes. Entries this build does not know have no place in
        // that order and are left out of the check.
        let ranks: Vec<(u8, u32)> = entries.iter().filter_map(|(name, _)| rank(name)).collect();
        if ranks.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(Unreadable::Malformed(
                "its entries are not in the order deck writes them".to_string(),
            ));
        }
        let mut files: BTreeMap<String, Vec<u8>> = entries.into_iter().collect();

        let manifest_json = files
            .remove("manifest.json")
            .ok_or_else(|| Unreadable::Missing("manifest.json".to_string()))?;
        // The version first, on its own, so a newer file whose manifest has
        // changed shape is still told apart from a broken one.
        #[derive(Deserialize)]
        struct Versions {
            container: Container,
            #[serde(default)]
            generator: String,
        }
        let versions: Versions = serde_json::from_slice(&manifest_json)
            .map_err(|err| Unreadable::Malformed(format!("manifest.json: {err}")))?;
        if versions.container.major > CONTAINER.major {
            return Err(Unreadable::Newer {
                container: versions.container,
                generator: versions.generator,
            });
        }
        if versions.container.major < CONTAINER.major {
            return Err(Unreadable::Malformed(format!(
                "container version {}.{} was never written by any deck",
                versions.container.major, versions.container.minor
            )));
        }
        let manifest: Manifest = serde_json::from_slice(&manifest_json)
            .map_err(|err| Unreadable::Malformed(format!("manifest.json: {err}")))?;

        for (name, hash) in &manifest.entries {
            let bytes = files
                .get(name)
                .ok_or_else(|| Unreadable::Missing(name.clone()))?;
            if &sha256(bytes) != hash {
                return Err(Unreadable::Changed(name.clone()));
            }
        }
        let mut ignored = Vec::new();
        for name in files.keys() {
            if !known(name) {
                ignored.push(name.clone());
            } else if !manifest.entries.contains_key(name) {
                // A known entry the manifest never vouched for was added after
                // packing.
                return Err(Unreadable::Changed(name.clone()));
            }
        }
        files.retain(|name, _| known(name));
        if !files.contains_key("deck/deck.json") {
            return Err(Unreadable::Missing("deck/deck.json".to_string()));
        }
        let bundle = Self { manifest, files };
        // Read every typed part once, so a file that opens is a file whose
        // parts all parse.
        bundle.header()?;
        bundle.groups()?;
        bundle.snapshots()?;
        bundle.review()?;
        Ok((bundle, ignored))
    }

    /// The deck's header.
    ///
    /// # Errors
    ///
    /// When `deck/deck.json` is missing or is not a header.
    pub fn header(&self) -> Result<Header, Unreadable> {
        self.json("deck/deck.json")?
            .ok_or_else(|| Unreadable::Missing("deck/deck.json".to_string()))
    }

    /// Every group, in order.
    ///
    /// # Errors
    ///
    /// When a group's file is not a group.
    pub fn groups(&self) -> Result<Vec<Group>, Unreadable> {
        let mut names: Vec<&String> = self
            .files
            .keys()
            .filter(|name| rank(name).is_some_and(|(kind, _)| kind == 3))
            .collect();
        names.sort_by_key(|name| rank(name));
        names
            .into_iter()
            .map(|name| {
                self.json(name)?
                    .ok_or_else(|| Unreadable::Missing(name.clone()))
            })
            .collect()
    }

    /// Whether the deck was sealed.
    #[must_use]
    pub fn sealed(&self) -> bool {
        self.files.contains_key("deck/done")
    }

    /// The review, when the deck has been read.
    ///
    /// # Errors
    ///
    /// When `review.json` is not a review.
    pub fn review(&self) -> Result<Option<Review>, Unreadable> {
        self.json("review.json")
    }

    /// Every frozen source file, with its kept lines.
    ///
    /// # Errors
    ///
    /// When the index does not parse, names an entry that is not there, or
    /// describes lines its entry does not hold.
    pub fn snapshots(&self) -> Result<Vec<snap::Snapshot>, Unreadable> {
        let Some(index) = self.json::<snap::Index>("snap/index.json")? else {
            return Ok(Vec::new());
        };
        index
            .files
            .into_iter()
            .map(|file| {
                if rank(&file.entry).is_none_or(|(kind, _)| kind != 6) {
                    return Err(Unreadable::Malformed(format!(
                        "snap/index.json names {:?}, which is not a snapshot",
                        file.entry
                    )));
                }
                let bytes = self
                    .files
                    .get(&file.entry)
                    .ok_or_else(|| Unreadable::Missing(file.entry.clone()))?;
                let text = String::from_utf8(bytes.clone())
                    .map_err(|_| Unreadable::Malformed(format!("{} is not text", file.entry)))?;
                let snapshot = snap::Snapshot { file, text };
                snapshot.check().map_err(Unreadable::Malformed)?;
                Ok(snapshot)
            })
            .collect()
    }

    fn json<T: serde::de::DeserializeOwned>(&self, name: &str) -> Result<Option<T>, Unreadable> {
        self.files
            .get(name)
            .map(|bytes| {
                serde_json::from_slice(bytes)
                    .map_err(|err| Unreadable::Malformed(format!("{name}: {err}")))
            })
            .transpose()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::line::LineRange;

    fn manifest() -> Manifest {
        Manifest {
            container: CONTAINER,
            protocol: crate::protocol::VERSION,
            created: "2026-10-20T09:30:00Z".to_string(),
            generator: "deck 0.1.7".to_string(),
            repo: Some(Repo {
                remote: Some("https://github.com/acme/ingest".to_string()),
                commit: Some("a1b2c3d".to_string()),
            }),
            entries: BTreeMap::new(),
            allowed: Vec::new(),
        }
    }

    /// A small deck: two groups out of order by name, a snapshot and a review.
    fn bundle() -> Bundle {
        let source: String = (1..=60).map(|n| format!("line {n}\n")).collect();
        let (kept, text) = snap::keep(&source, &[LineRange::new(30, 31)], 5);
        let index = snap::Index {
            files: vec![snap::File {
                path: "src/batch.ts".to_string(),
                entry: "snap/1.txt".to_string(),
                sha256: sha256(source.as_bytes()),
                commit: Some("a1b2c3d".to_string()),
                state: snap::State::Clean,
                kept,
                lines: None,
            }],
        };
        let mut files = BTreeMap::new();
        files.insert(
            "deck/deck.json".to_string(),
            br#"{"v":1,"id":"d-1","title":"Why the batch stalls"}"#.to_vec(),
        );
        files.insert(
            "deck/g10.json".to_string(),
            br#"{"id":"g10","say":"ten"}"#.to_vec(),
        );
        files.insert(
            "deck/g2.json".to_string(),
            br#"{"id":"g2","say":"two"}"#.to_vec(),
        );
        files.insert("deck/done".to_string(), Vec::new());
        files.insert(
            "snap/index.json".to_string(),
            serde_json::to_vec(&index).expect("serialises"),
        );
        files.insert("snap/1.txt".to_string(), text.into_bytes());
        files.insert(
            "review.json".to_string(),
            br#"{"v":1,"deck":"d-1","comments":[]}"#.to_vec(),
        );
        Bundle {
            manifest: manifest(),
            files,
        }
    }

    fn names(bytes: &[u8]) -> Vec<String> {
        zip::read(bytes, zip::Limits::default())
            .expect("reads")
            .into_iter()
            .map(|(name, _)| name)
            .collect()
    }

    #[test]
    fn a_packed_deck_opens_to_what_was_packed() {
        let packed = bundle().pack().expect("packs");
        let (opened, ignored) = Bundle::open(&packed).expect("opens");
        assert!(ignored.is_empty());
        assert_eq!(opened.files, bundle().files);
        assert_eq!(
            opened.header().expect("a header").title,
            "Why the batch stalls"
        );
        let groups: Vec<String> = opened
            .groups()
            .expect("groups")
            .into_iter()
            .map(|g| g.id)
            .collect();
        assert_eq!(groups, ["g2", "g10"]);
        assert!(opened.sealed());
        let snaps = opened.snapshots().expect("snapshots");
        assert_eq!(snaps[0].line(30), Some("line 30"));
        assert!(opened.review().expect("a review").is_some());
    }

    /// The bytes of `bundle()` packed, pinned by their hash.
    ///
    /// A hash rather than a golden file, because the checkout's line-ending
    /// rules would bend a binary file kept beside the text ones. When a change
    /// to the format is meant, this is the number to update — and a change
    /// nobody meant is the thing it is here to catch.
    #[test]
    fn the_same_deck_packs_to_the_same_bytes_everywhere() {
        let packed = bundle().pack().expect("packs");
        assert_eq!(
            sha256(&packed),
            "a8ed4046ce2aabe29c7649ce6c1fb32f87fe99c7a8130cfa368b9d18654834be",
            "the packed bytes moved; if the format was meant to change, update this"
        );
    }

    #[test]
    fn packing_what_was_opened_gives_the_same_bytes() {
        let packed = bundle().pack().expect("packs");
        let (opened, _) = Bundle::open(&packed).expect("opens");
        assert_eq!(opened.pack().expect("packs again"), packed);
    }

    #[test]
    fn entries_go_in_the_order_the_deck_reads() {
        assert_eq!(
            names(&bundle().pack().expect("packs")),
            [
                "mimetype",
                "manifest.json",
                "deck/deck.json",
                "deck/g2.json",
                "deck/g10.json",
                "deck/done",
                "snap/index.json",
                "snap/1.txt",
                "review.json",
            ]
        );
    }

    #[test]
    fn the_manifest_vouches_for_every_entry() {
        let packed = bundle().pack().expect("packs");
        let (opened, _) = Bundle::open(&packed).expect("opens");
        assert_eq!(opened.manifest.entries.len(), bundle().files.len());
        assert_eq!(
            opened.manifest.entries["deck/done"],
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    /// `bundle()` packed with one file changed after the manifest was made.
    fn tampered(name: &str, bytes: &[u8]) -> Vec<u8> {
        let packed = bundle().pack().expect("packs");
        let mut entries = zip::read(&packed, zip::Limits::default()).expect("reads");
        match entries.iter_mut().find(|(n, _)| n == name) {
            Some(entry) => entry.1 = bytes.to_vec(),
            // Slipped in where deck would have put it, so only the manifest
            // can tell.
            None => {
                let at = rank(name).map_or(entries.len(), |new| {
                    entries
                        .iter()
                        .position(|(n, _)| rank(n).is_some_and(|r| r > new))
                        .unwrap_or(entries.len())
                });
                entries.insert(at, (name.to_string(), bytes.to_vec()));
            }
        }
        zip::write(&entries)
    }

    #[test]
    fn a_file_changed_after_packing_is_refused() {
        let packed = tampered("deck/g2.json", br#"{"id":"g2","say":"changed"}"#);
        assert_eq!(
            Bundle::open(&packed),
            Err(Unreadable::Changed("deck/g2.json".to_string()))
        );
    }

    #[test]
    fn a_known_file_added_after_packing_is_refused() {
        let packed = tampered("deck/g3.json", br#"{"id":"g3","say":"slipped in"}"#);
        assert_eq!(
            Bundle::open(&packed),
            Err(Unreadable::Changed("deck/g3.json".to_string()))
        );
    }

    #[test]
    fn a_newer_files_extras_are_ignored_and_named() {
        let packed = tampered("draw/g2d1.svg", b"<svg/>");
        let (opened, ignored) = Bundle::open(&packed).expect("opens");
        assert_eq!(ignored, ["draw/g2d1.svg"]);
        assert!(!opened.files.contains_key("draw/g2d1.svg"));
    }

    #[test]
    fn a_newer_major_version_is_refused_by_name() {
        let mut newer = bundle();
        newer.manifest.container = Container { major: 2, minor: 0 };
        newer.manifest.generator = "deck 0.9.0".to_string();
        let packed = newer.write().expect("writes");
        assert_eq!(
            Bundle::open(&packed),
            Err(Unreadable::Newer {
                container: Container { major: 2, minor: 0 },
                generator: "deck 0.9.0".to_string(),
            })
        );
        // A newer minor version is still this format.
        let mut minor = bundle();
        minor.manifest.container.minor = 7;
        let packed = minor.write().expect("writes");
        assert!(Bundle::open(&packed).is_ok());
        // Packed again, it says what it now is: this version.
        let (opened, _) = Bundle::open(&packed).expect("opens");
        let again = opened.pack().expect("packs");
        let (reopened, _) = Bundle::open(&again).expect("opens");
        assert_eq!(reopened.manifest.container, CONTAINER);
        // And a major version below one is not "newer".
        let mut old = bundle();
        old.manifest.container = Container { major: 0, minor: 1 };
        assert!(matches!(
            Bundle::open(&old.write().expect("writes")),
            Err(Unreadable::Malformed(_))
        ));
    }

    #[test]
    fn a_zip_that_is_not_a_deck_is_told_apart() {
        let other = zip::write(&[("mimetype".to_string(), b"application/epub+zip".to_vec())]);
        assert_eq!(Bundle::open(&other), Err(Unreadable::NotADeck));
        let no_mimetype = zip::write(&[("deck/deck.json".to_string(), b"{}".to_vec())]);
        assert_eq!(Bundle::open(&no_mimetype), Err(Unreadable::NotADeck));
    }

    #[test]
    fn a_snapshot_that_lies_about_its_lines_is_refused() {
        let packed = tampered("snap/1.txt", b"just one line\n");
        // Changed is caught first; with the manifest agreeing, the lines are.
        assert!(Bundle::open(&packed).is_err());
        let mut lying = bundle();
        lying
            .files
            .insert("snap/1.txt".to_string(), b"just one line\n".to_vec());
        // Packing refuses it, so a lying snapshot never leaves deck.
        assert!(lying.pack().is_err());
        let packed = lying.write().expect("writes");
        assert!(matches!(
            Bundle::open(&packed),
            Err(Unreadable::Malformed(_))
        ));
    }

    #[test]
    fn more_than_a_deck_can_hold_is_refused_when_packing() {
        let mut huge = bundle();
        for n in 1..5000 {
            huge.files.insert(
                format!("deck/g{n}.json"),
                format!(r#"{{"id":"g{n}","say":""}}"#).into_bytes(),
            );
        }
        assert!(huge.pack().is_err());
    }

    #[test]
    fn entries_out_of_order_are_refused() {
        let packed = bundle().pack().expect("packs");
        let mut entries = zip::read(&packed, zip::Limits::default()).expect("reads");
        // Swap the two groups.
        entries.swap(3, 4);
        assert!(matches!(
            Bundle::open(&zip::write(&entries)),
            Err(Unreadable::Malformed(_))
        ));
    }

    #[test]
    fn only_a_decks_own_files_are_packed() {
        for stray in [
            "deck/draft.json",
            "deck/asks-g1.json",
            "deck/listening",
            "deck/g01.json",
        ] {
            let mut bundle = bundle();
            bundle.files.insert(stray.to_string(), Vec::new());
            assert!(bundle.pack().is_err(), "{stray}");
        }
    }
}
