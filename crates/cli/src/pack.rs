//! `deck pack` and `deck unpack`: a deck folder made into a `.deck` file, and
//! back.
//!
//! [`deck_core::bundle`] is the format: bytes in, bytes out. This is the half
//! with a disk and a repo to read. It decides what leaves the machine, and it
//! is the only place that can, because only here are the author's paths, the
//! repo and the files the deck points at all in reach.
//!
//! # What leaves, and what does not
//!
//! A deck folder holds the protocol files — `deck.json`, `g<N>.json` and
//! `done` — and, beside them, everything a live session leaves lying about: a
//! reader's unsent draft, renewed questions, the markers that say who is
//! listening. Only the first three are taken, by name. Nothing is taken
//! because it was not on a list of things to leave, so a file some later
//! version starts writing into the folder stays on the machine until somebody
//! decides it should travel.
//!
//! Each one is read through the protocol's own types, scrubbed, and written
//! out again. The round trip is part of the point: a field this build does not
//! know is dropped rather than carried blind, since nothing here can say what
//! it holds.
//!
//! # What code is read
//!
//! Only code inside the repo — or, with no repo, inside the deck's `cwd`. A
//! ref is a path an agent wrote, and `../secrets/creds`, an absolute path
//! anywhere on the disk, or a link in the repo that points out of it would
//! otherwise each freeze somebody's private file into a document made to be
//! sent away. So every file is resolved first, links followed, and only a
//! resolved path under the resolved root is ever opened. Anything else gets
//! no snapshot, exactly like a file that is not there, and stops the pack
//! unless the author says `--allow-missing`.
//!
//! # Scrubbing
//!
//! A deck is written on one machine and is full of it:
//!
//! - **`cwd`** goes. A `.deck` file is unscoped — any checkout may show it —
//!   which is what an absent `cwd` already means in the protocol. `"."` was
//!   the other choice, and it would have scoped the deck to whatever directory
//!   the reader happened to open it from. The repo it came from is said once,
//!   in the manifest, as the remote and the commit.
//! - **Paths** in refs, comments and moments become repo-relative, with
//!   forward slashes. A file outside the repo keeps its name alone, is not
//!   read, and is named to the author so they can see what was cut.
//! - **Free text** — the narration, titles, notes, pane names, the code in
//!   `before` and `after`, values, diagram labels and ids, pages, and what the
//!   reader wrote — loses the repo's absolute path in every spelling it might
//!   have been written in, and the home directory becomes `~`.
//! - **Questions** offered as pills are dropped. They were for the reader who
//!   was there.
//! - **The remote** loses any credentials, query or fragment, and a remote
//!   that is a path on this machine is not a remote anybody else can reach,
//!   so it goes.
//!
//! # Secrets
//!
//! Every entry is scanned after scrubbing — what would ship, not what was on
//! disk — against a short list of rules for credentials with a recognisable
//! shape ([`scan`]). The protocol files are scanned as the strings they hold,
//! decoded, so a key right after a `\n` in the narration is not hidden by the
//! escape. A hit in a snapshot is reported at its line in the source file.
//!
//! There are no entropy checks: a hash or a UUID is the ordinary content of a
//! code review, and a scanner that cried at every one would be switched off.
//! A hit refuses the pack and names itself by a fingerprint of the secret's
//! value alone, so one secret is one fingerprint wherever it turns up, and
//! one `--allow` with it is how an author says they looked and it is fine.
//! The fingerprints let through are written into the manifest, so the reader
//! can see somebody chose to.
//!
//! # Unpacking
//!
//! `deck unpack` writes a file's entries out as they are, with its manifest.
//! The folder that makes is already a bundle, so packing it again takes its
//! entries as they are — no second scrub, no fresh snapshots — and gives back
//! the file it came from. Only names a `.deck` file holds are taken from it,
//! and never through a link.

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::LazyLock;

use anyhow::Context as _;
use regex::Regex;
use serde::Serialize;
use serde::de::DeserializeOwned;

use deck_core::LineRange;
use deck_core::bundle::{self, Bundle, CONTAINER, Manifest, Repo, snap};
use deck_core::diagram::{Diagram, Step};
use deck_core::protocol::{Group, Header, Ref, Review};

/// How a `.deck` file is to be made.
#[derive(Debug, Clone)]
pub struct Options {
    /// Lines kept either side of each range. [`snap::CONTEXT`] unless asked.
    pub context: u32,
    /// Keep every line of every file pointed at, not just the ranges.
    pub whole_files: bool,
    /// Scanner fingerprints the author has looked at and lets through.
    pub allow: Vec<String>,
    /// Pack even when a file the deck points at cannot be read, or is outside
    /// the repo and so is not read.
    pub allow_missing: bool,
    /// Replace a file already at the output path.
    pub force: bool,
    /// When it was packed, as RFC 3339. `None` asks [`stamp`], which reads
    /// `SOURCE_DATE_EPOCH` and otherwise the clock.
    pub created: Option<String>,
    /// The home directory to write as `~`.
    pub home: Option<PathBuf>,
    /// A directory git is not to look above for a repo, so a deck in a
    /// directory that happens to sit inside some other repo is not taken for
    /// part of it. `None` looks as git always does.
    pub ceiling: Option<PathBuf>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            context: snap::CONTEXT,
            whole_files: false,
            allow: Vec::new(),
            allow_missing: false,
            force: false,
            created: None,
            home: deck_core::home::home(),
            ceiling: None,
        }
    }
}

/// Everything that would go into a `.deck` file, and what was found on the
/// way.
///
/// Nothing has been written when this exists. It is what a dry run prints and
/// what a pack refuses or writes.
#[derive(Debug, Clone)]
pub struct Packing {
    /// What would be packed.
    pub bundle: Bundle,
    /// The file name it would get when nobody names one.
    pub name: String,
    /// Every source file frozen, in the order they were first pointed at.
    pub snapped: Vec<snap::File>,
    /// Files the deck points at outside the repo, as they are on this
    /// machine. Each is in the file by its name alone, and none was read.
    pub outside: Vec<PathBuf>,
    /// Files the deck points at that were not frozen, and why.
    pub missing: Vec<(PathBuf, String)>,
    /// Scanner hits nobody has let through.
    pub hits: Vec<Hit>,
    /// Scanner hits the author let through with `--allow`.
    pub allowed: Vec<Hit>,
    /// `--allow` fingerprints that matched nothing.
    pub unused: Vec<String>,
    /// Files in the folder that stay on this machine.
    pub left: Vec<String>,
    /// Whether this was a folder `deck unpack` made, taken as it is.
    pub repacked: bool,
    allow_missing: bool,
}

/// One thing the secret scanner found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    /// The entry it is in, as `deck/g2.json`.
    pub entry: String,
    /// Where, for a person: `src/batch.ts:101` for a snapshot's line, the
    /// field as `deck/g2.json refs[0].note` in a protocol file.
    pub at: String,
    /// Which rule matched, as `github-pat`.
    pub rule: &'static str,
    /// The first four characters of the secret, and `****`.
    pub redacted: String,
    /// What `--allow` takes to let this one through: the first twelve hex
    /// digits of sha256 over the rule, a NUL and the secret's value.
    pub fingerprint: String,
}

impl std::fmt::Display for Hit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}  {}  {}  {}",
            self.at, self.rule, self.redacted, self.fingerprint
        )
    }
}

/// A `.deck` file that has been written.
#[derive(Debug, Clone)]
pub struct Packed {
    /// Where.
    pub path: PathBuf,
    /// What the author should know about what went in: paths cut to a name,
    /// files left out, secrets let through.
    pub notes: Vec<String>,
}

/// A `.deck` file that has been written out as a folder.
#[derive(Debug, Clone)]
pub struct Unpacked {
    /// The folder.
    pub dir: PathBuf,
    /// Entries a newer deck wrote that this one does not know, and so did not
    /// write out.
    pub ignored: Vec<String>,
}

/// Gather what `deck` would pack, without writing anything.
///
/// `deck` is a deck folder, or a folder `deck unpack` made — told apart by the
/// `manifest.json` only the second has.
///
/// # Errors
///
/// When it is neither, or a protocol file in it does not parse. A file the
/// deck points at that cannot be read, or a secret, is not an error here: it
/// is in what comes back, and [`Packing::refusal`] says whether it stops the
/// pack.
pub fn gather(deck: &Path, options: &Options) -> anyhow::Result<Packing> {
    anyhow::ensure!(deck.is_dir(), "{} is not a directory", deck.display());
    if deck.join("manifest.json").is_file() {
        return regather(deck, options);
    }
    anyhow::ensure!(
        deck.join("deck.json").is_file(),
        "{} is not a deck: no deck.json in it",
        deck.display()
    );

    let (groups, sealed, left) = own_files(deck)?;
    let mut header: Header = read_json(&deck.join("deck.json"))?;
    // Where relative paths in the deck resolve, and where git is asked.
    let base = header.cwd.clone().unwrap_or_else(|| {
        std::fs::canonicalize(deck)
            .ok()
            .and_then(|deck| deck.parent().map(Path::to_path_buf))
            .unwrap_or_else(|| PathBuf::from("."))
    });
    let git = Git::find(&base, options.ceiling.as_deref());
    let place = Place::new(&base, git.as_ref());
    let scrub = Scrub::new(&place, options.home.as_deref());
    let mut points = Points::new(&base, &place);

    let mut files: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    header.cwd = None;
    scrub.text(&mut header.title);
    files.insert("deck/deck.json".to_string(), json(&header)?);

    for (n, name) in groups {
        let path = deck.join(&name);
        let mut group: Group = read_json(&path)?;
        group.asks.clear();
        scrub.group(&mut group);
        for one in &mut group.refs {
            if let Ref::Code(code) = one {
                code.file = points.at(&code.file, Some(code.range));
            }
        }
        files.insert(format!("deck/g{n}.json"), json(&group)?);
    }
    if sealed {
        // The marker says nothing but that it is there.
        files.insert("deck/done".to_string(), Vec::new());
    }

    let review_at = crate::review_path(deck);
    if review_at.is_file() {
        let mut review: Review = read_json(&review_at)?;
        for comment in &mut review.comments {
            scrub.text(&mut comment.text);
            scrub.text(&mut comment.quote);
            if let Some(file) = &comment.file {
                comment.file = Some(points.at(file, comment.range));
            }
        }
        for moment in &mut review.transcript {
            scrub.text(&mut moment.text);
            if let Some(file) = &moment.file {
                moment.file = Some(points.at(file, moment.range));
            }
        }
        files.insert("review.json".to_string(), json(&review)?);
    }

    let commit = git.as_ref().and_then(|git| git.commit.clone());
    let mut snapped = Vec::new();
    let mut missing = Vec::new();
    for pointed in points.files.iter().filter(|p| !p.ranges.is_empty()) {
        // Only ever the resolved path, and only when it is under the root.
        let Some(resolved) = &pointed.readable else {
            let why = if pointed.outside {
                "it is outside the repo, and nothing outside it is read"
            } else {
                "it is not there"
            };
            missing.push((pointed.abs.clone(), why.to_string()));
            continue;
        };
        let source = match std::fs::read(resolved) {
            Ok(bytes) => match String::from_utf8(bytes) {
                Ok(source) => source,
                Err(_) => {
                    missing.push((pointed.abs.clone(), "it is not text".to_string()));
                    continue;
                }
            },
            Err(err) => {
                missing.push((pointed.abs.clone(), err.to_string()));
                continue;
            }
        };
        let (kept, text) = if options.whole_files {
            snap::keep(&source, &[LineRange::new(1, u32::MAX)], 0)
        } else {
            snap::keep(&source, &pointed.ranges, options.context)
        };
        let entry = format!("snap/{}.txt", snapped.len() + 1);
        let state = git
            .as_ref()
            .map_or(snap::State::Untracked, |git| git.state(&pointed.rel));
        files.insert(entry.clone(), text.into_bytes());
        snapped.push(snap::File {
            path: pointed.rel.clone(),
            entry,
            sha256: bundle::sha256(source.as_bytes()),
            commit: commit.clone(),
            state,
            kept,
        });
    }
    if !snapped.is_empty() {
        let index = snap::Index {
            files: snapped.clone(),
        };
        files.insert("snap/index.json".to_string(), json(&index)?);
    }

    let created = match &options.created {
        Some(created) => created.clone(),
        None => stamp(std::env::var("SOURCE_DATE_EPOCH").ok().as_deref())?,
    };
    let repo = git.as_ref().and_then(|git| {
        let repo = Repo {
            remote: git.remote.as_deref().and_then(clean_remote),
            commit: git.commit.clone(),
        };
        (repo.remote.is_some() || repo.commit.is_some()).then_some(repo)
    });
    let manifest = Manifest {
        container: CONTAINER,
        protocol: header.v,
        created,
        generator: format!("deck {}", env!("CARGO_PKG_VERSION")),
        repo,
        entries: BTreeMap::new(),
        allowed: Vec::new(),
    };
    let name = default_name(&header.title, &manifest.created);
    let mut packing = Packing {
        bundle: Bundle { manifest, files },
        name,
        snapped,
        outside: points.outside,
        missing,
        hits: Vec::new(),
        allowed: Vec::new(),
        unused: Vec::new(),
        left,
        repacked: false,
        allow_missing: options.allow_missing,
    };
    packing.screen(&options.allow);
    Ok(packing)
}

/// A folder `deck unpack` made, taken as the bundle it already is.
///
/// Nothing is scrubbed or snapshotted again: it was when it was packed, and
/// doing it twice would give a different file from the one it came from. It
/// is still scanned, since anybody may have edited it since, and a secret
/// put into an unpacked folder is still a secret.
///
/// Only names a `.deck` file holds are taken, and links are never followed:
/// whatever else turns up in the folder — a `.DS_Store`, somebody's notes —
/// is left behind and said, rather than packed or refused.
fn regather(dir: &Path, options: &Options) -> anyhow::Result<Packing> {
    let manifest: Manifest = read_json(&dir.join("manifest.json"))?;
    let mut files = BTreeMap::new();
    let mut left = Vec::new();
    walk(dir, "", &mut files, &mut left)?;
    files.remove("manifest.json");
    let bundle = Bundle { manifest, files };
    let header = bundle
        .header()
        .map_err(|err| anyhow::anyhow!("{}: {err}", dir.display()))?;
    let snapped = bundle
        .snapshots()
        .map_err(|err| anyhow::anyhow!("{}: {err}", dir.display()))?
        .into_iter()
        .map(|snapshot| snapshot.file)
        .collect();
    let name = default_name(&header.title, &bundle.manifest.created);
    let mut packing = Packing {
        bundle,
        name,
        snapped,
        outside: Vec::new(),
        missing: Vec::new(),
        hits: Vec::new(),
        allowed: Vec::new(),
        unused: Vec::new(),
        left,
        repacked: true,
        allow_missing: options.allow_missing,
    };
    packing.screen(&options.allow);
    Ok(packing)
}

/// Every file under `dir` that a `.deck` file holds, by its name inside it
/// with forward slashes; the names of everything else, and of every link,
/// go in `left`.
fn walk(
    dir: &Path,
    prefix: &str,
    files: &mut BTreeMap<String, Vec<u8>>,
    left: &mut Vec<String>,
) -> anyhow::Result<()> {
    let entries =
        std::fs::read_dir(dir).with_context(|| format!("cannot read {}", dir.display()))?;
    let mut entries: Vec<std::fs::DirEntry> = entries.collect::<Result<_, _>>()?;
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = entry.path();
        let named = format!("{prefix}{name}");
        // The entry's own type: a link is a link, wherever it points.
        let kind = entry.file_type()?;
        if kind.is_dir() {
            walk(&path, &format!("{named}/"), files, left)?;
        } else if kind.is_file()
            && (named == "manifest.json" || (bundle::known(&named) && named != "mimetype"))
        {
            let bytes =
                std::fs::read(&path).with_context(|| format!("cannot read {}", path.display()))?;
            files.insert(named, bytes);
        } else {
            left.push(named);
        }
    }
    Ok(())
}

/// The protocol files in a deck folder — its groups by number, and whether it
/// is sealed — and the names of everything else, which stays.
type OwnFiles = (Vec<(u32, String)>, bool, Vec<String>);

fn own_files(deck: &Path) -> anyhow::Result<OwnFiles> {
    let mut groups = Vec::new();
    let mut sealed = false;
    let mut left = Vec::new();
    let entries =
        std::fs::read_dir(deck).with_context(|| format!("cannot read {}", deck.display()))?;
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if !entry.file_type()?.is_file() {
            left.push(format!("{name}/"));
            continue;
        }
        match name.as_str() {
            "deck.json" => {}
            "done" => sealed = true,
            _ => match group_number(&name) {
                Some(n) => groups.push((n, name)),
                None => left.push(name),
            },
        }
    }
    groups.sort();
    left.sort();
    Ok((groups, sealed, left))
}

/// `N` for a file named `g<N>.json`, a number with no leading zero.
fn group_number(name: &str) -> Option<u32> {
    name.strip_prefix('g')
        .and_then(|rest| rest.strip_suffix(".json"))
        .filter(|n| !n.is_empty() && !n.starts_with('0') && n.bytes().all(|b| b.is_ascii_digit()))
        .and_then(|n| n.parse().ok())
}

impl Packing {
    /// Scan every entry, and sort the hits into those let through and those
    /// not.
    fn screen(&mut self, allow: &[String]) {
        let found = scan_bundle(&self.bundle.files);
        let already = &self.bundle.manifest.allowed;
        let (allowed, hits): (Vec<Hit>, Vec<Hit>) = found.into_iter().partition(|hit| {
            allow.contains(&hit.fingerprint) || already.contains(&hit.fingerprint)
        });
        self.unused = allow
            .iter()
            .filter(|fp| !allowed.iter().any(|hit| &&hit.fingerprint == fp))
            .cloned()
            .collect();
        let mut recorded = self.bundle.manifest.allowed.clone();
        recorded.extend(allowed.iter().map(|hit| hit.fingerprint.clone()));
        recorded.sort();
        recorded.dedup();
        self.bundle.manifest.allowed = recorded;
        self.hits = hits;
        self.allowed = allowed;
    }

    /// Why this cannot be packed, if it cannot: a secret nobody let through,
    /// or a file the deck points at that is not there or is outside the repo.
    #[must_use]
    pub fn refusal(&self) -> Option<String> {
        let mut why = Vec::new();
        if !self.missing.is_empty() && !self.allow_missing {
            let mut said = format!(
                "{} the deck points at cannot travel with it:\n",
                files(self.missing.len())
            );
            for (path, err) in &self.missing {
                said.push_str(&format!("\n  {}: {err}", path.display()));
            }
            said.push_str(
                "\n\nPoint at files inside the repo and pack it where they are, or \
                 pass --allow-missing to pack the deck without them: their panes \
                 will open to nothing.",
            );
            why.push(said);
        }
        if !self.hits.is_empty() {
            let mut said = format!(
                "refusing to pack: {} in it {} like {}.\n",
                match self.hits.len() {
                    1 => "one thing".to_string(),
                    n => format!("{n} things"),
                },
                if self.hits.len() == 1 {
                    "looks"
                } else {
                    "look"
                },
                if self.hits.len() == 1 {
                    "a secret"
                } else {
                    "secrets"
                },
            );
            for hit in &self.hits {
                said.push_str(&format!("\n  {hit}"));
            }
            let mut flags: Vec<String> = self
                .hits
                .iter()
                .map(|hit| format!("--allow {}", hit.fingerprint))
                .collect();
            flags.dedup();
            said.push_str(&format!(
                "\n\nTake it out of the deck and pack again. Or, for each one you \
                 have looked at and know is safe, pass its fingerprint — {} — and \
                 the manifest will say you did.",
                flags.join(" ")
            ));
            why.push(said);
        }
        (!why.is_empty()).then(|| why.join("\n\n"))
    }

    /// What the author should know about a pack that went ahead.
    #[must_use]
    pub fn notes(&self) -> Vec<String> {
        let mut notes = Vec::new();
        for path in &self.outside {
            notes.push(format!(
                "{} is outside the repo, so it is in the deck by its name alone \
                 and its code is not",
                path.display()
            ));
        }
        for (path, err) in &self.missing {
            notes.push(format!("{} was left out: {err}", path.display()));
        }
        for hit in &self.allowed {
            notes.push(format!("let through, as asked: {hit}"));
        }
        for fingerprint in &self.unused {
            notes.push(format!("--allow {fingerprint} matched nothing"));
        }
        notes
    }

    /// Everything a pack would write and why, as a listing for a person.
    ///
    /// # Errors
    ///
    /// When the bundle would not pack at all.
    pub fn dry_run(&self) -> anyhow::Result<String> {
        let bytes = self.bundle.pack().map_err(anyhow::Error::msg)?;
        let entries = bundle::zip::read(&bytes, bundle::zip::Limits::default())
            .map_err(|err| anyhow::anyhow!("{err}"))?;
        let mut out = format!(
            "{} would be {} bytes{}:\n",
            self.name,
            bytes.len(),
            if self.repacked {
                ", taken as it was unpacked"
            } else {
                ""
            }
        );
        let wide = entries
            .iter()
            .map(|(name, _)| name.len())
            .max()
            .unwrap_or(0);
        for (name, content) in &entries {
            out.push_str(&format!("\n  {name:<wide$}  {:>8} B", content.len()));
        }
        if !self.snapped.is_empty() {
            out.push_str("\n\nCode frozen into it:\n");
            for file in &self.snapped {
                let spans: Vec<String> = file
                    .kept
                    .iter()
                    .map(|span| format!("{}-{}", span.first, span.last))
                    .collect();
                let state = match file.state {
                    snap::State::Clean => "clean",
                    snap::State::Modified => "modified",
                    snap::State::Untracked => "untracked",
                };
                out.push_str(&format!(
                    "\n  {}  lines {}  {state}",
                    file.path,
                    if spans.is_empty() {
                        "none".to_string()
                    } else {
                        spans.join(", ")
                    }
                ));
            }
        }
        if !self.outside.is_empty() {
            out.push_str("\n\nOutside the repo, so in it by name alone and never read:\n");
            for path in &self.outside {
                out.push_str(&format!("\n  {}", path.display()));
            }
        }
        if !self.left.is_empty() {
            out.push_str(&format!(
                "\n\nStaying on this machine: {}",
                self.left.join(", ")
            ));
        }
        if !self.allowed.is_empty() {
            out.push_str("\n\nLet through, as asked:\n");
            for hit in &self.allowed {
                out.push_str(&format!("\n  {hit}"));
            }
        }
        match self.refusal() {
            Some(why) => out.push_str(&format!("\n\nIt would not be packed: {why}")),
            None => out.push_str("\n\nNothing in it looks like a secret."),
        }
        Ok(out)
    }
}

/// Make `deck` into a `.deck` file at `out`, or under its own name in the
/// current directory.
///
/// A directory for `out` means the file goes in it under its own name.
///
/// # Errors
///
/// When it cannot be gathered, [`Packing::refusal`] has something to say, a
/// file is already there and `force` was not asked for, or it cannot be
/// written.
pub fn pack(deck: &Path, out: Option<&Path>, options: &Options) -> anyhow::Result<Packed> {
    let packing = gather(deck, options)?;
    if let Some(why) = packing.refusal() {
        anyhow::bail!(why);
    }
    let bytes = packing.bundle.pack().map_err(anyhow::Error::msg)?;
    let path = match out {
        Some(out) if out.is_dir() => out.join(&packing.name),
        Some(out) => out.to_path_buf(),
        None => PathBuf::from(&packing.name),
    };
    write_new(&path, &bytes, options.force)?;
    Ok(Packed {
        path,
        notes: packing.notes(),
    })
}

/// Write `bytes` to `path`, refusing to replace a file unless `force`.
fn write_new(path: &Path, bytes: &[u8], force: bool) -> anyhow::Result<()> {
    use std::io::Write as _;
    if force {
        return std::fs::write(path, bytes)
            .with_context(|| format!("cannot write {}", path.display()));
    }
    let mut file = std::fs::File::options()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|err| {
            if err.kind() == std::io::ErrorKind::AlreadyExists {
                anyhow::anyhow!(
                    "{} is already there: pass --force to replace it, or -o to \
                     write somewhere else",
                    path.display()
                )
            } else {
                anyhow::anyhow!("cannot write {}: {err}", path.display())
            }
        })?;
    file.write_all(bytes)
        .with_context(|| format!("cannot write {}", path.display()))
}

/// Write the `.deck` file at `file` out as a folder: every entry as it is,
/// and its manifest.
///
/// `into` defaults to the file's name without `.deck`, with `.unpacked`, in
/// the current directory. The file is opened with every check
/// [`Bundle::open`] makes before anything is written.
///
/// # Errors
///
/// When the file cannot be read or opened, or `into` is not an empty
/// directory or a path that does not exist yet.
pub fn unpack(file: &Path, into: Option<&Path>) -> anyhow::Result<Unpacked> {
    let bytes = std::fs::read(file).with_context(|| format!("cannot read {}", file.display()))?;
    let (bundle, ignored) = Bundle::open(&bytes)
        .map_err(|err| anyhow::anyhow!("{} cannot be opened: {err}", file.display()))?;
    // The manifest as it was stored, byte for byte, so the folder packs back
    // to the same file.
    let manifest = bundle::zip::read(&bytes, bundle::zip::Limits::default())
        .map_err(|err| anyhow::anyhow!("{err}"))?
        .into_iter()
        .find_map(|(name, content)| (name == "manifest.json").then_some(content))
        .ok_or_else(|| anyhow::anyhow!("{} has no manifest.json", file.display()))?;
    let dir = into.map_or_else(|| unpacked_name(file), Path::to_path_buf);
    if dir.exists() {
        let empty = std::fs::read_dir(&dir)
            .with_context(|| format!("{} is there and is not a directory", dir.display()))?
            .next()
            .is_none();
        anyhow::ensure!(
            empty,
            "{} is not empty: unpack into a new directory, so nothing in it is mixed up \
             with the deck",
            dir.display()
        );
    }
    std::fs::create_dir_all(&dir).with_context(|| format!("cannot make {}", dir.display()))?;
    let write = |name: &str, content: &[u8]| -> anyhow::Result<()> {
        // Every name here is one `bundle::known` accepted: no `..`, no root.
        let path = name
            .split('/')
            .fold(dir.clone(), |path, part| path.join(part));
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("cannot make {}", parent.display()))?;
        }
        std::fs::write(&path, content).with_context(|| format!("cannot write {}", path.display()))
    };
    write("manifest.json", &manifest)?;
    for (name, content) in &bundle.files {
        write(name, content)?;
    }
    Ok(Unpacked { dir, ignored })
}

/// Where `deck unpack file.deck` puts it: `file.unpacked`.
#[must_use]
pub fn unpacked_name(file: &Path) -> PathBuf {
    let stem = file.file_stem().map_or_else(
        || "deck".to_string(),
        |stem| stem.to_string_lossy().into_owned(),
    );
    PathBuf::from(format!("{stem}.unpacked"))
}

/// The name a `.deck` file gets: the day it was packed and its title, as
/// `2026-10-20-why-the-batch-stalls.deck`.
#[must_use]
pub fn default_name(title: &str, created: &str) -> String {
    let day = created.get(..10).unwrap_or("undated");
    format!("{day}-{}.deck", slug(title))
}

/// A title as a file name: lowercase letters and digits, a dash for anything
/// else, never more than sixty characters.
fn slug(title: &str) -> String {
    let mut slug = String::new();
    for ch in title.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
        } else if !slug.is_empty() && !slug.ends_with('-') {
            slug.push('-');
        }
    }
    slug.truncate(60);
    let slug = slug.trim_end_matches('-');
    if slug.is_empty() {
        "deck".to_string()
    } else {
        slug.to_string()
    }
}

/// When a pack happens, as RFC 3339 in UTC to the second: `epoch` — the
/// value of `SOURCE_DATE_EPOCH` — when there is one, so a build can make the
/// same file twice, and otherwise now.
///
/// # Errors
///
/// When `epoch` is set but is not a whole number of seconds. Reproducible
/// builds ask for that to be an error rather than quietly the clock, which
/// would give a different file every time and nobody would know why.
pub fn stamp(epoch: Option<&str>) -> anyhow::Result<String> {
    let seconds = match epoch {
        Some(epoch) => epoch.trim().parse::<u64>().map_err(|_| {
            anyhow::anyhow!("SOURCE_DATE_EPOCH is {epoch:?}, which is not a number of seconds")
        })?,
        None => std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |since| since.as_secs()),
    };
    Ok(rfc3339(seconds))
}

/// Seconds since 1970 as `YYYY-MM-DDTHH:MM:SSZ`.
///
/// The civil date by Howard Hinnant's `civil_from_days`, which is exact for
/// every day there is, rather than a dependency for one line of output.
fn rfc3339(seconds: u64) -> String {
    let days = i64::try_from(seconds / 86_400).unwrap_or(i64::MAX / 2);
    let of_day = seconds % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        of_day / 3600,
        of_day / 60 % 60,
        of_day % 60
    )
}

/// A remote another machine can use, or `None` for one it cannot.
///
/// Credentials are taken out — `https://user:token@host/…` and
/// `user:token@host:owner/repo` alike — and so are a URL's query and
/// fragment, where a token can also ride. `git@` is kept: it is the user
/// every forge's ssh remote names, and not a secret. A remote that is a path
/// on this machine names nothing anywhere else, so it is dropped.
#[must_use]
pub fn clean_remote(remote: &str) -> Option<String> {
    let remote = remote.trim();
    if remote.is_empty() {
        return None;
    }
    let user = |user: &str, rest: &str| {
        if user == "git" {
            format!("git@{rest}")
        } else {
            rest.to_string()
        }
    };
    if let Some((scheme, rest)) = remote.split_once("://") {
        if scheme.eq_ignore_ascii_case("file") {
            return None;
        }
        let rest = rest.split(['?', '#']).next().unwrap_or_default();
        let (authority, path) = rest.split_at(rest.find('/').unwrap_or(rest.len()));
        let host = match authority.rsplit_once('@') {
            Some((who, host)) => user(who, host),
            None => authority.to_string(),
        };
        return Some(format!("{scheme}://{host}{path}"));
    }
    if remote.starts_with(['.', '/', '\\', '~']) {
        return None;
    }
    // `[user@]host:path`, where the host is more than a drive letter and the
    // colon comes before any slash. The user is whatever is before the last
    // `@` ahead of the first slash, so a `:` in it — a token — goes with it.
    let head = &remote[..remote.find('/').unwrap_or(remote.len())];
    let (who, rest) = match head.rfind('@') {
        Some(at) => (Some(&remote[..at]), &remote[at + 1..]),
        None => (None, remote),
    };
    let colon = rest.find(':')?;
    if colon <= 1 || rest[..colon].contains(['/', '\\']) {
        return None;
    }
    Some(who.map_or_else(|| rest.to_string(), |who| user(who, rest)))
}

/// What git says about the repo a deck was written in.
struct Git {
    /// The top of the working tree, as git printed it.
    root: String,
    /// `HEAD`, when there is one.
    commit: Option<String>,
    /// `origin`, as configured, credentials and all.
    remote: Option<String>,
    /// See [`Options::ceiling`].
    ceiling: Option<PathBuf>,
}

impl Git {
    /// The repo `dir` is in, or `None` when there is no git or no repo.
    fn find(dir: &Path, ceiling: Option<&Path>) -> Option<Self> {
        let root =
            git(dir, ceiling, &["rev-parse", "--show-toplevel"]).filter(|root| !root.is_empty())?;
        let at = Path::new(&root);
        let commit = git(at, ceiling, &["rev-parse", "--verify", "--quiet", "HEAD"])
            .filter(|commit| !commit.is_empty());
        let remote =
            git(at, ceiling, &["remote", "get-url", "origin"]).filter(|url| !url.is_empty());
        Some(Self {
            root,
            commit,
            remote,
            ceiling: ceiling.map(Path::to_path_buf),
        })
    }

    /// How the file at `path`, relative to the root, stands against `HEAD`.
    ///
    /// The path is taken literally — `src/[id].tsx` is that file, not a
    /// pattern matching `src/i.tsx` — and only a line naming it counts.
    /// Ignored files count as untracked: a reader at that commit does not
    /// have them either.
    fn state(&self, path: &str) -> snap::State {
        let Some(out) = git(
            Path::new(&self.root),
            self.ceiling.as_deref(),
            &[
                "status",
                "--porcelain",
                "-z",
                "--ignored",
                "--untracked-files=all",
                "--",
                path,
            ],
        ) else {
            return snap::State::Untracked;
        };
        for line in out.split('\0') {
            let (Some(code), Some(named)) = (line.get(..2), line.get(3..)) else {
                continue;
            };
            if named == path || (named.ends_with('/') && path.starts_with(named)) {
                return if code == "??" || code == "!!" {
                    snap::State::Untracked
                } else {
                    snap::State::Modified
                };
            }
        }
        if self.commit.is_some() {
            snap::State::Clean
        } else {
            snap::State::Untracked
        }
    }
}

/// What `git -C dir <args>` printed, or `None` when it could not say.
///
/// Pathspecs are literal, and the variables that would point git at some
/// other repo than the one `dir` is in are not passed on.
fn git(dir: &Path, ceiling: Option<&Path>, args: &[&str]) -> Option<String> {
    let mut command = Command::new("git");
    command
        .arg("--literal-pathspecs")
        .arg("-C")
        .arg(dir)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    for variable in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_COMMON_DIR",
        "GIT_OBJECT_DIRECTORY",
        "GIT_NAMESPACE",
        "GIT_PREFIX",
    ] {
        command.env_remove(variable);
    }
    if let Some(ceiling) = ceiling {
        // Without the `\\?\` a resolved Windows path carries, which git
        // does not read as a path.
        command.env("GIT_CEILING_DIRECTORIES", shown(ceiling));
    }
    let out = command.output().ok()?;
    out.status.success().then(|| {
        String::from_utf8_lossy(&out.stdout)
            .trim_end_matches(['\r', '\n'])
            .to_string()
    })
}

/// The directory code may be read from and paths are made relative to — the
/// repo, or the deck's `cwd` when there is no repo — in every spelling it
/// might be written in.
struct Place {
    /// Resolved, links followed. The one thing inside and outside are
    /// decided against.
    root: PathBuf,
    /// As text, longest first, for scrubbing prose: as git printed it,
    /// resolved, and as the deck spells it. A deck written on macOS says
    /// `/tmp`, which is `/private/tmp`, and either may turn up in its prose.
    spellings: Vec<String>,
}

/// Where a file the deck points at is, as far as packing goes.
enum Located {
    /// Under the root once resolved: its path there, and the resolved path,
    /// which is the only one ever read.
    Inside { rel: String, resolved: PathBuf },
    /// Not there to resolve, so never read; this is where it would be.
    Absent(String),
    /// Outside the root, or resolving to somewhere that is.
    Outside,
}

impl Place {
    fn new(base: &Path, git: Option<&Git>) -> Self {
        let given = git.map_or_else(|| base.to_path_buf(), |git| PathBuf::from(&git.root));
        let root = std::fs::canonicalize(&given).unwrap_or_else(|_| given.clone());
        let mut spellings = vec![shown(&given), shown(&root)];
        // The root as the deck spells it: the cwd as written, less however
        // far below the root it resolves to. A cwd of `/tmp/repo/sub` is
        // `/private/tmp/repo/sub`, one below `/private/tmp/repo`, so the deck
        // calls the root `/tmp/repo`.
        let plain = base
            .components()
            .all(|part| !matches!(part, Component::CurDir | Component::ParentDir));
        if plain
            && let Ok(resolved) = std::fs::canonicalize(base)
            && let Ok(below) = resolved.strip_prefix(&root)
        {
            let mut spelled = base.to_path_buf();
            if below.components().all(|_| spelled.pop()) {
                spellings.push(shown(&spelled));
            }
        }
        if let Some(git) = git {
            spellings.push(git.root.clone());
        }
        Self {
            root,
            spellings: variants(spellings),
        }
    }

    /// Where `path` is: under the root once every link in it is followed, or
    /// not.
    ///
    /// The resolved path alone decides. A path that is spelled under the
    /// root but resolves out of it — a link in the repo to `../priv` — is
    /// outside, and is never read.
    fn locate(&self, path: &Path) -> Located {
        if let Ok(resolved) = std::fs::canonicalize(path) {
            return match resolved.strip_prefix(&self.root).ok().and_then(parts) {
                Some(rel) => Located::Inside { rel, resolved },
                None => Located::Outside,
            };
        }
        // Not there, so nothing is read and only its name is at stake.
        let lexical = lexical(path);
        let named = path
            .parent()
            .and_then(|parent| std::fs::canonicalize(parent).ok())
            .zip(path.file_name())
            .map(|(parent, name)| parent.join(name));
        let rel = named
            .as_deref()
            .and_then(|named| named.strip_prefix(&self.root).ok())
            .and_then(parts)
            .or_else(|| {
                self.spellings
                    .iter()
                    .find_map(|spelling| lexical.strip_prefix(spelling).ok())
                    .and_then(parts)
            });
        rel.map_or(Located::Outside, Located::Absent)
    }
}

/// `under` as a repo path with forward slashes, or `None` when it is empty
/// or has anything in it but names.
fn parts(under: &Path) -> Option<String> {
    let parts: Option<Vec<String>> = under
        .components()
        .map(|part| match part {
            Component::Normal(part) => Some(part.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect();
    parts
        .filter(|parts| !parts.is_empty())
        .map(|parts| parts.join("/"))
}

/// `path` with `.` and `..` worked out on the text alone.
fn lexical(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    out
}

/// A path as text, without the `\\?\` Windows puts on a resolved one.
fn shown(path: &Path) -> String {
    let text = path.to_string_lossy();
    text.strip_prefix(r"\\?\").unwrap_or(&text).to_string()
}

/// Every way the paths in `spellings` might be written, longest first.
///
/// Trailing separators are trimmed. A Windows path is also written with its
/// slashes the other way, and in lower case — `C:\Users\ada`, `C:/Users/ada`,
/// `c:\users\ada` — since Windows does not care and neither do the people
/// pasting them. A spelling of three characters or fewer is dropped: a root
/// of `/` or `C:\` would strip the start of every path there is.
fn variants(spellings: Vec<String>) -> Vec<String> {
    let mut out = Vec::new();
    for spelling in spellings {
        let spelling = spelling.trim_end_matches(['/', '\\']).to_string();
        let windows = spelling.contains('\\')
            || (spelling.as_bytes().get(1) == Some(&b':')
                && spelling.as_bytes()[0].is_ascii_alphabetic());
        if windows {
            for one in [spelling.replace('/', "\\"), spelling.replace('\\', "/")] {
                let mut drive = one.clone();
                if let Some(first) = drive.get_mut(..1) {
                    first.make_ascii_lowercase();
                }
                out.push(one.to_lowercase());
                out.push(drive);
                out.push(one);
            }
        }
        out.push(spelling);
    }
    out.retain(|spelling| spelling.len() > 3);
    out.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
    out.dedup();
    out
}

/// Every source file the deck points at, in the order it first did.
struct Points<'a> {
    base: PathBuf,
    place: &'a Place,
    files: Vec<Pointed>,
    outside: Vec<PathBuf>,
}

/// One source file the deck points at.
struct Pointed {
    /// Where the deck said it is, made absolute.
    abs: PathBuf,
    /// What the deck will call it.
    rel: String,
    /// The resolved path to read, only for a file under the root.
    readable: Option<PathBuf>,
    /// Whether it is outside the root.
    outside: bool,
    /// Every range pointed at in it.
    ranges: Vec<LineRange>,
}

impl<'a> Points<'a> {
    fn new(base: &Path, place: &'a Place) -> Self {
        Self {
            base: base.to_path_buf(),
            place,
            files: Vec::new(),
            outside: Vec::new(),
        }
    }

    /// What the deck will call `file`, noting `range` in it to freeze.
    fn at(&mut self, file: &Path, range: Option<LineRange>) -> PathBuf {
        let abs = if file.is_absolute() {
            file.to_path_buf()
        } else {
            self.base.join(file)
        };
        let at = match self.files.iter().position(|pointed| pointed.abs == abs) {
            Some(at) => at,
            None => {
                let (rel, readable, outside) = match self.place.locate(&abs) {
                    Located::Inside { rel, resolved } => (rel, Some(resolved), false),
                    Located::Absent(rel) => (rel, None, false),
                    Located::Outside => {
                        self.outside.push(abs.clone());
                        let name = abs
                            .file_name()
                            .map_or_else(|| "file".to_string(), |n| n.to_string_lossy().into());
                        (name, None, true)
                    }
                };
                self.files.push(Pointed {
                    abs,
                    rel,
                    readable,
                    outside,
                    ranges: Vec::new(),
                });
                self.files.len() - 1
            }
        };
        if let Some(range) = range {
            self.files[at].ranges.push(range);
        }
        PathBuf::from(&self.files[at].rel)
    }
}

/// What comes out of free text: this machine's paths.
struct Scrub {
    /// The root's spellings, longest first. Gone, with the separator after.
    roots: Vec<String>,
    /// The home directory's spellings, longest first. Written as `~`.
    homes: Vec<String>,
}

impl Scrub {
    fn new(place: &Place, home: Option<&Path>) -> Self {
        let homes = home.map_or_else(Vec::new, |home| {
            let mut spellings = vec![shown(home)];
            if let Ok(resolved) = std::fs::canonicalize(home) {
                spellings.push(shown(&resolved));
            }
            variants(spellings)
        });
        Self {
            roots: place.spellings.clone(),
            homes,
        }
    }

    fn text(&self, text: &mut String) {
        for root in &self.roots {
            *text = swap(text, root, "", ".");
        }
        for home in &self.homes {
            *text = swap(text, home, "~/", "~");
        }
    }

    fn maybe(&self, text: &mut Option<String>) {
        if let Some(text) = text {
            self.text(text);
        }
    }

    fn group(&self, group: &mut Group) {
        self.text(&mut group.say);
        self.maybe(&mut group.title);
        if let Some(confidence) = &mut group.confidence {
            self.maybe(&mut confidence.why);
        }
        for one in &mut group.refs {
            match one {
                Ref::Code(code) => {
                    self.maybe(&mut code.name);
                    self.maybe(&mut code.note);
                    self.maybe(&mut code.before);
                    self.maybe(&mut code.after);
                    for value in &mut code.values {
                        self.text(&mut value.at);
                        self.text(&mut value.is);
                        for key in &mut value.keys {
                            self.maybe(&mut key.key);
                            self.maybe(&mut key.is);
                            self.maybe(&mut key.why);
                        }
                    }
                }
                Ref::Diagram(drawn) => {
                    self.maybe(&mut drawn.name);
                    self.maybe(&mut drawn.note);
                    self.diagram(&mut drawn.diagram);
                }
                Ref::Page(page) => {
                    self.maybe(&mut page.name);
                    self.maybe(&mut page.note);
                    self.text(&mut page.page);
                }
            }
        }
    }

    /// Every string in a picture. Node ids are scrubbed with the edges,
    /// clusters and steps that name them, by the same rule, so a picture
    /// still joins up afterwards.
    fn diagram(&self, diagram: &mut Diagram) {
        self.maybe(&mut diagram.title);
        for node in &mut diagram.nodes {
            self.text(&mut node.id);
            self.text(&mut node.label);
            self.maybe(&mut node.note);
        }
        for edge in &mut diagram.edges {
            self.text(&mut edge.from);
            self.text(&mut edge.to);
            self.maybe(&mut edge.label);
        }
        for cluster in &mut diagram.clusters {
            self.text(&mut cluster.label);
            for node in &mut cluster.nodes {
                self.text(node);
            }
        }
        for flow in &mut diagram.flows {
            self.text(&mut flow.name);
            for step in &mut flow.steps {
                match step {
                    Step::Node(node) | Step::Lit { node, .. } => self.text(node),
                }
            }
        }
    }
}

/// `text` with every `path` that stands as a path of its own replaced:
/// followed by a separator, the path and the separator become `joined`;
/// standing alone, the path becomes `alone`.
///
/// "Of its own" because `/home/ada` is not in `/home/adam`, nor in
/// `/x/home/ada`.
fn swap(text: &str, path: &str, joined: &str, alone: &str) -> String {
    let word = |ch: char| ch.is_alphanumeric() || ch == '_' || ch == '-';
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find(path) {
        let before = rest[..at].chars().next_back();
        let after = &rest[at + path.len()..];
        let mut next = after.chars();
        let starts = before.is_none_or(|ch| !word(ch) && ch != '.');
        match (starts, next.next()) {
            (true, Some('/' | '\\')) => {
                out.push_str(&rest[..at]);
                out.push_str(joined);
                rest = &after[1..];
            }
            (true, None) => {
                out.push_str(&rest[..at]);
                out.push_str(alone);
                rest = after;
            }
            // A full stop ends a sentence unless a name goes on after it.
            (true, Some(ch)) if !word(ch) && !(ch == '.' && next.next().is_some_and(word)) => {
                out.push_str(&rest[..at]);
                out.push_str(alone);
                rest = after;
            }
            _ => {
                out.push_str(&rest[..at + path.len()]);
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// One shape a credential has.
struct Rule {
    id: &'static str,
    /// The whole shape. Its first group, when it has one, is the secret
    /// itself, which is what is fingerprinted and redacted.
    pattern: Regex,
}

/// The shapes the scanner knows, most particular first.
///
/// Curated from gitleaks' rules: each is a prefix a provider puts on its keys
/// so that scanners like this one can find them, or a block that is nothing
/// but a key. The distinctive prefixes need nothing in front of them, so a
/// key glued to a word or an escape is still found; `sk-`, AWS ids and JWTs
/// are too short to stand alone and must start a word. Where two overlap on a
/// line — `sk-ant-` is also `sk-` — the first one listed is the one reported.
static RULES: LazyLock<Vec<Rule>> = LazyLock::new(|| {
    [
        (
            "aws-access-key-id",
            r"\b((?:A3T[A-Z0-9]|AKIA|ASIA|ABIA|ACCA)[A-Z0-9]{16})\b",
        ),
        ("github-pat", r"ghp_[A-Za-z0-9]{36}\b"),
        ("github-oauth", r"gho_[A-Za-z0-9]{36}\b"),
        ("github-app-token", r"(?:ghu|ghs)_[A-Za-z0-9]{36}\b"),
        ("github-refresh-token", r"ghr_[A-Za-z0-9]{36}\b"),
        ("github-fine-grained-pat", r"github_pat_[A-Za-z0-9_]{82}\b"),
        ("gitlab-pat", r"glpat-[A-Za-z0-9_-]{20,}"),
        ("slack-token", r"xox[baprs]-[A-Za-z0-9-]{10,}"),
        (
            "slack-webhook",
            r"hooks\.slack\.com/(?:services|workflows|triggers)/[A-Za-z0-9+/]{43,56}",
        ),
        ("stripe-key", r"(?:sk|rk)_live_[A-Za-z0-9]{24,}"),
        ("google-api-key", r"AIza[0-9A-Za-z_-]{35}"),
        ("anthropic-key", r"sk-ant-[A-Za-z0-9_-]{32,}"),
        (
            "openai-key",
            r"\bsk-(?:proj-|svcacct-|admin-)?[A-Za-z0-9_-]{32,}",
        ),
        (
            "private-key",
            r"-----BEGIN (?:[A-Z0-9]+ )*PRIVATE KEY(?: BLOCK)?-----",
        ),
        (
            "jwt",
            r"\beyJ[A-Za-z0-9_-]{8,}\.eyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}",
        ),
        // Any name with one of these words in it — `client_secret`,
        // `AWS_SECRET_ACCESS_KEY`, `DB_PASSWORD` — given a quoted value of
        // eight characters or more with no space in it. A value opening with
        // `$`, `<` or `{` is a placeholder, not a secret.
        (
            "generic-secret",
            r#"(?i)(?:password|passwd|secret|api_?key|token)[a-z0-9_]*["']?\s*[:=]\s*["']([^"'\s$<{][^"'\s]{7,})["']"#,
        ),
    ]
    .into_iter()
    .map(|(id, pattern)| Rule {
        id,
        pattern: Regex::new(pattern).expect("every rule is a valid pattern"),
    })
    .collect()
});

/// The names of the scanner's rules, in the order they are tried.
pub fn rules() -> impl Iterator<Item = &'static str> {
    RULES.iter().map(|rule| rule.id)
}

/// What a rule found on one line of some text.
struct Found {
    /// The line, counting from one.
    line: usize,
    rule: &'static str,
    /// The secret itself.
    value: String,
}

/// Everything in `text` that looks like a secret, line by line. One match
/// per place: where rules overlap, the first rule's.
fn find(text: &str) -> Vec<Found> {
    let mut found = Vec::new();
    for (at, line) in text.lines().enumerate() {
        let mut taken: Vec<std::ops::Range<usize>> = Vec::new();
        for rule in RULES.iter() {
            for caught in rule.pattern.captures_iter(line) {
                let Some(whole) = caught.get(0) else {
                    continue;
                };
                let span = whole.range();
                if taken
                    .iter()
                    .any(|seen| seen.start < span.end && span.start < seen.end)
                {
                    continue;
                }
                taken.push(span);
                found.push(Found {
                    line: at + 1,
                    rule: rule.id,
                    value: caught.get(1).unwrap_or(whole).as_str().to_string(),
                });
            }
        }
    }
    found
}

fn hit(entry: &str, at: String, found: &Found) -> Hit {
    Hit {
        entry: entry.to_string(),
        at,
        rule: found.rule,
        redacted: format!("{}****", found.value.chars().take(4).collect::<String>()),
        fingerprint: fingerprint(found.rule, &found.value),
    }
}

/// Everything in `bytes`, the entry called `entry`, that looks like a secret.
///
/// A JSON entry is scanned as the strings it holds, decoded, and a hit names
/// the field it is in. Anything else is scanned as text, and a hit names its
/// line.
#[must_use]
pub fn scan(entry: &str, bytes: &[u8]) -> Vec<Hit> {
    if entry.ends_with(".json")
        && let Ok(value) = serde_json::from_slice::<serde_json::Value>(bytes)
    {
        let mut hits = Vec::new();
        strings(&value, "", &mut |path, text| {
            let several = text.lines().nth(1).is_some();
            for found in find(text) {
                let mut at = if path.is_empty() {
                    entry.to_string()
                } else {
                    format!("{entry} {path}")
                };
                if several {
                    at.push_str(&format!(" line {}", found.line));
                }
                hits.push(hit(entry, at, &found));
            }
        });
        return hits;
    }
    scan_lines(entry, bytes, |line| format!("{entry}:{line}"))
}

/// `bytes` scanned as text, each hit placed by `at` from its line.
fn scan_lines(entry: &str, bytes: &[u8], at: impl Fn(usize) -> String) -> Vec<Hit> {
    find(&String::from_utf8_lossy(bytes))
        .iter()
        .map(|found| hit(entry, at(found.line), found))
        .collect()
}

/// Every string in `value`, with where it is: `refs[0].note`.
fn strings(value: &serde_json::Value, path: &str, each: &mut dyn FnMut(&str, &str)) {
    match value {
        serde_json::Value::String(text) => each(path, text),
        serde_json::Value::Array(items) => {
            for (n, item) in items.iter().enumerate() {
                strings(item, &format!("{path}[{n}]"), each);
            }
        }
        serde_json::Value::Object(fields) => {
            for (name, field) in fields {
                let path = if path.is_empty() {
                    name.clone()
                } else {
                    format!("{path}.{name}")
                };
                strings(field, &path, each);
            }
        }
        _ => {}
    }
}

/// Every entry of a bundle scanned, a snapshot's hits placed at their line
/// in the source file rather than in the entry.
fn scan_bundle(files: &BTreeMap<String, Vec<u8>>) -> Vec<Hit> {
    let index: Option<snap::Index> = files
        .get("snap/index.json")
        .and_then(|bytes| serde_json::from_slice(bytes).ok());
    let mut hits = Vec::new();
    for (entry, bytes) in files {
        let frozen = index
            .as_ref()
            .and_then(|index| index.files.iter().find(|file| &file.entry == entry));
        hits.extend(match frozen {
            Some(file) => scan_lines(entry, bytes, |line| {
                source_line(&file.kept, line).map_or_else(
                    || format!("{entry}:{line}"),
                    |source| format!("{}:{source}", file.path),
                )
            }),
            None => scan(entry, bytes),
        });
    }
    hits
}

/// Line `n` of a snapshot's text, counting from one, as a line of the source
/// file it froze.
fn source_line(kept: &[LineRange], n: usize) -> Option<u32> {
    let n = u64::try_from(n).ok()?;
    let mut before = 0u64;
    for span in kept {
        let len = u64::from(span.len());
        if n <= before + len {
            return u32::try_from(u64::from(span.first) + (n - before) - 1).ok();
        }
        before += len;
    }
    None
}

/// What `--allow` takes for a secret: twelve hex digits of
/// `sha256(rule + "\0" + value)`. The value alone, without the quotes or the
/// name it was given, so the same secret under the same rule is the same
/// fingerprint wherever it turns up — in the narration and in the code — and
/// the secret is not in it.
fn fingerprint(rule: &str, value: &str) -> String {
    let mut hex = bundle::sha256(format!("{rule}\0{value}").as_bytes());
    hex.truncate(12);
    hex
}

/// `value` as a protocol file: pretty, with a newline at the end, as deck
/// writes them.
fn json<T: Serialize>(value: &T) -> anyhow::Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn read_json<T: DeserializeOwned>(path: &Path) -> anyhow::Result<T> {
    let bytes = std::fs::read(path).with_context(|| format!("cannot read {}", path.display()))?;
    serde_json::from_slice(&bytes).with_context(|| format!("{} does not parse", path.display()))
}

/// Named for what a count of files reads as.
fn files(n: usize) -> String {
    match n {
        1 => "a file".to_string(),
        n => format!("{n} files"),
    }
}

#[cfg(test)]
mod tests;
