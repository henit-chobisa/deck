//! `deck open file.deck`: a packed deck, put where the window can read it.
//!
//! The window reads a deck as a folder, and goes on reading it — that is how
//! it shows the first group of a deck an agent is still writing. A `.deck`
//! file is the same deck finished and frozen, so the cheapest honest way to
//! show one is to write it out as the folder the window already knows how to
//! read, and point the window there.
//!
//! # Where it goes
//!
//! `~/.deck/opened/<sha256 of the file>/`, which makes the cache its own
//! check. The same bytes always land in the same place, so opening a file a
//! second time finds what the first time wrote; and different bytes — a file
//! somebody edited, or a different deck that happens to share a name — can
//! never land on top of it. Every time, the file is opened again with every
//! check [`Bundle::open`] makes, and what is on disk is compared with it
//! before it is trusted: a cache is a copy, and a copy can be edited.
//!
//! The deck's own files are written read-only. Nothing in deck writes to
//! them, and a reader who goes looking in the cache and edits a group should
//! be stopped by the file rather than find the deck quietly saying something
//! its author never sent. The folders stay writable, because the window keeps
//! its own notes beside a deck — a draft, whether anybody is listening — and
//! those belong to this reader, not to the deck.
//!
//! | path                    | what it is                                    |
//! |-------------------------|-----------------------------------------------|
//! | `deck/`                 | the deck folder the window opens              |
//! | `snap/`                 | the code it carries, as [`snap`] describes    |
//! | `manifest.json`         | the file's manifest                           |
//! | `review.json`           | a review packed with it, when there was one   |
//! | `opened.json`           | [`Origin`]: which file this came from, and where its review goes |
//!
//! # The repo it came from
//!
//! A deck knows the remote and the commit it was written at. Opened inside a
//! checkout of that same repo, the reader has the code already, and may have
//! it at a later commit; [`matching`] says which. It asks git about the
//! directory the reader is in and nothing else: no fetch, no network, and a
//! remote that is absent or does not match is simply not the same repo.

use std::path::{Path, PathBuf};

use anyhow::Context as _;
use deck_core::bundle::{self, Bundle, Manifest, Repo};
use serde::{Deserialize, Serialize};

/// The folder in a cache folder holding one record per file it was opened
/// from: `origins/<sha256 of the file's path>.json`, each an [`Origin`].
pub const ORIGINS: &str = "origins";

/// Where the cache for every opened file lives: `~/.deck/opened`.
#[must_use]
pub fn cache() -> Option<PathBuf> {
    deck_core::home::deck().map(|deck| deck.join("opened"))
}

/// Which `.deck` file a deck was opened from, and where the reader's review
/// of it is written.
///
/// One per file, not one per cache folder. Two copies of the same bytes —
/// `a/x.deck` and `b/y.deck` — share the read-only deck, since it is the same
/// deck, but each has a review of its own beside it; so every process that
/// needs to know where the review goes is handed the origin it was opened
/// with, rather than asking the cache which file was opened last. The record
/// under [`ORIGINS`] is for a reader looking in the cache, not for deck.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Origin {
    /// The file, as it was named when it was opened, made absolute.
    pub file: PathBuf,
    /// The sha256 of the file, which names its cache folder and is written
    /// into its review.
    pub sha256: String,
    /// Where the review goes: see [`review_to`].
    pub review: PathBuf,
    /// A review already where this one would have gone, of some other deck
    /// or another copy of this one, and so left as it is. See [`review_to`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub beside: Option<PathBuf>,
}

/// A `.deck` file, opened and in the cache.
#[derive(Debug, Clone)]
pub struct Opened {
    /// The cache folder: `~/.deck/opened/<sha256>`.
    pub dir: PathBuf,
    /// The deck folder inside it, which is what the window opens.
    pub deck: PathBuf,
    /// Which file, and where its review goes.
    pub origin: Origin,
    /// What the file says about itself.
    pub manifest: Manifest,
    /// Whether what was already in the cache was used, rather than written
    /// afresh.
    pub reused: bool,
}

/// Open the `.deck` file at `file` into the cache under `under`.
///
/// `under` is [`cache`] outside of tests. The file is read and opened with
/// every check [`Bundle::open`] makes, whether or not it has been opened
/// before. What is already in the cache for it is used only when every file
/// the deck holds is there, byte for byte, read-only, and nothing a deck
/// could hold has been added beside them; otherwise it is written again.
///
/// # Errors
///
/// When the file cannot be read or is not a deck that opens, or the cache
/// cannot be written.
pub fn open(file: &Path, under: &Path) -> anyhow::Result<Opened> {
    let bytes = std::fs::read(file).with_context(|| format!("cannot read {}", file.display()))?;
    let (bundle, _ignored) = Bundle::open(&bytes)
        .map_err(|err| anyhow::anyhow!("{} cannot be opened: {err}", file.display()))?;
    let sha = bundle::sha256(&bytes);
    let dir = under.join(&sha);
    let manifest = manifest_json(&bundle.manifest)?;

    let reused = intact(&dir, &bundle, &manifest);
    if !reused {
        clear(&dir).with_context(|| format!("cannot clear {}", dir.display()))?;
        // Written beside it and moved into place whole, so a window that
        // starts while another is still writing never reads half a deck.
        let part = under.join(format!("{sha}.{}.part", std::process::id()));
        let _ = clear(&part);
        write_out(&part, &bundle, &manifest)?;
        if let Err(err) = std::fs::rename(&part, &dir) {
            // Another `deck open` of the same file got there first. What it
            // wrote is the same bytes, so this copy is not needed.
            let _ = clear(&part);
            anyhow::ensure!(
                intact(&dir, &bundle, &manifest),
                "cannot put {} in place: {err}",
                dir.display()
            );
        }
    }

    let file = std::path::absolute(file).unwrap_or_else(|_| file.to_path_buf());
    let id = bundle.header().map(|header| header.id).unwrap_or_default();
    let (review, beside) = review_to(&file, &dir, &id, &sha);
    let origin = Origin {
        file,
        sha256: sha,
        review,
        beside,
    };
    let record = origin_record(&dir, &origin.file);
    if let Some(parent) = record.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("cannot make {}", parent.display()))?;
    }
    std::fs::write(&record, serde_json::to_vec_pretty(&origin)?)
        .with_context(|| format!("cannot write {}", record.display()))?;
    Ok(Opened {
        deck: dir.join("deck"),
        dir,
        origin,
        manifest: bundle.manifest,
        reused,
    })
}

/// Where the record of `file` having been opened into the cache folder `dir`
/// goes: keyed by the file's path, resolved, so each copy has its own.
fn origin_record(dir: &Path, file: &Path) -> PathBuf {
    let resolved = std::fs::canonicalize(file).unwrap_or_else(|_| file.to_path_buf());
    let key = bundle::sha256(resolved.to_string_lossy().as_bytes());
    dir.join(ORIGINS).join(format!("{key}.json"))
}

/// What was recorded when `file` was opened into the cache folder `dir`.
#[must_use]
pub fn origin(dir: &Path, file: &Path) -> Option<Origin> {
    let text = std::fs::read_to_string(origin_record(dir, file)).ok()?;
    serde_json::from_str(&text).ok()
}

/// The manifest of the cache folder holding the deck folder at `root`.
#[must_use]
pub fn manifest(root: &Path) -> Option<Manifest> {
    let text = std::fs::read_to_string(root.parent()?.join("manifest.json")).ok()?;
    serde_json::from_str(&text).ok()
}

/// Every source file the deck at `root` carries, when it came from a file.
///
/// Read from the cache, not the file: the cache was checked against the file
/// when it was opened.
#[must_use]
pub fn snapshots(root: &Path) -> Vec<bundle::snap::Snapshot> {
    let Some(dir) = root.parent() else {
        return Vec::new();
    };
    let Some(index) = std::fs::read_to_string(dir.join("snap").join("index.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<bundle::snap::Index>(&text).ok())
    else {
        return Vec::new();
    };
    index
        .files
        .into_iter()
        .filter_map(|file| {
            // Every entry is one the index named and `Bundle::open` accepted:
            // `snap/<n>.txt`, never a path of the reader's.
            let text = file
                .entry
                .split('/')
                .fold(dir.to_path_buf(), |path, part| path.join(part));
            let text = std::fs::read_to_string(text).ok()?;
            let snapshot = bundle::snap::Snapshot { file, text };
            snapshot.check().is_ok().then_some(snapshot)
        })
        .collect()
}

/// Where the review of the `.deck` file at `file` — deck `id`, its bytes
/// hashing to `sha` — is written, and the review already there that it was
/// kept apart from, if there was one.
///
/// Beside it, under the same name, as `<name>.review`: a file somebody was
/// sent sits in a folder they chose, and the answer belongs next to the
/// question. When that folder cannot be written — a mounted image, a
/// read-only share, somebody else's downloads — it goes in the cache folder
/// `dir` instead, and the window says so, so the reader is never left
/// believing a review went somewhere it did not.
///
/// A review already at that name is used only when it answers this very
/// file: the same deck and the same bytes. Anything else — the review of an
/// earlier copy the author has since packed again, or of another deck that
/// happened to get the same name — is somebody's finished work, and is
/// never written over. This one goes beside it as `<name>.<sha>.review`,
/// short, and the window names both.
#[must_use]
pub fn review_to(file: &Path, dir: &Path, id: &str, sha: &str) -> (PathBuf, Option<PathBuf>) {
    let stem = file
        .file_stem()
        .map_or_else(|| "deck".into(), |stem| stem.to_string_lossy());
    let folder = file
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let folder = if writable(folder) { folder } else { dir };
    let plain = folder.join(format!("{stem}.review"));
    if !plain.exists() || answers(&plain, id, sha) {
        return (plain, None);
    }
    (
        folder.join(format!("{stem}.{}.review", short(sha))),
        Some(plain),
    )
}

/// Whether the review at `path` answers the `.deck` file of deck `id` whose
/// bytes hash to `sha`. A review that will not parse answers nothing.
#[must_use]
pub fn answers(path: &Path, id: &str, sha: &str) -> bool {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<deck_core::protocol::Review>(&text).ok())
        .is_some_and(|review| is_ours(&review, id, sha))
}

/// Whether `review` answers the `.deck` file of deck `id` whose bytes hash to
/// `sha`: the same deck, and no other file named.
///
/// A review that names no file was written before reviews named one, and is
/// taken as this deck's rather than orphaned: only a review that names
/// another file is somebody else's.
#[must_use]
pub fn is_ours(review: &deck_core::protocol::Review, id: &str, sha: &str) -> bool {
    review.deck == id
        && review
            .deck_sha256
            .as_deref()
            .is_none_or(|named| named == sha)
}

/// Whether a file can be made in `folder`, found out by making one.
///
/// Asked of the folder rather than read off its permissions, which say
/// nothing about a read-only volume, an ACL, or a sandbox.
fn writable(folder: &Path) -> bool {
    let probe = folder.join(format!(".deck-probe-{}", std::process::id()));
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
    {
        Ok(_) => {
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

/// How the repo the reader is in stands against the one a deck was written in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Match {
    /// Not that repo, or no repo, or a deck that does not say which repo it
    /// came from. The deck's own copy is all there is to show.
    Elsewhere,
    /// That repo, at the commit the deck was packed at.
    Same {
        /// The commit.
        commit: String,
        /// The top of the reader's working tree, where its files are read.
        root: PathBuf,
    },
    /// That repo, at another commit: the code may have moved since.
    Moved {
        /// The commit the deck was packed at.
        packed: String,
        /// The commit the reader's checkout is at.
        head: String,
        /// The top of the reader's working tree, where its files are read.
        root: PathBuf,
    },
}

/// Whether `cwd` is inside a checkout of the repo `repo` names, and at which
/// commit.
///
/// The same repo means the same `origin`, compared as [`remote_key`] spells
/// it, so the ssh and https spellings of one repo are one repo — read both as
/// the reader wrote it and as their `insteadOf` settings rewrite it. A deck with
/// no remote, or no commit, is never matched: there is nothing to say it was
/// this repo rather than any other with the same files in it. `ceiling`
/// stops git looking above a directory, as `GIT_CEILING_DIRECTORIES` does;
/// tests use it so a temporary directory inside some other repo is not taken
/// for part of it.
#[must_use]
pub fn matching(cwd: &Path, repo: Option<&Repo>, ceiling: Option<&Path>) -> Match {
    matching_with(cwd, repo, ceiling, None)
}

/// [`matching`], with git reading `config` as its global settings when it is
/// given — an empty file in tests, so they pass whatever the machine's own
/// settings say.
fn matching_with(
    cwd: &Path,
    repo: Option<&Repo>,
    ceiling: Option<&Path>,
    config: Option<&Path>,
) -> Match {
    let Some(Repo {
        remote: Some(remote),
        commit: Some(packed),
    }) = repo
    else {
        return Match::Elsewhere;
    };
    let git = |dir: &Path, args: &[&str]| local_git(dir, ceiling, config, args);
    let Some(root) = git(cwd, &["rev-parse", "--show-toplevel"]).filter(|root| !root.is_empty())
    else {
        return Match::Elsewhere;
    };
    let root = PathBuf::from(root);
    // Either spelling of the reader's remote will do: as it is written, and
    // as `insteadOf` rewrites it. A rewrite is this machine's way of reaching
    // the repo, and the deck may have been packed on either side of one.
    let Some(wanted) = remote_key(remote) else {
        return Match::Elsewhere;
    };
    let same = [
        git(&root, &["config", "--get", "remote.origin.url"]),
        git(&root, &["remote", "get-url", "origin"]),
    ]
    .iter()
    .flatten()
    .any(|here| remote_key(here).as_deref() == Some(wanted.as_str()));
    if !same {
        return Match::Elsewhere;
    }
    match git(&root, &["rev-parse", "--verify", "--quiet", "HEAD"]).filter(|head| !head.is_empty())
    {
        Some(head) if head == *packed => Match::Same { commit: head, root },
        Some(head) => Match::Moved {
            packed: packed.clone(),
            head,
            root,
        },
        // A repo with nothing committed yet is not at any commit a deck was
        // packed at.
        None => Match::Elsewhere,
    }
}

/// What `git -C dir <args>` printed, run with none of the caller's `GIT_*`
/// variables — any of which can point git at some other repo, or rewrite
/// what it says about this one — and without the system's settings. The
/// reader's own settings stay unless `config` replaces them: they are where
/// `safe.directory` lives, which a checkout owned by somebody else needs.
fn local_git(
    dir: &Path,
    ceiling: Option<&Path>,
    config: Option<&Path>,
    args: &[&str],
) -> Option<String> {
    let mut command = std::process::Command::new("git");
    for (name, _) in std::env::vars_os() {
        if name.to_string_lossy().starts_with("GIT_") {
            command.env_remove(name);
        }
    }
    command
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .arg("-C")
        .arg(dir)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    if let Some(config) = config {
        command.env("GIT_CONFIG_GLOBAL", config);
    }
    if let Some(ceiling) = ceiling {
        command.env("GIT_CEILING_DIRECTORIES", ceiling);
    }
    let out = command.output().ok()?;
    out.status.success().then(|| {
        String::from_utf8_lossy(&out.stdout)
            .trim_end_matches(['\r', '\n'])
            .to_string()
    })
}

/// A remote as `host/owner/repo`, for telling whether two remotes are the
/// same repo — or `None` for one that names no repo anywhere else, such as a
/// path on this machine.
///
/// `git@github.com:acme/ingest.git`, `ssh://git@github.com/acme/ingest`
/// and `https://GitHub.com/acme/ingest/` are one repo, spelled three ways,
/// and all three are `github.com/acme/ingest`: the scheme, the user, any
/// credentials and a port go, a trailing `/` or `.git` comes off the path,
/// and all of it is lowercased: the forges deck is used with read owner and
/// repo names without regard to case.
#[must_use]
pub fn remote_key(remote: &str) -> Option<String> {
    let cleaned = crate::pack::clean_remote(remote)?;
    let (host, path) = match cleaned.split_once("://") {
        Some((_, rest)) => {
            let (authority, path) = rest.split_at(rest.find('/').unwrap_or(rest.len()));
            let host = authority
                .rsplit_once('@')
                .map_or(authority, |(_, host)| host);
            // A port, but not the colon inside an IPv6 address's brackets.
            let host = match host.rfind(':') {
                Some(colon) if !host[colon..].contains(']') => &host[..colon],
                _ => host,
            };
            (host.to_string(), path.to_string())
        }
        None => {
            let (head, path) = cleaned.split_once(':')?;
            let host = head.rsplit_once('@').map_or(head, |(_, host)| host);
            (host.to_string(), path.to_string())
        }
    };
    let path = path.trim_matches('/');
    let path = path
        .strip_suffix(".git")
        .unwrap_or(path)
        .trim_end_matches('/');
    if host.is_empty() || path.is_empty() {
        return None;
    }
    Some(format!("{host}/{path}").to_lowercase())
}

/// A commit as the window shows it: its first seven characters.
#[must_use]
pub fn short(commit: &str) -> &str {
    commit.get(..7).unwrap_or(commit)
}

/// The manifest as the cache keeps it.
fn manifest_json(manifest: &Manifest) -> anyhow::Result<Vec<u8>> {
    let mut json = serde_json::to_vec_pretty(manifest)?;
    json.push(b'\n');
    Ok(json)
}

/// Where the entry `name` goes under `dir`. Every name is one
/// [`bundle::known`] accepted: no `..`, no root.
fn place(dir: &Path, name: &str) -> PathBuf {
    name.split('/')
        .fold(dir.to_path_buf(), |path, part| path.join(part))
}

/// Whether the cache folder `dir` holds exactly `bundle`, as it was written.
///
/// A link anywhere in it, to anywhere, and it does not: deck never writes
/// one, and a cache that holds one could be pointing the window at a file
/// nobody vouched for. Nothing here follows a link to find out.
fn intact(dir: &Path, bundle: &Bundle, manifest: &[u8]) -> bool {
    if has_link(dir) {
        return false;
    }
    let same = |path: &Path, content: &[u8]| {
        std::fs::symlink_metadata(path)
            .is_ok_and(|meta| meta.is_file() && meta.permissions().readonly())
            && std::fs::read(path).is_ok_and(|bytes| bytes == content)
    };
    if !same(&dir.join("manifest.json"), manifest) {
        return false;
    }
    if !bundle
        .files
        .iter()
        .all(|(name, content)| same(&place(dir, name), content))
    {
        return false;
    }
    // Nothing added that the window would read as part of the deck: a group
    // dropped in beside the others would be shown as if the author wrote it.
    // A reader's own notes — a draft, the markers a session leaves — are not
    // names a deck holds, and are left alone.
    let Ok(entries) = std::fs::read_dir(dir.join("deck")) else {
        return false;
    };
    entries.flatten().all(|entry| {
        let name = format!("deck/{}", entry.file_name().to_string_lossy());
        !bundle::known(&name) || bundle.files.contains_key(&name)
    })
}

/// Whether `dir`, or anything under it, is a link. Folders are walked as
/// themselves, never through a link, so the walk cannot leave the cache.
fn has_link(dir: &Path) -> bool {
    let Ok(meta) = std::fs::symlink_metadata(dir) else {
        return false;
    };
    if meta.file_type().is_symlink() {
        return true;
    }
    if !meta.is_dir() {
        return false;
    }
    std::fs::read_dir(dir)
        .is_ok_and(|entries| entries.flatten().any(|entry| has_link(&entry.path())))
}

/// Write `bundle` out under `dir`, every file read-only.
fn write_out(dir: &Path, bundle: &Bundle, manifest: &[u8]) -> anyhow::Result<()> {
    // The deck folder even for a deck with nothing in it but its header, so
    // the window always has a folder to open.
    std::fs::create_dir_all(dir.join("deck"))
        .with_context(|| format!("cannot make {}", dir.display()))?;
    let write = |path: PathBuf, content: &[u8]| -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("cannot make {}", parent.display()))?;
        }
        std::fs::write(&path, content)
            .with_context(|| format!("cannot write {}", path.display()))?;
        let mut permissions = std::fs::metadata(&path)?.permissions();
        permissions.set_readonly(true);
        std::fs::set_permissions(&path, permissions)
            .with_context(|| format!("cannot make {} read-only", path.display()))
    };
    write(dir.join("manifest.json"), manifest)?;
    for (name, content) in &bundle.files {
        write(place(dir, name), content)?;
    }
    Ok(())
}

/// Take away whatever is at `path`: a folder deck wrote, with everything in
/// it — or, when it is a link or a file, that link or file and nothing it
/// points at.
///
/// Asked of the path itself before anything else, because every other way
/// of asking follows a link: a cache folder somebody replaced with a link to
/// their own files would otherwise have those files unlocked and deleted.
fn clear(path: &Path) -> std::io::Result<()> {
    match std::fs::symlink_metadata(path) {
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err),
        Ok(meta) if meta.is_dir() => {
            unlock(path);
            std::fs::remove_dir_all(path)
        }
        // A link, or a file where a folder should be: only the entry goes.
        Ok(_) => std::fs::remove_file(path),
    }
}

/// Make everything under `dir` writable again, so it can be taken away.
///
/// Windows will not delete a read-only file, whatever its folder allows.
/// `dir` itself is asked first, without following it: a link is not a
/// folder of deck's, and nothing it points at is unlocked.
fn unlock(dir: &Path) {
    if !std::fs::symlink_metadata(dir).is_ok_and(|meta| meta.is_dir()) {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        // Asked of the entry itself: a link is neither followed nor changed,
        // whatever it points at. Taking the folder away removes the link and
        // leaves its target alone.
        let Ok(meta) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_dir() {
            unlock(&path);
        } else {
            let mut permissions = meta.permissions();
            if permissions.readonly() {
                #[allow(clippy::permissions_set_readonly_false)]
                permissions.set_readonly(false);
                let _ = std::fs::set_permissions(&path, permissions);
            }
        }
    }
}

#[cfg(test)]
mod tests;
