//! Writing a deck from the command line.
//!
//! An agent has a shell in every setup it runs in — a terminal, an editor's
//! panel, a tool call. So one command reaches all of them, and these are the
//! four verbs behind it: start a deck, add a group, seal it, wait for the
//! answer.
//!
//! # Why this exists at all
//!
//! A deck is JSON in a directory, and an agent could write that itself. Every
//! deck written during this project's own development was written that way, by
//! hand, and it was the expensive path: more tokens per deck than a command
//! takes, and every field a fresh chance to spell something wrong that fails
//! silently at render time.
//!
//! The point of the commands is that the protocol document is for people
//! writing clients, and an agent should never have to read it. What it reads
//! instead is an argument list.
//!
//! # What is here and what is not
//!
//! This crate writes files and nothing else. It does not open a window, and it
//! knows nothing about one. The window is `deck-app`, and the two meet only at
//! a path on disk.

use anyhow::Context as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use deck_core::diagram::Diagram;
use deck_core::protocol::{DiagramRef, Group, Header, Ref, RefSpec, Review, VERSION};

pub mod refs;

/// The file that says a deck is finished.
const SEAL: &str = "done";

/// Start a deck, and return the directory it lives in.
///
/// `at` is the directory to put it in. The id is minted here rather than asked
/// for: an agent that had to invent one would either collide with itself or
/// spend a sentence deciding, and neither is worth its attention.
///
/// # Errors
///
/// When the directory cannot be made, or the header cannot be written.
pub fn new(
    at: &Path,
    title: &str,
    cwd: Option<PathBuf>,
    total: Option<u32>,
) -> anyhow::Result<PathBuf> {
    std::fs::create_dir_all(at).with_context(|| format!("cannot make {}", at.display()))?;

    // Made, not made-or-found.
    //
    // `create_dir_all` is happy to hand back a directory that already exists,
    // and an id that collided would quietly merge two decks — the second
    // agent's groups appearing in the first agent's story, with nothing said.
    // The id carries a clock, but clocks are coarse enough that two commands
    // in the same instant can mint the same one; `create_dir` fails on a
    // collision instead of joining it, and a few tries settle it.
    let mut made = None;
    for attempt in 0..8 {
        let id = mint(attempt);
        let root = at.join(format!("{id}.deck"));
        match std::fs::create_dir(&root) {
            Ok(()) => {
                made = Some((id, root));
                break;
            }
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(err) => return Err(err.into()),
        }
    }
    let (id, root) = made
        .ok_or_else(|| anyhow::anyhow!("could not find an unused deck id in {}", at.display()))?;

    let header = Header {
        v: VERSION,
        id,
        cwd,
        title: title.to_string(),
        total,
    };
    write_atomically(
        &root.join("deck.json"),
        &serde_json::to_vec_pretty(&header)?,
    )?;
    Ok(root)
}

/// Something to point at, before it has been given an id.
///
/// Ids are minted here rather than asked for. They have to be unique across the
/// whole deck — a comment names one, and two panes called `r1` in different
/// groups are indistinguishable in the review — and the only place that knows
/// both the group's position and the ref's is this crate.
pub enum Pointing {
    /// Lines of a file, as a `--ref` argument named them.
    Code(refs::Named),
    /// A picture, read from a file.
    Drawn(Diagram),
}

/// Add a group to the deck at `root`.
///
/// `ord` is worked out from what is already there, so groups can be written one
/// command at a time without the agent counting. The group's file is named for
/// its `ord`, which is only a convenience — order comes from the field.
///
/// # Errors
///
/// When the deck cannot be read, or the group cannot be written.
pub fn group(root: &Path, say: &str, pointing: Vec<Pointing>) -> anyhow::Result<PathBuf> {
    anyhow::ensure!(
        root.join("deck.json").is_file(),
        "{} is not a deck: no deck.json in it",
        root.display()
    );
    anyhow::ensure!(
        !root.join(SEAL).exists(),
        "{} is sealed: a deck that has said it is finished cannot grow",
        root.display()
    );

    let ord = next_ord(root);
    let refs = pointing
        .into_iter()
        .enumerate()
        .map(|(ix, one)| {
            let nth = ix + 1;
            match one {
                Pointing::Code(named) => Ref::Code(RefSpec {
                    id: format!("g{ord}r{nth}"),
                    file: named.file,
                    range: named.range,
                    note: named.note,
                    after: None,
                }),
                Pointing::Drawn(diagram) => Ref::Diagram(DiagramRef {
                    id: format!("g{ord}d{nth}"),
                    diagram,
                    note: None,
                }),
            }
        })
        .collect();

    let group = Group {
        id: format!("g{ord}"),
        ord: Some(ord),
        say: say.to_string(),
        refs,
    };

    let path = root.join(format!("g{ord}.json"));
    write_atomically(&path, &serde_json::to_vec_pretty(&group)?)?;
    Ok(path)
}

/// Say the deck is finished.
///
/// # Errors
///
/// When the marker cannot be written.
pub fn seal(root: &Path) -> anyhow::Result<()> {
    anyhow::ensure!(
        root.join("deck.json").is_file(),
        "{} is not a deck: no deck.json in it",
        root.display()
    );
    std::fs::write(root.join(SEAL), b"")?;
    Ok(())
}

/// Where the review for the deck at `root` will appear.
#[must_use]
pub fn review_path(root: &Path) -> PathBuf {
    let name = root
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("deck");
    root.with_file_name(format!("{name}.review"))
}

/// Read the review for the deck at `root`, if the reader has submitted one.
///
/// # Errors
///
/// When the file exists but will not parse.
pub fn review(root: &Path) -> anyhow::Result<Option<Review>> {
    let path = review_path(root);
    match std::fs::read_to_string(&path) {
        Ok(text) => Ok(Some(serde_json::from_str(&text)?)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err.into()),
    }
}

/// The next group's position, from what has already been written.
fn next_ord(root: &Path) -> u32 {
    let Ok(entries) = std::fs::read_dir(root) else {
        return 1;
    };
    let taken = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name();
            let name = name.to_str()?;
            let digits = name.strip_prefix('g')?.strip_suffix(".json")?;
            digits.parse::<u32>().ok()
        })
        .max()
        .unwrap_or(0);
    taken + 1
}

/// A deck id: the second it was made, and enough after it to tell two apart.
///
/// `salt` separates ids minted inside the same instant, which is what happens
/// when two commands run together. The clock alone is not enough: its low bits
/// are the same for both, and the whole point of the id is that it names one
/// deck.
fn mint(salt: u32) -> String {
    let since = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH);
    let now = since.as_ref().map_or(0, |since| since.as_secs());
    // Microseconds, not nanoseconds. The platform clock quantises to the
    // microsecond, so the low four digits of a nanosecond count are only ever
    // ten different values — which turns a rare collision into a common one.
    let tick = since.map_or(0, |since| since.subsec_micros()) % 10_000;
    format!("d-{now}-{:04}", (tick + salt) % 10_000)
}

/// Write `bytes` to `path`, or not at all.
///
/// Through a temporary file and a rename. A reader may be watching the
/// directory and will act on a file the moment it appears; a half-written
/// group would be read as a broken one, marked as seen, and never looked at
/// again.
fn write_atomically(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let temporary = path.with_extension("part");
    let write = || -> std::io::Result<()> {
        let mut file = std::fs::File::create(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temporary, path)
    };
    write().with_context(|| format!("cannot write {}", path.display()))
}
