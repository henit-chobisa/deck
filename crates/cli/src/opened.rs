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

/// The record in a cache folder of which file it was opened from.
pub const ORIGIN: &str = "opened.json";

/// Where the cache for every opened file lives: `~/.deck/opened`.
#[must_use]
pub fn cache() -> Option<PathBuf> {
    deck_core::home::deck().map(|deck| deck.join("opened"))
}

/// Which `.deck` file a cache folder was opened from, and where the reader's
/// review of it is written.
///
/// Kept in the cache rather than worked out again from the file, because
/// everything that asks — the window when it submits, the process waiting to
/// print the review, the window opened again tomorrow — has only the deck
/// folder to go on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Origin {
    /// The file, as it was named when it was opened, made absolute.
    pub file: PathBuf,
    /// Where the review goes: see [`review_to`].
    pub review: PathBuf,
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
        if dir.exists() {
            unlock(&dir);
            std::fs::remove_dir_all(&dir)
                .with_context(|| format!("cannot clear {}", dir.display()))?;
        }
        // Written beside it and moved into place whole, so a window that
        // starts while another is still writing never reads half a deck.
        let part = under.join(format!("{sha}.{}.part", std::process::id()));
        if part.exists() {
            unlock(&part);
            let _ = std::fs::remove_dir_all(&part);
        }
        write_out(&part, &bundle, &manifest)?;
        if let Err(err) = std::fs::rename(&part, &dir) {
            // Another `deck open` of the same file got there first. What it
            // wrote is the same bytes, so this copy is not needed.
            unlock(&part);
            let _ = std::fs::remove_dir_all(&part);
            anyhow::ensure!(
                intact(&dir, &bundle, &manifest),
                "cannot put {} in place: {err}",
                dir.display()
            );
        }
    }

    let file = std::path::absolute(file).unwrap_or_else(|_| file.to_path_buf());
    let origin = Origin {
        review: review_to(&file, &dir),
        file,
    };
    let json = serde_json::to_vec_pretty(&origin)?;
    std::fs::write(dir.join(ORIGIN), json)
        .with_context(|| format!("cannot write {}", dir.join(ORIGIN).display()))?;
    Ok(Opened {
        deck: dir.join("deck"),
        dir,
        origin,
        manifest: bundle.manifest,
        reused,
    })
}

/// Which file the deck folder at `root` was opened from, when it came from
/// one.
///
/// Only a folder named `deck` inside a cache folder that says so: an
/// ordinary deck folder is `<id>.deck`, and has no record beside it.
#[must_use]
pub fn origin(root: &Path) -> Option<Origin> {
    if root.file_name()? != "deck" {
        return None;
    }
    let text = std::fs::read_to_string(root.parent()?.join(ORIGIN)).ok()?;
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

/// Where the review of the `.deck` file at `file` is written.
///
/// Beside it, under the same name, as `<name>.review`: a file somebody was
/// sent sits in a folder they chose, and the answer belongs next to the
/// question. When that folder cannot be written — a mounted image, a
/// read-only share, somebody else's downloads — it goes in the cache folder
/// `dir` instead, and the window says so, so the reader is never left
/// believing a review went somewhere it did not.
#[must_use]
pub fn review_to(file: &Path, dir: &Path) -> PathBuf {
    let stem = file
        .file_stem()
        .map_or_else(|| "deck".into(), |stem| stem.to_string_lossy());
    let name = format!("{stem}.review");
    let folder = file
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    if writable(folder) {
        folder.join(name)
    } else {
        dir.join(name)
    }
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
/// The same repo means the same `origin`, cleaned the way `deck pack`
/// cleaned the deck's ([`crate::pack::clean_remote`]), compared whole. A
/// deck with no remote, or a commit, is never matched: there is nothing to
/// say it was this repo rather than any other with the same files in it.
/// `ceiling` stops git looking above a directory, as
/// `GIT_CEILING_DIRECTORIES` does; tests use it so a temporary directory
/// inside some other repo is not taken for part of it.
#[must_use]
pub fn matching(cwd: &Path, repo: Option<&Repo>, ceiling: Option<&Path>) -> Match {
    let Some(Repo {
        remote: Some(remote),
        commit: Some(packed),
    }) = repo
    else {
        return Match::Elsewhere;
    };
    let Some(git) = crate::pack::Git::find(cwd, ceiling) else {
        return Match::Elsewhere;
    };
    let here = git.remote.as_deref().and_then(crate::pack::clean_remote);
    if here.as_deref() != Some(remote.as_str()) {
        return Match::Elsewhere;
    }
    match git.commit.clone() {
        Some(head) if head == *packed => Match::Same {
            commit: head,
            root: PathBuf::from(git.root),
        },
        Some(head) => Match::Moved {
            packed: packed.clone(),
            head,
            root: PathBuf::from(git.root),
        },
        // A repo with nothing committed yet is not at any commit a deck was
        // packed at.
        None => Match::Elsewhere,
    }
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
fn intact(dir: &Path, bundle: &Bundle, manifest: &[u8]) -> bool {
    let same = |path: &Path, content: &[u8]| {
        std::fs::metadata(path).is_ok_and(|meta| meta.permissions().readonly())
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

/// Make everything under `dir` writable again, so it can be taken away.
///
/// Windows will not delete a read-only file, whatever its folder allows.
fn unlock(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            unlock(&path);
        } else if let Ok(meta) = std::fs::metadata(&path) {
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
