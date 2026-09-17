//! The `deck` command.
//!
//! Static deck verbs write files or open the window. Live verbs cross the
//! owning window's acknowledged local mailbox and still return as commands.
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
        /// What the range should become: the lines marked as going, and this
        /// spliced in under them, drawn as a change rather than a highlight.
        ///
        /// Applies to the `--ref` it follows, so it goes straight after the one
        /// it changes. The file on disk is never touched.
        #[arg(long, value_name = "REPLACEMENT")]
        after: Vec<String>,
        /// A picture, as a JSON file. See PROTOCOL.md for its shape.
        #[arg(long, value_name = "FILE.json")]
        diagram: Vec<PathBuf>,
    },

    /// Move the live spotlight to code already shown in an authored group.
    Show {
        /// The `.deck` directory owned by the native window.
        deck: PathBuf,
        /// A file and one-based line range: `src/view.rs:106-110`.
        #[arg(long = "ref", value_name = "FILE:FIRST-LAST")]
        reference: String,
        /// Select an authored group instead of the currently visible one.
        #[arg(long)]
        group: Option<String>,
        /// Disambiguate when one group shows the same file more than once.
        #[arg(long)]
        pane: Option<String>,
        /// Retry identity. Reusing it with the same request returns its result.
        #[arg(long)]
        request_id: Option<String>,
        /// Give up if the visible view has not applied the target by then.
        #[arg(long, default_value_t = 5)]
        timeout: u64,
    },

    /// Say something to the reader during a live walk.
    ///
    /// Lands beside whatever is on screen and goes into the transcript whether
    /// or not a voice is set up, because what you said is part of the walk even
    /// when nobody heard it.
    Say {
        /// The `.deck` directory owned by the native window.
        deck: PathBuf,
        /// What to say. Markdown, as a group's `--say` is.
        #[arg(long)]
        text: String,
        /// Record it without reading it aloud.
        #[arg(long)]
        silent: bool,
        /// Give up if the window has not taken it by then.
        #[arg(long, default_value_t = 5)]
        timeout: u64,
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

    /// Read a `Stop` event and say whether the reply should have been a deck.
    ///
    /// Run by the agent, never by a person: `deck setup` offers to register it,
    /// and from then on it reads each finished reply and refuses the ones that
    /// name two or more `file:line` locations — the shape of an argument the
    /// reader would otherwise have to assemble themselves.
    #[command(hide = true)]
    Hook,

    /// Read durable reader events from an open live session.
    Next {
        /// The `.deck` directory whose native session owns the event stream.
        deck: PathBuf,
        /// Return the current lifecycle immediately without consuming an event.
        #[arg(long)]
        status: bool,
        /// Return the first event after this generation-scoped cursor.
        #[arg(long)]
        after: Option<String>,
        /// Give up after this many seconds.
        #[arg(long, default_value_t = 2)]
        timeout: u64,
    },

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
                    zen: config.zen.clone(),
                    speech: config.speech.clone(),
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
                after,
                diagram,
            } => report(gather(&refs, &after, &diagram).and_then(|refs| {
                deck_cli::group(&deck, &say, refs).map(|path| {
                    println!("{}", path.display());
                })
            })),
            What::Setup => report(crate::setup::run()),
            What::Hook => report(crate::hook::run()),
            What::Show {
                deck,
                reference,
                group,
                pane,
                request_id,
                timeout,
            } => Err(show(&deck, &reference, group, pane, request_id, timeout)),
            What::Say {
                deck,
                text,
                silent,
                timeout,
            } => Err(say(&deck, &text, !silent, timeout)),
            What::Seal { deck } => report(deck_cli::seal(&deck)),
            What::Next {
                deck,
                status,
                after,
                timeout,
            } => Err(next(&deck, status, after.as_deref(), timeout)),
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
    /// How far the screen goes down when the reader asks for quiet.
    pub zen: deck_core::config::Zen,
    /// How the narration sounds when it is read aloud.
    pub speech: deck_core::config::Speech,
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
fn gather(
    refs: &[String],
    after: &[String],
    diagrams: &[PathBuf],
) -> anyhow::Result<Vec<Pointing>> {
    let mut out = Vec::with_capacity(refs.len() + diagrams.len());
    let changes = paired(refs.len(), after)?;

    for (ix, argument) in refs.iter().enumerate() {
        let mut named = deck_cli::refs::parse(argument)?;
        named.after.clone_from(&changes[ix]);
        out.push(Pointing::Code(named));
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

/// Which `--ref` each `--after` belongs to.
///
/// Clap hands back the two lists separately, and the pairing an agent writing
/// the command would assume — that an `--after` changes the `--ref` in front of
/// it — is in neither of them. So the ordering is read back off the command
/// line, which is the one place it survives.
///
/// # Errors
///
/// When an `--after` has no `--ref` before it to belong to.
fn paired(refs: usize, after: &[String]) -> anyhow::Result<Vec<Option<String>>> {
    let mut changes = vec![None; refs];
    if after.is_empty() {
        return Ok(changes);
    }

    let mut seen = 0usize;
    let mut taken = 0usize;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        // `--ref x` and `--ref=x` are the same argument written two ways, and
        // only the first form eats the value that follows it.
        let inline = arg.contains('=');
        let flag = arg.split('=').next().unwrap_or(&arg).to_string();
        match flag.as_str() {
            "--ref" => {
                seen += 1;
                if !inline {
                    args.next();
                }
            }
            "--after" => {
                anyhow::ensure!(
                    seen > 0,
                    "`--after` has no `--ref` in front of it: it changes the lines of the ref \
                     it follows, so it goes straight after one"
                );
                if let Some(said) = after.get(taken) {
                    changes[seen - 1] = Some(said.clone());
                }
                taken += 1;
                if !inline {
                    args.next();
                }
            }
            _ => {}
        }
    }

    Ok(changes)
}

/// Move a visible code pane only after the native view acknowledges it.
fn show(
    deck: &std::path::Path,
    reference: &str,
    group: Option<String>,
    pane: Option<String>,
    request_id: Option<String>,
    timeout: u64,
) -> ExitCode {
    let named = match deck_cli::refs::parse(reference) {
        Ok(named) if named.note.is_none() => named,
        Ok(_) => {
            println!(
                "{}",
                serde_json::json!({
                    "status": "invalid-request",
                    "reason": "a live --ref is only FILE:FIRST-LAST; pane narration stays in the authored group"
                })
            );
            return ExitCode::FAILURE;
        }
        Err(err) => {
            println!(
                "{}",
                serde_json::json!({ "status": "invalid-request", "reason": err.to_string() })
            );
            return ExitCode::FAILURE;
        }
    };
    let Some(runtime) = deck_core::home::deck() else {
        println!(
            "{}",
            serde_json::json!({ "status": "failed", "reason": "no home directory for live Deck state" })
        );
        return ExitCode::FAILURE;
    };
    let client = match deck_cli::live::Client::connect(&runtime, deck) {
        Ok(client) => client,
        Err(deck_cli::live::ClientError::NotOpen(reason)) => {
            println!(
                "{}",
                serde_json::json!({ "status": "not-open", "reason": reason })
            );
            return ExitCode::from(4);
        }
        Err(err) => {
            println!(
                "{}",
                serde_json::json!({ "status": "failed", "reason": err.to_string() })
            );
            return ExitCode::FAILURE;
        }
    };
    let body = deck_cli::live::RequestBody::Show {
        file: named.file,
        range: named.range,
        group,
        pane,
    };
    let timeout = std::time::Duration::from_secs(timeout);
    let result = match request_id {
        Some(id) => client.request_with_id(id, body, timeout),
        None => client.request(body, timeout),
    };
    match result {
        Ok(response) => {
            let exit = match response.status {
                deck_cli::live::ResponseStatus::Applied
                | deck_cli::live::ResponseStatus::Unchanged => ExitCode::SUCCESS,
                deck_cli::live::ResponseStatus::Waiting
                | deck_cli::live::ResponseStatus::Hidden
                | deck_cli::live::ResponseStatus::Ambiguous
                | deck_cli::live::ResponseStatus::NotFound
                | deck_cli::live::ResponseStatus::OutOfRange
                | deck_cli::live::ResponseStatus::SourceChanged
                | deck_cli::live::ResponseStatus::Paused
                | deck_cli::live::ResponseStatus::Paced => ExitCode::from(5),
                deck_cli::live::ResponseStatus::Ready => ExitCode::FAILURE,
            };
            match serde_json::to_string(&response) {
                Ok(json) => println!("{json}"),
                Err(err) => {
                    eprintln!("deck: show result will not print: {err}");
                    return ExitCode::FAILURE;
                }
            }
            exit
        }
        Err(deck_cli::live::ClientError::Timeout) => {
            println!(
                "{}",
                serde_json::json!({ "status": "timeout", "reason": "the view did not apply the target before the timeout" })
            );
            ExitCode::from(3)
        }
        Err(deck_cli::live::ClientError::NotOpen(reason)) => {
            println!(
                "{}",
                serde_json::json!({ "status": "not-open", "reason": reason })
            );
            ExitCode::from(4)
        }
        Err(err) => {
            println!(
                "{}",
                serde_json::json!({ "status": "failed", "reason": err.to_string() })
            );
            ExitCode::FAILURE
        }
    }
}

/// Put the agent's words into an open walk.
fn say(deck: &std::path::Path, text: &str, aloud: bool, timeout: u64) -> ExitCode {
    let Some(runtime) = deck_core::home::deck() else {
        println!(
            "{}",
            serde_json::json!({ "status": "failed", "reason": "no home directory" })
        );
        return ExitCode::FAILURE;
    };
    let client = match deck_cli::live::Client::connect(&runtime, deck) {
        Ok(client) => client,
        Err(deck_cli::live::ClientError::NotOpen(reason)) => {
            println!(
                "{}",
                serde_json::json!({ "status": "not-open", "reason": reason })
            );
            return ExitCode::from(4);
        }
        Err(err) => {
            println!(
                "{}",
                serde_json::json!({ "status": "failed", "reason": err.to_string() })
            );
            return ExitCode::FAILURE;
        }
    };

    match client.request(
        deck_cli::live::RequestBody::Say {
            text: text.to_string(),
            aloud,
        },
        std::time::Duration::from_secs(timeout),
    ) {
        Ok(response) => {
            match serde_json::to_string(&response) {
                Ok(json) => println!("{json}"),
                Err(err) => eprintln!("deck: live reply will not print: {err}"),
            }
            if response.status == deck_cli::live::ResponseStatus::Applied {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(5)
            }
        }
        Err(deck_cli::live::ClientError::Timeout) => {
            println!("{}", serde_json::json!({ "status": "timeout" }));
            ExitCode::from(3)
        }
        Err(err) => {
            println!(
                "{}",
                serde_json::json!({ "status": "failed", "reason": err.to_string() })
            );
            ExitCode::FAILURE
        }
    }
}

/// Read live status through the same acknowledged mailbox later events use.
fn next(deck: &std::path::Path, status: bool, after: Option<&str>, timeout: u64) -> ExitCode {
    if status && after.is_some() {
        println!(
            "{}",
            serde_json::json!({
                "status": "invalid-request",
                "reason": "--after reads an event and cannot be combined with --status"
            })
        );
        return ExitCode::FAILURE;
    }

    let Some(runtime) = deck_core::home::deck() else {
        println!(
            "{}",
            serde_json::json!({
                "status": "failed",
                "reason": "no home directory for live Deck state"
            })
        );
        return ExitCode::FAILURE;
    };
    let client = match deck_cli::live::Client::connect(&runtime, deck) {
        Ok(client) => client,
        Err(deck_cli::live::ClientError::NotOpen(reason)) => {
            println!(
                "{}",
                serde_json::json!({ "status": "not-open", "reason": reason })
            );
            return ExitCode::from(4);
        }
        Err(err) => {
            println!(
                "{}",
                serde_json::json!({ "status": "failed", "reason": err.to_string() })
            );
            return ExitCode::FAILURE;
        }
    };

    if !status {
        // The reader's half of the loop. A cursor rather than "whatever
        // happened since you asked", so an agent that was busy or restarting
        // does not lose a must-fix somebody pressed while it was away.
        let after = match after.map(str::parse::<u64>) {
            Some(Ok(seq)) => Some(seq),
            Some(Err(_)) => {
                println!(
                    "{}",
                    serde_json::json!({
                        "status": "invalid-request",
                        "reason": "--after takes a cursor from an earlier event"
                    })
                );
                return ExitCode::FAILURE;
            }
            None => None,
        };
        return match client.events(after, std::time::Duration::from_secs(timeout)) {
            Ok(Some(event)) => match serde_json::to_string(&event) {
                Ok(json) => {
                    println!("{json}");
                    ExitCode::SUCCESS
                }
                Err(err) => {
                    eprintln!("deck: live event will not print: {err}");
                    ExitCode::FAILURE
                }
            },
            // Nothing happened, which is the usual answer while somebody reads.
            Ok(None) => {
                println!("{}", serde_json::json!({ "status": "quiet" }));
                ExitCode::from(3)
            }
            Err(err) => {
                println!(
                    "{}",
                    serde_json::json!({ "status": "failed", "reason": err.to_string() })
                );
                ExitCode::FAILURE
            }
        };
    }

    match client.request(
        deck_cli::live::RequestBody::Status,
        std::time::Duration::from_secs(timeout),
    ) {
        Ok(response) => match serde_json::to_string(&response) {
            Ok(json) => {
                println!("{json}");
                ExitCode::SUCCESS
            }
            Err(err) => {
                eprintln!("deck: live status will not print: {err}");
                ExitCode::FAILURE
            }
        },
        Err(deck_cli::live::ClientError::Timeout) => {
            println!(
                "{}",
                serde_json::json!({
                    "status": "timeout",
                    "reason": format!("no live status after {timeout}s")
                })
            );
            ExitCode::from(3)
        }
        Err(deck_cli::live::ClientError::NotOpen(reason)) => {
            println!(
                "{}",
                serde_json::json!({ "status": "not-open", "reason": reason })
            );
            ExitCode::from(4)
        }
        Err(err) => {
            println!(
                "{}",
                serde_json::json!({ "status": "failed", "reason": err.to_string() })
            );
            ExitCode::FAILURE
        }
    }
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
