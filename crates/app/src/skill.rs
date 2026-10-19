//! Telling an agent that deck exists.
//!
//! The commands are the whole integration — any agent with a shell can drive
//! them, and every agent worth running has one. What is missing without this is
//! the sentence that makes a model *reach* for deck: one that has never heard
//! of it writes a summary in chat, because that is what it has always done.
//!
//! So deck ships a skill and installs it. It is a Markdown file with a
//! description saying when to use it, and the four commands under that.
//!
//! # Why it is installed rather than documented
//!
//! A README nobody reads is a README the agent never reads either. The skill
//! has to be in the directory the agent already looks in, and putting it there
//! is one `mkdir` and one write — cheaper than asking somebody to do it.

use std::path::{Path, PathBuf};

/// The skill itself, carried in the binary.
///
/// Embedded rather than installed alongside, because deck is one file: a
/// `brew install` that put a Markdown document somewhere and then had to find
/// it again would have two things to get wrong.
const SKILL: &str = include_str!("../skill/SKILL.md");

/// The rest of the skill, written into `references/` beside `SKILL.md`.
///
/// `SKILL.md` is the loop and a table of which of these to read when. It is
/// read whole every time the skill is used and carried for the whole session,
/// and after a compaction only its first few thousand tokens come back — so it
/// stays short, and how to write a deck is read when a deck is written.
const REFERENCES: &[(&str, &str)] = &[
    (
        "answering.md",
        include_str!("../skill/references/answering.md"),
    ),
    ("groups.md", include_str!("../skill/references/groups.md")),
    ("pages.md", include_str!("../skill/references/pages.md")),
    (
        "pictures.md",
        include_str!("../skill/references/pictures.md"),
    ),
    (
        "situations.md",
        include_str!("../skill/references/situations.md"),
    ),
    ("writing.md", include_str!("../skill/references/writing.md")),
];

/// Where the references go, inside the skill's directory.
const REFERENCES_DIR: &str = "references";

/// Where the one real copy lives, when the machine has a shared store.
///
/// `~/.agents/skills` is a convention several tools already keep: one skill on
/// disk, and a symlink into each agent that reads it. Deck follows it rather
/// than inventing a sixth copy — a machine with five copies of a skill has five
/// chances to be out of date.
const SHARED: &str = ".agents/skills";

/// An agent that reads skills, and where it reads them from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Agent {
    /// What it is called, for something a person reads.
    pub name: &'static str,
    /// The directory it lives in, under `$HOME`. Its existence is what says the
    /// agent is installed.
    pub root: &'static str,
    /// Where it looks for skills, best first.
    ///
    /// More than one because the name is not settled: Cursor keeps them in
    /// `skills-cursor`, everything else in `skills`. Whichever already exists
    /// wins; when none does, the first is made.
    pub skills: &'static [&'static str],
    /// Whether it reads the shared store itself.
    ///
    /// Amp does — `amp skill list` shows everything in `~/.agents/skills`
    /// without a link — so linking would be putting a second copy in front of
    /// one it already reads. It still needs somewhere of its own when there is
    /// no shared store, which is what `skills` is for.
    pub reads_shared: bool,
}

/// Every agent deck knows how to tell.
pub const AGENTS: &[Agent] = &[
    Agent {
        name: "Claude Code",
        root: ".claude",
        skills: &[".claude/skills"],
        reads_shared: false,
    },
    Agent {
        name: "Codex",
        root: ".codex",
        skills: &[".codex/skills"],
        reads_shared: false,
    },
    Agent {
        name: "Cursor",
        root: ".cursor",
        skills: &[".cursor/skills-cursor", ".cursor/skills"],
        reads_shared: false,
    },
    Agent {
        name: "Amp",
        root: ".amp",
        // Its own `--global` directory, used only when there is no shared
        // store for it to read.
        skills: &[".config/agents/skills"],
        reads_shared: true,
    },
];

impl Agent {
    /// Whether this agent is on the machine at all.
    ///
    /// Its own directory, not the skills directory inside it: an agent that has
    /// never been given a skill has no `skills/` yet, and that is exactly the
    /// one worth offering.
    #[must_use]
    pub fn installed(self, home: &Path) -> bool {
        home.join(self.root).exists()
    }

    /// Where the skill would go: whichever skills directory it already has, or
    /// the first if it has none.
    #[must_use]
    pub fn skill_dir(self, home: &Path) -> PathBuf {
        self.skills
            .iter()
            .map(|at| home.join(at))
            .find(|path| path.is_dir())
            .unwrap_or_else(|| home.join(self.skills[0]))
    }

    /// Where the skill would go.
    #[must_use]
    pub fn skill_path(self, home: &Path) -> PathBuf {
        self.skill_dir(home).join("deck")
    }

    /// Whether deck has already told this one.
    #[must_use]
    pub fn told(self, home: &Path) -> bool {
        self.skill_path(home).exists() || self.skill_path(home).symlink_metadata().is_ok()
    }
}

/// Every agent found on this machine.
#[must_use]
pub fn found(home: &Path) -> Vec<Agent> {
    AGENTS
        .iter()
        .copied()
        .filter(|agent| agent.installed(home))
        .collect()
}

/// Put the skill where every agent found will read it.
///
/// One real copy in the shared store, and a symlink from each agent — which is
/// the convention those directories already keep, and means `deck setup` run
/// again updates every agent at once rather than five files independently.
///
/// A machine with no shared store gets real copies instead. A symlink into a
/// directory nobody else uses would be a link to nothing.
///
/// # Errors
///
/// When the shared copy cannot be written. An agent whose own directory refuses
/// the link is reported and skipped — one unwritable directory is not a reason
/// to tell none of them.
pub fn install_all(home: &Path) -> anyhow::Result<Vec<Told>> {
    let mut told = Vec::new();

    // The one real copy, when there is a shared store to put it in.
    let shared = home
        .join(SHARED)
        .is_dir()
        .then(|| PathBuf::from(SHARED).join("deck"));
    if let Some(shared) = shared.as_ref() {
        let at = home.join(shared).join("SKILL.md");
        write(&at)?;
        told.push(Told::Written(at));
    }

    for agent in found(home) {
        // One that reads the shared store is already told. Linking would put a
        // second copy in front of the one it is reading.
        if agent.reads_shared && shared.is_some() {
            told.push(Told::Reads(agent.name));
            continue;
        }

        let at = agent.skill_path(home);
        let dir = agent.skill_dir(home);
        let from = dir.strip_prefix(home).unwrap_or(&dir).to_path_buf();

        match point(&at, shared.as_deref().map(|shared| pointing(&from, shared))) {
            Ok(how) => told.push(how),
            Err(err) => told.push(Told::Refused(agent.name, err.to_string())),
        }
    }
    Ok(told)
}

/// Bring every copy of the skill deck has put down up to date with this deck,
/// and say which were rewritten.
///
/// Every deck that starts runs this, so the skill follows the binary however
/// the binary arrived. `deck upgrade`, the automatic update and the install
/// script all start the new deck with `--version` before they are done, so the
/// skill is current by the time any of them finishes, and the next agent
/// session reads it. Before this only `deck setup` wrote the skill, and an
/// upgraded deck went on being described to agents as the one it replaced.
///
/// Only copies deck put there, and only while they are still what it put
/// there. An agent somebody said no to in setup has no copy and is not given
/// one. A copy somebody has edited since no longer matches its stamp, and is
/// theirs. A copy a newer deck wrote is left to it, so an old deck still on the
/// machine does not drag the skill back each time it runs. A copy from before
/// stamps existed was written by `deck setup`, and is taken as deck's.
///
/// Quiet about failures: a skill that cannot be rewritten is no reason for
/// `deck group` to fail, and `deck setup` is there to say why.
pub fn keep_up(home: &Path) -> Vec<PathBuf> {
    let shared = std::iter::once(home.join(SHARED).join("deck"));
    let own = AGENTS.iter().map(|agent| agent.skill_path(home));
    let mut rewritten = Vec::new();
    for dir in shared.chain(own) {
        // Where it really is: a link, of the directory or of the file itself,
        // is followed so the link stays a link. Through a directory link this
        // is the shared copy, already rewritten above, so it is not written
        // twice. The references are read and written beside the `SKILL.md`
        // the agent reads, which through a directory link is the shared copy
        // too.
        let Ok(at) = std::fs::canonicalize(dir.join("SKILL.md")) else {
            continue;
        };
        let Some(was) = on_disk(&dir, &at) else {
            continue;
        };
        let ours = format!("{RUNNING} {:x}\n", print_of_ours());
        let said = std::fs::read_to_string(stamp_path(&at)).ok();
        if was.is_ours() {
            // Already this deck's words. Stamped if it is not yet, so an edit
            // made to it later is seen as one; and if a write was cut off
            // between the skill and its stamp, the stamp catches up.
            if said.as_deref() != Some(ours.as_str()) {
                let _ = replace(&stamp_path(&at), ours.as_bytes());
            }
            continue;
        }
        if !decks(&was, said.as_deref()) {
            continue;
        }
        if write_into(&dir, &at).is_ok() {
            rewritten.push(at);
        }
    }
    rewritten
}

/// Whether a copy that is not this deck's words is still deck's to rewrite,
/// going by what its stamp `said`.
///
/// No stamp: written by `deck setup` before stamps existed, and deck's —
/// unless something is in `references/` that this deck does not carry, since
/// no deck that old wrote references. A newer deck's stamp: left to it. A
/// stamp: deck's only while the words are still the ones it wrote, and only
/// for a deck strictly newer than the one that wrote them. The same version
/// with other words is a second build of it, and two builds that each took the
/// skill back would trade it on every run. A stamp that will not read is
/// nobody's to guess at.
///
/// Three more, each a copy deck left half written rather than one somebody
/// changed:
///
/// - A `pending` stamp is a write that was cut off. Whatever it left is a mix
///   of two decks' words and matches no fingerprint, so the stamp says so
///   before the first file goes down.
/// - References that are all this deck's own, word for word, are nothing of
///   anybody's. Beside this deck's `SKILL.md` they are a copy missing a file
///   or two — the second of two agents whose `SKILL.md` links to one file,
///   whose stamp sits beside that file and was written with the first.
/// - Beside a `SKILL.md` stamped the way decks before references stamped it,
///   they are an upgrade that stopped after the references, or an older
///   `deck setup` that wrote its `SKILL.md` and left them there.
fn decks(was: &OnDisk, said: Option<&str>) -> bool {
    let only_ours = was.references.iter().all(|(name, bytes)| {
        reference(name).is_some_and(|text| text.as_bytes() == bytes.as_slice())
    });
    let Some(said) = said else {
        // Decks from before stamps wrote no references, so any there are
        // somebody's.
        return only_ours;
    };
    let pending = pending(said);
    let stamped = stamped(said);
    // A newer deck's copy is left to it, whatever it looks like: one that
    // dropped a reference would otherwise be given it back here, and take it
    // away again on its own next run.
    let by = pending
        .as_ref()
        .map(|it| &it.by)
        .or(stamped.as_ref().map(|it| &it.0));
    if by.is_some_and(|by| crate::upgrade::newer(by, RUNNING)) {
        return false;
    }
    if only_ours && was.skill == SKILL.as_bytes() {
        return true;
    }
    if let Some(pending) = pending {
        // Every file is either what was there before the write began or what
        // this deck writes. Anything else was changed since, by somebody.
        let was_or_ours = |name: &str, bytes: &[u8], ours: Option<&str>| {
            ours.is_some_and(|text| text.as_bytes() == bytes)
                || pending
                    .was
                    .iter()
                    .any(|(it, print)| it == name && *print == fnv(bytes))
        };
        return was_or_ours("SKILL.md", &was.skill, Some(SKILL))
            && was.references.iter().all(|(name, bytes)| {
                was_or_ours(&format!("{REFERENCES_DIR}/{name}"), bytes, reference(name))
            });
    }
    let Some((by, wrote)) = stamped else {
        return false;
    };
    let unchanged = wrote == was.print() || (only_ours && wrote == fnv(&was.skill));
    unchanged && crate::upgrade::newer(RUNNING, &by)
}

/// The text of the reference this deck carries under `name`.
fn reference(name: &str) -> Option<&'static str> {
    REFERENCES
        .iter()
        .find(|(ours, _)| *ours == name)
        .map(|(_, text)| *text)
}

/// Whether every agent deck has told reads the skill this deck carries.
#[must_use]
pub fn current(home: &Path) -> bool {
    found(home).iter().all(|agent| {
        let dir = agent.skill_path(home);
        on_disk(&dir, &dir.join("SKILL.md")).is_some_and(|was| was.is_ours())
    })
}

/// A copy of the skill as it is on disk: its `SKILL.md`, and every file in
/// its `references/`, by name.
struct OnDisk {
    skill: Vec<u8>,
    references: Vec<(String, Vec<u8>)>,
}

impl OnDisk {
    /// Whether these are exactly the words this deck carries, and no others.
    fn is_ours(&self) -> bool {
        self.skill == SKILL.as_bytes()
            && self.references.len() == REFERENCES.len()
            && self
                .references
                .iter()
                .zip(REFERENCES)
                .all(|((name, was), (ours, text))| name == ours && was == text.as_bytes())
    }

    /// The fingerprint a stamp holds for this copy.
    fn print(&self) -> u64 {
        print(
            &self.skill,
            self.references
                .iter()
                .map(|(name, bytes)| (name.as_str(), bytes.as_slice())),
        )
    }
}

/// What is on disk at `dir`, with its `SKILL.md` read from `at`, or nothing
/// when there is no `SKILL.md` to read.
///
/// Every file in `references/` is read, not only the ones this deck writes: a
/// file somebody added is a change to the copy like any other, and makes it
/// theirs. Dot files are left out, being a write in progress rather than part
/// of the skill.
fn on_disk(dir: &Path, at: &Path) -> Option<OnDisk> {
    let skill = std::fs::read(at).ok()?;
    let mut references = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir.join(REFERENCES_DIR)) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            // A directory deck never makes, which makes the copy somebody's.
            if !entry.path().is_file() {
                references.push((format!("{name}/"), Vec::new()));
                continue;
            }
            references.push((name, std::fs::read(entry.path()).ok()?));
        }
    }
    references.sort();
    Some(OnDisk { skill, references })
}

/// The fingerprint of a skill and its references, in name order.
///
/// With no references it is the fingerprint of `SKILL.md` alone, which is what
/// every stamp written before there were references holds — so a copy one of
/// those decks wrote is still recognised as deck's, and is brought up to date.
fn print<'a>(skill: &[u8], references: impl Iterator<Item = (&'a str, &'a [u8])>) -> u64 {
    references.fold(fnv(skill), |hash, (name, bytes)| {
        let hash = fnv_on(hash, b"\0");
        let hash = fnv_on(hash, name.as_bytes());
        let hash = fnv_on(hash, b"\0");
        fnv_on(hash, bytes)
    })
}

/// The fingerprint of the skill this deck carries.
fn print_of_ours() -> u64 {
    print(
        SKILL.as_bytes(),
        REFERENCES
            .iter()
            .map(|(name, text)| (*name, text.as_bytes())),
    )
}

/// The version this deck is, written beside every skill it writes.
const RUNNING: &str = env!("CARGO_PKG_VERSION");

/// Beside a `SKILL.md`, which deck wrote it and what it wrote.
///
/// A dot file, so no agent reads it as part of the skill.
fn stamp_path(at: &Path) -> PathBuf {
    at.with_file_name(".deck-stamp")
}

/// The version and fingerprint in `at`'s stamp, if it has one.
#[cfg(test)]
fn stamp(at: &Path) -> Option<(String, u64)> {
    stamped(&std::fs::read_to_string(stamp_path(at)).ok()?)
}

/// A write that has begun and not finished: which deck began it, and the
/// fingerprint of each file as it was before.
///
/// Written before the first file of a write goes down and replaced by the
/// fingerprint once the last is down, as `0.1.7 pending SKILL.md=… references/
/// writing.md=…`. A deck from before references cannot read it, and leaves the
/// copy alone, as it does any stamp it cannot read.
struct Pending {
    by: String,
    was: Vec<(String, u64)>,
}

/// The write a stamp says is in progress, if it says one is.
fn pending(said: &str) -> Option<Pending> {
    let mut words = said.split_whitespace();
    let by = words.next()?.to_string();
    if words.next()? != "pending" {
        return None;
    }
    let was = words
        .map(|word| {
            let (name, print) = word.rsplit_once('=')?;
            Some((name.to_string(), u64::from_str_radix(print, 16).ok()?))
        })
        .collect::<Option<_>>()?;
    Some(Pending { by, was })
}

/// The stamp a write puts down before its first file: what `before` held.
fn pending_stamp(before: Option<&OnDisk>) -> String {
    let mut said = format!("{RUNNING} pending");
    if let Some(before) = before {
        said.push_str(&format!(" SKILL.md={:x}", fnv(&before.skill)));
        for (name, bytes) in &before.references {
            said.push_str(&format!(" {REFERENCES_DIR}/{name}={:x}", fnv(bytes)));
        }
    }
    said.push('\n');
    said
}

/// The version and fingerprint a stamp says.
fn stamped(said: &str) -> Option<(String, u64)> {
    let (by, wrote) = said.trim().split_once(' ')?;
    Some((by.to_string(), u64::from_str_radix(wrote, 16).ok()?))
}

/// FNV-1a: a fingerprint that is the same on every machine and every Rust,
/// which the standard library's hasher does not promise.
fn fnv(bytes: &[u8]) -> u64 {
    fnv_on(0xcbf2_9ce4_8422_2325, bytes)
}

/// FNV-1a carried on from `hash`, so several pieces make one fingerprint.
fn fnv_on(hash: u64, bytes: &[u8]) -> u64 {
    bytes.iter().fold(hash, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// What happened to one agent.
#[derive(Debug, Clone)]
pub enum Told {
    /// The skill itself was written here.
    Written(PathBuf),
    /// A link was made here, pointing at the shared copy.
    Linked(PathBuf),
    /// This agent reads the shared copy already, and needed nothing.
    Reads(&'static str),
    /// This agent could not be told, and why.
    Refused(&'static str, String),
}

/// Make `at` a link to `hop`, or write the skill there when there is nowhere to
/// point.
fn point(at: &Path, hop: Option<PathBuf>) -> anyhow::Result<Told> {
    if let Some(dir) = at.parent() {
        std::fs::create_dir_all(dir)?;
    }
    // Whatever is there goes first, link or directory. The skill travels with
    // the binary, so what is on disk is never the newer of the two — and a
    // symlink cannot be made over something that already exists.
    let _ = std::fs::remove_file(at);
    let _ = std::fs::remove_dir_all(at);

    match hop {
        Some(hop) => match link(&hop, at) {
            Ok(()) => Ok(Told::Linked(at.to_path_buf())),
            // Windows needs a privilege for symlinks that an ordinary account
            // does not have. A copy is the same skill, just one more file to
            // keep in step — and `deck setup` rewrites all of them anyway.
            Err(_) => {
                let file = at.join("SKILL.md");
                write(&file)?;
                Ok(Told::Written(file))
            }
        },
        None => {
            let file = at.join("SKILL.md");
            write(&file)?;
            Ok(Told::Written(file))
        }
    }
}

/// The link text from a skills directory to the shared copy.
///
/// Both are given relative to the home directory, so this is one `..` per
/// component of `from` and then down to `to`: from `.claude/skills` to
/// `.agents/skills/deck` is `../../.agents/skills/deck`.
///
/// Relative rather than absolute, and written the way the links already on such
/// a machine are written — so a home directory that moves does not break every
/// link at once.
fn pointing(from: &Path, to: &Path) -> PathBuf {
    let mut hop = PathBuf::new();
    for _ in from.components() {
        hop.push("..");
    }
    hop.join(to)
}

/// Make `at` a symbolic link to `hop`, on whichever platform this is.
fn link(hop: &Path, at: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(hop, at)
    }
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_dir(hop, at)
    }
}

/// Write the skill to `at`, its references beside it, and stamp it.
fn write(at: &Path) -> anyhow::Result<()> {
    write_into(at.parent().unwrap_or(Path::new(".")), at)
}

/// Write the references into `dir` and the skill to `at`, making whatever
/// directories they need, and stamp it.
///
/// The references go first and `SKILL.md` last, because agents read the skill
/// while deck runs. Between the two an agent finds the old `SKILL.md`, which
/// names no reference, beside the new references — never a new `SKILL.md`
/// sending it to a file that is not there yet. A reference this deck does not
/// carry is taken away: whatever was in `references/` was deck's, or this copy
/// would not be being written.
///
/// The stamp says `pending` for as long as the write takes, with what each file
/// held before. A write cut off halfway leaves two decks' words mixed, which no
/// fingerprint matches; with the files' old prints the next deck can tell that
/// mix from an edit made since, and finish the one without losing the other.
fn write_into(dir: &Path, at: &Path) -> anyhow::Result<()> {
    if let Some(parent) = at.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let before = on_disk(dir, at);
    replace(&stamp_path(at), pending_stamp(before.as_ref()).as_bytes())?;
    let references = dir.join(REFERENCES_DIR);
    std::fs::create_dir_all(&references)?;
    for (name, text) in REFERENCES {
        replace(&references.join(name), text.as_bytes())?;
    }
    if let Ok(entries) = std::fs::read_dir(&references) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !name.starts_with('.') && !REFERENCES.iter().any(|(ours, _)| *ours == name) {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
    replace(at, SKILL.as_bytes())?;
    let wrote = format!("{RUNNING} {:x}\n", print_of_ours());
    replace(&stamp_path(at), wrote.as_bytes())?;
    Ok(())
}

/// Put `bytes` at `at` all at once.
///
/// Written beside it and renamed over it, because agents start sessions and
/// run deck in parallel. One reading the skill while it was being written in
/// place would get half of it.
fn replace(at: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let name = at.file_name().unwrap_or_default().to_string_lossy();
    let aside = at.with_file_name(format!(".{name}.{}", std::process::id()));
    std::fs::write(&aside, bytes)?;
    std::fs::rename(&aside, at).inspect_err(|_| {
        let _ = std::fs::remove_file(&aside);
    })
}

#[cfg(test)]
mod tests {
    // Spelled out rather than `#[test]`: this crate glob-imports GPUI
    // elsewhere, which exports a `test` attribute of its own.
    use core::prelude::v1::test;

    use super::*;

    /// The whole skill, `SKILL.md` and every reference, as one text: a rule
    /// pinned here is pinned wherever in the skill it lives.
    static ALL: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
        std::iter::once(SKILL)
            .chain(REFERENCES.iter().map(|(_, text)| *text))
            .collect::<Vec<_>>()
            .join("\n")
    });

    fn scratch(name: &str) -> PathBuf {
        let at = std::env::temp_dir().join(format!("deck-skill-{name}"));
        let _ = std::fs::remove_dir_all(&at);
        std::fs::create_dir_all(&at).expect("the scratch directory is writable");
        at
    }

    #[test]
    fn the_skill_says_when_to_use_it() {
        // The description is what a model reads to decide whether this moment
        // is a deck moment, and it is the only part of the file that is read
        // before the decision is made.
        let front = SKILL.split("---").nth(1).expect("the file has frontmatter");
        assert!(front.contains("name: deck"));
        assert!(
            front.contains("description:"),
            "a skill with no description is a skill nothing reaches for"
        );
    }

    #[test]
    fn the_skill_names_every_command_it_needs() {
        // Including the ones for while somebody is in front of the deck. An
        // agent that never learns a verb exists writes walks that sit still,
        // and `deck next` was missing from the file entirely.
        for verb in [
            "deck new",
            "deck group",
            "deck seal",
            "deck open",
            "deck wait",
            "deck next",
            "deck show",
            "deck say",
            "deck bring",
            "deck fold",
            "deck clear",
        ] {
            assert!(ALL.contains(verb), "the skill never mentions `{verb}`");
        }
    }

    #[test]
    fn every_group_the_skill_writes_would_pass_decks_own_check() {
        // The examples are what an agent copies, so they are held to the rule
        // the tool prints when a group is written: points in the narration, and
        // every named pane referred to by name. This is the file marking its
        // own homework, and it has failed it before — thirteen examples wrapped
        // their points in backticks and taught every agent to do the same.
        let mut checked = 0;
        for block in ALL.split("```bash").skip(1) {
            let Some(block) = block.split("```").next() else {
                continue;
            };
            if !block.contains("deck group") || !block.contains("--say") {
                continue;
            }
            // The arguments are single-quoted: the say first, the refs after.
            // A `--title` may come before the say, and names no pane.
            let block = block.find("--say").map_or(block, |at| &block[at..]);
            let quoted: Vec<&str> = block.split('\'').skip(1).step_by(2).collect();
            let Some((say, refs)) = quoted.split_first() else {
                continue;
            };
            assert!(
                say.contains("[point "),
                "an example group points at nothing:\n{say}"
            );
            for one in refs {
                // A `--values` list is JSON and starts with its bracket; it
                // names no pane.
                if one.trim_start().starts_with('[') {
                    continue;
                }
                let Some(open) = one.find('[') else {
                    continue;
                };
                let Some(close) = one[open..].find(']') else {
                    continue;
                };
                let name = &one[open..=open + close];
                assert!(
                    say.contains(name),
                    "an example names a pane {name} its narration never mentions:\n{say}"
                );
            }
            checked += 1;
        }
        assert!(checked >= 2, "only {checked} example groups were found");
    }

    #[test]
    fn the_skill_treats_a_picture_as_a_pane_like_any_other() {
        // Diagrams fell out of use once panes got names and points, because
        // every new rule was written about code and a picture could not be
        // addressed by a sentence at all.
        for said in [
            "A picture is a pane, so it takes a name and a point like any other",
            "One point can hold a file and a picture at once",
            "[point 106-110 checkout]",
            "One file and one picture, never two files",
            "[point q]",
            "--diagram 'connect.json [flow] where the click stops'",
            "When the answer is a shape rather than a file, draw it there and then",
            "--diagram for a picture you write now",
        ] {
            assert!(ALL.contains(said), "the skill never says `{said}`");
        }
    }

    #[test]
    fn the_skill_treats_a_page_as_a_pane_like_any_other() {
        // The same failure as diagrams, one release later: pages shipped and the
        // skill said nothing, so no agent would ever have written one. Every
        // rule a page needs is one an agent gets wrong by default — a document
        // instead of a diagram, an animation on its own clock, a hard-coded
        // colour beside a themed window.
        for said in [
            "## 5. When the answer only makes sense moving",
            "--page 'race.html [race] two workers, one count'",
            "Forty words, and that is the whole rule",
            "It starts at rest, and moves when you point",
            "deck:point",
            "deck:remark",
            "deck-points",
            "--deck-accent",
            "Never write a literal colour",
            "Nothing loads from anywhere",
            "--page for markup that moves",
            "Then ask which panes each group needs",
            "almost always wants a file beside it",
            "If you would have to draw it twice, it is a page",
            "this is the pane that gets forgotten",
            "Draw for the room you have",
            "Never scale text with a `viewBox`",
            "Boxes and arrows are a picture (§4) — always.",
            "Move it so they can follow it",
            "Real numbers, in real units",
            "What pages are for",
            "Move it smoothly",
            "Build once, then only change.",
            "Ease over time, not over frames.",
            "Never animate an HTML element's layout.",
            "Make the thing travel.",
            "It only ever moves forward.",
            "Do it again, in the same place",
            "Every node needs an edge",
        ] {
            assert!(ALL.contains(said), "the skill never says `{said}`");
        }
    }

    #[test]
    fn the_skill_calibrates_the_tone_rather_than_asking_for_enthusiasm() {
        // "Be engaging" produces exclamation marks and forced jokes. What makes
        // a walk worth coming back to is mechanical: the reader arriving first,
        // a name they repeat, momentum between groups, and knowing when to drop
        // all of it because somebody is in trouble.
        for said in [
            "Make it a pleasure, and never a performance",
            "Let them get there first",
            "Give the thing a name they will repeat",
            "Leave the door open between groups",
            "Vary the length",
            "And know when to put it away",
            "would they retell any of it tomorrow",
        ] {
            assert!(ALL.contains(said), "the skill never says `{said}`");
        }
    }

    #[test]
    fn the_skill_draws_beside_the_code_and_builds_in_shells() {
        // From a real deck: asked to explain an event stream, the agent drew
        // one flow and then walked every handler as text. The reader wanted to
        // watch the event move, group after group, and got files.
        for said in [
            "Plan the context: when to draw what",
            "what is the cheapest way to put the right model in",
            "Map first",
            "Walk journeys, not files",
            "Keep a \"you are here\"",
            "Chapters, only when one deck cannot hold it",
            "Agree the chapters first, then walk them one at a time",
            "Never write a later chapter before they have finished the\n     current one",
            "this is the one deck whose bar goes up after the research",
            "It is a reachability question",
            "The bug lives in runtime state",
            "And not a page when nothing moves",
            "And not a page when a thing only moves between places",
            "How to draw each one is §4 and §5",
            "Beside the code that causes it, by default",
            "30 or\n   more lines apart",
            "Keep one drawing across groups",
            "Build it in shells",
            "Shell 1 makes the idea alive",
            "Say which shell you are on, and what is still outside",
            "Shells add no groups",
            "Plain words, active voice, one claim a sentence",
            "Spend the deck where the cases are",
            "One decision at a time",
        ] {
            assert!(ALL.contains(said), "the skill never says `{said}`");
        }
    }

    #[test]
    fn a_page_keeps_its_words_apart() {
        // A Gantt's counter was drawn over its own legend, and its last two
        // times ran into each other, in a pane narrower than it was pictured.
        let flat = ALL.split_whitespace().collect::<Vec<_>>().join(" ");
        for said in [
            "No text touches other text",
            "a legend, a counter and a title each get a band of their own",
            "picture the page at about 400 pixels wide",
        ] {
            assert!(flat.contains(said), "the skill never says `{said}`");
        }
    }

    #[test]
    fn the_skill_offers_a_catalog_of_shapes_by_question() {
        // Left to itself an agent draws boxes, then a Gantt for everything. A
        // menu keyed by the question gets the right shape the first time.
        let flat = ALL.split_whitespace().collect::<Vec<_>>().join(" ");
        for said in [
            "A catalog of shapes that have worked",
            "Pick by the question, not by habit",
            "lanes are for two or more actors, and one actor is one row",
            "Should we make this change?",
            "Why is this number what it is?",
            "Why does the user see an old value?",
            "Some of these have a shape that has already landed with readers",
            "When nothing here fits, name the question first",
            "the code sits beside it (\"Where it goes\", in \"Plan the context\")",
        ] {
            assert!(flat.contains(said), "the skill never says `{said}`");
        }
    }

    #[test]
    fn the_skill_starts_a_deck_where_a_person_was_standing() {
        // The failure this answers, from a real deck: an opening about
        // `useState` and `destinationData` with nothing in it about the
        // importer, the second run, or the screen somebody was looking at. The
        // code is the part the reader can reconstruct; the story is not.
        for said in [
            "Root context first: where somebody was standing",
            "Where they were",
            "What they did",
            "What they wanted, or what they got instead",
            "This matters most when you are about to change something",
            "If you cannot write the story, you do not yet know what the code is for",
            "does the opening say where somebody was standing",
            "And when feeling it is faster than being told",
        ] {
            assert!(ALL.contains(said), "the skill never says `{said}`");
        }
    }

    #[test]
    fn the_skill_says_the_waiter_is_not_optional() {
        // Seen in the wild with more than one agent: groups written, deck
        // sealed, turn ended. The reader walks it, comments, submits — and
        // nothing is listening, so nothing ever comes back.
        for said in [
            "Sealing is not delivering",
            "This step is not optional, and it is the one that gets skipped",
            "Never announce the deck instead of leaving it alone",
            "One listener, and only one",
            "Every way they can reach you comes back the same way",
            "They can answer the sentence, not only the code",
            "One claim per sentence, said plainly enough to be wrong",
            "a process wakes you by ending",
            "Answered a question? Run `deck open` again",
            "This is also how you hear back",
            "Every line of that is required, and the last one most of all",
        ] {
            assert!(ALL.contains(said), "the skill never says `{said}`");
        }
    }

    #[test]
    fn the_skill_ties_the_prose_to_the_panes_and_the_light() {
        // The failure these answer, all seen in real decks: prose that names no
        // pane, panes nothing in the prose refers to, points written inside
        // backticks so an empty chip is left on the page, and a sentence that
        // only parses while the direction is still in it.
        for said in [
            "Panes have names, and the names are how you work",
            "Never place a pane",
            "Name every pane you mention, and mention every pane you name",
            "Keep a name across groups",
            "Write it bare — never in backticks",
            "the sentence has to read with the point gone",
            "Points are not a voice feature",
            "A change is not the subject",
        ] {
            assert!(ALL.contains(said), "the skill never says `{said}`");
        }
    }

    #[test]
    fn the_skill_never_wraps_a_point_in_a_code_chip() {
        // Thirteen of its own examples did, which is where agents learnt it.
        // Only the examples are checked: the prose quotes the syntax to talk
        // about it, and a chip there is typography rather than a direction
        // anybody copies into a `say`.
        let wrapped: usize = ALL
            .split("```bash")
            .skip(1)
            .filter_map(|block| block.split("```").next())
            .map(|block| block.matches("`[point ").count())
            .sum();
        assert_eq!(wrapped, 0, "an example still writes a point in backticks");
    }

    #[test]
    fn the_skill_carries_the_judgement_and_not_only_the_commands() {
        // The commands are the easy half. What separates a deck worth walking
        // from a list of files is the advice — and a skill that only listed
        // verbs would produce the second every time.
        for said in [
            "cite it, show it",
            "one thing you want to say",
            "never just the what",
            "planned a list",
            "must be tight either way",
            "do not over-complicate",
            "Predict before reveal",
            "whiteboard, not reading them a changelog",
            "Story, not a file dump",
        ] {
            assert!(ALL.contains(said), "the skill never says `{said}`");
        }
    }

    #[test]
    fn the_skill_says_what_silence_is_not() {
        // The one place an agent can do real harm: reporting approval that
        // nobody gave.
        assert!(ALL.contains("Never convert an absent review into agreement"));
        assert!(
            ALL.contains("background"),
            "and how to wait without a timeout"
        );
    }

    #[test]
    fn the_skill_teaches_the_order_and_not_only_the_commands() {
        // Both ways the timing goes wrong, and they pull in opposite
        // directions. Batching — research in silence, write four groups, then
        // open — is the one that makes somebody stop using deck. Stalling —
        // open, then go back to reading — leaves them watching a pulsing dot.
        // The file has to name both, or fixing one reintroduces the other.
        assert!(ALL.contains("Do the reading first"));
        assert!(
            ALL.contains("with nothing in the deck yet"),
            "the bar goes up before the research, which is the whole fix"
        );
        assert!(
            ALL.contains("watching you think"),
            "and the other end of it, from the reader's side"
        );
    }

    #[test]
    fn the_skill_shows_a_picture_and_its_code_in_one_group() {
        // A group takes both, and for a flow that is the normal shape. Said in
        // prose it was still read as "a diagram instead of refs", so there is
        // a worked example — which is what a model actually copies.
        assert!(ALL.contains("Put the picture and the code in the same group"));
        assert!(
            ALL.contains("--diagram 'connect.json"),
            "and an example that uses both flags at once"
        );
    }

    #[test]
    fn the_skill_says_a_flow_is_a_picture() {
        // The gap this closes: the skill named `--diagram` once, in a list of
        // flags, and sent the reader to PROTOCOL.md for its shape — a document
        // whose second paragraph says agents do not read it. So a deck
        // answering "what happens when I click this" came back as six code
        // refs and no picture.
        assert!(ALL.contains("A question about a flow is a diagram"));
        assert!(
            ALL.contains("\"nodes\""),
            "and the shape is here, not in a document it was told not to read"
        );
    }

    #[test]
    fn the_diagram_in_the_skill_is_one_deck_can_draw() {
        // An example that does not parse teaches a shape the tool refuses. It
        // is the only part of this file that can be checked rather than read,
        // so it is checked.
        let json = ALL
            .split("```json")
            .find(|block| block.contains("\"nodes\""))
            .and_then(|block| block.split("```").next())
            .expect("the skill shows a diagram");

        let drawn: deck_core::diagram::Diagram =
            serde_json::from_str(json).expect("and it is one deck can read");
        assert_eq!(drawn.nodes.len(), 5);
        assert_eq!(drawn.edges.len(), 4);
    }

    #[test]
    fn the_skill_says_a_range_has_to_be_a_whole_thing() {
        // Ranges that stopped in the middle of the function they pointed at:
        // the reader gets an opening brace and no closing one, and spends
        // their first seconds working out what they are looking at. "Tight
        // ranges" on its own pushed straight into it.
        assert!(ALL.contains("must be tight either way"));
        assert!(
            ALL.contains("signature through closing brace"),
            "and what whole means: signature through closing brace"
        );
    }

    #[test]
    fn the_skill_says_not_to_open_the_deck_when_it_is_finished() {
        // Sealing is not a cue to put a window in somebody's face. The bar is
        // the invitation, and when to read is the reader's to choose — it is
        // almost never the moment the agent happens to finish.
        assert!(ALL.contains("You never open the deck itself"));
        assert!(ALL.contains("Sealing a deck is not a cue to put a window"));
    }

    #[test]
    fn after_an_answer_the_row_is_looked_at_again() {
        // The question just asked is the best evidence of the next one, and a
        // row still offering it word for word was never read again.
        assert!(ALL.contains("### After every answer, look at the row again"));
        assert!(ALL.contains("deck asks <path> --group g3"));
        assert!(ALL.contains("before you go back to `deck wait`"));
        assert!(
            ALL.contains("leave it alone"),
            "and not churned for nothing"
        );
    }

    #[test]
    fn the_skill_says_how_to_explain_and_not_only_what_to_show() {
        // The commands were carried and the teaching was not. Predicting
        // before revealing, and reaching for the reader's own codebase rather
        // than for a generic metaphor, are what separate a deck that explains
        // from one that displays.
        assert!(ALL.contains("Predict before reveal"));
        // The pills under a group: moves the reader makes, read from them.
        assert!(ALL.contains("### Fill the slots"));
        assert!(ALL.contains("--ask"));
        assert!(ALL.contains("Anchor to THEIR system and domain"));
        assert!(
            ALL.contains("at a\n   whiteboard, not reading them a changelog"),
            "and the shape the whole thing should have"
        );
    }

    #[test]
    fn the_trigger_is_something_a_model_can_count() {
        // What this is for: an agent finished investigating a stalled job and
        // typed the root cause into the terminal, seven file:line citations
        // deep, with a timeline and a list of what it had not verified. Every
        // trigger in the description was a judgement call — "about to
        // explain", "about to say line 40" — and judgement loses to momentum
        // at the one moment it is needed, which is right after the work.
        //
        // So the rule is arithmetic. Two citations is a deck. A model can
        // check that against a draft without deciding anything.
        let front = SKILL.split("---").nth(1).expect("the file has frontmatter");
        assert!(front.contains("USE THIS BY DEFAULT"));
        // The count has to be in the *description*, which is what a model
        // reads while deciding. In the body it is read after the decision has
        // already gone the other way.
        assert!(front.contains("TWO OR MORE"));
        assert!(
            front.contains("Do NOT wait to be asked"),
            "asking is the failure this tool exists to prevent"
        );
        assert!(ALL.contains("Build the deck. Do not ask first."));
        // Named situations, because "explain code" is easy to not recognise
        // yourself doing — and because each wants a different shape.
        assert!(ALL.contains("The situations this comes up in"));
        for one in [
            "A PR or a branch review",
            "A root cause",
            "A plan, before you write it",
            "Findings after an investigation",
            // The ones an agent is least likely to recognise as explaining
            // code at all, which is exactly why they are written down.
            "An error or a stack trace",
            "A failing test",
            "Why did you do it that way?",
            "It does not actually do that.",
            "They asked the same thing twice",
        ] {
            assert!(ALL.contains(one), "the skill never names `{one}`");
        }
        assert!(
            ALL.contains("per concern"),
            "and a branch is several decks, not one"
        );
        assert!(
            ALL.contains("do not send the prose version as well"),
            "a summary that carries the argument makes the deck redundant"
        );
    }

    #[test]
    fn the_skill_says_how_to_propose_a_change() {
        // `after` was in the protocol and drawn by the renderer from the
        // beginning, and there was no way to write one: the CLI hardcoded
        // None. So every proposed change came out as a plain highlight with
        // the diff described in the prose, and the reader held it in their
        // head. The flag exists now, and a flag no skill mentions is a flag
        // nothing uses.
        //
        // Then a second failure on top of the first: the flag existed, the
        // skill mentioned it once inside a bullet list, and an agent asked to
        // show what it had changed reached for a highlight every time. Naming
        // the two tools as two tools is the fix.
        for said in [
            "--after",
            "A code pane is a highlight or a diff, and you need both",
            "**A highlight** is `--ref` on its own",
            "**A diff** is `--ref` with `--after` or `--before` behind it",
            "The test is whether you are pointing or proposing",
            "A diff is a proposal, so draw it before you make the edit",
            "Never show it as a highlight",
            "Point at one half at a time",
            "[point +140-143]",
            "reach for `--before`",
            "New code is still a change, and this is the one that gets missed",
            "Empty is the whole trick",
            "keep the range to what is actually new",
            "Putting the old code in `--after`",
        ] {
            assert!(ALL.contains(said), "the skill never says `{said}`");
        }
    }

    #[test]
    fn the_walk_is_for_talking_and_the_work_comes_after() {
        // #36: asked in the conversation to change something, or after
        // locking in a decision, agents went off and edited files mid-walk —
        // and the reader sat in front of a panel that had stopped answering.
        assert!(ALL.contains("The walk is for talking. Do not start the work."));
        assert!(
            ALL.contains("Never change their code."),
            "said as a rule, not a preference"
        );
        assert!(
            ALL.contains("everything you agreed to during the walk"),
            "and what was agreed is carried into the review, not lost"
        );
    }

    #[test]
    fn the_skill_says_who_is_talking() {
        // The complaint that started this: decks read as a narrator rather
        // than a person, so nobody wanted to open a second one. A persona has
        // to be a situation the model can act on rather than an adjective —
        // "you are a great teacher" is inert, "they will interrupt the moment
        // you tell them something they know" is not.
        assert!(ALL.contains("Who is talking"));
        assert!(
            ALL.contains("One flourish per deck"),
            "and a budget, or it performs enthusiasm in every group"
        );
        assert!(
            ALL.contains("Voice never buys correctness"),
            "and it must never outrank the proof rule"
        );
    }

    #[test]
    fn the_skill_gives_a_shape_for_the_first_group() {
        // A header is the claim; the first group is what makes somebody want
        // the second. Naming the shapes that work matters less than naming the
        // three that do not, because those are what a model reaches for.
        assert!(ALL.contains("The first group is the hook"));
        assert!(ALL.contains("show the destination and retreat"));
        assert!(
            ALL.contains("Put the hard part in the middle, never last"),
            "a deck that saves its idea for the end spent the walk on nothing"
        );
    }

    #[test]
    fn the_skill_can_tell_a_dry_sentence_from_a_live_one() {
        // The mechanical test is the point. "Be specific" is unfollowable;
        // "would this still be true after swapping three nouns" is something a
        // model can run on its own draft.
        assert!(ALL.contains("swapping three nouns"));
        assert!(ALL.contains("What dry looks like"));
    }

    #[test]
    fn the_skill_keeps_one_name_on_the_thing_that_moves() {
        // The haziness rule. A paragraph that renames its subject every
        // sentence is describing the system rather than following the thing
        // through it, and the reader has nothing to hold. The "it" test is the
        // mechanical half — a model can run it on its own draft.
        assert!(ALL.contains("One thing travels, and it keeps its name"));
        assert!(ALL.contains("name the travelling thing once, and do not rename it"));
        assert!(
            ALL.contains(r#"If "it" in your third"#),
            "and the check that catches it"
        );
        assert!(ALL.contains("read your subjects down the page"));
    }

    #[test]
    fn the_skill_tells_an_agent_how_to_answer_live() {
        // The loop is `deck wait` returning a question rather than a review,
        // and an agent that does not know that sits waiting for a submit that
        // will not come until the conversation is over.
        assert!(ALL.contains("When they answer back"));
        assert!(ALL.contains(r#"{"asked": …}"#));
        assert!(
            ALL.contains("Answer fast"),
            "somebody is watching a panel with a pulse in it"
        );
        assert!(
            ALL.contains("Two or three sentences"),
            "they are listening, and cannot skim"
        );
        assert!(ALL.contains("subagent"), "hand off the slow part");
        // The default changed from *held for the review* to *interrupt*, on
        // purpose (#14). What this pinned survives it: a remark held for the
        // review still never wakes the agent, so silence still means reading.
        assert!(
            ALL.contains("A remark interrupts you unless the reader says otherwise"),
            "the default is said where an agent will read it"
        );
        // Read across the line breaks, so rewrapping the paragraph is not a
        // failure and losing the sentence is.
        let flat = ALL.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(
            flat.contains("a quiet waiter is a reader reading, not a reader with nothing to say"),
            "and silence is still a reader reading"
        );
    }

    #[test]
    fn the_skill_asks_for_the_answer_and_no_running_commentary() {
        // The agent used to be told to send a note before every step, so the
        // panel had something to show. It made answers slower to say they
        // were coming (#23). The panel shows a pulse by itself now, and the
        // skill must not send an agent back to narrating its own work.
        assert!(ALL.contains("Do not report on your progress."));
        assert!(
            !ALL.contains("deck doing"),
            "the verb still exists for old agents, and is no longer taught"
        );
        assert!(
            !ALL.contains("no answer yet"),
            "a label deck no longer shows"
        );
    }

    #[test]
    fn the_skill_names_panes_instead_of_placing_them() {
        // "The left pane" is decided by the window's width after the sentence
        // was written, and was wrong often enough that a reader filed it.
        assert!(ALL.contains("Name panes; never place them"));
        assert!(ALL.contains("[retry]"));
        assert!(
            !ALL.contains("in the second pane"),
            "the skill's own example must not do the thing it forbids"
        );
    }

    #[test]
    fn the_skill_says_how_to_explain_while_pointing() {
        // The first answers given with pointing read the code out a line at a
        // time, moved the light every sentence, and answered "again" by saying
        // the same thing slower. A reader called it garbage, and it was.
        assert!(ALL.contains("Do not read the code out"));
        assert!(ALL.contains("Start with the problem, in their world"));
        assert!(
            ALL.contains("The idea and the code, together"),
            "an analogy that never names the code leaves the reader nothing to find"
        );
        assert!(
            ALL.contains("Every point has to earn itself"),
            "a light with nothing said under it is noise"
        );
        assert!(
            ALL.contains("If you cannot say what is under the light, do not move the light"),
            "the rule needs the line that makes it a rule"
        );
        assert!(
            ALL.contains("Two tests, before the light moves"),
            "a rule an agent cannot check while writing is a wish"
        );
        assert!(ALL.contains("not one for each sentence"));
        assert!(ALL.contains("Never spell code aloud"));
        assert!(
            ALL.contains("say what it means to do"),
            "not spelling code must never become not explaining it"
        );
        assert!(
            ALL.contains("means the way in was wrong, not the speed"),
            "a repeated question is a signal to change angle"
        );
        assert!(ALL.contains("One `deck say` for each answer"));
        assert!(
            ALL.contains("bring it in"),
            "an agent that thinks it is stuck with the authored panes will refuse the question"
        );
        assert!(
            ALL.contains("When the answer is code, show the code"),
            "an alternative described in sentences is homework"
        );
    }

    #[test]
    fn the_skill_teaches_the_agent_to_point_as_it_talks() {
        // Without this an agent moves the whole spotlight for every sentence,
        // which takes the code out from under the reader, or it moves nothing
        // and narrates twenty lines with no way of saying which one it means.
        assert!(ALL.contains("[point 106-110]"));
        assert!(
            ALL.contains("not when you sent the command"),
            "or an agent will expect the light to follow its own pace"
        );
        assert!(
            ALL.contains("the file's own line numbers"),
            "counting from the top of the shown range is the obvious wrong guess"
        );
    }

    #[test]
    fn the_skill_teaches_the_beats_a_voice_understands() {
        // Written in the prose and never drawn: deck takes them out of the band
        // and hands them to the voice, which judges each length from context.
        // An agent that does not know they exist writes prose that is read flat.
        assert!(ALL.contains("Write it so it can be heard"));
        assert!(ALL.contains("[pause long]"));
        assert!(
            ALL.contains("The reader never sees them"),
            "or an agent will avoid them, thinking they show"
        );
        assert!(
            ALL.contains("not one per sentence"),
            "a beat everywhere is the same as a beat nowhere"
        );
    }

    #[test]
    fn the_skill_does_not_promise_a_tone_control() {
        // Chirp 3 has pace, pauses and pronunciation — and no way to ask for a
        // style. Telling an agent otherwise would have it writing directions
        // nothing reads.
        assert!(ALL.contains("There is no way to ask for a\ntone"));
        assert!(ALL.contains("Do not write stage directions"));
    }

    #[test]
    fn the_skill_asks_for_a_sentence_that_adds_nothing() {
        // "Too fast" is not word count — it is never being allowed to put your
        // weight down. Every few sentences, one that lands what is already
        // there rather than adding to it.
        assert!(ALL.contains("Give them somewhere to stand"));
        assert!(ALL.contains("write one that adds nothing"));
    }

    #[test]
    fn the_skill_sends_them_to_reproduce_a_serious_one() {
        // The only check in a deck that does not depend on the agent being
        // right: the reader runs it themselves and the code answers. Guarded so
        // it stays rare — a reproduction on every group is homework.
        assert!(ALL.contains("When it is serious, send them to go and look"));
        assert!(ALL.contains("their world, not yours"));
        assert!(
            ALL.contains("Say the expected result"),
            "a reproduction that does not reproduce is information too"
        );
        assert!(
            ALL.contains("Not\nevery group and not most groups"),
            "the guard against doing it every time"
        );
    }

    #[test]
    fn the_skill_makes_the_groups_join_up() {
        // Grounding, and the connector test. A deck whose groups join with
        // "and then" is a list; one that joins with "but" is an argument.
        assert!(ALL.contains("grounded"));
        assert!(ALL.contains("AND THEN"));
        assert!(
            ALL.contains("at least one BUT"),
            "if nothing surprised you, you have not found the story"
        );
    }

    #[test]
    fn the_worked_example_is_a_group_worth_copying() {
        // Models copy examples over prose. Every `--say` here used to be one
        // short sentence, which taught thin narration on every example the
        // file gave — while the prose asked for depth.
        assert!(
            ALL.contains("a pane has no width"),
            "the example shows a real group, not a placeholder"
        );
        assert!(ALL.contains("A `say` of one sentence is almost always a group"));
    }

    #[test]
    fn an_agent_is_found_by_its_own_directory() {
        // Not by its skills directory: one that has never been given a skill
        // has no `skills/` yet, and that is exactly the one worth telling.
        let home = scratch("found");
        std::fs::create_dir_all(home.join(".claude")).unwrap();

        let agents = found(&home);
        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0].name, "Claude Code");
    }

    #[test]
    fn an_agent_that_reads_the_shared_store_is_left_alone() {
        // Amp lists everything in `~/.agents/skills` without a link. A link
        // would be a second copy in front of the one it already reads.
        let home = scratch("reads");
        std::fs::create_dir_all(home.join(".agents/skills")).unwrap();
        std::fs::create_dir_all(home.join(".amp")).unwrap();

        let told = install_all(&home).unwrap();
        assert!(matches!(told[1], Told::Reads("Amp")));
        assert!(
            !home.join(".config/agents/skills/deck").exists(),
            "and nothing of its own was made"
        );
    }

    #[test]
    fn without_a_shared_store_it_gets_one_of_its_own() {
        let home = scratch("amp-alone");
        std::fs::create_dir_all(home.join(".amp")).unwrap();

        install_all(&home).unwrap();
        assert!(home.join(".config/agents/skills/deck/SKILL.md").is_file());
    }

    #[test]
    fn one_real_copy_and_a_link_from_each_agent() {
        // The convention those directories already keep. Five copies of a
        // skill is five chances to be out of date.
        let home = scratch("shared");
        std::fs::create_dir_all(home.join(".agents/skills")).unwrap();
        std::fs::create_dir_all(home.join(".claude/skills")).unwrap();
        std::fs::create_dir_all(home.join(".codex")).unwrap();

        let told = install_all(&home).expect("a skill is written");
        assert_eq!(told.len(), 3, "the shared copy, and one link each");

        let real = home.join(".agents/skills/deck/SKILL.md");
        assert_eq!(std::fs::read_to_string(&real).unwrap(), SKILL);

        for agent in [".claude/skills/deck", ".codex/skills/deck"] {
            let link = home.join(agent);
            assert!(link.symlink_metadata().unwrap().is_symlink(), "{agent}");
            assert_eq!(
                std::fs::read_to_string(link.join("SKILL.md")).unwrap(),
                SKILL,
                "{agent} reads the shared copy"
            );
        }
    }

    #[test]
    fn a_link_is_written_the_way_the_others_are() {
        // Relative, so a home directory that moves does not break every link
        // at once — and so it reads the same as the links already beside it.
        assert_eq!(
            pointing(
                Path::new(".claude/skills"),
                Path::new(".agents/skills/deck")
            ),
            PathBuf::from("../../.agents/skills/deck")
        );
        assert_eq!(
            pointing(
                Path::new(".cursor/skills-cursor"),
                Path::new(".agents/skills/deck")
            ),
            PathBuf::from("../../.agents/skills/deck")
        );
    }

    #[test]
    fn without_a_shared_store_each_agent_gets_a_real_copy() {
        // A link into a directory nobody else uses would be a link to nothing.
        let home = scratch("copies");
        std::fs::create_dir_all(home.join(".claude")).unwrap();

        install_all(&home).unwrap();
        let at = home.join(".claude/skills/deck/SKILL.md");
        assert!(at.is_file());
        assert!(
            !home
                .join(".claude/skills/deck")
                .symlink_metadata()
                .unwrap()
                .is_symlink()
        );
        assert_eq!(std::fs::read_to_string(at).unwrap(), SKILL);
    }

    #[test]
    fn an_agent_keeps_the_skills_directory_it_already_has() {
        // Cursor calls it `skills-cursor`. Making a second one beside it would
        // be a directory nothing reads.
        let home = scratch("cursor");
        std::fs::create_dir_all(home.join(".cursor/skills-cursor")).unwrap();

        let cursor = AGENTS
            .iter()
            .find(|agent| agent.name == "Cursor")
            .expect("Cursor is one deck knows");
        assert_eq!(cursor.skill_dir(&home), home.join(".cursor/skills-cursor"));
    }

    #[test]
    fn telling_again_replaces_what_was_there() {
        // The skill travels with the binary, so the one in the binary is always
        // the newer of the two. A deck upgraded past a skill describing its old
        // commands is worse than a deck with no skill.
        let home = scratch("replace");
        std::fs::create_dir_all(home.join(".claude/skills")).unwrap();

        install_all(&home).unwrap();
        std::fs::write(home.join(".claude/skills/deck/SKILL.md"), "older").unwrap();

        install_all(&home).unwrap();
        assert_eq!(
            std::fs::read_to_string(home.join(".claude/skills/deck/SKILL.md")).unwrap(),
            SKILL
        );
    }

    /// What 0.0.1 would have left at `at`: its own words, and its stamp, and
    /// no references, which no deck that old wrote.
    fn an_older_deck_wrote(at: &Path) {
        let _ = std::fs::remove_dir_all(at.with_file_name(REFERENCES_DIR));
        std::fs::write(at, "older").unwrap();
        std::fs::write(stamp_path(at), format!("0.0.1 {:x}\n", fnv(b"older"))).unwrap();
    }

    #[test]
    fn an_upgraded_deck_brings_the_skill_it_installed_with_it() {
        // The upgrade that left an agent reading a skill with no `ask` in it.
        // Shared store, two linked agents, and the shared copy left behind at
        // an older version.
        let home = scratch("keep-up");
        std::fs::create_dir_all(home.join(".agents/skills")).unwrap();
        std::fs::create_dir_all(home.join(".claude/skills")).unwrap();
        std::fs::create_dir_all(home.join(".codex")).unwrap();
        install_all(&home).unwrap();
        let shared = home.join(".agents/skills/deck/SKILL.md");
        an_older_deck_wrote(&shared);

        assert_eq!(
            keep_up(&home),
            vec![std::fs::canonicalize(&shared).unwrap()],
            "written once, through no link"
        );
        for at in [&shared, &home.join(".claude/skills/deck/SKILL.md")] {
            assert_eq!(std::fs::read_to_string(at).unwrap(), SKILL);
        }
        assert!(keep_up(&home).is_empty(), "and left alone once it matches");
    }

    #[test]
    fn keeping_up_rewrites_real_copies_and_installs_nothing_new() {
        // Copies, as on a machine with no shared store. Codex has one; Cursor
        // is on the machine and was never given one, which is somebody's no.
        let home = scratch("keep-up-copies");
        std::fs::create_dir_all(home.join(".codex")).unwrap();
        install_all(&home).unwrap();
        std::fs::create_dir_all(home.join(".cursor")).unwrap();
        let codex = home.join(".codex/skills/deck/SKILL.md");
        an_older_deck_wrote(&codex);

        assert_eq!(keep_up(&home), vec![std::fs::canonicalize(&codex).unwrap()]);
        assert_eq!(std::fs::read_to_string(&codex).unwrap(), SKILL);
        assert!(
            !home.join(".cursor/skills/deck").exists(),
            "no skill behind a no"
        );
        assert!(
            !home.join(".agents/skills/deck").exists(),
            "nor a shared store"
        );
    }

    #[test]
    fn a_skill_somebody_edited_is_theirs() {
        let home = scratch("keep-up-edited");
        std::fs::create_dir_all(home.join(".claude")).unwrap();
        install_all(&home).unwrap();
        let at = home.join(".claude/skills/deck/SKILL.md");
        std::fs::write(&at, "mine now").unwrap();

        assert!(keep_up(&home).is_empty());
        assert_eq!(std::fs::read_to_string(&at).unwrap(), "mine now");
        assert!(
            !current(&home),
            "and setup still has something to ask about"
        );
    }

    #[test]
    fn an_older_deck_does_not_drag_the_skill_back() {
        // What a newer deck wrote, as this deck finds it: different words,
        // stamped by a version past this one.
        let home = scratch("keep-up-newer");
        std::fs::create_dir_all(home.join(".claude")).unwrap();
        install_all(&home).unwrap();
        let at = home.join(".claude/skills/deck/SKILL.md");
        let newer = "the skill of a deck still to come";
        std::fs::write(&at, newer).unwrap();
        std::fs::write(
            stamp_path(&at),
            format!("99.0.0 {:x}\n", fnv(newer.as_bytes())),
        )
        .unwrap();

        assert!(keep_up(&home).is_empty());
        assert_eq!(std::fs::read_to_string(&at).unwrap(), newer);
    }

    #[test]
    fn a_skill_from_before_stamps_is_taken_as_decks_and_stamped() {
        // Every machine set up by 0.1.5 or earlier.
        let home = scratch("keep-up-unstamped");
        std::fs::create_dir_all(home.join(".claude")).unwrap();
        install_all(&home).unwrap();
        let at = home.join(".claude/skills/deck/SKILL.md");
        std::fs::write(&at, "older").unwrap();
        std::fs::remove_file(stamp_path(&at)).unwrap();

        assert_eq!(keep_up(&home).len(), 1);
        assert_eq!(std::fs::read_to_string(&at).unwrap(), SKILL);
        assert_eq!(stamp(&at), Some((RUNNING.to_string(), print_of_ours())));
        assert!(current(&home));
    }

    #[cfg(unix)]
    #[test]
    fn a_skill_file_that_is_a_link_stays_a_link() {
        // Dotfiles kept elsewhere and linked in, file by file.
        let home = scratch("keep-up-file-link");
        let kept = home.join("dotfiles/SKILL.md");
        std::fs::create_dir_all(kept.parent().unwrap()).unwrap();
        std::fs::write(&kept, "older").unwrap();
        let dir = home.join(".claude/skills/deck");
        std::fs::create_dir_all(&dir).unwrap();
        std::os::unix::fs::symlink(&kept, dir.join("SKILL.md")).unwrap();

        keep_up(&home);
        assert!(
            dir.join("SKILL.md")
                .symlink_metadata()
                .unwrap()
                .is_symlink()
        );
        assert_eq!(std::fs::read_to_string(&kept).unwrap(), SKILL);
    }

    #[test]
    fn a_skill_already_current_is_stamped_so_a_later_edit_is_seen() {
        let home = scratch("keep-up-restamp");
        std::fs::create_dir_all(home.join(".claude")).unwrap();
        install_all(&home).unwrap();
        let at = home.join(".claude/skills/deck/SKILL.md");
        std::fs::remove_file(stamp_path(&at)).unwrap();

        assert!(keep_up(&home).is_empty(), "nothing to rewrite");
        assert!(stamp(&at).is_some(), "but now stamped");
        std::fs::write(&at, "mine now").unwrap();
        keep_up(&home);
        assert_eq!(std::fs::read_to_string(&at).unwrap(), "mine now");
    }

    #[test]
    fn two_builds_of_one_version_do_not_trade_the_skill() {
        let home = scratch("keep-up-same-version");
        std::fs::create_dir_all(home.join(".claude")).unwrap();
        install_all(&home).unwrap();
        let at = home.join(".claude/skills/deck/SKILL.md");
        let other = "the same version, built from other words";
        std::fs::write(&at, other).unwrap();
        std::fs::write(
            stamp_path(&at),
            format!("{RUNNING} {:x}\n", fnv(other.as_bytes())),
        )
        .unwrap();

        assert!(keep_up(&home).is_empty());
        assert_eq!(std::fs::read_to_string(&at).unwrap(), other);
    }

    #[test]
    fn a_stamp_that_will_not_read_leaves_the_skill_alone() {
        let home = scratch("keep-up-garbage");
        std::fs::create_dir_all(home.join(".claude")).unwrap();
        install_all(&home).unwrap();
        let at = home.join(".claude/skills/deck/SKILL.md");
        std::fs::write(&at, "mine").unwrap();
        std::fs::write(stamp_path(&at), "not a stamp").unwrap();

        assert!(keep_up(&home).is_empty());
        assert_eq!(std::fs::read_to_string(&at).unwrap(), "mine");
    }

    #[test]
    fn the_pills_are_the_readers_moves_and_never_their_decision() {
        // Measured against what one reader actually typed into 81 groups:
        // the pills that landed were moves (say it simpler, are you sure,
        // does the other one do this too), named after the exact thing on
        // screen. And a pill that decides for them takes away the one thing
        // deck exists to leave with the reader.
        assert!(ALL.contains("two of them are what the reader was about to type"));
        assert!(ALL.contains("### Read the reader first"));
        assert!(ALL.contains("**Never the decision.**"));
        assert!(ALL.contains("Doubt the claim."));
        let mut examples = 0;
        for example in ALL
            .lines()
            .filter_map(|line| line.trim().strip_prefix("--ask \""))
        {
            let ask = example.split('"').next().unwrap_or_default();
            assert!(
                ask.chars().count() <= 60,
                "{ask} is longer than it tells the agent to be"
            );
            assert!(
                !ask.contains('`'),
                "{ask}: a backtick inside double quotes runs in the shell"
            );
            for decision in ["Go with", "Lock it", "Approve", "would you pick"] {
                assert!(!ask.contains(decision), "{ask}: a pill never decides");
            }
            examples += 1;
        }
        assert!(examples >= 3, "the method shows a whole row of pills");
    }

    #[test]
    fn the_skill_md_stays_short_enough_to_survive_a_compaction() {
        // It was 2,551 lines, all of it read on every use and carried for the
        // whole session. A compaction keeps the first 5,000 tokens of a skill,
        // which ended a third of the way through the voice sections: the loop,
        // the waiter and the review were the part a long session lost. The
        // guidance for skills is under 500 lines; the 20,000 bytes is the
        // 5,000 tokens, at about four bytes a token.
        assert!(
            SKILL.lines().count() < 500,
            "SKILL.md is {} lines; move detail into references/",
            SKILL.lines().count()
        );
        assert!(
            SKILL.len() < 20_000,
            "SKILL.md is {} bytes, more than a compaction keeps",
            SKILL.len()
        );
    }

    #[test]
    fn every_reference_is_named_where_it_is_read() {
        // A reference SKILL.md never names is one no agent opens, and a name
        // with no file behind it sends an agent to read nothing.
        for (name, _) in REFERENCES {
            assert!(
                SKILL.contains(&format!("(references/{name})")),
                "SKILL.md never sends an agent to {name}"
            );
        }
        for linked in SKILL.split("(references/").skip(1) {
            let name = linked.split(')').next().unwrap_or_default();
            assert!(
                REFERENCES.iter().any(|(ours, _)| *ours == name),
                "SKILL.md names references/{name}, which deck does not carry"
            );
        }
        let mut names: Vec<_> = REFERENCES.iter().map(|(name, _)| *name).collect();
        names.sort_unstable();
        assert_eq!(
            names,
            REFERENCES.iter().map(|(name, _)| *name).collect::<Vec<_>>(),
            "kept in name order, which is the order they are read from disk in"
        );
    }

    #[test]
    fn a_long_reference_opens_with_its_contents() {
        // An agent may read the top of a long file and stop. The contents is
        // what tells it what else is in there.
        for (name, text) in REFERENCES {
            if text.lines().count() > 100 {
                assert!(text.contains("## Contents"), "{name} has no contents");
            }
        }
    }

    #[test]
    fn the_references_are_written_beside_the_skill() {
        let home = scratch("references");
        std::fs::create_dir_all(home.join(".agents/skills")).unwrap();
        std::fs::create_dir_all(home.join(".claude/skills")).unwrap();
        install_all(&home).unwrap();

        for dir in [".agents/skills/deck", ".claude/skills/deck"] {
            for (name, text) in REFERENCES {
                let at = home.join(dir).join("references").join(name);
                assert_eq!(&std::fs::read_to_string(&at).unwrap(), text, "{dir}/{name}");
            }
        }
        assert!(current(&home));
    }

    #[test]
    fn a_reference_a_newer_deck_dropped_is_taken_away() {
        let home = scratch("references-stale");
        std::fs::create_dir_all(home.join(".claude")).unwrap();
        install_all(&home).unwrap();
        let dir = home.join(".claude/skills/deck");
        let gone = dir.join("references/gone.md");
        std::fs::write(&gone, "a reference this deck no longer carries").unwrap();
        // Stamped by an older deck, as if it had written that file.
        let at = dir.join("SKILL.md");
        std::fs::write(&at, "older").unwrap();
        let was = on_disk(&dir, &at).unwrap();
        std::fs::write(stamp_path(&at), format!("0.0.1 {:x}\n", was.print())).unwrap();

        assert_eq!(keep_up(&home).len(), 1);
        assert!(!gone.exists());
        assert!(current(&home));
    }

    #[test]
    fn a_reference_somebody_edited_makes_the_copy_theirs() {
        let home = scratch("references-edited");
        std::fs::create_dir_all(home.join(".claude")).unwrap();
        install_all(&home).unwrap();
        let mine = home.join(".claude/skills/deck/references/writing.md");
        std::fs::write(&mine, "mine now").unwrap();
        // And an older deck's SKILL.md beside it, stamped as that deck
        // stamps: the file alone, so the edited reference does not match.
        let at = home.join(".claude/skills/deck/SKILL.md");
        std::fs::write(&at, "older").unwrap();
        std::fs::write(stamp_path(&at), format!("0.0.1 {:x}\n", fnv(b"older"))).unwrap();

        assert!(keep_up(&home).is_empty());
        assert_eq!(std::fs::read_to_string(&mine).unwrap(), "mine now");
        assert!(!current(&home));
    }

    #[test]
    fn a_skill_from_before_references_is_brought_up_to_date() {
        // Every machine set up by 0.1.7-rc.1 or earlier: one file, stamped
        // with the fingerprint of that file alone.
        let home = scratch("references-upgrade");
        std::fs::create_dir_all(home.join(".claude")).unwrap();
        install_all(&home).unwrap();
        let dir = home.join(".claude/skills/deck");
        std::fs::remove_dir_all(dir.join("references")).unwrap();
        an_older_deck_wrote(&dir.join("SKILL.md"));

        assert_eq!(keep_up(&home).len(), 1);
        assert!(current(&home), "SKILL.md and every reference");
    }

    #[test]
    fn a_write_cut_off_halfway_is_finished_next_time() {
        // The stamp said the write had begun and what each file held, one
        // reference went down, then it stopped: two decks' words, matching no
        // fingerprint.
        let home = scratch("references-cut-off");
        std::fs::create_dir_all(home.join(".claude")).unwrap();
        install_all(&home).unwrap();
        let dir = home.join(".claude/skills/deck");
        let at = dir.join("SKILL.md");
        std::fs::write(&at, "older").unwrap();
        let old = fnv(b"older");
        std::fs::write(
            stamp_path(&at),
            format!("0.0.1 pending SKILL.md={old:x} references/writing.md={old:x}\n"),
        )
        .unwrap();

        assert_eq!(keep_up(&home).len(), 1);
        assert!(current(&home));
    }

    #[test]
    fn a_write_says_it_has_begun_before_its_first_file() {
        // A references/ that cannot be made stops the write after the stamp:
        // what is left must say a write began, and what was there before it.
        let home = scratch("references-pending");
        std::fs::create_dir_all(home.join(".claude")).unwrap();
        install_all(&home).unwrap();
        let dir = home.join(".claude/skills/deck");
        let at = dir.join("SKILL.md");
        an_older_deck_wrote(&at);
        std::fs::write(dir.join("references"), "not a directory").unwrap();

        assert!(keep_up(&home).is_empty(), "the write cannot finish");
        let said = std::fs::read_to_string(stamp_path(&at)).unwrap();
        let pending = pending(&said).expect("the stamp says a write began");
        assert_eq!(pending.by, RUNNING);
        assert_eq!(pending.was, vec![("SKILL.md".to_string(), fnv(b"older"))]);
    }

    #[test]
    fn an_edit_made_after_a_write_was_cut_off_is_theirs() {
        let home = scratch("references-pending-edited");
        std::fs::create_dir_all(home.join(".claude")).unwrap();
        install_all(&home).unwrap();
        let at = home.join(".claude/skills/deck/SKILL.md");
        std::fs::write(&at, "mine now").unwrap();
        std::fs::write(
            stamp_path(&at),
            format!("0.0.1 pending SKILL.md={:x}\n", fnv(b"older")),
        )
        .unwrap();

        assert!(keep_up(&home).is_empty());
        assert_eq!(std::fs::read_to_string(&at).unwrap(), "mine now");
    }

    #[test]
    fn a_newer_decks_write_is_left_to_it() {
        // Cut off or finished: a newer deck's copy is not this deck's to
        // finish or to take back, even one missing a reference this deck has.
        let home = scratch("references-newer");
        std::fs::create_dir_all(home.join(".claude")).unwrap();
        install_all(&home).unwrap();
        let dir = home.join(".claude/skills/deck");
        let at = dir.join("SKILL.md");
        std::fs::remove_file(dir.join("references/pages.md")).unwrap();
        let was = on_disk(&dir, &at).unwrap();
        for said in [
            "99.0.0 pending\n".to_string(),
            format!("99.0.0 {:x}\n", was.print()),
        ] {
            std::fs::write(stamp_path(&at), said).unwrap();
            assert!(keep_up(&home).is_empty());
            assert!(!dir.join("references/pages.md").exists());
        }
    }

    #[test]
    fn a_reference_added_beside_an_unstamped_skill_is_theirs() {
        // Decks from before stamps wrote no references.
        let home = scratch("references-unstamped");
        std::fs::create_dir_all(home.join(".claude")).unwrap();
        install_all(&home).unwrap();
        let dir = home.join(".claude/skills/deck");
        let at = dir.join("SKILL.md");
        std::fs::write(&at, "older").unwrap();
        std::fs::remove_file(stamp_path(&at)).unwrap();
        std::fs::write(dir.join("references/mine.md"), "mine").unwrap();

        assert!(keep_up(&home).is_empty());
        assert!(dir.join("references/mine.md").exists());
    }

    #[test]
    fn an_upgrade_that_stopped_after_the_references_is_finished() {
        // A deck from before references: its SKILL.md and its stamp, and this
        // deck's references beside them. An older `deck setup` run after this
        // deck leaves the same.
        let home = scratch("references-stopped");
        std::fs::create_dir_all(home.join(".claude")).unwrap();
        install_all(&home).unwrap();
        let at = home.join(".claude/skills/deck/SKILL.md");
        std::fs::write(&at, "older").unwrap();
        std::fs::write(stamp_path(&at), format!("0.0.1 {:x}\n", fnv(b"older"))).unwrap();

        assert_eq!(keep_up(&home).len(), 1);
        assert!(current(&home));
    }

    #[cfg(unix)]
    #[test]
    fn two_agents_linking_one_skill_file_both_get_the_references() {
        // The stamp sits beside the one file and is written with the first
        // agent. The second finds this deck's SKILL.md and no references.
        let home = scratch("references-file-links");
        let kept = home.join("dotfiles/SKILL.md");
        std::fs::create_dir_all(kept.parent().unwrap()).unwrap();
        std::fs::write(&kept, "older").unwrap();
        for agent in [".claude/skills/deck", ".codex/skills/deck"] {
            let dir = home.join(agent);
            std::fs::create_dir_all(&dir).unwrap();
            std::os::unix::fs::symlink(&kept, dir.join("SKILL.md")).unwrap();
        }

        keep_up(&home);
        for agent in [".claude/skills/deck", ".codex/skills/deck"] {
            let dir = home.join(agent);
            assert!(
                dir.join("SKILL.md")
                    .symlink_metadata()
                    .unwrap()
                    .is_symlink()
            );
            assert!(
                on_disk(&dir, &dir.join("SKILL.md")).unwrap().is_ours(),
                "{agent} has every reference"
            );
        }
        assert_eq!(stamp(&kept), Some((RUNNING.to_string(), print_of_ours())));
    }

    #[test]
    fn a_directory_somebody_added_to_the_references_makes_the_copy_theirs() {
        let home = scratch("references-directory");
        std::fs::create_dir_all(home.join(".claude")).unwrap();
        install_all(&home).unwrap();
        let dir = home.join(".claude/skills/deck");
        let at = dir.join("SKILL.md");
        std::fs::write(&at, "older").unwrap();
        std::fs::create_dir_all(dir.join("references/mine")).unwrap();
        let was = on_disk(&dir, &at).unwrap();
        std::fs::write(stamp_path(&at), format!("0.0.1 {:x}\n", fnv(b"older"))).unwrap();

        assert!(!was.is_ours());
        assert!(keep_up(&home).is_empty());
        assert!(dir.join("references/mine").is_dir());
        assert!(!current(&home));
    }

    #[test]
    fn a_value_the_narration_names_goes_on_its_line() {
        // A reader asked to have a plan explained "using values", and the
        // answer named 42, Acme and opt-9 in a paragraph while every line they
        // belonged to sat bare beside it. The rule said values must be
        // observed, so an example was kept in prose; an example said as one
        // now goes on the lines.
        let flat = ALL.split_whitespace().collect::<Vec<_>>().join(" ");
        for said in [
            "Your own narration names a value.",
            "That is a request for values on the lines, every time",
            "Only values you have, or an example you said is one.",
            "An example is the exception, and it is the commonest case in a plan.",
            "Never mix an example's values and observed ones in one group.",
            "When the answer is a value, put it on the line.",
        ] {
            assert!(flat.contains(said), "the skill never says `{said}`");
        }
    }
}
