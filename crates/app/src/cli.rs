//! The `deck` command.
//!
//! Five verbs. Four of them write files and return; the fifth opens a window.
//!
//! The help text these produce is deck's whole interface for an agent — the
//! protocol document is for people writing clients, and a model should never
//! need it. So the wording here is the wording that matters: every option says
//! what it is for rather than what it is called.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use deck_cli::Pointing;
use deck_core::layout::Layout;
use deck_core::theme::{Mode, Paper};

/// Present code for review, and read the answer.
#[derive(Parser)]
#[command(name = "deck", version)]
pub struct Cli {
    #[command(subcommand)]
    what: What,
}

#[derive(Subcommand)]
enum What {
    /// Put a deck on screen.
    ///
    /// A bar appears first saying a deck is ready; the reader opens it when
    /// they are ready to give a review, which is not the same moment as the
    /// agent being ready to ask for one.
    Open {
        /// The `.deck` directories to show.
        ///
        /// More than one is a queue: the bar says how many are waiting and the
        /// reader walks them. Five worktrees finishing at once is the case this
        /// is for.
        #[arg(required = true)]
        deck: Vec<PathBuf>,
        /// Wait for the window to close, then print the review as JSON.
        ///
        /// Exits non-zero if the reader closed the deck without submitting.
        #[arg(long)]
        wait: bool,
        /// Which neutrals to paint on. Overrides `~/.deck/config.toml`.
        #[arg(long, value_enum)]
        paper: Option<Paper>,
        /// Borrow the colours from an editor you already use.
        ///
        /// The page, the text, an accent, and whatever syntax colours it has.
        /// Never its chrome: a theme designs its own status line and tab bar,
        /// and those are answers to questions deck is not asking.
        ///
        /// `nvim`, `zed`, `vscode`, `cursor` or `windsurf` takes whatever the
        /// editor is set to. Add a colon and a theme name — `vscode:Solarized
        /// Dark` — to try one on without changing the editor's own settings.
        #[arg(long, value_name = "EDITOR[:THEME]")]
        theme: Option<String>,
    },

    /// Start a deck, and print the directory it lives in.
    ///
    /// Everything after this takes that path.
    New {
        /// The question or claim the deck answers.
        ///
        /// A heading, in sentence case. It is read cold, before anything else
        /// about the deck is on screen.
        #[arg(long)]
        title: String,
        /// The project the deck is about. Defaults to the working directory.
        ///
        /// Every relative path in a ref resolves against it.
        #[arg(long)]
        cwd: Option<PathBuf>,
        /// How many groups you intend to write.
        ///
        /// Shown as `2 of 4` while the rest are on their way.
        #[arg(long)]
        total: Option<u32>,
        /// Where to put the deck directory.
        #[arg(long, default_value_os_t = decks())]
        at: PathBuf,
    },

    /// Add a group: one thing to say, and the code that shows it.
    Group {
        /// The `.deck` directory, as `deck new` printed it.
        deck: PathBuf,
        /// What you want to say. Markdown: `**bold**`, `*look here*`,
        /// `` `code` ``, blank line between paragraphs.
        #[arg(long)]
        say: String,
        /// A file and the lines to light up, and optionally a note after a
        /// space: `src/batch.ts:140-148 decremented *twice*`.
        ///
        /// Give it once per pane. A single number is one line.
        #[arg(long = "ref", value_name = "FILE:FIRST-LAST [NOTE]")]
        refs: Vec<String>,
        /// A picture, as a JSON file. See PROTOCOL.md for its shape.
        #[arg(long, value_name = "FILE.json")]
        diagram: Vec<PathBuf>,
    },

    /// Say the deck is finished. Nothing more can be added after this.
    Seal {
        /// The `.deck` directory.
        deck: PathBuf,
    },

    /// Choose how deck looks, once.
    ///
    /// Asks whether to borrow your editor's colours, and writes the answer to
    /// `~/.deck/config.toml`. Everything else about deck is opened by an agent;
    /// this is the one thing only you can answer.
    Setup,

    /// Block until the reader submits, then print the review as JSON.
    Wait {
        /// The `.deck` directory.
        deck: PathBuf,
        /// Give up after this many seconds. Zero waits forever, which is the
        /// right answer for an agent: run this in the background, end your
        /// turn, and be woken when the reader submits.
        #[arg(long, default_value_t = 0)]
        timeout: u64,
    },
}

/// Where decks go when nobody says otherwise.
fn decks() -> PathBuf {
    deck_core::home::deck().unwrap_or_default().join("decks")
}

impl Cli {
    /// Do what was asked.
    ///
    /// Everything but `open` finishes here. `open` needs a window, which needs
    /// the main thread, so it hands back the deck for the caller to run.
    pub fn run(self) -> Result<Option<Opening>, ExitCode> {
        match self.what {
            What::Open {
                deck,
                wait,
                paper,
                theme,
            } => {
                // The file, then the flag over it. A flag is somebody trying
                // something on; the file is what they decided.
                let config = match crate::config::read() {
                    Ok(config) => config,
                    Err(err) => {
                        eprintln!("deck: {err}");
                        return Err(ExitCode::FAILURE);
                    }
                };
                crate::setup::mention_once();
                // The flag over the file: a flag is somebody trying something
                // on, the file is what they decided.
                let asked = theme.or_else(|| config.theme.editor.clone());
                // `vscode:Solarized Dark` — the editor, and which of its themes.
                let (editor, named) = match asked.as_deref() {
                    Some(asked) => match asked.split_once(':') {
                        Some((editor, name)) => (
                            deck_theme::Editor::from(editor.to_string()),
                            Some(name.to_string()),
                        ),
                        None => (deck_theme::Editor::from(asked.to_string()), None),
                    },
                    None => (deck_theme::Editor::default(), None),
                };

                // Hand the window to a process of its own, unless somebody
                // asked to be held here.
                if !wait && detach() {
                    return Ok(None);
                }

                // The reading itself waits for a window. An editor with no
                // theme set has one for light and one for dark, and which it
                // would show depends on the machine — which only the platform
                // can say, and only once it is up.
                Ok(Some(Opening {
                    deck,
                    wait,
                    editor,
                    named,
                    paper: paper.unwrap_or(config.theme.paper),
                    colors: config.theme.colors,
                    dark: config.theme.mode,
                    layout: config.layout,
                }))
            }
            What::New {
                title,
                cwd,
                total,
                at,
            } => {
                let cwd = cwd.or_else(|| std::env::current_dir().ok());
                report(deck_cli::new(&at, &title, cwd, total).map(|root| {
                    println!("{}", root.display());
                }))
            }
            What::Group {
                deck,
                say,
                refs,
                diagram,
            } => report(gather(&refs, &diagram).and_then(|refs| {
                deck_cli::group(&deck, &say, refs).map(|path| {
                    println!("{}", path.display());
                })
            })),
            What::Setup => report(crate::setup::run()),
            What::Seal { deck } => report(deck_cli::seal(&deck)),
            What::Wait { deck, timeout } => Err(wait(&deck, timeout)),
        }
    }
}

/// The decks that have been asked for, and how.
pub struct Opening {
    /// The directories to show, in the order they were named.
    pub deck: Vec<PathBuf>,
    /// Whether to skip the bar.
    /// Whether to print the review once the window has gone.
    pub wait: bool,
    /// Which editor to borrow colours from, if any.
    pub editor: deck_theme::Editor,
    /// And which of its themes, when the reader named one.
    pub named: Option<String>,
    /// Which neutrals, when nothing is imported.
    pub paper: Paper,
    /// Colours written by hand, which win over both.
    pub colors: deck_core::theme::Overrides,
    /// Light, dark, or whatever the machine says.
    pub dark: Mode,
    /// How panes are arranged.
    pub layout: Layout,
}

/// Read what the command line said to point at.
///
/// Set on the copy of deck that actually holds the window.
///
/// Its absence is what says "you are the one that was typed", and so the one
/// that should hand the window on and get out of the way.
const HOLDING: &str = "DECK_HOLDS_THE_WINDOW";

/// Start a copy of this command that owns the window, and say so.
///
/// `deck open` is the window: it runs for as long as the bar or the deck is on
/// screen, which is minutes. That is the wrong shape for the thing an agent
/// runs — it opens a deck in the middle of writing one, and a command that does
/// not come back is one that never writes the next group. So the process that
/// was typed starts another to hold the window, and returns.
///
/// A new session, not just a new process. An agent's shell reaps the whole
/// process group when its command finishes, and a window in that group goes
/// with it.
///
/// `false` when the handover could not be made, and the caller should hold the
/// window itself: blocking is a worse command, but it is a working one, and a
/// deck nobody can open is not.
fn detach() -> bool {
    if std::env::var_os(HOLDING).is_some() {
        return false;
    }
    let Ok(exe) = std::env::current_exe() else {
        return false;
    };

    let mut holder = std::process::Command::new(exe);
    holder
        .args(std::env::args_os().skip(1))
        .env(HOLDING, "1")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        // SAFETY: between fork and exec only async-signal-safe calls are
        // allowed, and `setsid` is one of them.
        unsafe {
            holder.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt as _;
        /// `DETACHED_PROCESS` — no console, and not this one's.
        const DETACHED: u32 = 0x0000_0008;
        holder.creation_flags(DETACHED);
    }

    holder.spawn().is_ok()
}

/// Refs first, then diagrams, in the order they were given. Ids are not minted
/// here: they have to be unique across the deck, and only the crate that knows
/// which group this is becoming can promise that.
fn gather(refs: &[String], diagrams: &[PathBuf]) -> anyhow::Result<Vec<Pointing>> {
    let mut out = Vec::with_capacity(refs.len() + diagrams.len());

    for argument in refs {
        out.push(Pointing::Code(deck_cli::refs::parse(argument)?));
    }

    for path in diagrams {
        let text = std::fs::read_to_string(path)
            .map_err(|err| anyhow::anyhow!("cannot read {}: {err}", path.display()))?;
        let diagram = serde_json::from_str(&text)
            .map_err(|err| anyhow::anyhow!("{} is not a diagram: {err}", path.display()))?;
        out.push(Pointing::Drawn(diagram));
    }

    Ok(out)
}

/// Wait for a review to land beside the deck, and print it.
fn wait(deck: &std::path::Path, timeout: u64) -> ExitCode {
    /// How often to look. A review lands once, and a second either way is
    /// nothing next to how long a person takes to write one.
    const EVERY: std::time::Duration = std::time::Duration::from_millis(400);

    let until =
        (timeout > 0).then(|| std::time::Instant::now() + std::time::Duration::from_secs(timeout));

    loop {
        match deck_cli::review(deck) {
            Ok(Some(review)) => {
                match serde_json::to_string_pretty(&review) {
                    Ok(json) => println!("{json}"),
                    Err(err) => {
                        eprintln!("deck: the review will not print: {err}");
                        return ExitCode::FAILURE;
                    }
                }
                return ExitCode::SUCCESS;
            }
            Ok(None) => {}
            Err(err) => {
                eprintln!(
                    "deck: {} has a review that will not parse: {err}",
                    deck.display()
                );
                return ExitCode::FAILURE;
            }
        }

        if until.is_some_and(|until| std::time::Instant::now() >= until) {
            eprintln!("deck: no review after {timeout}s");
            return ExitCode::from(3);
        }
        std::thread::sleep(EVERY);
    }
}

/// Say what went wrong, and stop.
///
/// A command has an exit code, which is the whole reason the agent half of this
/// works: a deck that is ignored in silence leaves the agent believing it
/// presented something the reader never saw.
fn report(done: anyhow::Result<()>) -> Result<Option<Opening>, ExitCode> {
    match done {
        Ok(()) => Err(ExitCode::SUCCESS),
        Err(err) => {
            eprintln!("deck: {err}");
            Err(ExitCode::FAILURE)
        }
    }
}
