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

/// Refuse narration with a code block typed into it.
///
/// The band renders inline marks and nothing else — bold, emphasis, a code
/// chip — because a group's `say` is prose with a few words picked out, not a
/// document. A fenced block put through it comes out as one long wrapped
/// paragraph with the fence markers still in it, which is unreadable.
///
/// It is also the wrong shape twice over. Code that exists belongs in a ref,
/// where it is highlighted and can be commented on. Code that does not exist
/// yet belongs in `after`, where it is drawn as a change against the lines it
/// replaces. A block in the narration is the tool being used to describe code
/// instead of to point at it, which is the one thing it is for.
///
/// # Errors
///
/// When `say` contains a fenced code block.
fn show_code_do_not_type_it(say: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        !say.contains("```"),
        "the narration has a code block in it, and the band cannot draw one — \
         it renders `**bold**`, `*emphasis*` and `` `code` `` and nothing \
         else, so a fence comes out as one long wrapped paragraph with the \
         backticks still in it.\n\nCode that already exists goes in a `--ref`, \
         where it is highlighted and can be commented on. Code that does not \
         exist yet goes in `--after` on the ref it replaces, where it is drawn \
         as a change. Either way the reader sees it as code."
    );
    Ok(())
}

/// How far apart two ranges in one file have to be to deserve separate panes.
///
/// Under this they are one block, and two panes showing it are two panes of
/// nearly the same code — the second one opening a few lines below the first
/// and repeating most of it. Over it they are genuinely two places, which is
/// worth a pane each: a declaration and its use four hundred lines down is the
/// case that earns it.
const APART: u32 = 30;

/// Refuse a group that shows one block of one file twice.
///
/// The skill says one file per pane and an agent does it anyway, which is the
/// same lesson twice: a rule that can be ignored is not a rule. This one
/// cannot be, and the message says what to write instead — a refused group
/// costs a command, and a deck that shows the same twelve lines in two panes
/// costs the reader the whole point of a grid.
///
/// # Errors
///
/// When two code refs name one file with ranges closer together than [`APART`].
fn one_pane_per_block(pointing: &[Pointing]) -> anyhow::Result<()> {
    let code: Vec<&refs::Named> = pointing
        .iter()
        .filter_map(|one| match one {
            Pointing::Code(named) => Some(named),
            Pointing::Drawn(_) => None,
        })
        .collect();

    for (ix, a) in code.iter().enumerate() {
        for b in &code[ix + 1..] {
            if a.file != b.file {
                continue;
            }
            let (first, last) = (
                a.range.first.min(b.range.first),
                a.range.last.max(b.range.last),
            );
            anyhow::ensure!(
                last.saturating_sub(first) > APART,
                "two panes of `{}` show the same block: {} and {}. One pane of \
                 {}-{} says it once — the reader sees the whole thing instead \
                 of the top of it and then most of it again.\n\nIf they really \
                 are two places, they are more than {APART} lines apart and \
                 this will not complain.",
                a.file.display(),
                a.range,
                b.range,
                first,
                last,
            );
        }
    }
    Ok(())
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

    show_code_do_not_type_it(say)?;
    one_pane_per_block(&pointing)?;

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
                    after: named.after,
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

#[cfg(test)]
mod tests {
    // Placed first because it is the rule most likely to be loosened by
    // somebody who has just been refused by it.

    use super::*;

    /// A code ref, from the syntax an agent would write.
    fn pointing(refs: &[&str]) -> Vec<Pointing> {
        refs.iter()
            .map(|r| Pointing::Code(crate::refs::parse(r).expect("a ref that parses")))
            .collect()
    }

    #[test]
    fn code_typed_into_the_narration_is_refused() {
        // The band renders inline marks and nothing else, so a fence came out
        // as one long wrapped paragraph with the backticks still in it. It is
        // also the wrong shape: code that exists belongs in a ref, and code
        // that does not exist yet belongs in `after`.
        let at = scratch("fenced");
        let root = new(&at, "One", None, None).unwrap();

        let err = group(
            &root,
            "Seed it first.\n\n```ts\nconst m = new Map();\n```",
            pointing(&["pull.ts:220-221 these two lines"]),
        )
        .expect_err("a fence in the narration is refused");

        assert!(
            err.to_string().contains("--after"),
            "and says where it goes"
        );
    }

    #[test]
    fn a_code_chip_in_the_narration_is_fine() {
        // One backtick is a chip and the band draws those. Only a fence is the
        // problem, and the check must not take the thing it is built around.
        let at = scratch("chip");
        let root = new(&at, "One", None, None).unwrap();

        group(
            &root,
            "The counter at `pending -= 1` runs **twice**.",
            pointing(&["pull.ts:220-221 here"]),
        )
        .expect("inline code is what the band is for");
    }

    #[test]
    fn one_block_of_one_file_does_not_get_two_panes() {
        // The case this was written for: 220-226 and 228-240 of the same file,
        // two lines apart. The second pane opened below the first and repeated
        // most of it, which is two panes spent saying one thing.
        let at = scratch("same-block");
        let root = new(&at, "One", None, None).unwrap();

        let err = group(
            &root,
            "say",
            pointing(&["pull.ts:220-226 scope arrives", "pull.ts:228-240 we ask"]),
        )
        .expect_err("two panes of one block is refused");

        let said = err.to_string();
        assert!(
            said.contains("220-240"),
            "and it says what to write instead"
        );
    }

    #[test]
    fn two_places_in_one_file_are_allowed() {
        // A declaration and its only use four hundred lines down is the case
        // that earns a pane each, and it is the reason this is a distance
        // rather than a ban on naming a file twice.
        let at = scratch("far-apart");
        let root = new(&at, "One", None, None).unwrap();

        group(
            &root,
            "say",
            pointing(&["pull.ts:40-44 declared", "pull.ts:300-304 used"]),
        )
        .expect("far apart is two places, not one block");
    }

    #[test]
    fn adjacent_ranges_in_different_files_are_the_whole_point() {
        // An enum and the column that stores it. Nothing about this rule may
        // discourage the thing the grid exists for.
        let at = scratch("two-files");
        let root = new(&at, "One", None, None).unwrap();

        group(
            &root,
            "say",
            pointing(&["query.py:14-22 the enum", "models.py:16-18 the column"]),
        )
        .expect("two files is what a group is for");
    }

    fn scratch(name: &str) -> PathBuf {
        let at = std::env::temp_dir().join(format!("deck-cli-{name}"));
        let _ = std::fs::remove_dir_all(&at);
        std::fs::create_dir_all(&at).expect("the scratch directory is writable");
        at
    }

    #[test]
    fn a_new_deck_is_a_directory_with_a_header_in_it() {
        let at = scratch("new");
        let root = new(&at, "The counter stalls", None, Some(3)).expect("a deck is written");

        assert!(root.join("deck.json").is_file());
        let header: Header =
            serde_json::from_str(&std::fs::read_to_string(root.join("deck.json")).unwrap())
                .expect("and the header parses");
        assert_eq!(header.title, "The counter stalls");
        assert_eq!(header.total, Some(3));
        assert!(root.ends_with(format!("{}.deck", header.id)));
    }

    #[test]
    fn two_decks_made_at_once_are_two_decks() {
        // The id carries the nanosecond clock, but ids are short and two calls
        // can land in the same bucket. Sharing a directory would put one
        // agent's groups in another agent's story, silently.
        let at = scratch("twice");
        let a = new(&at, "One", None, None).unwrap();
        let b = new(&at, "Two", None, None).unwrap();

        assert_ne!(a, b);
        assert!(a.join("deck.json").is_file() && b.join("deck.json").is_file());
    }

    #[test]
    fn groups_number_themselves() {
        // So an agent can write one command at a time without keeping count,
        // which is the whole reason the position is worked out here.
        let at = scratch("ord");
        let root = new(&at, "One", None, None).unwrap();

        group(&root, "first", Vec::new()).unwrap();
        group(&root, "second", Vec::new()).unwrap();

        let second: Group =
            serde_json::from_str(&std::fs::read_to_string(root.join("g2.json")).unwrap()).unwrap();
        assert_eq!(second.ord, Some(2));
        assert_eq!(second.id, "g2");
    }

    #[test]
    fn a_ref_is_named_for_its_group_as_well_as_its_place() {
        // Two panes called `r1` in different groups are indistinguishable in a
        // review, which names the ref a comment was made on. So the id carries
        // the group, and the agent never spells one.
        let at = scratch("ids");
        let root = new(&at, "One", None, None).unwrap();

        let pointing = || {
            vec![Pointing::Code(
                crate::refs::parse("a.rs:1-2").expect("a ref that parses"),
            )]
        };
        group(&root, "first", pointing()).unwrap();
        group(&root, "second", pointing()).unwrap();

        let id_in = |file: &str| -> String {
            let group: Group =
                serde_json::from_str(&std::fs::read_to_string(root.join(file)).unwrap()).unwrap();
            group.refs[0].id().to_string()
        };
        assert_eq!(id_in("g1.json"), "g1r1");
        assert_eq!(id_in("g2.json"), "g2r1");
    }

    #[test]
    fn what_was_not_said_is_not_written() {
        // Absent is absent. A deck full of `"after": null` is noise in a file
        // an agent pays to produce and a reader may have to read.
        let at = scratch("terse");
        let root = new(&at, "One", None, None).unwrap();
        group(
            &root,
            "hello",
            vec![Pointing::Code(
                crate::refs::parse("a.rs:1-2").expect("a ref that parses"),
            )],
        )
        .unwrap();

        let written = std::fs::read_to_string(root.join("g1.json")).unwrap();
        assert!(!written.contains("null"), "wrote a null: {written}");
        let header = std::fs::read_to_string(root.join("deck.json")).unwrap();
        assert!(!header.contains("null"), "wrote a null: {header}");
    }

    #[test]
    fn a_sealed_deck_will_not_grow() {
        // The seal is a promise to the reader that nothing more is coming. A
        // group arriving after it would be one the reader is never told about.
        let at = scratch("sealed");
        let root = new(&at, "One", None, None).unwrap();
        seal(&root).unwrap();

        assert!(group(&root, "late", Vec::new()).is_err());
    }

    #[test]
    fn a_directory_that_is_not_a_deck_is_said_so() {
        let at = scratch("bare");
        assert!(group(&at, "hello", Vec::new()).is_err());
        assert!(seal(&at).is_err());
    }

    #[test]
    fn the_review_is_looked_for_beside_the_deck_not_inside_it() {
        let at = scratch("review");
        let root = new(&at, "One", None, None).unwrap();

        let path = review_path(&root);
        assert_eq!(path.parent(), root.parent());
        assert!(path.extension().is_some_and(|end| end == "review"));
        assert!(review(&root).unwrap().is_none(), "none written yet");
    }
}
