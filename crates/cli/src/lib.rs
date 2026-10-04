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

/// The file a waiter keeps warm while it is listening.
///
/// In the deck rather than the live runtime, which is where the owner's own
/// marker lives: the deck directory is already the surface these two processes
/// share — `done`, `closed`, the review — and it needs no key derived from a
/// path to find.
const HEARD: &str = "listening";

/// How stale that file may be before nobody is on the other end.
///
/// Four seconds against a beat of one. Generous on purpose: the cost of
/// believing a waiter has gone when it has not is telling somebody their work
/// is going nowhere while it is in fact going somewhere, and that is a worse
/// lie than the silence it replaces.
const STILL_THERE: std::time::Duration = std::time::Duration::from_secs(4);

/// The file a waiter leaves when it goes away with a question in its hand.
///
/// `deck wait` returns the moment the reader asks something, and the beat
/// stops with it. For as long as the agent then spends on the answer there
/// is no waiter at all — and the window, seeing none, told the reader nobody
/// was listening while the agent was in the middle of answering them.
const ANSWERING: &str = "answering";

/// How long an agent may be away with a question before it has gone.
///
/// Long, because a real answer can take a subagent and several minutes, and
/// the cost of giving up early is the lie described above. Not for ever: an
/// agent that was killed mid-answer never comes back, and after this the
/// window says so.
const AWAY_WITH_IT: std::time::Duration = std::time::Duration::from_secs(10 * 60);

/// How long after it has spoken an agent may take to start waiting again.
///
/// The answer has landed, and the next thing the agent does is `deck wait`.
/// A minute and a half covers a slow turn; one that never comes back is not
/// believed for the full ten minutes.
const BACK_SOON: std::time::Duration = std::time::Duration::from_secs(90);

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
        notes.extend(unordered(&page.html).into_iter().map(|name| {
            format!(
                "`data-from=\"{name}\"` is not in the page's `deck-points`, so it never \
                 shows: `data-from` arrives at a step in that list and stays for the \
                 ones after it"
            )
        }));
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

/// The `data-from` names a page uses and never lists in `deck-points`.
///
/// `data-from` counts steps in that list's order, so a name outside it has
/// no place in the order and the element stays hidden for the whole walk.
fn unordered(html: &str) -> Vec<String> {
    let listed: Vec<&str> = html
        .find("name=\"deck-points\"")
        .and_then(|at| {
            let after = &html[at..];
            let from = after.find("content=\"")? + 9;
            let end = after[from..].find('"')?;
            Some(after[from..from + end].split_whitespace().collect())
        })
        .unwrap_or_default();
    let mut missing = Vec::new();
    let mut rest = html;
    while let Some(at) = rest.find("data-from=\"") {
        rest = &rest[at + 11..];
        let Some(end) = rest.find('"') else {
            break;
        };
        if let Some(name) = rest[..end].split_whitespace().next()
            && !listed.contains(&name)
            && !missing.iter().any(|known: &String| known == name)
        {
            missing.push(name.to_string());
        }
        rest = &rest[end + 1..];
    }
    missing
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

/// The names a page answers to: its ids, whatever it declares in
/// `deck-points`, and every name its elements light or show on —
/// `data-on`, `data-show`, `data-from`.
///
/// Shared with the window, which needs the same answer before a page has a
/// view to ask, so the two can never disagree about which page a point is in.
#[must_use]
pub fn answered_by(html: &str) -> Vec<String> {
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
    for attribute in ["id=\"", "data-on=\"", "data-show=\"", "data-from=\""] {
        let mut rest = html;
        while let Some(at) = rest.find(attribute) {
            // `data-id="…"` is not an id, nor `grid="…"` one of these.
            let whole =
                at == 0 || !rest[..at].ends_with(|ch: char| ch.is_alphanumeric() || ch == '-');
            rest = &rest[at + attribute.len()..];
            let Some(end) = rest.find('"') else {
                break;
            };
            if whole {
                out.extend(rest[..end].split_whitespace().map(str::to_string));
            }
            rest = &rest[end + 1..];
        }
    }
    out.sort();
    out.dedup();
    out
}

/// The words a page shows, which is the only thing it is rationed on.
///
/// Numbers and measurements are free; see [`counted`].
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
                None => return counted(&text),
            }
        }
        match rest.find('>') {
            Some(end) => rest = &rest[end + 1..],
            None => break,
        }
    }
    text.push_str(rest);
    counted(&text)
}

/// The words in `text` that are words. A number is not, and nor is a number
/// with its unit — `200ms`, `3.5k`, `-12%` — so the axis of an honest chart
/// costs nothing: the limit is there to stop sentences, and ticks are not
/// sentences.
fn counted(text: &str) -> usize {
    text.split_whitespace()
        .filter(|token| {
            // A word unless it starts, once its punctuation is set aside,
            // with a digit: `(200ms)` and `~5ms` are numbers, `.gitignore`
            // and `--force` are words.
            token
                .chars()
                .find(|ch| ch.is_alphanumeric())
                .is_some_and(char::is_alphabetic)
        })
        .count()
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

/// Say a waiter is on the other end of this deck, and still is.
///
/// Called on a beat while `wait` polls. A file's modification time is the
/// whole signal — there is nothing to read out of it.
pub fn listening(root: &Path) {
    let _ = std::fs::write(root.join(HEARD), b"");
}

/// Stop saying it.
pub fn stopped_listening(root: &Path) {
    let _ = std::fs::remove_file(root.join(HEARD));
}

/// Say the waiter has gone away to answer what the reader asked.
///
/// Written as `wait` returns a question, so the agent stays *there* in the
/// reader's eyes for as long as it is working on the answer.
pub fn answering(root: &Path) {
    let _ = std::fs::write(root.join(ANSWERING), b"asked");
}

/// Say the agent is still at it: it has just done something to the deck.
///
/// Only while it is away with a question, and back onto the long clock —
/// an agent that says *"looking"* and then shows a pane is working again,
/// not about to wait. One that was never asked anything is not made present
/// by doing things, and one given up on is not brought back.
pub fn still_answering(root: &Path) {
    if presence(root) == Presence::Answering {
        let _ = std::fs::write(root.join(ANSWERING), b"asked");
    }
}

/// Say the agent has answered, and should be waiting again shortly.
pub fn answered(root: &Path) {
    if presence(root) == Presence::Answering {
        let _ = std::fs::write(root.join(ANSWERING), b"said");
    }
}

/// Stop saying any of it: a waiter is back, or the walk is over.
pub fn stopped_answering(root: &Path) {
    let _ = std::fs::remove_file(root.join(ANSWERING));
}

/// Who is on the other end of a deck.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Presence {
    /// A waiter is running: what the reader writes reaches it at once.
    Listening,
    /// The agent took a question and is working on it, or has just answered.
    Answering,
    /// Nothing has been heard from an agent for long enough to say so.
    Nobody,
}

/// Who is on the other end of this deck right now.
///
/// By how fresh the marks are rather than whether they exist, because a
/// waiter that was killed never got to tidy up — and a file left behind by
/// one that died an hour ago must not read as somebody listening.
#[must_use]
pub fn presence(root: &Path) -> Presence {
    // A mark dated a little into the future is a clock that stepped back,
    // not a mark from long ago: it counts by how far off it is.
    let fresh = |name: &str, within: std::time::Duration| {
        std::fs::metadata(root.join(name))
            .and_then(|marked| marked.modified())
            .is_ok_and(|at| {
                at.elapsed()
                    .map_or_else(|ahead| ahead.duration() < within, |since| since < within)
            })
    };
    if fresh(HEARD, STILL_THERE) {
        return Presence::Listening;
    }
    let spoke = std::fs::read(root.join(ANSWERING)).is_ok_and(|mark| mark == b"said");
    if fresh(ANSWERING, if spoke { BACK_SOON } else { AWAY_WITH_IT }) {
        return Presence::Answering;
    }
    Presence::Nobody
}

/// Whether anybody is on the other end: waiting, or away answering.
#[must_use]
pub fn is_heard(root: &Path) -> bool {
    presence(root) != Presence::Nobody
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
    // The walk is over, whatever the agent was in the middle of.
    stopped_answering(root);
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

/// Prose as it was meant, when the line breaks arrived as the two characters
/// `\n`.
///
/// An agent writing `--say "one.\n\nTwo."` in double quotes sends a backslash
/// and an `n`: the shell does not turn that into a line break, and the deck
/// showed it to the reader as `\n\n` in the middle of a sentence (#4). So they
/// become the breaks they were written as — `\r\n` too.
///
/// Three things are left as written:
///
/// - **Inside backticks.** There somebody is quoting code, and `"\n"` in code
///   is a backslash and an `n`. A backtick only opens code if another closes
///   it, as the renderer has it: one on its own is just a character, and must
///   not switch this off for the rest of the text.
/// - **`\\n`**, which is how to write a literal `\n` outside backticks.
/// - **A Windows path.** In `C:\new` and `C:\Users\name` the `\n` belongs to
///   a word that began with a drive — one letter and a colon — and is followed
///   by a small letter or a digit. Only a drive: `Note:\nnext` is a label and
///   a break, and any other backslash in a word proves nothing. A relative
///   `src\new.rs` and a share `\\nas\new` are not saved by this, and belong
///   in backticks.
#[must_use]
pub fn real_breaks(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut code = false;
    // Whether the word being read already looks like a path.
    let mut path = false;
    let mut at = 0;
    while at < chars.len() {
        let ch = chars[at];
        let next = chars.get(at + 1).copied();
        match ch {
            '`' if code || closes(&chars[at + 1..]) => {
                code = !code;
                out.push(ch);
            }
            '\\' if !code && next == Some('\\') && chars.get(at + 2) == Some(&'n') => {
                // `\\n`: a literal backslash and an `n`, asked for.
                out.push_str("\\n");
                at += 2;
            }
            '\\' if !code
                && next == Some('r')
                && chars.get(at + 2) == Some(&'\\')
                && chars.get(at + 3) == Some(&'n') =>
            {
                out.push('\n');
                path = false;
                at += 3;
            }
            '\\' if !code && next == Some('n') => {
                // A sentence that ends on a path ends the path: in
                // `C:\Users\name.\nNext` the break is a break.
                let ended = at.checked_sub(1).is_some_and(|before| {
                    matches!(chars[before], '.' | ',' | ';' | ')' | '!' | '?')
                });
                let in_path = (path || drive(&chars[..at])) && !ended;
                let goes_on = chars
                    .get(at + 2)
                    .is_some_and(|after| after.is_ascii_lowercase() || after.is_ascii_digit());
                if in_path && goes_on {
                    out.push('\\');
                    path = true;
                } else {
                    out.push('\n');
                    path = false;
                    at += 1;
                }
            }
            '\\' => {
                path = path || drive(&chars[..at]);
                out.push(ch);
            }
            _ => {
                if ch.is_whitespace() {
                    path = false;
                }
                out.push(ch);
            }
        }
        at += 1;
    }
    out
}

/// Whether what was just read is a drive: one letter and a colon, starting
/// a word. `Note:` and `10:` end in a colon too, and are not drives.
fn drive(read: &[char]) -> bool {
    match read {
        [.., before, letter, ':'] => letter.is_ascii_alphabetic() && !before.is_alphanumeric(),
        [letter, ':'] => letter.is_ascii_alphabetic(),
        _ => false,
    }
}

/// Whether a backtick closes the one just read, before the paragraph ends:
/// the renderer pairs them a paragraph at a time.
fn closes(rest: &[char]) -> bool {
    rest.iter()
        .zip(rest.iter().skip(1).chain(std::iter::once(&' ')))
        .take_while(|(a, b)| !(**a == '\n' && **b == '\n'))
        .any(|(a, _)| *a == '`')
}

#[cfg(test)]
mod tests {
    #[test]
    fn an_agent_away_with_a_question_is_still_there() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let deck = dir.path();
        let age = |name: &str, by: std::time::Duration| {
            let file = std::fs::File::options()
                .write(true)
                .open(deck.join(name))
                .expect("the mark is there");
            file.set_modified(std::time::SystemTime::now() - by)
                .expect("and can be aged");
        };
        let minutes = |n: u64| std::time::Duration::from_secs(n * 60);

        assert_eq!(presence(deck), Presence::Nobody);

        // The waiter returns a question and its beat stops: still there.
        listening(deck);
        assert_eq!(presence(deck), Presence::Listening);
        answering(deck);
        stopped_listening(deck);
        assert_eq!(presence(deck), Presence::Answering);
        assert!(is_heard(deck));

        // Nine minutes into a slow answer it does something to the deck, and
        // the clock starts again: nine minutes later still, it is there.
        age(ANSWERING, minutes(9));
        still_answering(deck);
        age(ANSWERING, minutes(9));
        assert_eq!(presence(deck), Presence::Answering);

        // It says "looking" — expected back soon — and then goes on working.
        // That is the long clock again, not the short one.
        answered(deck);
        still_answering(deck);
        age(ANSWERING, minutes(5));
        assert_eq!(presence(deck), Presence::Answering);

        // Having answered and done nothing since, it is given a minute and
        // a half to start waiting, not ten.
        answered(deck);
        age(ANSWERING, minutes(2));
        assert_eq!(presence(deck), Presence::Nobody);

        // Given up on, it is not brought back by a late word.
        still_answering(deck);
        answered(deck);
        assert_eq!(presence(deck), Presence::Nobody);

        // A clock that stepped back a few seconds does not lose it.
        answering(deck);
        let file = std::fs::File::options()
            .write(true)
            .open(deck.join(ANSWERING))
            .expect("the mark is there");
        file.set_modified(std::time::SystemTime::now() + std::time::Duration::from_secs(5))
            .expect("and can be dated ahead");
        assert_eq!(presence(deck), Presence::Answering);

        // Speaking does not make an agent present that was never asked, and
        // closing the deck ends it whatever the agent was doing.
        stopped_answering(deck);
        answered(deck);
        still_answering(deck);
        assert_eq!(presence(deck), Presence::Nobody);
        answering(deck);
        shut(deck).expect("the deck can be closed");
        assert_eq!(presence(deck), Presence::Nobody);
    }

    #[test]
    fn a_waiter_that_died_is_not_one_that_is_listening() {
        // The mark is read by its age, not its existence, because the common
        // way a waiter ends is being killed — and `Drop` does not run for that.
        // A file left behind by one that died an hour ago must not read as
        // somebody sitting on the other end.
        let room = tempfile::tempdir().expect("a directory");
        let deck = room.path();

        assert!(!is_heard(deck), "nothing said yet");

        listening(deck);
        assert!(is_heard(deck), "and now something has");

        // Aged past the window, the way a killed waiter leaves it.
        let stale =
            std::time::SystemTime::now() - (STILL_THERE + std::time::Duration::from_secs(1));
        let file = std::fs::File::options()
            .write(true)
            .open(deck.join(HEARD))
            .expect("the mark is there");
        file.set_modified(stale).expect("and can be aged");
        assert!(!is_heard(deck), "a cold mark is nobody");

        stopped_listening(deck);
        assert!(!is_heard(deck));
    }

    // Placed first because it is the rule most likely to be loosened by
    // somebody who has just been refused by it.

    use super::*;

    #[test]
    fn an_axis_is_not_words() {
        assert_eq!(
            shown_words("<text>0ms</text> <text>200ms</text> <text>1.5s</text>"),
            0
        );
        assert_eq!(
            shown_words("<b>-12%</b> 3.5k $40 p99 retry"),
            2,
            "p99 and retry are words"
        );
        assert_eq!(shown_words("queue full"), 2);
        assert_eq!(shown_words("(200ms) ~5ms"), 0);
        assert_eq!(
            shown_words(".gitignore --force"),
            2,
            "words in punctuation are words"
        );
    }

    #[test]
    fn a_step_a_page_counts_from_has_to_be_in_its_order() {
        let page = r#"<meta name="deck-points" content="one two">
            <i data-from="two"></i><i data-from="three"></i>"#;
        assert_eq!(unordered(page), ["three"]);
    }

    #[test]
    fn a_page_answers_to_what_its_parts_light_on() {
        let page = r#"<meta name="deck-points" content="rest hop">
            <div id="box" class="node" data-on="hop settle">a</div>
            <div data-show="arrive" data-from="leave">b</div>
            <div data-id="not-a-name" grid="nor-this">c</div>"#;
        let names = answered_by(page);
        for name in ["rest", "hop", "box", "settle", "arrive", "leave"] {
            assert!(names.contains(&name.to_string()), "{name}");
        }
        assert!(!names.contains(&"not-a-name".to_string()));
        assert!(!names.contains(&"nor-this".to_string()));
    }

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

    #[test]
    fn a_break_written_as_two_characters_is_a_break() {
        assert_eq!(real_breaks(r"One.\n\nTwo."), "One.\n\nTwo.");
        assert_eq!(real_breaks("Already\n\nreal."), "Already\n\nreal.");
        assert_eq!(real_breaks(r"One.\r\n\r\nTwo."), "One.\n\nTwo.");
        assert_eq!(
            real_breaks(r"Changed. [point 1-6]\n\nWhy."),
            "Changed. [point 1-6]\n\nWhy."
        );
        assert_eq!(
            real_breaks(r"then.\nnext"),
            "then.\nnext",
            "a small letter after"
        );
        // Only breaks. A tab written that way stays, and so does a backslash
        // at the very end.
        assert_eq!(real_breaks(r"a\tb"), r"a\tb");
        assert_eq!(
            real_breaks(r"a\tb\nc"),
            "a\\tb\nc",
            "and the break after it is one"
        );
        assert_eq!(real_breaks(r"a\nb\nc"), "a\nb\nc");
        // A label is not a drive.
        assert_eq!(real_breaks(r"Note:\nnext line"), "Note:\nnext line");
        assert_eq!(
            real_breaks(r"Steps:\n1. first\n2. second"),
            "Steps:\n1. first\n2. second"
        );
        assert_eq!(real_breaks(r"at 10:\n5 items"), "at 10:\n5 items");
        assert_eq!(real_breaks("ends in \\"), "ends in \\");
    }

    #[test]
    fn code_keeps_its_backslashes() {
        assert_eq!(
            real_breaks(r"Split on `\n` and join.\nNext."),
            "Split on `\\n` and join.\nNext."
        );
        assert_eq!(
            real_breaks(r"```\nlet a = 1;\n```"),
            r"```\nlet a = 1;\n```",
            "a fence is backticks too, and is refused elsewhere anyway"
        );
        // One backtick on its own opens nothing — the renderer draws it as a
        // character — so it must not stop the breaks after it.
        assert_eq!(
            real_breaks(r"The ` key opens a chip.\n\nNext."),
            "The ` key opens a chip.\n\nNext."
        );
    }

    #[test]
    fn a_backtick_is_closed_within_its_paragraph() {
        // The stray one in the first paragraph must not pair with the opener
        // in the second and turn the code inside out.
        assert_eq!(
            real_breaks("The ` key.\n\nSplit on `\\n` here.\\nLast."),
            "The ` key.\n\nSplit on `\\n` here.\nLast."
        );
    }

    #[test]
    fn a_literal_backslash_n_can_be_asked_for() {
        assert_eq!(
            real_breaks(r"write \\n for a break"),
            r"write \n for a break"
        );
    }

    #[test]
    fn a_windows_path_keeps_its_backslashes() {
        assert_eq!(
            real_breaks(r"see C:\new\notes.txt"),
            r"see C:\new\notes.txt"
        );
        assert_eq!(real_breaks(r"in C:\Users\name."), r"in C:\Users\name.");
        assert_eq!(real_breaks(r"see `C:\new`"), r"see `C:\new`");
        // A path does not swallow the break after it.
        assert_eq!(
            real_breaks(r"in C:\Users\henit.\n\nNext."),
            "in C:\\Users\\henit.\n\nNext."
        );
        // Nor does the sentence it ends.
        assert_eq!(
            real_breaks(r"in C:\Users\henit.\nnext"),
            "in C:\\Users\\henit.\nnext"
        );
        assert_eq!(
            real_breaks(r"(see C:\Users\x).\nnext"),
            "(see C:\\Users\\x).\nnext"
        );
        // The cost that is left, pinned so it is a decision: a relative path
        // outside backticks.
        assert_eq!(real_breaks(r"see src\new.rs"), "see src\new.rs");
    }
}
