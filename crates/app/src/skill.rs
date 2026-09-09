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
        Some(hop) => {
            std::os::unix::fs::symlink(hop, at)?;
            Ok(Told::Linked(at.to_path_buf()))
        }
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

/// Write the skill to `at`, making whatever directory it needs.
fn write(at: &Path) -> anyhow::Result<()> {
    if let Some(dir) = at.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(at, SKILL)?;
    Ok(())
}
