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
//!   forward slashes. A file outside the repo keeps its name alone, and is
//!   named to the author so they can see what was cut.
//! - **Free text** — the narration, titles, notes, the code in `before` and
//!   `after`, values, diagram labels, pages, and what the reader wrote — loses
//!   the repo's absolute path, and the home directory becomes `~`.
//! - **Questions** offered as pills are dropped. They were for the reader who
//!   was there.
//! - **The remote** loses any credentials in its URL, and a remote that is a
//!   path on this machine is not a remote anybody else can reach, so it goes.
//!
//! # Secrets
//!
//! Every entry is scanned after scrubbing — what would ship, not what was on
//! disk — against a short list of rules for credentials with a recognisable
//! shape ([`scan`]). There are no entropy checks: a hash or a UUID is the
//! ordinary content of a code review, and a scanner that cried at every one
//! would be switched off. A hit refuses the pack and names itself by a
//! fingerprint, and `--allow` with that fingerprint is how an author says they
//! looked and it is fine. The fingerprints let through are written into the
//! manifest, so the reader can see somebody chose to.
//!
//! # Unpacking
//!
//! `deck unpack` writes a file's entries out as they are, with its manifest.
//! The folder that makes is already a bundle, so packing it again takes every
//! byte as it is — no second scrub, no fresh snapshots — and gives back the
//! file it came from.

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
use deck_core::diagram::Diagram;
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
    /// Pack even when a file the deck points at cannot be read.
    pub allow_missing: bool,
    /// Replace a file already at the output path.
    pub force: bool,
    /// When it was packed, as RFC 3339. `None` asks [`stamp`], which reads
    /// `SOURCE_DATE_EPOCH` and otherwise the clock.
    pub created: Option<String>,
    /// The home directory to write as `~`.
    pub home: Option<PathBuf>,
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
    /// machine. Each is in the file by its name alone.
    pub outside: Vec<PathBuf>,
    /// Files the deck points at that could not be read, and why.
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
    /// The line, counting from one.
    pub line: usize,
    /// Which rule matched, as `github-pat`.
    pub rule: &'static str,
    /// The first four characters of the match, and `****`.
    pub redacted: String,
    /// What `--allow` takes to let this one through: the first twelve hex
    /// digits of sha256 over the rule, a NUL and the match.
    pub fingerprint: String,
}

impl std::fmt::Display for Hit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{}  {}  {}  {}",
            self.entry, self.line, self.rule, self.redacted, self.fingerprint
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
    let git = Git::find(&base);
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
        let source = match std::fs::read(&pointed.abs) {
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
        let (state, commit) = match (&git, pointed.inside) {
            (Some(git), true) => (git.state(&pointed.rel), commit.clone()),
            _ => (snap::State::Untracked, None),
        };
        files.insert(entry.clone(), text.into_bytes());
        snapped.push(snap::File {
            path: pointed.rel.clone(),
            entry,
            sha256: bundle::sha256(source.as_bytes()),
            commit,
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
fn regather(dir: &Path, options: &Options) -> anyhow::Result<Packing> {
    let manifest: Manifest = read_json(&dir.join("manifest.json"))?;
    let mut files = BTreeMap::new();
    walk(dir, "", &mut files)?;
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
        left: Vec::new(),
        repacked: true,
        allow_missing: options.allow_missing,
    };
    packing.screen(&options.allow);
    Ok(packing)
}

/// Every file under `dir`, by its name inside it with forward slashes.
fn walk(dir: &Path, prefix: &str, files: &mut BTreeMap<String, Vec<u8>>) -> anyhow::Result<()> {
    let entries =
        std::fs::read_dir(dir).with_context(|| format!("cannot read {}", dir.display()))?;
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = entry.path();
        let named = format!("{prefix}{name}");
        if entry.file_type()?.is_dir() {
            walk(&path, &format!("{named}/"), files)?;
        } else {
            let bytes =
                std::fs::read(&path).with_context(|| format!("cannot read {}", path.display()))?;
            files.insert(named, bytes);
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
        let found: Vec<Hit> = self
            .bundle
            .files
            .iter()
            .flat_map(|(entry, bytes)| scan(entry, bytes))
            .collect();
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
    /// or a file the deck points at that is not there.
    #[must_use]
    pub fn refusal(&self) -> Option<String> {
        let mut why = Vec::new();
        if !self.missing.is_empty() && !self.allow_missing {
            let mut said = format!(
                "{} the deck points at cannot be read, so its code cannot travel with it:\n",
                files(self.missing.len())
            );
            for (path, err) in &self.missing {
                said.push_str(&format!("\n  {}: {err}", path.display()));
            }
            said.push_str(
                "\n\nPack it where those files are, or pass --allow-missing to pack \
                 the deck without them: their panes will open to nothing.",
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
            let flags: Vec<String> = self
                .hits
                .iter()
                .map(|hit| format!("--allow {}", hit.fingerprint))
                .collect();
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
                "{} is outside the repo, so it is in the deck by its name alone",
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
            out.push_str("\n\nOutside the repo, so in it by name alone:\n");
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
/// Credentials in a URL — `https://user:token@host/…` — are taken out. A
/// remote that is a path on this machine names nothing anywhere else, so it
/// is dropped. An scp-style `git@host:owner/repo` is kept: `git` is not a
/// secret.
#[must_use]
pub fn clean_remote(remote: &str) -> Option<String> {
    let remote = remote.trim();
    if remote.is_empty() {
        return None;
    }
    if let Some((scheme, rest)) = remote.split_once("://") {
        if scheme.eq_ignore_ascii_case("file") {
            return None;
        }
        let (authority, path) = rest.split_at(rest.find('/').unwrap_or(rest.len()));
        let host = authority
            .rsplit_once('@')
            .map_or(authority, |(_, host)| host);
        return Some(format!("{scheme}://{host}{path}"));
    }
    // `host:path`, where the host is more than a drive letter and the colon
    // comes before any slash.
    let scp = remote.find(':').is_some_and(|colon| {
        colon > 1 && !remote[..colon].contains(['/', '\\']) && !remote.starts_with('.')
    });
    scp.then(|| remote.to_string())
}

/// What git says about the repo a deck was written in.
struct Git {
    /// The top of the working tree, as git printed it.
    root: String,
    /// `HEAD`, when there is one.
    commit: Option<String>,
    /// `origin`, as configured, credentials and all.
    remote: Option<String>,
}

impl Git {
    /// The repo `dir` is in, or `None` when there is no git or no repo.
    fn find(dir: &Path) -> Option<Self> {
        let root = git(dir, &["rev-parse", "--show-toplevel"]).filter(|root| !root.is_empty())?;
        let at = Path::new(&root);
        let commit = git(at, &["rev-parse", "--verify", "--quiet", "HEAD"])
            .filter(|commit| !commit.is_empty());
        let remote = git(at, &["remote", "get-url", "origin"]).filter(|url| !url.is_empty());
        Some(Self {
            root,
            commit,
            remote,
        })
    }

    /// How the file at `path`, relative to the root, stands against `HEAD`.
    ///
    /// Ignored files count as untracked: a reader at that commit does not
    /// have them either.
    fn state(&self, path: &str) -> snap::State {
        match git(
            Path::new(&self.root),
            &["status", "--porcelain", "--ignored", "--", path],
        ) {
            Some(out) if out.is_empty() && self.commit.is_some() => snap::State::Clean,
            Some(out) if out.starts_with("??") || out.starts_with("!!") || out.is_empty() => {
                snap::State::Untracked
            }
            Some(_) => snap::State::Modified,
            None => snap::State::Untracked,
        }
    }
}

/// What `git -C dir <args>` printed, or `None` when it could not say.
fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    out.status.success().then(|| {
        String::from_utf8_lossy(&out.stdout)
            .trim_end_matches(['\r', '\n'])
            .to_string()
    })
}

/// The directory paths are made relative to — the repo, or the deck's `cwd`
/// when there is no repo — in every spelling it might be written in.
struct Place {
    /// Resolved, so a file resolved the same way can be compared with it.
    root: PathBuf,
    /// As text, longest first: as git printed it, resolved, and as the deck
    /// named it. A deck written by hand on macOS says `/tmp`, which is
    /// `/private/tmp`, and either may turn up in its prose.
    spellings: Vec<String>,
}

impl Place {
    fn new(base: &Path, git: Option<&Git>) -> Self {
        let given = git.map_or_else(|| base.to_path_buf(), |git| PathBuf::from(&git.root));
        let root = std::fs::canonicalize(&given).unwrap_or_else(|_| given.clone());
        let mut spellings = vec![shown(&given), shown(&root)];
        if std::fs::canonicalize(base).is_ok_and(|base| base == root) {
            spellings.push(shown(base));
        }
        if let Some(git) = git {
            spellings.push(git.root.clone());
            spellings.push(git.root.replace('/', "\\"));
        }
        // A root of `/` or `C:\` would strip the start of every path there is.
        spellings.retain(|spelling| spelling.trim_end_matches(['/', '\\']).len() > 3);
        spellings.sort_by_key(|spelling| std::cmp::Reverse(spelling.len()));
        spellings.dedup();
        Self { root, spellings }
    }

    /// `path` relative to the root with forward slashes, or `None` when it is
    /// not under it.
    fn relative(&self, path: &Path) -> Option<String> {
        let resolved = std::fs::canonicalize(path).ok().or_else(|| {
            let parent = std::fs::canonicalize(path.parent()?).ok()?;
            Some(parent.join(path.file_name()?))
        });
        let under = resolved
            .as_deref()
            .and_then(|resolved| resolved.strip_prefix(&self.root).ok())
            .map(Path::to_path_buf)
            .or_else(|| {
                self.spellings
                    .iter()
                    .find_map(|spelling| path.strip_prefix(spelling).ok())
                    .map(Path::to_path_buf)
            })?;
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
}

/// A path as text, without the `\\?\` Windows puts on a resolved one.
fn shown(path: &Path) -> String {
    let text = path.to_string_lossy();
    text.strip_prefix(r"\\?\").unwrap_or(&text).to_string()
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
    /// Where it is on this machine.
    abs: PathBuf,
    /// What the deck will call it.
    rel: String,
    /// Whether it is under the root.
    inside: bool,
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
                let (rel, inside) = match self.place.relative(&abs) {
                    Some(rel) => (rel, true),
                    None => {
                        self.outside.push(abs.clone());
                        let name = abs
                            .file_name()
                            .map_or_else(|| "file".to_string(), |n| n.to_string_lossy().into());
                        (name, false)
                    }
                };
                self.files.push(Pointed {
                    abs,
                    rel,
                    inside,
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
    /// Written as `~`.
    home: Option<String>,
}

impl Scrub {
    fn new(place: &Place, home: Option<&Path>) -> Self {
        Self {
            roots: place.spellings.clone(),
            home: home
                .map(shown)
                .filter(|home| home.trim_end_matches(['/', '\\']).len() > 3),
        }
    }

    fn text(&self, text: &mut String) {
        for root in &self.roots {
            *text = swap(text, root, "", ".");
        }
        if let Some(home) = &self.home {
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
                    self.maybe(&mut drawn.note);
                    self.diagram(&mut drawn.diagram);
                }
                Ref::Page(page) => {
                    self.maybe(&mut page.note);
                    self.text(&mut page.page);
                }
            }
        }
    }

    fn diagram(&self, diagram: &mut Diagram) {
        self.maybe(&mut diagram.title);
        for node in &mut diagram.nodes {
            self.text(&mut node.label);
            self.maybe(&mut node.note);
        }
        for edge in &mut diagram.edges {
            self.maybe(&mut edge.label);
        }
        for cluster in &mut diagram.clusters {
            self.text(&mut cluster.label);
        }
        for flow in &mut diagram.flows {
            self.text(&mut flow.name);
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
    pattern: Regex,
}

/// The shapes the scanner knows, most particular first.
///
/// Curated from gitleaks' rules: each is a prefix a provider puts on its keys
/// so that scanners like this one can find them, or a block that is nothing
/// but a key. Where two overlap on a line — `sk-ant-` is also `sk-` — the
/// first one listed is the one reported.
static RULES: LazyLock<Vec<Rule>> = LazyLock::new(|| {
    [
        (
            "aws-access-key-id",
            r"\b(?:A3T[A-Z0-9]|AKIA|ASIA|ABIA|ACCA)[A-Z0-9]{16}\b",
        ),
        ("github-pat", r"\bghp_[A-Za-z0-9]{36}\b"),
        ("github-oauth", r"\bgho_[A-Za-z0-9]{36}\b"),
        ("github-app-token", r"\b(?:ghu|ghs)_[A-Za-z0-9]{36}\b"),
        ("github-refresh-token", r"\bghr_[A-Za-z0-9]{36}\b"),
        ("github-fine-grained-pat", r"\bgithub_pat_[A-Za-z0-9_]{82}\b"),
        ("gitlab-pat", r"\bglpat-[A-Za-z0-9_-]{20,}"),
        ("slack-token", r"\bxox[baprs]-[A-Za-z0-9-]{10,}"),
        (
            "slack-webhook",
            r"hooks\.slack\.com/(?:services|workflows|triggers)/[A-Za-z0-9+/]{43,56}",
        ),
        ("stripe-key", r"\b(?:sk|rk)_live_[A-Za-z0-9]{24,}\b"),
        ("google-api-key", r"\bAIza[0-9A-Za-z_-]{35}"),
        ("anthropic-key", r"\bsk-ant-[A-Za-z0-9_-]{32,}"),
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
        (
            "generic-secret",
            r#"(?i)\b(?:password|passwd|secret|api_key|apikey|token)\\?["']?\s*[:=]\s*\\?["'][^"'\\\r\n]{8,}\\?["']"#,
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

/// Everything in `bytes`, the entry called `entry`, that looks like a secret.
///
/// Read as text, line by line, so a hit can say where it is. One match per
/// place: where rules overlap, the first rule's.
#[must_use]
pub fn scan(entry: &str, bytes: &[u8]) -> Vec<Hit> {
    let text = String::from_utf8_lossy(bytes);
    let mut hits = Vec::new();
    for (at, line) in text.lines().enumerate() {
        let mut taken: Vec<std::ops::Range<usize>> = Vec::new();
        for rule in RULES.iter() {
            for found in rule.pattern.find_iter(line) {
                let span = found.range();
                if taken
                    .iter()
                    .any(|seen| seen.start < span.end && span.start < seen.end)
                {
                    continue;
                }
                taken.push(span);
                let matched = found.as_str();
                hits.push(Hit {
                    entry: entry.to_string(),
                    line: at + 1,
                    rule: rule.id,
                    redacted: format!("{}****", matched.chars().take(4).collect::<String>()),
                    fingerprint: fingerprint(rule.id, matched),
                });
            }
        }
    }
    hits
}

/// What `--allow` takes for a match: twelve hex digits of
/// `sha256(rule + "\0" + match)`. The same secret under the same rule is the
/// same fingerprint wherever it turns up, and the secret is not in it.
fn fingerprint(rule: &str, matched: &str) -> String {
    let mut hex = bundle::sha256(format!("{rule}\0{matched}").as_bytes());
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
