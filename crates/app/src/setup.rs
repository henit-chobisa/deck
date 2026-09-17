//! The one conversation deck has with the reader.
//!
//! Everything else about deck is opened by an agent and answered with a key.
//! This is the exception: how it looks, and which agents know it exists, are
//! decisions only the person can make, and asking once beats guessing every
//! time.
//!
//! # Why it is a command and not a first run
//!
//! `deck open` is run by an *agent*. A prompt there would block a process
//! nobody is watching, waiting on an answer nobody was asked for. So the
//! questions live in `deck setup`, which a person runs, and `deck open` only
//! mentions it — once, on the way past, when there is no config yet.
//!
//! # Why it is coloured
//!
//! Deck is a program about looking at things. A setup that came out as a wall
//! of grey prose would be the first impression, and the first impression would
//! be wrong. The colour here is the deck's own accent and nothing else — one
//! colour, used for the thing you are being asked and the thing you just chose.

use std::io::{IsTerminal as _, Write as _};
use std::path::Path;

use deck_core::config::Config;
use deck_core::theme::Mode;
use deck_theme::Editor;

/// Set up the voice a live walk speaks with.
///
/// Its own command because voice is an option, not the feature. A live walk
/// without one still shows the author's words and lights the panes it is
/// talking about; this only decides whether you also hear it.
///
/// # Errors
///
/// When the terminal cannot be read, or the config cannot be written.
pub fn live() -> anyhow::Result<()> {
    let mut config = crate::config::read().unwrap_or_default();

    println!();
    println!("  {}  {}", mark(), bold("deck live"));
    println!(
        "  {}",
        dim("A live walk works silently. This is whether it also speaks.")
    );
    println!();
    rule();

    println!();
    println!("  {}", bold("Aloud"));
    println!(
        "  {}",
        dim("Off is a reasonable answer, and the usual one in an office.")
    );
    println!();
    config.speech.aloud = ask_yes("Read the narration aloud?")?;
    if !config.speech.aloud {
        chose("silent");
        let path = crate::config::path()
            .ok_or_else(|| anyhow::anyhow!("no home directory to write a config into"))?;
        write(&path, &config)?;
        close(&path);
        return Ok(());
    }

    println!();
    rule();
    println!();
    println!("  {}", bold("Whose voice"));
    println!(
        "  {}",
        dim("Deck pipes the words to whatever you name, and plays nothing")
    );
    println!(
        "  {}",
        dim("itself — so the provider is yours, and so is what leaves the machine.")
    );
    println!();
    println!("    {}  Google Cloud — the neural voices", accent("1"));
    println!("    {}  this machine's own voice", accent("2"));
    println!(
        "    {}  a program I name (Kokoro, Piper, a script)",
        accent("3")
    );
    println!();

    match ask("Which?", "1")?.as_str() {
        "2" => {
            config.speech.engine = deck_core::config::Engine::System;
            listen(&mut config)?;
        }
        "3" => {
            config.speech.engine = deck_core::config::Engine::Command;
            println!();
            println!(
                "  {}",
                dim("It reads the words on stdin and plays them. No quotes needed.")
            );
            println!();
            let said = ask("Command", "kokoro-cli --voice af_heart -")?;
            config.speech.command = Some(if said.is_empty() {
                "kokoro-cli --voice af_heart -".to_string()
            } else {
                said
            });
            chose(config.speech.command.as_deref().unwrap_or(""));
        }
        _ => google(&mut config)?,
    }

    let path = crate::config::path()
        .ok_or_else(|| anyhow::anyhow!("no home directory to write a config into"))?;
    write(&path, &config)?;
    close(&path);
    Ok(())
}

/// Ask, and write the answers to `~/.deck/config.toml`.
///
/// # Errors
///
/// When the terminal cannot be read, or the config cannot be written.
pub fn run() -> anyhow::Result<()> {
    let mut config = crate::config::read().unwrap_or_default();

    open();
    look(&mut config)?;
    listen(&mut config)?;
    let path = crate::config::path()
        .ok_or_else(|| anyhow::anyhow!("no home directory to write a config into"))?;
    write(&path, &config)?;

    tell()?;
    catch()?;
    close(&path);
    Ok(())
}

// ─── the questions ──────────────────────────────────────────────────────────

/// What deck is, in three lines, before anything is asked.
///
/// Somebody running `deck setup` may have installed it a minute ago on a
/// friend's word. Two sentences is cheap and answers *what am I setting up*.
fn open() {
    println!();
    println!("  {}  {}", mark(), bold("deck"));
    println!(
        "  {}",
        dim("Your agent points at code. You walk it, comment, submit.")
    );
    println!();
    rule();
}

/// How it should look.
fn look(config: &mut Config) -> anyhow::Result<()> {
    let editors: Vec<Editor> = Editor::all()
        .into_iter()
        .filter(|editor| editor.installed())
        .collect();

    println!();
    println!("  {}", bold("Colours"));

    if editors.is_empty() {
        println!(
            "  {}",
            dim("No editor deck can read was found, so it will use its own.")
        );
        config.theme.editor = None;
        config.theme.mode = mode()?;
        return Ok(());
    }

    println!(
        "  {}",
        dim("Deck can borrow your editor's — the page, the text, the syntax.")
    );
    println!(
        "  {}",
        dim("Never its chrome, which is designed for a different window.")
    );
    println!();

    if ask_yes(&format!("Borrow from {}?", list(&editors)))? {
        let editor = pick(&editors)?;
        config.theme.editor = Some(editor.key().to_string());
        // Whatever the editor is in. Light or dark is its answer now, not one
        // deck has to be told.
        config.theme.mode = Mode::Auto;
        chose(&format!("following {}", editor.name()));
    } else {
        config.theme.editor = None;
        config.theme.mode = mode()?;
        chose(match config.theme.mode {
            Mode::Light => "deck's own, light",
            Mode::Dark => "deck's own, dark",
            Mode::Auto => "deck's own, following the machine",
        });
    }
    Ok(())
}

/// Light or dark, when nothing is being imported.
fn mode() -> anyhow::Result<Mode> {
    println!();
    println!("    {}  light", accent("1"));
    println!("    {}  dark", accent("2"));
    println!("    {}  whichever the machine is in", accent("3"));
    println!();

    Ok(match ask("Which?", "3")?.as_str() {
        "1" => Mode::Light,
        "2" => Mode::Dark,
        _ => Mode::Auto,
    })
}

/// Which of the editors found.
fn pick(editors: &[Editor]) -> anyhow::Result<Editor> {
    if let [only] = editors {
        return Ok(*only);
    }
    println!();
    for (ix, editor) in editors.iter().enumerate() {
        println!("    {}  {}", accent(&(ix + 1).to_string()), editor.name());
    }
    println!();

    let answer = ask("Which?", "1")?;
    let at = answer.parse::<usize>().unwrap_or(1).max(1) - 1;
    Ok(editors.get(at).copied().unwrap_or(editors[0]))
}

/// Point deck at Google's neural voices, and prove the key works.
///
/// Tested before it is written. A key that is wrong, or a project without the
/// API enabled, fails at the first sentence of a live walk — by which point the
/// reader is in a window with no terminal to read the error in. Finding out
/// here costs one request.
fn google(config: &mut Config) -> anyhow::Result<()> {
    config.speech.engine = deck_core::config::Engine::Google;

    println!();
    println!(
        "  {}",
        dim("Chirp 3: HD includes 1,000,000 characters a month, and it renews.")
    );
    println!(
        "  {}",
        dim("A five-group deck is about five thousand, so ordinary use is free.")
    );
    println!(
        "  {}",
        dim("Billing has to be enabled on the project even so — a card on file,")
    );
    println!(
        "  {}",
        dim("which Google requires before it will serve any of it.")
    );
    println!();
    println!(
        "    {}",
        accent("console.cloud.google.com → APIs → Text-to-Speech")
    );
    println!("    {}", accent("→ enable it, then Credentials → API key"));
    println!();

    let key = ask("Key", "")?;
    if key.is_empty() {
        println!();
        println!(
            "  {}",
            dim("Nothing written. Set DECK_SPEECH_KEY instead if you would rather")
        );
        println!(
            "  {}",
            dim("not keep it in a file, then run `deck live` again.")
        );
        return Ok(());
    }

    println!();
    println!("    {}  Charon — level, unhurried", accent("1"));
    println!("    {}  Kore — brighter", accent("2"));
    println!("    {}  Puck — quicker", accent("3"));
    println!();
    let voice = match ask("Which?", "1")?.as_str() {
        "2" => "en-US-Chirp3-HD-Kore",
        "3" => "en-US-Chirp3-HD-Puck",
        _ => "en-US-Chirp3-HD-Charon",
    };
    config.speech.voice = Some(voice.to_string());
    config.speech.key = Some(key);

    println!();
    print!("  {} ", dim("Trying it…"));
    std::io::stdout().flush()?;
    match crate::speech::test(&config.speech) {
        Ok(()) => {
            println!("{}", accent("heard it?"));
            chose(&format!(
                "{voice}, at {} words a minute",
                config.speech.rate
            ));
        }
        Err(why) => {
            println!();
            println!("  {} {}", dim("!"), dim(&why.to_string()));
            println!(
                "  {}",
                dim("Written anyway, so you can fix the key and try again.")
            );
        }
    }
    Ok(())
}

/// Which voice reads a deck aloud.
///
/// A deck can be walked live — `l` in the window — and that is worth one
/// question here, because the answer is a download rather than a setting and
/// nobody finds it by looking.
///
/// The gap between the voices a machine has by default and the neural ones is
/// most of the difference between listening to a deck and switching it off after
/// a paragraph. Deck would rather say that plainly than pick a compact voice,
/// let it sound like 1998, and have the reader conclude the feature is bad.
fn listen(config: &mut Config) -> anyhow::Result<()> {
    if !cfg!(target_os = "macos") {
        return Ok(());
    }
    let voices = crate::speech::voices();
    if voices.is_empty() {
        return Ok(());
    }

    println!();
    rule();
    println!();
    println!("  {}", bold("Reading aloud"));
    println!(
        "  {}",
        dim("`l` goes live: the rail slides in, and the narration is read to you.")
    );
    println!("  {}", dim("Spoken on this machine —"));
    println!(
        "  {}",
        dim("nothing about your code is sent anywhere to be turned into sound.")
    );
    println!();

    let natural: Vec<_> = voices.iter().filter(|voice| voice.natural()).collect();
    if natural.is_empty() {
        // The honest version. There is nothing to choose between here, and
        // offering a list of compact voices would imply there was.
        println!(
            "  {}",
            dim("Only the compact voices are installed, and they sound it.")
        );
        println!("  {}", dim("The good ones are a free download:"));
        println!();
        println!("    {}", accent(crate::speech::WHERE));
        println!();
        println!(
            "  {}",
            dim("Then `deck setup` again and it will be offered here.")
        );
        println!(
            "  {}",
            dim("Or point deck at any other voice in ~/.deck/config.toml:")
        );
        println!("    {}", accent("[speech]"));
        println!("    {}", accent("engine  = \"command\""));
        println!(
            "    {}",
            accent("command = \"kokoro-cli --voice af_heart -\"")
        );
        return Ok(());
    }

    let best = natural[0].name.clone();
    println!("  {}", dim(&format!("Found {}.", list_voices(&natural))));
    println!();
    if ask_yes(&format!("Read decks with {best}?"))? {
        config.speech.voice = Some(best.clone());
        chose(&format!("{best}, at {} words a minute", config.speech.rate));
    } else {
        config.speech.voice = None;
        chose("the system voice");
    }
    Ok(())
}

/// The voices found, as a sentence.
fn list_voices(voices: &[&crate::speech::Installed]) -> String {
    let named: Vec<&str> = voices.iter().take(3).map(|one| one.name.as_str()).collect();
    match voices.len() {
        0 => String::new(),
        n if n > 3 => format!("{}, and {} more", named.join(", "), n - 3),
        _ => named.join(", "),
    }
}

/// Offer to put the skill where each agent will read it.
///
/// The commands are the whole integration, but an agent that has never heard of
/// deck writes a summary in chat instead — because that is what it has always
/// done. The skill is the sentence that makes it reach for a deck, and a
/// sentence in a README is a sentence the agent never sees.
fn tell() -> anyhow::Result<()> {
    let Some(home) = deck_core::home::home() else {
        return Ok(());
    };
    let agents = crate::skill::found(&home);
    if agents.is_empty() {
        return Ok(());
    }

    println!();
    rule();
    println!();
    println!("  {}", bold("Your agents"));
    println!(
        "  {}",
        dim("A skill telling them when to reach for a deck, and how to write one.")
    );
    println!(
        "  {}",
        dim("Without it they will keep summarising in chat.")
    );
    println!();

    let names = agents
        .iter()
        .map(|agent| agent.name)
        .collect::<Vec<_>>()
        .join(", ");
    // Said differently the second time. A reader running setup again to change
    // a colour should not be asked a question that reads as though nothing
    // happened last time.
    let again = agents.iter().all(|agent| agent.told(&home));
    let question = if again {
        format!("Update the skill for {names}?")
    } else {
        format!("Install it for {names}?")
    };
    if !ask_yes(&question)? {
        println!();
        println!(
            "  {}",
            dim("Nothing written. `deck setup` again when you want it.")
        );
        return Ok(());
    }

    println!();
    for told in crate::skill::install_all(&home)? {
        match told {
            crate::skill::Told::Written(at) => {
                println!("    {} {}", accent("+"), short(&at, &home));
            }
            crate::skill::Told::Linked(at) => {
                println!("    {} {}", accent("→"), short(&at, &home));
            }
            crate::skill::Told::Reads(name) => {
                println!(
                    "    {} {}",
                    accent("·"),
                    dim(&format!("{name} reads it already"))
                );
            }
            // One agent's directory refusing the link is not a reason to skip
            // the rest.
            crate::skill::Told::Refused(name, why) => {
                println!("    {} {}", dim("!"), dim(&format!("{name}: {why}")));
            }
        }
    }
    Ok(())
}

/// Offer to catch the replies the skill does not.
///
/// The skill is the sentence that makes an agent reach for a deck, and it
/// works often enough to be worth installing. It does not always work. An agent
/// that has just finished an investigation has enormous momentum toward typing
/// out what it found, and a suggestion loses to that momentum often enough that
/// a reader who installed deck can go a week without seeing one.
///
/// This is the answer to that, and it is offered separately because it is a
/// different kind of thing: the skill advises, this one refuses. Somebody who
/// wants the tool available but not insistent should be able to say no here and
/// keep everything else.
fn catch() -> anyhow::Result<()> {
    let Some(home) = deck_core::home::home() else {
        return Ok(());
    };
    if !crate::hook::possible(&home) {
        return Ok(());
    }

    println!();
    rule();
    println!();
    println!("  {}", bold("The catch"));
    println!(
        "  {}",
        dim("A skill is a suggestion, and an agent mid-flow will talk past it.")
    );
    println!(
        "  {}",
        dim("This reads each finished reply, and when one names two or more")
    );
    println!(
        "  {}",
        dim("file:line locations, sends it back to build the deck instead.")
    );
    println!();

    if crate::hook::installed(&home) {
        println!("  {} {}", accent("·"), dim("already on"));
        return Ok(());
    }
    if !ask_yes("Turn it on for Claude Code?")? {
        println!();
        println!(
            "  {}",
            dim("Left off. `deck setup` again when you want it.")
        );
        return Ok(());
    }

    println!();
    match crate::hook::install(&home)? {
        crate::hook::Put::Written(at) => {
            println!("    {} {}", accent("+"), short(&at, &home));
            println!(
                "    {}",
                dim("Takes effect in agent sessions started from now on.")
            );
        }
        crate::hook::Put::Already => println!("    {} {}", accent("·"), dim("already on")),
        crate::hook::Put::Absent => {}
    }
    Ok(())
}

/// What to do next, and where the answers went.
fn close(config: &Path) {
    println!();
    rule();
    println!();
    println!("  {}", bold("Try it"));
    println!("    {}", accent("deck new --title \"…\""));
    println!(
        "    {}",
        dim("or just ask your agent to walk you through a change.")
    );
    println!();
    println!("  {}", dim(&format!("Answers: {}", short_home(config))));
    println!("  {}", dim("Run `deck setup` again to change them."));
    println!();
}

// ─── asking ─────────────────────────────────────────────────────────────────

/// Ask a yes-or-no. Anything but a plain no is a yes.
fn ask_yes(question: &str) -> anyhow::Result<bool> {
    let answer = ask(question, "yes")?;
    Ok(!matches!(answer.to_lowercase().as_str(), "n" | "no"))
}

/// Ask, and read a line back. `default` is what an empty answer means, and is
/// shown so nobody has to guess what Enter does.
fn ask(question: &str, default: &str) -> anyhow::Result<String> {
    print!("  {question} {} ", dim(&format!("[{default}]")));
    std::io::stdout().flush()?;

    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer)?;

    // A person's Enter ends the prompt's line, so whatever comes next starts on
    // a fresh one. Piped input carries no such keystroke, and everything after
    // it would otherwise be written onto the end of the question.
    if !std::io::stdin().is_terminal() {
        println!();
    }
    Ok(answer.trim().to_string())
}

/// Say back what was just chosen.
///
/// Every question, immediately. A setup that asks four things and reports at
/// the end makes the reader hold their own answers in their head.
fn chose(what: &str) {
    println!("  {} {}", accent("✓"), dim(what));
}

// ─── ink ────────────────────────────────────────────────────────────────────

/// Whether to colour anything at all.
///
/// Not when the output is a pipe — a log with escape codes in it is a log
/// somebody has to clean — and not when `NO_COLOR` is set, which is the one
/// convention every terminal program agrees on.
fn ink() -> bool {
    std::io::stdout().is_terminal() && std::env::var_os("NO_COLOR").is_none()
}

/// Deck's accent, and nothing else.
fn accent(text: &str) -> String {
    paint(text, "\x1b[38;5;166m")
}

fn bold(text: &str) -> String {
    paint(text, "\x1b[1m")
}

fn dim(text: &str) -> String {
    paint(text, "\x1b[2m")
}

fn paint(text: &str, with: &str) -> String {
    if ink() {
        format!("{with}{text}\x1b[0m")
    } else {
        text.to_string()
    }
}

/// Deck's own mark: a line, and a lit line under it.
///
/// The same two bars the bar in the corner of the screen wears, which is the
/// only place deck has a logo at all.
fn mark() -> String {
    format!("{}{}", dim("▔"), accent("▁"))
}

/// A rule the width of the words, not of the terminal.
fn rule() {
    println!("  {}", dim(&"─".repeat(64)));
}

/// A path with this machine's home directory taken out of it.
fn short_home(at: &Path) -> String {
    deck_core::home::home().map_or_else(|| at.display().to_string(), |home| short(at, &home))
}

/// A path with the home directory taken out of it.
fn short(at: &Path, home: &Path) -> String {
    at.strip_prefix(home).map_or_else(
        |_| at.display().to_string(),
        |rest| format!("~/{}", rest.display()),
    )
}

/// The editors found, as a sentence.
fn list(editors: &[Editor]) -> String {
    editors
        .iter()
        .map(|editor| editor.name())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Write the config, keeping the reader's own comments out of harm's way.
///
/// Written afresh rather than edited: the file is small, every field has a
/// default, and a rewriter that tried to preserve comments would be a TOML
/// editor rather than a preference. A reader who has hand-written more than
/// this asks for should edit the file directly, which is why the last line
/// says where it is.
fn write(path: &Path, config: &Config) -> anyhow::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let text = toml::to_string_pretty(config)?;
    std::fs::write(path, text)?;
    Ok(())
}

/// Say this exists, to somebody who has never run it.
///
/// Once, on the way past, and to stderr — the agent's own output is stdout and
/// a line of advice in the middle of a review would be a parse error somewhere.
pub fn mention_once() {
    if crate::config::path().is_some_and(|path| path.exists()) {
        return;
    }
    eprintln!(
        "deck: run `deck setup` to choose how deck looks, and to tell your agents it exists."
    );
}
