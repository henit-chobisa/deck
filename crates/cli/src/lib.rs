//! Writing a deck from the command line.
//!
//! An agent has a shell in every setup it runs in — a terminal, an editor's
//! panel, a tool call. So one command reaches all of them, and these are the
//! verbs behind it: start a deck, add a group, seal it, wait for the answer,
//! and cross the acknowledged local mailbox of a live native session.
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
//! This crate performs filesystem transport and nothing else. It does not open
//! a window and knows nothing about one. The window is `deck-app`, and the two
//! meet only through committed files and an advisory ownership lock.

use anyhow::Context as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use deck_core::protocol::{DiagramRef, Group, Header, Ref, RefSpec, Review, VERSION};

pub mod live;
pub mod refs;

/// The file that says a deck is finished.
const SEAL: &str = "done";

/// The file that says the reader shut the deck without answering.
///
/// Inside the deck rather than beside it, unlike the review: a review is the
/// deck's product and outlives it, and this is a fact about the deck itself
/// that goes when it does.
const SHUT: &str = "closed";

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
    /// A picture, as a `--diagram` argument named it.
    Drawn(refs::Picture),
    /// A page, as a `--page` argument named it.
    Written(refs::Written),
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

/// Refuse a ref that points at code which is not there.
///
/// This is the one check that answers "how do I know the agent did not make it
/// up". Not all of it — nothing here can tell whether the *claim* about a line
/// is true — but a citation is two halves, and the half that says *these lines,
/// in this file* can be checked against the disk in a millisecond.
///
/// An agent that has read a file and an agent that has guessed at one produce
/// the same confident prose. They do not produce the same line numbers. So the
/// guess is caught here, while the agent is still running and can go and look,
/// rather than by the reader opening a pane that says the file is missing —
/// which is the moment a deck stops being worth trusting.
///
/// Checked against the deck's own `cwd`, because that is what every relative
/// ref in it resolves against.
///
/// # Errors
///
/// When a ref names a file that is not there, or lines the file does not have.
fn point_at_code_that_exists(root: &Path, pointing: &[Pointing]) -> anyhow::Result<()> {
    let Ok(text) = std::fs::read_to_string(root.join("deck.json")) else {
        return Ok(());
    };
    let Ok(header) = serde_json::from_str::<Header>(&text) else {
        return Ok(());
    };
    // An unscoped deck resolves its refs against wherever it is read, so there
    // is no one place to check them against.
    let Some(base) = header.cwd else {
        return Ok(());
    };

    for one in pointing {
        let Pointing::Code(named) = one else {
            continue;
        };
        let at = if named.file.is_absolute() {
            named.file.clone()
        } else {
            base.join(&named.file)
        };

        let Ok(source) = std::fs::read_to_string(&at) else {
            anyhow::bail!(
                "no file at `{}`.\n\nA ref points at code the reader will open, so it \
                 has to be code that is there. Check the path — it is resolved against \
                 `{}`, the project this deck was opened for.",
                named.file.display(),
                base.display(),
            );
        };

        let lines = source.lines().count();
        let last = named.range.last;
        anyhow::ensure!(
            last as usize <= lines,
            "`{}` has {lines} lines, and the ref asks for {}.\n\nRead the file and \
             cite what is in it. A range past the end is the shape a guess takes.",
            named.file.display(),
            named.range,
        );
    }
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
            Pointing::Drawn(_) | Pointing::Written(_) => None,
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

/// Refuse two panes of one group with the same name.
///
/// The prose names a pane to say which one it means. Two answering to the same
/// name is the ambiguity naming was brought in to remove.
fn one_pane_per_name(pointing: &[Pointing]) -> anyhow::Result<()> {
    let names: Vec<&str> = pointing
        .iter()
        .filter_map(|one| match one {
            Pointing::Code(named) => named.name.as_deref(),
            Pointing::Drawn(picture) => picture.name.as_deref(),
            Pointing::Written(page) => page.name.as_deref(),
        })
        .collect();
    for (ix, name) in names.iter().enumerate() {
        anyhow::ensure!(
            !names[ix + 1..].contains(name),
            "two panes in this group are both named [{name}]. A name is how the \
             prose says which pane it means, so each one needs its own."
        );
    }
    Ok(())
}

/// What this group writes that will never light up.
///
/// Not a refusal. A group with no points is a legal group and sometimes the
/// right one — a single short claim over a single ref does not need a finger.
/// But the common failure is not a judgement call: the agent writes the prose
/// it would have written in chat, sends it with two refs beside it, and the
/// reader gets a paragraph about a change with nothing moving under it. The
/// light is the reason this is not a message.
///
/// Said on the way past, where whoever wrote the group is still listening.
#[must_use]
pub fn what_will_not_light(say: &str, pointing: &[Pointing]) -> Vec<String> {
    let mut notes = Vec::new();
    let code: Vec<&refs::Named> = pointing
        .iter()
        .filter_map(|one| match one {
            Pointing::Code(named) => Some(named),
            Pointing::Drawn(_) | Pointing::Written(_) => None,
        })
        .collect();
    let written: Vec<&refs::Written> = pointing
        .iter()
        .filter_map(|one| match one {
            Pointing::Written(page) => Some(page),
            Pointing::Code(_) | Pointing::Drawn(_) => None,
        })
        .collect();
    let drawn: Vec<&refs::Picture> = pointing
        .iter()
        .filter_map(|one| match one {
            Pointing::Drawn(picture) => Some(picture),
            Pointing::Code(_) | Pointing::Written(_) => None,
        })
        .collect();
    if code.is_empty() && drawn.is_empty() && written.is_empty() {
        return notes;
    }

    // A picture is pointed at by the name of a block, and a group that is only
    // a picture may honestly have nothing to point at yet.
    if !say.contains("[point ") && !code.is_empty() {
        notes.push(
            "no points in the say, so nothing moves while it is read: \
             write `[point 118-121]` on the lines each sentence is about"
                .to_string(),
        );
    }

    // A point that names something no pane answers to. The failure this caught
    // the first time: a page whose states were `make`, `link` and `attach`, and
    // an `id` scan that only knew about elements — so the code lit up beside a
    // picture that never moved, in silence.
    let mut answers: Vec<String> = Vec::new();
    for picture in &drawn {
        answers.extend(picture.diagram.nodes.iter().map(|node| node.id.clone()));
    }
    for page in &written {
        answers.extend(answered_by(&page.html));
    }
    for block in blocks(say) {
        // `106` and `106-110` are lines, not blocks, and belong to a file.
        let lines = block.split('-').all(|part| part.parse::<u32>().is_ok());
        if lines || answers.contains(&block) {
            continue;
        }
        notes.push(format!(
            "`[point {block}]` names nothing in this group: no diagram has a \
             block by that name, and no page declares it. A page says what it \
             answers to with `<meta name=\"deck-points\" content=\"...\">`"
        ));
    }

    let named: Vec<&str> = code
        .iter()
        .filter_map(|one| one.name.as_deref())
        .chain(drawn.iter().filter_map(|one| one.name.as_deref()))
        .collect();
    if named.is_empty() && code.len() + drawn.len() > 1 {
        notes.push(
            "no pane is named, so the prose cannot say which one it means: \
             put `[a-name]` at the front of a ref's note and use it in the say"
                .to_string(),
        );
    }
    for name in named {
        if !say.contains(&format!("[{name}]")) {
            notes.push(format!(
                "the pane named [{name}] is never mentioned in the say, so the \
                 reader has no way to know which pane it is"
            ));
        }
    }
    notes
}

/// Every payload a `[point ...]` in `say` carries, block names and all.
fn blocks(say: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = say;
    while let Some(at) = rest.find("[point ") {
        rest = &rest[at + 7..];
        let Some(end) = rest.find(']') else {
            break;
        };
        for part in rest[..end].split([',', ' ']) {
            let part = part.trim();
            if !part.is_empty() {
                out.push(part.to_string());
            }
        }
        rest = &rest[end + 1..];
    }
    out
}

/// The names a page answers to: its ids, and whatever it declares.
fn answered_by(html: &str) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(at) = html.find("name=\"deck-points\"")
        && let Some(from) = html[at..].find("content=\"")
        && let Some(end) = html[at + from + 9..].find('"')
    {
        let start = at + from + 9;
        out.extend(
            html[start..start + end]
                .split_whitespace()
                .map(str::to_string),
        );
    }
    let mut rest = html;
    while let Some(at) = rest.find("id=\"") {
        rest = &rest[at + 4..];
        let Some(end) = rest.find('"') else {
            break;
        };
        out.push(rest[..end].to_string());
        rest = &rest[end + 1..];
    }
    out
}

/// The words a page shows, which is the only thing it is rationed on.
///
/// Markup, so this is a scan and not a parse: tags come out, the contents of
/// `script` and `style` come out with them, and what is left is what a reader
/// would see. Text a script writes at run time gets past it, and that is
/// accepted — the rule is here to stop a page being written as a document, not
/// to police one.
#[must_use]
pub fn shown_words(html: &str) -> usize {
    let mut text = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(at) = rest.find('<') {
        text.push_str(&rest[..at]);
        rest = &rest[at..];
        let closes = |rest: &str, tag: &str| -> Option<usize> {
            rest.to_ascii_lowercase().find(&format!("</{tag}"))
        };
        let lower = rest.to_ascii_lowercase();
        if lower.starts_with("<script") || lower.starts_with("<style") {
            let tag = if lower.starts_with("<script") {
                "script"
            } else {
                "style"
            };
            match closes(rest, tag) {
                Some(end) => rest = &rest[end..],
                None => return text.split_whitespace().count(),
            }
        }
        match rest.find('>') {
            Some(end) => rest = &rest[end + 1..],
            None => break,
        }
    }
    text.push_str(rest);
    text.split_whitespace().count()
}

/// How many words a page may show before it has become a document.
///
/// The prose carries the argument. A page is for the thing that only makes
/// sense moving, and one with paragraphs in it is a second narration competing
/// with the band — which is the failure this whole pane is one bad decision
/// away from.
pub const WORDS: usize = 40;

/// Refuse a page that has been written as a document.
fn a_page_is_not_a_document(pointing: &[Pointing]) -> anyhow::Result<()> {
    for one in pointing {
        let Pointing::Written(page) = one else {
            continue;
        };
        let words = shown_words(&page.html);
        anyhow::ensure!(
            words <= WORDS,
            "this page shows {words} words, and {WORDS} is the limit. A page is \
             for the one idea in a deck that only makes sense moving — the prose \
             carries the argument, and words on the page are a second narration \
             competing with it. Labels, numbers and a field name, not sentences."
        );
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
    one_pane_per_name(&pointing)?;
    point_at_code_that_exists(root, &pointing)?;
    a_page_is_not_a_document(&pointing)?;

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
                    name: named.name,
                    after: named.after,
                    before: named.before,
                }),
                Pointing::Written(page) => Ref::Page(deck_core::protocol::PageRef {
                    id: format!("g{ord}p{nth}"),
                    page: page.html,
                    note: page.note,
                    name: page.name,
                }),
                Pointing::Drawn(picture) => Ref::Diagram(DiagramRef {
                    id: format!("g{ord}d{nth}"),
                    diagram: picture.diagram,
                    note: picture.note,
                    name: picture.name,
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

/// Say the reader closed the deck without answering.
///
/// The other end of `wait`. Without this a waiter had exactly two ways to
/// finish — a review, or a question asked mid-walk — and closing the window was
/// neither, so it sat there until its timeout or for ever. The agent went on
/// believing somebody was still reading.
///
/// # Errors
///
/// When the marker cannot be written.
pub fn shut(root: &Path) -> std::io::Result<()> {
    std::fs::write(root.join(SHUT), b"")
}

/// Whether the reader closed this deck without answering.
#[must_use]
pub fn was_shut(root: &Path) -> bool {
    root.join(SHUT).exists()
}

/// Forget that it was ever closed.
///
/// Opening the same deck again is an ordinary thing to do — it is what the bar
/// is for — and a marker left behind would kill the next waiter before the
/// reader had looked at anything.
pub fn reopened(root: &Path) {
    let _ = std::fs::remove_file(root.join(SHUT));
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
    fn a_ref_must_point_at_a_file_that_is_there() {
        // The question somebody always asks: how do I know the agent did not
        // make this up. Not all of it can be checked — nothing here knows
        // whether the *claim* about a line is true — but a citation has two
        // halves, and "these lines, in this file" is checkable against the
        // disk. An agent that read the file and one that guessed produce the
        // same confident prose; they do not produce the same line numbers.
        let at = scratch("missing-file");
        let project = at.join("project");
        std::fs::create_dir_all(&project).unwrap();
        let root = new(&at, "One", Some(project.clone()), None).unwrap();

        let err = group(&root, "say", pointing(&["nope.rs:1-4 not there"]))
            .expect_err("a ref to a file that does not exist is refused");
        assert!(err.to_string().contains("no file at"));
    }

    #[test]
    fn a_ref_must_stay_inside_the_file() {
        // A range past the end is the shape a guess takes.
        let at = scratch("past-the-end");
        let project = at.join("project");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::write(project.join("small.rs"), "one\ntwo\nthree\n").unwrap();
        let root = new(&at, "One", Some(project), None).unwrap();

        group(&root, "say", pointing(&["small.rs:1-3 all of it"]))
            .expect("a range the file has is fine");

        let err = group(&root, "say", pointing(&["small.rs:40-60 invented"]))
            .expect_err("a range the file does not have is refused");
        let said = err.to_string();
        assert!(said.contains("3 lines"), "and it says how long the file is");
    }

    #[test]
    fn an_unscoped_deck_is_left_alone() {
        // Without a cwd a deck renders wherever it is opened, so there is no
        // one checkout its refs can be checked against. Refusing on a guess at
        // the wrong tree would be worse than not checking.
        let at = scratch("unscoped");
        let root = new(&at, "One", None, None).unwrap();

        group(&root, "say", pointing(&["anywhere.rs:1-4 unknowable"]))
            .expect("an unscoped deck cannot be checked, so it is not");
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
    fn a_group_that_will_not_light_says_so_without_refusing() {
        let named = |arg: &str| Pointing::Code(crate::refs::parse(arg).expect("a ref that parses"));
        let panes = vec![
            named("a.rs:1-2 [retry] the loop"),
            named("b.rs:1-2 [write]"),
        ];

        // The failure this exists for: prose written as though it were a chat
        // message, with panes beside it that nothing in the words points at.
        let quiet = what_will_not_light("I changed the retry loop and the write.", &panes);
        assert_eq!(quiet.len(), 3, "{quiet:#?}");
        assert!(quiet[0].contains("no points"), "{quiet:#?}");
        assert!(quiet[1].contains("[retry]"), "{quiet:#?}");
        assert!(quiet[2].contains("[write]"), "{quiet:#?}");

        let walked = what_will_not_light(
            "[point 1] The loop in [retry] gives up early, and [write] never runs.",
            &panes,
        );
        assert!(walked.is_empty(), "{walked:#?}");

        // One pane and one claim is allowed to be quiet: a finger pointing at
        // the only thing on screen is not telling anybody anything.
        let alone = what_will_not_light("It returns before the write.", &[named("a.rs:1-2")]);
        assert_eq!(alone.len(), 1, "{alone:#?}");
        assert!(alone[0].contains("no points"), "{alone:#?}");
    }

    #[test]
    fn two_panes_cannot_answer_to_one_name() {
        // A name is how the prose says which pane it means. Two with the same
        // one is the ambiguity names were brought in to remove.
        let at = scratch("names");
        let root = new(&at, "One", None, None).unwrap();
        let named = |arg: &str| Pointing::Code(crate::refs::parse(arg).expect("a ref that parses"));

        let refused = group(
            &root,
            "the [retry] pane",
            vec![named("a.rs:1-2 [retry]"), named("b.rs:1-2 [retry]")],
        );
        assert!(refused.is_err());
        assert!(
            group(
                &root,
                "fine",
                vec![named("a.rs:1-2 [retry]"), named("b.rs:1-2 [write]")]
            )
            .is_ok()
        );
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
