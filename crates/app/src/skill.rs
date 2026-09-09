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

/// Write the skill to `at`, making whatever directory it needs.
fn write(at: &Path) -> anyhow::Result<()> {
    if let Some(dir) = at.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(at, SKILL)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    // Spelled out rather than `#[test]`: this crate glob-imports GPUI
    // elsewhere, which exports a `test` attribute of its own.
    use core::prelude::v1::test;

    use super::*;

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
        for verb in [
            "deck new",
            "deck group",
            "deck seal",
            "deck open",
            "deck wait",
        ] {
            assert!(SKILL.contains(verb), "the skill never mentions `{verb}`");
        }
    }

    #[test]
    fn the_skill_carries_the_judgement_and_not_only_the_commands() {
        // The commands are the easy half. What separates a deck worth walking
        // from a list of files is the advice — and a skill that only listed
        // verbs would produce the second every time.
        for said in [
            "cite it, show it",
            "one thing you are saying",
            "not just the what",
            "planned a list",
            "Tight ranges",
            "Plain words",
            "Open at the surprise",
            "never clever",
            "read it back",
        ] {
            assert!(SKILL.contains(said), "the skill never says `{said}`");
        }
    }

    #[test]
    fn the_skill_says_what_silence_is_not() {
        // The one place an agent can do real harm: reporting approval that
        // nobody gave.
        assert!(SKILL.contains("Never turn that into agreement"));
        assert!(
            SKILL.contains("background"),
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
        assert!(SKILL.contains("Do these in order"));
        assert!(
            SKILL.contains("with nothing in the deck yet"),
            "the bar goes up before the research, which is the whole fix"
        );
        assert!(
            SKILL.contains("watching you think"),
            "and the other end of it, from the reader's side"
        );
    }

    #[test]
    fn the_skill_shows_a_picture_and_its_code_in_one_group() {
        // A group takes both, and for a flow that is the normal shape. Said in
        // prose it was still read as "a diagram instead of refs", so there is
        // a worked example — which is what a model actually copies.
        assert!(SKILL.contains("Put the picture and the code in the same group"));
        assert!(
            SKILL.contains("--diagram connect.json"),
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
        assert!(SKILL.contains("A question about a flow is a diagram"));
        assert!(
            SKILL.contains("\"nodes\""),
            "and the shape is here, not in a document it was told not to read"
        );
    }

    #[test]
    fn the_diagram_in_the_skill_is_one_deck_can_draw() {
        // An example that does not parse teaches a shape the tool refuses. It
        // is the only part of this file that can be checked rather than read,
        // so it is checked.
        let json = SKILL
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
}
