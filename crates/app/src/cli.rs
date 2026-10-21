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
        /// The `.deck` directories to show, or `.deck` files somebody sent.
        ///
        /// More than one is a queue: the bar says how many are waiting and the
        /// reader walks them. Five worktrees finishing at once is the case this
        /// is for.
        ///
        /// A file is opened into `~/.deck/opened/`, read-only, and shows the
        /// code it carries. Its review is written beside it, as
        /// `<name>.review`.
        #[arg(required = true)]
        deck: Vec<PathBuf>,
        /// Hand the window to a process of its own and return straight away.
        ///
        /// Off by default, and the default is the one to want: `deck open`
        /// holds until the reader answers, then prints the review as JSON and
        /// exits — so opening a deck and hearing back are one command, and
        /// there is no second one to forget. Exits 4 if they closed it without
        /// submitting, 3 on a timeout.
        ///
        /// Run it in the background, or it blocks the agent that ran it.
        ///
        /// `-d` is for the case where somebody else is doing the listening:
        /// a `deck wait` of its own, or nobody, on purpose.
        #[arg(short = 'd', long = "detach")]
        detached: bool,
        /// Which neutrals to paint on. Overrides `~/.deck/config.toml`.
        #[arg(long, value_enum)]
        paper: Option<Paper>,
        /// Light or dark, whatever the machine is set to. Overrides the file.
        ///
        /// For trying one on, and for a screenshot that has to look the same
        /// on somebody else's machine.
        #[arg(long, value_enum)]
        mode: Option<deck_core::theme::Mode>,
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
        /// `` `code` ``, blank line between paragraphs — a real one. A break
        /// written as `\n` is read as one too, outside backticks.
        #[arg(long, value_parser = prose)]
        say: String,
        /// What this group shows, as a heading in sentence case: `The retry
        /// runs twice`.
        ///
        /// It heads the window while the group is on screen. The deck's title
        /// is the question the whole deck answers; this is the step of the
        /// answer on screen now, so it is never the deck's title again.
        #[arg(long)]
        title: Option<String>,
        /// How sure you are of this group's claim: high, medium or low.
        ///
        /// Your basis, not a feeling. Start from the strongest evidence you
        /// used: high, you watched it behave (ran it, reproduced it, ran a
        /// test covering it, read a trace from the real system); medium, you
        /// read every line it depends on; low, you inferred it. Drop a level
        /// for each of: indirect (sandbox, old commit, a similar input),
        /// incomplete (a config value, another service), conflicting
        /// (something you saw disagrees), or a never/always claim. Count each
        /// gap once. A second, independent kind of evidence cancels one drop
        /// it does not share. Below low, do not make the claim. Between two,
        /// pick the lower.
        #[arg(long, value_name = "LEVEL")]
        confidence: Option<String>,
        /// The basis, in one line the reader can check: on high, what showed
        /// it; below high, the drop or what is missing. Needed below high.
        #[arg(long)]
        why: Option<String>,
        /// A file and the lines to light up, and optionally a note after a
        /// space: `src/batch.ts:140-148 decremented *twice*`.
        ///
        /// Give it once per pane. A single number is one line.
        ///
        /// Start the note with a name in brackets to name the pane —
        /// `src/batch.ts:140-148 [retry] decremented *twice*` — and write
        /// `[retry]` in `--say` to mean that pane. Never "left" or "right".
        #[arg(long = "ref", value_name = "FILE:FIRST-LAST [NOTE]")]
        refs: Vec<String>,
        /// What the range should become: the lines marked as going, and this
        /// spliced in under them, drawn as a change rather than a highlight.
        ///
        /// Applies to the `--ref` it follows, so it goes straight after the one
        /// it changes. The file on disk is never touched.
        #[arg(long, value_name = "REPLACEMENT")]
        after: Vec<String>,
        /// What the range replaced, for a change you have already made: this
        /// drawn as going, the lines themselves as what arrived.
        ///
        /// The mirror of `--after`, for the case where the edit is on disk
        /// before the deck is written. Applies to the `--ref` it follows.
        ///
        /// Pass it empty — `--before ""` — when the range replaced nothing and
        /// is purely new. The lines are marked as arrived with nothing drawn
        /// above them, which is what says *these are the ones I added*.
        #[arg(long, value_name = "WHAT_IT_REPLACED", allow_hyphen_values = true)]
        before: Vec<String>,
        /// What lines of the range held when the code ran, as a JSON list:
        /// `[{"line":6,"at":"orderId","is":"undefined","lit":true}]`. Applies
        /// to the `--ref` it follows.
        ///
        /// Only values you have seen, only on lines the `say` points at, and
        /// only the one or two the claim turns on. A value with `keys` gets a
        /// preview under its line that opens to them.
        #[arg(long, value_name = "JSON")]
        values: Vec<String>,
        /// A picture, as a JSON file, with the same optional `[name]` and
        /// note a `--ref` takes: `flows/import.json [flow] how it travels`.
        /// See PROTOCOL.md for the file's shape.
        #[arg(long, value_name = "FILE.json [NAME] [NOTE]")]
        diagram: Vec<String>,
        /// A page, as an HTML file, for the one idea that only makes sense
        /// moving. Same `[name]` and note: `merge.html [rows] which field wins`.
        /// Nothing is fetched, and at most forty words may show.
        #[arg(long, value_name = "FILE.html [NAME] [NOTE]")]
        page: Vec<String>,
        /// A question the reader can press to ask you, shown as a pill under
        /// the window. Give it up to five times.
        ///
        /// Write what the reader was about to type, in their words, with a
        /// name from this group in it: `What's an outbox, basics first?`,
        /// `Draw it: enqueue → worker → email`, `Are you sure the PDF is the
        /// slow part?`. Never a decision for them: no "Go with A", no
        /// "Which would you pick?". The skill has the full method.
        #[arg(long, value_name = "QUESTION")]
        ask: Vec<String>,
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
        /// Disambiguate when one group shows the same file more than once:
        /// the pane's name, or its ref id.
        #[arg(long)]
        pane: Option<String>,
        /// What the shown range should become: the lines are drawn as going
        /// and this is spliced in under them, as a change rather than a
        /// highlight. The file on disk is never touched.
        ///
        /// Use it when the answer is code. Showing the change beats describing
        /// it, and the reader can comment on the lines you are proposing.
        #[arg(long, value_name = "REPLACEMENT")]
        after: Option<String>,
        /// What the shown lines held when the code ran, as a JSON list:
        /// `[{"line":6,"at":"orderId","is":"undefined","lit":true}]`.
        ///
        /// A single value is written after its name. One with `keys` gets a
        /// preview under its line that opens to them. Use it when the answer
        /// is what a value was, not how the code reads.
        #[arg(long, value_name = "JSON")]
        values: Option<String>,
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
        ///
        /// `[point 106-110]` inside it lights those lines of the code on
        /// screen, from the moment the words after it are said. The reader
        /// never sees the direction, and neither does the voice.
        #[arg(long, value_parser = prose)]
        text: String,
        /// Record it without reading it aloud.
        #[arg(long)]
        silent: bool,
        /// Give up if the window has not taken it by then.
        #[arg(long, default_value_t = 5)]
        timeout: u64,
    },

    /// No longer shown, and not worth sending.
    ///
    /// It used to put a line of your words in the panel while you worked. The
    /// panel shows a pulse by itself now. This is still accepted, so an older
    /// skill that sends it does not fail, and it changes nothing on screen.
    Doing {
        /// The `.deck` directory owned by the native window.
        deck: PathBuf,
        /// What you are doing: `reading the retry loop`, `checking the fonts`.
        #[arg(long)]
        text: String,
        /// Give up if the window has not taken it by then.
        #[arg(long, default_value_t = 5)]
        timeout: u64,
    },

    /// Fold panes away to their spines, or open them again.
    ///
    /// A folded pane is still there: its name on a spine at the window's edge,
    /// one click from coming back. Folding is how you make room without taking
    /// anything away.
    Fold {
        /// The `.deck` directory owned by the native window.
        deck: PathBuf,
        /// The pane to fold, by the name the prose calls it. Repeatable.
        #[arg(long = "pane", value_name = "NAME")]
        panes: Vec<String>,
        /// Open them again instead of folding them.
        #[arg(long)]
        open: bool,
        /// Give up if the window has not taken it by then.
        #[arg(long, default_value_t = 5)]
        timeout: u64,
    },

    /// Write values onto panes already on screen, several at once.
    ///
    /// Pair each `--on` with the `--values` after it: the pane's name, or its
    /// file, then a JSON list. All of them land in the same frame, which two
    /// `deck show` calls cannot do — shows move the reader and are paced.
    Values {
        /// The `.deck` directory owned by the native window.
        deck: PathBuf,
        /// A pane, by its name or its file. Repeatable.
        #[arg(long = "on", value_name = "PANE")]
        on: Vec<String>,
        /// What that pane's lines held, as a JSON list. One per `--on`.
        #[arg(long, value_name = "JSON")]
        values: Vec<String>,
        /// Give up if the window has not taken it by then.
        #[arg(long, default_value_t = 5)]
        timeout: u64,
    },

    /// Bring a file into the room that this group never showed.
    ///
    /// For a question the deck was not written for. Say what to show and what
    /// it displaces, and the room moves once.
    ///
    /// Think first about whether you need anything already on screen. If you
    /// do, fold the panes you do not need by name and this arrives beside the
    /// ones you kept. If you do not, `--fold-group` folds the whole group
    /// behind one spine and this has the room to itself.
    ///
    /// It is temporary: closing it, or turning to another group, takes it away
    /// and gives the group back.
    Bring {
        /// The `.deck` directory owned by the native window.
        deck: PathBuf,
        /// A file and one-based line range, with an optional `[name]` and note:
        /// `src/batch.ts:140-148 [retry] the loop that gives up`.
        #[arg(
            long = "ref",
            value_name = "FILE:FIRST-LAST [NAME] [NOTE]",
            conflicts_with_all = ["diagram", "page"],
            required_unless_present_any = ["diagram", "page"]
        )]
        reference: Option<String>,
        /// Or a picture, written now, for a question no file answers: how a
        /// request travels, what calls what, the states a job moves through.
        /// Same `[name]` and note as a ref: `flows/retry.json [flow] the path`.
        #[arg(long, value_name = "FILE.json [NAME] [NOTE]")]
        diagram: Option<String>,
        /// Or a page, written now, for a question that only makes sense
        /// moving: `unlink.html [unlink] what deleting costs`.
        #[arg(long, value_name = "FILE.html [NAME] [NOTE]")]
        page: Option<String>,
        /// What the range should become, drawn as a change rather than a
        /// highlight. The file on disk is never touched.
        #[arg(long, value_name = "REPLACEMENT")]
        after: Option<String>,
        /// Fold the whole group behind one spine to make room for it.
        #[arg(long)]
        fold_group: bool,
        /// Or fold only these panes of it, by name. Repeatable.
        #[arg(long = "fold", value_name = "NAME")]
        fold: Vec<String>,
        /// Give up if the window has not taken it by then.
        #[arg(long, default_value_t = 5)]
        timeout: u64,
    },

    /// Take the agent's hand off the page during a live walk.
    ///
    /// The frame round the pane being talked about and the lit lines inside it
    /// fade out. Both also go by themselves a few seconds after the voice stops;
    /// this is for ending a point on purpose, or finishing the walk.
    Clear {
        /// The `.deck` directory owned by the native window.
        deck: PathBuf,
        /// Give up if the window has not taken it by then.
        #[arg(long, default_value_t = 5)]
        timeout: u64,
    },

    /// Give a group new questions, after you have answered one of its own.
    ///
    /// What the reader just asked tells you what is left worth asking. Keep
    /// the questions that still are, drop the one you answered and any it
    /// made stale, and add what the answer opened. The ones that stay keep
    /// their places; new ones join after them. The window waits for your
    /// answer to finish before the row renews. Works on a sealed deck.
    Asks {
        /// The `.deck` directory.
        deck: PathBuf,
        /// The group whose questions these are: `g3`.
        #[arg(long)]
        group: String,
        /// A question for the renewed row. Give it up to five times; the same
        /// rules as `deck group --ask`.
        #[arg(long, value_name = "QUESTION")]
        ask: Vec<String>,
    },

    /// Say the deck is finished. Nothing more can be added after this.
    Seal {
        /// The `.deck` directory.
        deck: PathBuf,
    },

    /// Make a finished deck into one `.deck` file anybody can open: the deck,
    /// the code it points at frozen with a little context, and the review.
    ///
    /// Only the deck's own files go in — never a draft, a pill or a marker.
    /// Paths become repo-relative and the repo is named by its remote and
    /// commit; your home directory becomes `~`; questions offered as pills
    /// are dropped. Everything is scanned for secrets first, and a hit
    /// refuses the pack, naming each one by a fingerprint.
    ///
    /// `deck pack d-1.deck`, `deck pack d-1.deck --dry-run`,
    /// `deck pack d-1.deck -o review.deck --allow 3f9a1c0b2d4e`.
    Pack {
        /// The `.deck` directory, or a directory `deck unpack` made, which is
        /// packed back exactly as it is.
        deck: PathBuf,
        /// Where to write the file. Defaults to `YYYY-MM-DD-<title>.deck` in
        /// the current directory; a directory here puts it in there.
        #[arg(short = 'o', long = "out", value_name = "FILE")]
        out: Option<PathBuf>,
        /// Replace a file already at that path.
        #[arg(long)]
        force: bool,
        /// Say what would be written — every entry and its size, every file
        /// frozen and how it stands against the commit, every path cut to
        /// its name, every secret found — and write nothing. Exits 1 when it
        /// would refuse.
        #[arg(long)]
        dry_run: bool,
        /// Lines kept either side of each range the deck points at.
        #[arg(long, value_name = "N", default_value_t = deck_core::bundle::snap::CONTEXT)]
        context: u32,
        /// Keep every line of every file pointed at, not just the ranges.
        ///
        /// Every extra line is a line the secret scanner has to pass and a
        /// line the reader was never shown, so it is off unless asked for.
        #[arg(long, conflicts_with = "context")]
        whole_files: bool,
        /// Let one secret-scanner hit through, by the fingerprint the refusal
        /// printed. Give it once per hit you have looked at and know is safe;
        /// the manifest records that you did.
        #[arg(long, value_name = "FINGERPRINT")]
        allow: Vec<String>,
        /// Pack even when a file the deck points at cannot be read. Its panes
        /// will open to nothing.
        #[arg(long)]
        allow_missing: bool,
    },

    /// Write a `.deck` file out as a directory: every entry as it is, and
    /// its manifest.
    ///
    /// The file is checked whole before anything is written. `deck pack` on
    /// the directory gives back the same file, byte for byte.
    Unpack {
        /// The `.deck` file.
        file: PathBuf,
        /// Where to write it: a new or empty directory. Defaults to the file's
        /// name with `.unpacked` in place of `.deck`, here.
        dir: Option<PathBuf>,
    },

    /// Write a deck out for somebody who does not have deck: one HTML file
    /// that is the window, a group to a screen, or a PDF — portrait, a page
    /// or two to a group, or `--landscape`, the window on paper — with the
    /// `.deck` inside it either way.
    ///
    /// The file asks for nothing from the network and runs no script; a page
    /// pane is drawn in a frame that can run none either. A deck directory is
    /// packed first, in memory, exactly as `deck pack` would pack it —
    /// scrubbed and scanned for secrets, refused on a hit — so an export
    /// leaves the machine holding what a `.deck` file would and no more.
    ///
    /// `deck export d-1.deck`, `deck export review.deck -o review.pdf`,
    /// `deck export d-1.deck --format pdf --landscape`,
    /// `deck export d-1.deck --groups 1,3-5 --no-comments`.
    Export {
        /// The `.deck` directory, a directory `deck unpack` made, or a
        /// `.deck` file.
        deck: PathBuf,
        /// What to write: `html` or `pdf`. Defaults to what `-o` ends in,
        /// and to `html`.
        #[arg(long, value_enum)]
        format: Option<Format>,
        /// A PDF on its side, a page to a group exactly as the window shows
        /// it, with what was said on a page after it. Portrait is the
        /// default: a cover, then each group on a page or two of its own.
        #[arg(long)]
        landscape: bool,
        /// The colours to paint it in: `editor` (the default) is whatever
        /// `deck open` would paint the window in — your editor's theme if
        /// one is set in `~/.deck/config.toml`, light or dark as it says;
        /// `dark` or `light` is deck's own; `vscode:Solarized Dark` and the
        /// rest try an editor's theme on, as `deck open --theme` does.
        #[arg(long, value_name = "THEME", default_value = "editor")]
        theme: String,
        /// Only these groups, by number: `1,3-5`. The `.deck` inside holds
        /// only them too — their code, and what was said about them — so
        /// leaving a group out leaves it out of everything the file carries.
        /// Remarks about the deck as a whole go only with every group.
        #[arg(long, value_name = "LIST")]
        groups: Option<String>,
        /// Leave out what the reader and the agent said: from the page, and
        /// the review from the `.deck` inside it too.
        #[arg(long)]
        no_comments: bool,
        /// Where to write it. Defaults to `YYYY-MM-DD-<title>.html` in the
        /// current directory; a directory here puts it in there.
        #[arg(short = 'o', long = "out", value_name = "FILE")]
        out: Option<PathBuf>,
        /// Replace a file already at that path.
        #[arg(long)]
        force: bool,
        /// Let one secret-scanner hit through, by the fingerprint the refusal
        /// printed, as `deck pack --allow`. A `.deck` file is scanned again
        /// too, and what its manifest already let through stays let through.
        #[arg(long, value_name = "FINGERPRINT")]
        allow: Vec<String>,
        /// Export a deck directory even when a file it points at cannot be
        /// read, as `deck pack --allow-missing`; its panes say so. A `.deck`
        /// file has nothing missing for this to apply to.
        #[arg(long)]
        allow_missing: bool,
    },

    /// Choose whether a deck can walk you through itself, and in whose voice.
    ///
    /// Everything else works without this: the narration lands in the panel,
    /// clicking a sentence lights the code it is about, and the conversation
    /// is always there. A voice is an upgrade to that, which is why it is its
    /// own command rather than something that starts talking the first time
    /// somebody presses a key. Once it is set up, `w` in the window starts and
    /// stops it — and until it is, the window does not offer the key at all.
    #[command(aliases = ["live", "read"])]
    Walk,

    /// Choose how deck looks, once.
    ///
    /// Asks whether to borrow your editor's colours, and writes the answer to
    /// `~/.deck/config.toml`. Everything else about deck is opened by an agent;
    /// this is the one thing only you can answer.
    Setup,

    /// Replace deck with a newer deck.
    ///
    /// Asks whether prereleases count, then downloads the newest release that
    /// has a binary attached, checks it against its published checksum, checks
    /// that it starts, and moves it into place. Windows that are already open
    /// are unaffected — the next deck anybody opens is the new one.
    ///
    /// Not for a copy a package manager installed: there it says so and stops,
    /// because replacing that file wins until the next `brew upgrade` disagrees.
    Upgrade {
        /// Take prereleases too, without being asked.
        #[arg(long)]
        prerelease: bool,
        /// Stable releases only, without being asked.
        #[arg(long, conflicts_with = "prerelease")]
        stable: bool,
    },

    /// Whether deck shares anonymous usage counts, and exactly what.
    ///
    /// Counts and versions only — never code, paths, titles, comments or
    /// questions — and nothing at all until you have said yes. `show` prints
    /// exactly what the next send would carry. Also off whenever
    /// `DO_NOT_TRACK` is set, `DECK_TELEMETRY=0`, under CI, or with
    /// `[updates] automatic = false`. Every field is listed at
    /// https://trydeck.dev/telemetry/.
    Telemetry {
        /// `on`, `off`, `status` (the default), or `show`.
        #[arg(value_enum, default_value_t = Telemetry::Status)]
        choice: Telemetry,
    },

    /// Read a hook event from Claude Code and answer it.
    ///
    /// Run by the agent, never by a person: `deck setup` offers to register it.
    /// On a `Stop` it reads the finished reply and refuses one that names two
    /// or more `file:line` locations — the shape of an argument the reader
    /// would otherwise have to assemble themselves. On a `SessionStart` after
    /// a compaction, in a session that has used deck, it says to load the deck
    /// skill again, which the compaction cut off.
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

/// A group's `--say` or a live `--text`, with breaks written as `\n` made
/// into breaks.
///
/// As clap's value parser, so nothing downstream has to know. Only those two:
/// they are the narration, the one place a paragraph break belongs. A ref's
/// note, a title and `deck doing` are one line each, and `--after` and
/// `--before` are code. See [`deck_cli::real_breaks`].
fn prose(text: &str) -> Result<String, std::convert::Infallible> {
    Ok(deck_cli::real_breaks(text))
}

/// `deck open`'s paths, with every `.deck` file opened into the cache and
/// replaced by the deck folder there. A directory is left as it is.
///
/// Opening is idempotent, so the process that is handed the window, which is
/// given the same arguments, finds what this one wrote and uses it.
///
/// Each comes back with the [`deck_cli::opened::Origin`] it was opened with —
/// which file, and where its review goes — and `None` for a folder. Two
/// copies of the same bytes share one deck folder, so the folder alone
/// cannot say which of them this is; everything after this is handed the
/// origin instead.
type Unpacked = (Vec<PathBuf>, Vec<Option<deck_cli::opened::Origin>>);
fn unpacked(paths: Vec<PathBuf>) -> anyhow::Result<Unpacked> {
    let mut decks = Vec::with_capacity(paths.len());
    let mut origins = Vec::with_capacity(paths.len());
    for path in paths {
        if !path.is_file() {
            decks.push(path);
            origins.push(None);
            continue;
        }
        let cache = deck_cli::opened::cache()
            .ok_or_else(|| anyhow::anyhow!("there is no home directory to open it into"))?;
        let opened = deck_cli::opened::open(&path, &cache)?;
        decks.push(opened.deck);
        origins.push(Some(opened.origin));
    }
    Ok((decks, origins))
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
                detached,
                paper,
                mode,
                theme,
            } => {
                // A file is a packed deck, and the window reads folders: it is
                // opened into the cache, checked, and the folder there is what
                // is shown from here on. Before anything else, so every path
                // below — the listener as much as the window — has the folder.
                let (deck, origins) = match unpacked(deck) {
                    Ok(opened) => opened,
                    Err(err) => {
                        eprintln!("deck: {err:#}");
                        return Err(ExitCode::FAILURE);
                    }
                };
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

                // Detached outright, and that is the end of it.
                if detached && detach() {
                    return Ok(None);
                }

                // Otherwise: hand the window to a process of its own, and stay
                // here as the thing that listens.
                //
                // Two processes rather than one, and the reason is the whole
                // design. A process wakes an agent by *ending* — not by
                // printing, not by a file appearing — so whatever must wake it
                // has to be something that can afford to die. A window cannot:
                // a reader asking a question mid-walk still wants the deck in
                // front of them. So the window lives over there, and the thing
                // that dies lives here.
                //
                // Which means this one command covers every way a reader can
                // reach an agent: a question, an interruption, a review, a
                // close. Run it again after answering and it listens again.
                if std::env::var_os(HOLDING).is_none() && detach() {
                    // Cleared here, before listening, and not left to the
                    // window.
                    //
                    // The window clears it too — but only when the reader opens
                    // the deck off the bar, which may be minutes away or never.
                    // Until then a `closed` from the last time is still sitting
                    // there, and the listener reads it the moment it starts and
                    // reports a deck closed without an answer that nobody has
                    // even looked at yet.
                    for path in &deck {
                        deck_cli::reopened(path);
                    }
                    let review = origins[0].as_ref().map(|origin| origin.review.as_path());
                    return Err(wait(&deck[0], review, 0));
                }

                // The reading itself waits for a window. An editor with no
                // theme set has one for light and one for dark, and which it
                // would show depends on the machine — which only the platform
                // can say, and only once it is up.
                Ok(Some(Opening {
                    deck,
                    origins,
                    // Whoever reaches here is the window, and the window never
                    // reports: the process that spawned it is listening, and
                    // this one's output goes nowhere.
                    wait: false,
                    editor,
                    named,
                    speech: config.speech.clone(),
                    paper: paper.unwrap_or(config.theme.paper),
                    colors: config.theme.colors,
                    dark: mode.unwrap_or(config.theme.mode),
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
                    crate::usage::deck_created();
                    println!("{}", root.display());
                }))
            }
            What::Group {
                deck,
                say,
                title,
                confidence,
                why,
                refs,
                after,
                before,
                values,
                diagram,
                page,
                ask,
            } => report(
                gather(&refs, &after, &before, &values, &diagram, &page).and_then(|refs| {
                    // Said before the path, because an agent that reads one line of
                    // output reads the last one.
                    for note in deck_cli::what_will_not_light(&say, &refs) {
                        eprintln!("deck: {note}");
                    }
                    deck_cli::confidence(confidence.as_deref(), why.as_deref())
                        .and_then(|judged| {
                            deck_cli::group_judged(
                                &deck,
                                title.as_deref(),
                                judged,
                                &say,
                                refs,
                                &ask,
                            )
                        })
                        .map(|path| {
                            crate::usage::record(crate::usage::Count::GroupWritten);
                            crate::usage::record_many(
                                crate::usage::Count::QuestionsOffered,
                                ask.len() as u64,
                            );
                            println!("{}", path.display());
                        })
                }),
            ),
            What::Walk => report(crate::setup::live()),
            What::Setup => report(crate::setup::run()),
            What::Upgrade { prerelease, stable } => {
                // Neither flag is a question to ask; either one is an answer
                // already given.
                let unstable = prerelease.then_some(true).or(stable.then_some(false));
                report(crate::upgrade::run(unstable))
            }
            What::Hook => report(crate::hook::run()),
            What::Telemetry { choice } => report(telemetry(choice)),
            What::Show {
                deck,
                reference,
                group,
                pane,
                after,
                values,
                request_id,
                timeout,
            } => Err(show(
                &deck, &reference, group, pane, after, values, request_id, timeout,
            )),
            What::Say {
                deck,
                text,
                silent,
                timeout,
            } => Err(say(&deck, &text, !silent, timeout)),
            What::Doing {
                deck,
                text,
                timeout,
            } => Err(ask(
                &deck,
                deck_cli::live::RequestBody::Doing { text },
                timeout,
            )),
            What::Clear { deck, timeout } => {
                Err(ask(&deck, deck_cli::live::RequestBody::Clear, timeout))
            }
            What::Fold {
                deck,
                panes,
                open,
                timeout,
            } => Err(ask(
                &deck,
                deck_cli::live::RequestBody::Fold {
                    panes,
                    group: false,
                    open,
                },
                timeout,
            )),
            What::Values {
                deck,
                on,
                values,
                timeout,
            } => Err(write_values(&deck, &on, &values, timeout)),
            What::Bring {
                deck,
                reference,
                diagram,
                page,
                after,
                fold_group,
                fold,
                timeout,
            } => Err(bring(
                &deck,
                Bringing {
                    reference: reference.as_deref(),
                    diagram: diagram.as_deref(),
                    page: page.as_deref(),
                    after,
                },
                fold_group,
                fold,
                timeout,
            )),
            What::Asks { deck, group, ask } => {
                report(deck_cli::renew_asks(&deck, &group, &ask).map(|path| {
                    crate::usage::record(crate::usage::Count::QuestionsRenewed);
                    crate::usage::record_many(
                        crate::usage::Count::QuestionsOffered,
                        ask.len() as u64,
                    );
                    println!("{}", path.display());
                }))
            }
            What::Seal { deck } => report(deck_cli::seal(&deck)),
            What::Pack {
                deck,
                out,
                force,
                dry_run,
                context,
                whole_files,
                allow,
                allow_missing,
            } => {
                let options = deck_cli::pack::Options {
                    context,
                    whole_files,
                    allow,
                    allow_missing,
                    force,
                    ..deck_cli::pack::Options::default()
                };
                if dry_run {
                    // A listing that ends in a refusal exits 1, so a script
                    // can tell a dry run that would pack from one that would
                    // not without reading it.
                    let listed = deck_cli::pack::gather(&deck, &options)
                        .and_then(|packing| Ok((packing.dry_run()?, packing.refusal().is_some())));
                    return match listed {
                        Ok((listing, refused)) => {
                            println!("{listing}");
                            Err(if refused {
                                ExitCode::FAILURE
                            } else {
                                ExitCode::SUCCESS
                            })
                        }
                        Err(err) => report(Err(err)),
                    };
                }
                report(
                    deck_cli::pack::pack(&deck, out.as_deref(), &options).map(|packed| {
                        // Said before the path, because an agent that reads
                        // one line of output reads the last one.
                        for note in &packed.notes {
                            eprintln!("deck: {note}");
                        }
                        println!("{}", packed.path.display());
                    }),
                )
            }
            What::Unpack { file, dir } => report(
                deck_cli::pack::unpack(&file, dir.as_deref()).map(|unpacked| {
                    for name in &unpacked.ignored {
                        eprintln!("deck: {name} is from a newer deck, and was left out");
                    }
                    println!("{}", unpacked.dir.display());
                }),
            ),
            What::Export {
                deck,
                format,
                landscape,
                theme,
                groups,
                no_comments,
                out,
                force,
                allow,
                allow_missing,
            } => report(
                export(&Exporting {
                    deck: &deck,
                    format,
                    landscape,
                    theme: &theme,
                    groups: groups.as_deref(),
                    comments: !no_comments,
                    out: out.as_deref(),
                    force,
                    allow,
                    allow_missing,
                })
                .map(|path| println!("{}", path.display())),
            ),
            What::Next {
                deck,
                status,
                after,
                timeout,
            } => Err(next(&deck, status, after.as_deref(), timeout)),
            What::Wait { deck, timeout } => Err(wait(&deck, None, timeout)),
        }
    }
}

/// What `deck export` writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Format {
    /// One self-contained HTML file.
    Html,
    /// A PDF, portrait unless `--landscape`.
    Pdf,
}

/// Everything `deck export` was asked.
struct Exporting<'a> {
    deck: &'a std::path::Path,
    format: Option<Format>,
    landscape: bool,
    theme: &'a str,
    groups: Option<&'a str>,
    comments: bool,
    out: Option<&'a std::path::Path>,
    force: bool,
    allow: Vec<String>,
    allow_missing: bool,
}

/// `deck export`: the deck, packed or opened, written as HTML or a PDF.
/// Returns where.
fn export(asked: &Exporting) -> anyhow::Result<PathBuf> {
    // The format is what was asked, or what the file is called; the two
    // disagreeing is a mistake worth saying rather than guessing at.
    let named = asked
        .out
        .filter(|out| !out.is_dir())
        .and_then(|out| out.extension())
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase);
    let from_name = match named.as_deref() {
        Some("html" | "htm") => Some(Format::Html),
        Some("pdf") => Some(Format::Pdf),
        _ => None,
    };
    if let (Some(format), Some(named)) = (asked.format, from_name) {
        anyhow::ensure!(
            format == named,
            "--format and the file name disagree: drop one of them"
        );
    }
    let format = asked.format.or(from_name).unwrap_or(Format::Html);
    anyhow::ensure!(
        !(asked.landscape && format == Format::Html),
        "--landscape is for a PDF: the HTML export is the window already"
    );
    let groups = match asked.groups {
        Some(list) => deck_render::export::groups(list).map_err(anyhow::Error::msg)?,
        None => Vec::new(),
    };

    // A file is opened and scanned; a directory is packed first, exactly as
    // `deck pack` packs it. Either way what goes into the page is what would
    // go into a `.deck`, and it is refused in pack's words.
    let options = deck_cli::pack::Options {
        allow: asked.allow.clone(),
        allow_missing: asked.allow_missing,
        ..deck_cli::pack::Options::default()
    };
    let packing = if asked.deck.is_file() {
        if asked.allow_missing {
            eprintln!(
                "deck: --allow-missing is for a deck directory: a .deck file holds \
                 whatever it was packed with, and nothing in it can be missing"
            );
        }
        let bytes = std::fs::read(asked.deck)
            .map_err(|err| anyhow::anyhow!("cannot read {}: {err}", asked.deck.display()))?;
        let (bundle, ignored) = deck_core::bundle::Bundle::open(&bytes)
            .map_err(|err| anyhow::anyhow!("{}: {err}", asked.deck.display()))?;
        for name in &ignored {
            eprintln!("deck: {name} is from a newer deck, and was left out");
        }
        deck_cli::pack::opened(bundle, &options)
            .map_err(|err| anyhow::anyhow!("{}: {err}", asked.deck.display()))?
    } else {
        deck_cli::pack::gather(asked.deck, &options)?
    };
    if let Some(why) = packing.refusal() {
        anyhow::bail!(why);
    }
    for note in packing.notes() {
        eprintln!("deck: {note}");
    }
    let (bundle, name) = (packing.bundle, packing.name);
    // A group that is not there is a typo, not a request for nothing.
    let count = bundle.groups().map_err(anyhow::Error::msg)?.len();
    if let Some(n) = groups
        .iter()
        .find(|&&n| usize::try_from(n).is_ok_and(|n| n > count))
    {
        anyhow::bail!("`{n}` is not a group: this deck has {count}");
    }
    let stem = name.trim_end_matches(".deck").to_string();

    let (palette, dark, syntax) = export_theme(asked.theme)?;
    let options = deck_render::export::Options {
        groups,
        comments: asked.comments,
        dark,
    };
    let (bytes, file) = match format {
        Format::Html => (
            deck_render::export::deck(&bundle, &palette, syntax, &options)
                .map_err(anyhow::Error::msg)?
                .into_bytes(),
            format!("{stem}.html"),
        ),
        Format::Pdf => {
            let shape = if asked.landscape {
                deck_render::export::Format::Landscape
            } else {
                deck_render::export::Format::Portrait
            };
            let writing = deck_render::export::Writing {
                name: name.clone(),
                // For reviewing the spacing: the 4-point grid and every
                // box's outline, drawn over the pages.
                grid: std::env::var("DECK_EXPORT_GRID").as_deref() == Ok("1"),
            };
            let (bytes, warnings) =
                deck_render::export::deck_pdf(&bundle, &palette, syntax, &options, shape, &writing)
                    .map_err(anyhow::Error::msg)?;
            // Said before the path, as pack says its notes.
            for warning in warnings {
                eprintln!("deck: {warning}");
            }
            (bytes, format!("{stem}.pdf"))
        }
    };
    let path = match asked.out {
        Some(out) if out.is_dir() => out.join(&file),
        Some(out) => out.to_path_buf(),
        None => PathBuf::from(&file),
    };
    deck_cli::pack::write_new(&path, &bytes, asked.force)?;
    Ok(path)
}

/// The palette an export is painted in, whether it is dark, and the syntax
/// colours, for `--theme`.
///
/// `editor` is the window's own answer, by the window's own rules: the
/// config's editor theme and mode, the palette derived from the theme when
/// one is read and deck's own otherwise, and anything written by hand over
/// both. Only the machine's light or dark is asked differently — the window
/// asks the platform once it is up, and an export has no window.
fn export_theme(
    theme: &str,
) -> anyhow::Result<(deck_core::theme::Palette, bool, deck_core::theme::Syntax)> {
    let config = crate::config::read()?;
    let (asked, mode) = match theme {
        "dark" => (None, Mode::Dark),
        "light" => (None, Mode::Light),
        "editor" => (config.theme.editor.clone(), config.theme.mode),
        other => (Some(other.to_string()), config.theme.mode),
    };
    let machine_dark = machine_dark();
    let imported = match asked.as_deref() {
        Some(asked) => {
            let (editor, named) = match asked.split_once(':') {
                Some((editor, name)) => (editor.to_string(), Some(name)),
                None => (asked.to_string(), None),
            };
            deck_theme::read(deck_theme::Editor::from(editor), named, machine_dark).unwrap_or_else(
                |err| {
                    // As the window does: said, then carried on in deck's own.
                    eprintln!("deck: cannot read the theme: {err}");
                    None
                },
            )
        }
        None => None,
    };
    let dark = mode.is_dark(imported.map_or(machine_dark, |imported| imported.bg.is_dark()));
    let mut palette = match imported {
        Some(imported) => deck_core::theme::derive(imported),
        None => crate::palette::current_on(dark, config.theme.paper),
    };
    config.theme.colors.apply(&mut palette);
    Ok((
        palette,
        dark,
        imported.map(|imported| imported.syntax).unwrap_or_default(),
    ))
}

/// Whether the machine is set to dark, asked without a window.
///
/// On a Mac the setting is a default anybody can read; elsewhere there is no
/// one place to ask, and light is what an unset machine shows.
fn machine_dark() -> bool {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("defaults")
            .args(["read", "-g", "AppleInterfaceStyle"])
            .output()
            .is_ok_and(|out| String::from_utf8_lossy(&out.stdout).trim() == "Dark")
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

/// The decks that have been asked for, and how.
pub struct Opening {
    /// The directories to show, in the order they were named.
    pub deck: Vec<PathBuf>,
    /// For each, the `.deck` file it was opened from, when it was.
    pub origins: Vec<Option<deck_cli::opened::Origin>>,
    /// Whether to hold the process and print the review once the window has
    /// gone. True unless `--detach` was asked for.
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
    before: &[String],
    values: &[String],
    diagrams: &[String],
    pages: &[String],
) -> anyhow::Result<Vec<Pointing>> {
    let mut out = Vec::with_capacity(refs.len() + diagrams.len() + pages.len());
    let changes = paired(refs.len(), after, "--after")?;
    let replaced = paired(refs.len(), before, "--before")?;
    let held = paired(refs.len(), values, "--values")?;

    for (ix, argument) in refs.iter().enumerate() {
        anyhow::ensure!(
            changes[ix].is_none() || replaced[ix].is_none(),
            "`{}` has both a `--after` and a `--before` on it. A range is what \
             is going or what arrived, and it cannot be drawn as both: use \
             `--after` for a change you have not made, `--before` for one you \
             have.",
            argument.split_whitespace().next().unwrap_or(argument)
        );
        let mut named = deck_cli::refs::parse(argument)?;
        named.after.clone_from(&changes[ix]);
        named.before.clone_from(&replaced[ix]);
        if let Some(json) = &held[ix] {
            named.values = serde_json::from_str(json).map_err(|err| {
                anyhow::anyhow!(
                    "the `--values` after `{}` is not a list of values: {err}. Each one is \
                     {{\"line\":6,\"at\":\"orderId\",\"is\":\"undefined\"}}",
                    argument.split_whitespace().next().unwrap_or(argument)
                )
            })?;
            // A value is evidence for lines the pane lights; one outside them
            // would be drawn on code nobody is being shown.
            if let Some(stray) = named
                .values
                .iter()
                .find(|value| !value_fits(value, named.range, named.after.as_deref()))
            {
                anyhow::bail!(
                    "a value is on line {}, outside `{}`: put values on the lines the ref lights",
                    stray.line,
                    argument.split_whitespace().next().unwrap_or(argument)
                );
            }
        }
        out.push(Pointing::Code(named));
    }

    for argument in diagrams {
        out.push(Pointing::Drawn(deck_cli::refs::picture(argument)?));
    }

    for argument in pages {
        out.push(Pointing::Written(deck_cli::refs::written(argument)?));
    }

    Ok(out)
}

/// Whether a value sits on lines its ref shows: the range itself, or — for a
/// value marked `after` — the proposed lines, numbered as they will be from the
/// range's first line. A proposal can be longer than what it replaces, and an
/// `after` value with no proposal has nothing to sit on.
pub(crate) fn value_fits(
    value: &deck_core::protocol::Value,
    range: deck_core::LineRange,
    after: Option<&str>,
) -> bool {
    if value.after {
        let Some(after) = after else {
            return false;
        };
        // A proposal that deletes has no arriving lines, and nothing to sit on.
        let lines = u32::try_from(after.lines().count()).unwrap_or(u32::MAX);
        value.line >= range.first && value.line < range.first.saturating_add(lines)
    } else {
        value.line >= range.first && value.line <= range.last
    }
}

/// Which `--ref` each `--after`, or each `--before`, belongs to.
///
/// Clap hands back the two lists separately, and the pairing an agent writing
/// the command would assume — that the change belongs to the `--ref` in front
/// of it — is in neither of them. So the ordering is read back off the command
/// line, which is the one place it survives.
///
/// # Errors
///
/// When one of them has no `--ref` before it to belong to.
fn paired(refs: usize, said: &[String], which: &str) -> anyhow::Result<Vec<Option<String>>> {
    let mut changes = vec![None; refs];
    if said.is_empty() {
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
            other if other == which => {
                anyhow::ensure!(
                    seen > 0,
                    "`{which}` has no `--ref` in front of it: it changes the lines of the ref \
                     it follows, so it goes straight after one"
                );
                if let Some(text) = said.get(taken) {
                    changes[seen - 1] = Some(text.clone());
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
#[allow(clippy::too_many_arguments)]
fn show(
    deck: &std::path::Path,
    reference: &str,
    group: Option<String>,
    pane: Option<String>,
    after: Option<String>,
    values: Option<String>,
    request_id: Option<String>,
    timeout: u64,
) -> ExitCode {
    let values: Vec<deck_core::protocol::Value> = match values.as_deref().map(serde_json::from_str)
    {
        None => Vec::new(),
        Some(Ok(values)) => values,
        Some(Err(err)) => {
            println!(
                "{}",
                serde_json::json!({ "status": "invalid-request", "reason": format!("--values is not a list of values: {err}") })
            );
            return ExitCode::FAILURE;
        }
    };
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
    if let Some(stray) = values
        .iter()
        .find(|value| !value_fits(value, named.range, after.as_deref()))
    {
        println!(
            "{}",
            serde_json::json!({
                "status": "invalid-request",
                "reason": format!("a value is on line {}, outside {reference}: put values on the lines you show", stray.line)
            })
        );
        return ExitCode::FAILURE;
    }
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
        after,
        values,
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
                | deck_cli::live::ResponseStatus::Unchanged => {
                    // It reached the deck: the agent is still at its answer.
                    deck_cli::still_answering(deck);
                    ExitCode::SUCCESS
                }
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
    ask(
        deck,
        deck_cli::live::RequestBody::Say {
            text: text.to_string(),
            aloud,
        },
        timeout,
    )
}

/// What `deck bring` was asked to put in the room.
///
/// Three ways to say it and one of them is set: lines of a file, a picture, or
/// a page. They travel together because the rest of the command — what to fold,
/// how long to wait — is the same whichever it is.
struct Bringing<'a> {
    /// A file and a range, as `--ref` named them.
    reference: Option<&'a str>,
    /// A picture, as `--diagram` named it.
    diagram: Option<&'a str>,
    /// A page, as `--page` named it.
    page: Option<&'a str>,
    /// What the range should become, when the ref is a proposed change.
    after: Option<String>,
}

/// Bring a file the group never showed into the room.
fn bring(
    deck: &std::path::Path,
    bringing: Bringing<'_>,
    fold_group: bool,
    fold: Vec<String>,
    timeout: u64,
) -> ExitCode {
    let Bringing {
        reference,
        diagram,
        page,
        after,
    } = bringing;
    // A picture and a file arrive the same way and displace the same panes. The
    // only difference is what the window is handed.
    let refused = |err: anyhow::Error| {
        println!(
            "{}",
            serde_json::json!({ "status": "invalid-request", "reason": err.to_string() })
        );
        ExitCode::FAILURE
    };

    if let Some(argument) = diagram {
        let picture = match deck_cli::refs::picture(argument) {
            Ok(picture) => picture,
            Err(err) => return refused(err),
        };
        return ask(
            deck,
            deck_cli::live::RequestBody::Draw {
                diagram: picture.diagram,
                name: picture.name,
                note: picture.note,
                fold_group,
                fold,
            },
            timeout,
        );
    }

    if let Some(argument) = page {
        let written = match deck_cli::refs::written(argument) {
            Ok(written) => written,
            Err(err) => return refused(err),
        };
        let words = deck_cli::shown_words(&written.html);
        if words > deck_cli::WORDS {
            return refused(anyhow::anyhow!(
                "this page shows {words} words, and {} is the limit. The prose \
                 carries the argument; a page carries the movement.",
                deck_cli::WORDS
            ));
        }
        return ask(
            deck,
            deck_cli::live::RequestBody::Render {
                html: written.html,
                name: written.name,
                note: written.note,
                fold_group,
                fold,
            },
            timeout,
        );
    }

    let Some(reference) = reference else {
        return refused(anyhow::anyhow!(
            "nothing to bring: pass --ref, --diagram or --page"
        ));
    };
    let named = match deck_cli::refs::parse(reference) {
        Ok(named) => named,
        Err(err) => return refused(err),
    };
    ask(
        deck,
        deck_cli::live::RequestBody::Bring {
            file: named.file,
            range: named.range,
            name: named.name,
            note: named.note,
            after,
            fold_group,
            fold,
        },
        timeout,
    )
}

/// Send one request that answers only with whether the window took it.
/// Put values onto several panes at once.
fn write_values(
    deck: &std::path::Path,
    on: &[String],
    values: &[String],
    timeout: u64,
) -> ExitCode {
    let refuse = |reason: String| {
        println!(
            "{}",
            serde_json::json!({ "status": "invalid-request", "reason": reason })
        );
        ExitCode::FAILURE
    };
    if on.is_empty() || on.len() != values.len() {
        return refuse("give each `--on` one `--values` straight after it".into());
    }
    let mut sets = Vec::with_capacity(on.len());
    for (pane, json) in on.iter().zip(values) {
        match serde_json::from_str::<Vec<deck_core::protocol::Value>>(json) {
            Ok(values) => sets.push(deck_cli::live::ValuesFor {
                pane: pane.clone(),
                values,
            }),
            Err(err) => {
                return refuse(format!(
                    "the `--values` for {pane} is not a list of values: {err}"
                ));
            }
        }
    }
    ask(deck, deck_cli::live::RequestBody::Values { sets }, timeout)
}

fn ask(deck: &std::path::Path, body: deck_cli::live::RequestBody, timeout: u64) -> ExitCode {
    let spoke = matches!(body, deck_cli::live::RequestBody::Say { .. });
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

    match client.request(body, std::time::Duration::from_secs(timeout)) {
        Ok(response) => {
            match serde_json::to_string(&response) {
                Ok(json) => println!("{json}"),
                Err(err) => eprintln!("deck: live reply will not print: {err}"),
            }
            if response.status == deck_cli::live::ResponseStatus::Applied {
                // It reached the deck, so the agent is plainly alive: having
                // spoken it is expected back at `deck wait`, and anything
                // else means it is still working on the answer.
                if spoke {
                    deck_cli::answered(deck);
                } else {
                    deck_cli::still_answering(deck);
                }
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
fn wait(deck: &std::path::Path, review: Option<&std::path::Path>, timeout: u64) -> ExitCode {
    // This also carries interruptions now. Four hundred milliseconds was
    // harmless for a final review, but added a visible pause to every reply.
    const EVERY: std::time::Duration = std::time::Duration::from_millis(80);

    let until =
        (timeout > 0).then(|| std::time::Instant::now() + std::time::Duration::from_secs(timeout));

    // The author's side of the huddle. The agent is already blocked here, so
    // this is where a question has to reach it — a second verb it would have to
    // poll is a habit no agent has, and the author stayed asleep because of it.
    let runtime = deck_core::home::deck();

    // Say so, on a beat, for as long as this runs. Without it the window has no
    // way to tell an armed waiter from no waiter at all, and a reader spends
    // ten minutes on comments that nobody has asked for.
    const BEAT: std::time::Duration = std::time::Duration::from_secs(1);

    /// Stops the beat however this function leaves — and there are a dozen
    /// ways it can. Killed outright it never runs, which is why the window
    /// reads the mark's age rather than whether it is there.
    struct Listening<'a>(&'a std::path::Path);
    impl Drop for Listening<'_> {
        fn drop(&mut self) {
            deck_cli::stopped_listening(self.0);
        }
    }

    let _beat = Listening(deck);
    let mut last_said = std::time::Instant::now();
    deck_cli::listening(deck);
    // Back from whatever it went away to answer.
    deck_cli::stopped_answering(deck);

    loop {
        // Submit wins over queued nudges: the final review already contains
        // them, and asking for another answer after submit is a dead end.
        // A deck opened from a file has its review wherever its origin
        // says, which the folder alone cannot tell.
        let found = review.map_or_else(|| deck_cli::review(deck), deck_cli::review_at);
        match found {
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

        // A waiter may start before the owner, or outlive a replacement owner.
        // Connecting once left it deaf for the rest of either session.
        if let Some(runtime) = runtime.as_ref()
            && let Ok(client) = deck_cli::live::Client::connect(runtime, deck)
        {
            match client.asked() {
                Ok(Some(asked)) => {
                    match serde_json::to_string(&serde_json::json!({ "asked": asked.moment })) {
                        Ok(json) => {
                            use std::io::Write as _;
                            let mut stdout = std::io::stdout().lock();
                            if let Err(err) =
                                writeln!(stdout, "{json}").and_then(|()| stdout.flush())
                            {
                                eprintln!("deck: the question will not print: {err}");
                                return ExitCode::FAILURE;
                            }
                            if let Err(err) = client.delivered(asked.seq) {
                                eprintln!("deck: the event cursor could not be saved: {err}");
                                return ExitCode::FAILURE;
                            }
                            // Leaving with the reader's question. The beat
                            // stops as this returns; this says the agent has
                            // not gone, it is answering.
                            deck_cli::answering(deck);
                        }
                        Err(err) => {
                            eprintln!("deck: the question will not print: {err}");
                            return ExitCode::FAILURE;
                        }
                    }
                    return ExitCode::SUCCESS;
                }
                Ok(None) => {}
                Err(err) => {
                    eprintln!("deck: reader events could not be read: {err}");
                    return ExitCode::FAILURE;
                }
            }
        }

        // Closed without answering. Checked after the review and the question
        // so that neither can be lost to a race: a reader who submits and then
        // closes has submitted, and that is the answer that goes back.
        if deck_cli::was_shut(deck) {
            eprintln!("deck: closed without a review");
            return ExitCode::from(4);
        }

        if until.is_some_and(|until| std::time::Instant::now() >= until) {
            eprintln!("deck: no review after {timeout}s");
            return ExitCode::from(3);
        }
        if last_said.elapsed() >= BEAT {
            deck_cli::listening(deck);
            last_said = std::time::Instant::now();
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

/// What `deck telemetry` is asked to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Telemetry {
    /// Share anonymous usage counts.
    On,
    /// Share nothing, and forget anything counted.
    Off,
    /// Say whether counts are shared, and if not, why not.
    Status,
    /// Print exactly what the next send would carry.
    Show,
}

/// `deck telemetry`.
fn telemetry(choice: Telemetry) -> anyhow::Result<()> {
    match choice {
        Telemetry::On => crate::usage::choose(true),
        Telemetry::Off => crate::usage::choose(false),
        Telemetry::Show => {
            println!("{}", crate::usage::show());
            return Ok(());
        }
        Telemetry::Status => {}
    }
    match crate::usage::status() {
        Ok(()) => println!(
            "Sharing anonymous usage counts. `deck telemetry show` prints them; \
             `deck telemetry off` stops."
        ),
        Err(off) => println!("Not sharing anything: {}.", off.reason()),
    }
    println!("Everything that is sent: https://trydeck.dev/telemetry/");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The narration has its written breaks made real; the code beside it is
    /// left exactly as it was given.
    #[test]
    fn only_the_narration_has_its_breaks_made_real() {
        let cli = Cli::try_parse_from([
            "deck",
            "group",
            "a.deck",
            "--say",
            r"One.\n\nTwo.",
            "--ref",
            "a.rs:1-2",
            "--after",
            r#"print("a\n");"#,
        ])
        .expect("parses");
        let What::Group { say, after, .. } = cli.what else {
            panic!("a group");
        };
        assert_eq!(say, "One.\n\nTwo.");
        assert_eq!(after, [r#"print("a\n");"#]);
    }

    /// `pack` takes its options as written, and `--whole-files` cannot be
    /// asked for alongside a context it would ignore.
    #[test]
    fn pack_reads_its_options() {
        let cli = Cli::try_parse_from([
            "deck",
            "pack",
            "d.deck",
            "-o",
            "out.deck",
            "--allow",
            "a1",
            "--allow",
            "b2",
            "--context",
            "3",
            "--dry-run",
        ])
        .expect("parses");
        let What::Pack {
            out,
            allow,
            context,
            dry_run,
            force,
            ..
        } = cli.what
        else {
            panic!("a pack");
        };
        assert_eq!(out, Some(PathBuf::from("out.deck")));
        assert_eq!(allow, ["a1", "b2"]);
        assert_eq!(context, 3);
        assert!(dry_run && !force);
        assert!(
            Cli::try_parse_from(["deck", "pack", "d.deck", "--whole-files", "--context", "3"])
                .is_err()
        );
        let cli = Cli::try_parse_from(["deck", "unpack", "a.deck"]).expect("parses");
        assert!(matches!(cli.what, What::Unpack { dir: None, .. }));
    }

    /// `export` takes its options as written, and paints in the editor's
    /// theme unless told otherwise.
    #[test]
    fn export_reads_its_options() {
        let cli = Cli::try_parse_from([
            "deck",
            "export",
            "d.deck",
            "--groups",
            "1,3-5",
            "--no-comments",
            "-o",
            "out.html",
            "--allow",
            "a1",
        ])
        .expect("parses");
        let What::Export {
            format,
            theme,
            groups,
            no_comments,
            out,
            force,
            allow,
            ..
        } = cli.what
        else {
            panic!("an export");
        };
        assert_eq!(format, None);
        assert_eq!(theme, "editor");
        assert_eq!(groups.as_deref(), Some("1,3-5"));
        assert!(no_comments && !force);
        assert_eq!(out, Some(PathBuf::from("out.html")));
        assert_eq!(allow, ["a1"]);
        let cli =
            Cli::try_parse_from(["deck", "export", "d.deck", "--format", "pdf"]).expect("parses");
        assert!(matches!(
            cli.what,
            What::Export {
                format: Some(Format::Pdf),
                ..
            }
        ));
    }

    /// A `.deck` file written by hand, holding `say` as its one group's
    /// narration, in `dir`.
    fn hand_built(dir: &std::path::Path, say: &str, allowed: &[String]) -> PathBuf {
        let group = serde_json::json!({ "id": "g1", "say": say }).to_string();
        let mut files = std::collections::BTreeMap::new();
        files.insert(
            "deck/deck.json".to_string(),
            br#"{"v":1,"id":"d-1","title":"A deck from somebody else"}"#.to_vec(),
        );
        files.insert("deck/g1.json".to_string(), group.into_bytes());
        let bundle = deck_core::bundle::Bundle {
            manifest: deck_core::bundle::Manifest {
                container: deck_core::bundle::CONTAINER,
                protocol: 1,
                created: "2026-10-20T09:30:00Z".to_string(),
                generator: "deck test".to_string(),
                repo: None,
                entries: std::collections::BTreeMap::new(),
                allowed: allowed.to_vec(),
            },
            files,
        };
        let path = dir.join("hand.deck");
        std::fs::write(&path, bundle.pack().expect("packs")).expect("writes");
        path
    }

    fn exporting<'a>(deck: &'a std::path::Path, out: &'a std::path::Path) -> Exporting<'a> {
        Exporting {
            deck,
            format: None,
            landscape: false,
            theme: "dark",
            groups: None,
            comments: true,
            out: Some(out),
            force: false,
            allow: Vec::new(),
            allow_missing: false,
        }
    }

    /// A `.deck` file is scanned again before it is exported: opening it
    /// proved nobody changed it, not that nothing in it is a secret. A hit
    /// is refused in pack's words, and its fingerprint lets it through.
    #[test]
    fn a_deck_file_is_scanned_before_it_is_exported() {
        let dir = tempfile::tempdir().expect("a temp dir");
        // Put together here, so no whole token is ever in the source.
        let token = format!("ghp_{}", "a1B2".repeat(9));
        let deck = hand_built(
            dir.path(),
            &format!("The token is {token} in the config."),
            &[],
        );
        let out = dir.path().join("out.html");
        let err = export(&exporting(&deck, &out)).expect_err("refused");
        let said = err.to_string();
        assert!(said.contains("refusing to pack"), "{said}");
        assert!(!out.exists());
        let fingerprint = said
            .split("--allow ")
            .nth(1)
            .and_then(|rest| rest.split_whitespace().next())
            .expect("a fingerprint to pass")
            .to_string();
        let path = export(&Exporting {
            allow: vec![fingerprint.clone()],
            ..exporting(&deck, &out)
        })
        .expect("let through");
        assert!(
            std::fs::read_to_string(path)
                .unwrap()
                .contains("Powered by Deck")
        );
        let options = deck_cli::pack::Options {
            allow: vec![fingerprint.clone()],
            ..deck_cli::pack::Options::default()
        };
        let open = |path: &std::path::Path| {
            deck_core::bundle::Bundle::open(&std::fs::read(path).unwrap())
                .unwrap()
                .0
        };
        let asked = deck_cli::pack::opened(open(&deck), &options).unwrap();
        assert!(
            asked
                .notes()
                .iter()
                .any(|n| n.starts_with("let through, as asked: "))
        );

        // A file whose own manifest lets the hit through is honoured, and
        // said to be that file's word rather than the exporter's.
        let theirs = hand_built(
            dir.path(),
            &format!("The token is {token} in the config."),
            std::slice::from_ref(&fingerprint),
        );
        let packing =
            deck_cli::pack::opened(open(&theirs), &deck_cli::pack::Options::default()).unwrap();
        assert!(packing.refusal().is_none());
        let notes = packing.notes();
        assert!(
            notes
                .iter()
                .any(|n| n.starts_with("let through by whoever packed it: ")),
            "{notes:?}"
        );
        assert!(!notes.iter().any(|n| n.contains("as asked")), "{notes:?}");
    }

    /// A group that is not in the deck is a typo, and said as one.
    #[test]
    fn a_group_the_deck_does_not_have_is_refused() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let deck = hand_built(dir.path(), "Nothing secret here.", &[]);
        let out = dir.path().join("out.html");
        let err = export(&Exporting {
            groups: Some("1,9"),
            ..exporting(&deck, &out)
        })
        .expect_err("refused");
        assert_eq!(err.to_string(), "`9` is not a group: this deck has 1");
    }

    /// A PDF is asked for by `--format` or by the file's name, and is a
    /// PDF carrying its deck; `--landscape` is refused for HTML, which is
    /// the window already.
    #[test]
    fn a_pdf_is_written_when_asked_for() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let deck = hand_built(dir.path(), "Nothing secret here.", &[]);
        for (format, out) in [(Some(Format::Pdf), "a.out"), (None, "b.pdf")] {
            let path = export(&Exporting {
                format,
                ..exporting(&deck, &dir.path().join(out))
            })
            .expect("written");
            let bytes = std::fs::read(path).unwrap();
            assert!(bytes.starts_with(b"%PDF-"));
        }
        let err = export(&Exporting {
            landscape: true,
            ..exporting(&deck, &dir.path().join("c.html"))
        })
        .expect_err("refused");
        assert!(
            err.to_string().contains("--landscape is for a PDF"),
            "{err}"
        );
        let err = export(&Exporting {
            format: Some(Format::Html),
            ..exporting(&deck, &dir.path().join("d.pdf"))
        })
        .expect_err("refused");
        assert!(err.to_string().contains("disagree"), "{err}");
    }
}
