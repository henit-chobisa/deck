//! Asking a headless Neovim what its colours actually are.
//!
//! Neovim has no theme file. A colour scheme is a script — it may branch on the
//! terminal, on the time of day, on another plugin having loaded — so the only
//! honest way to know what it produced is to run it and look.
//!
//! So deck starts a headless nvim with the reader's own config, asks the
//! highlight groups it cares about, and reads the answer back as JSON. It costs
//! a subprocess and a second of startup, once, at the moment a deck opens.
//!
//! The alternative was parsing colour scheme Lua, which cannot work in general
//! and would be wrong in exactly the interesting cases.

use std::collections::HashMap;
use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use deck_core::theme::{Imported, Rgb, Syntax, pick_accent};

/// The script handed to nvim.
///
/// Written out rather than shipped as a file: it has to survive being installed
/// anywhere, and a script deck cannot find is a theme deck cannot import.
///
/// `nvim_get_hl` with `link = false` resolves a group to the colours it ends up
/// with rather than the group it points at, which is what a link chain three
/// deep would otherwise hide.
const ASK: &str = r##"
local function hex(n) return n and string.format("#%06x", n) or nil end
local out = {}
for _, g in ipairs({
  "Normal", "Comment", "Keyword", "String", "Function", "Type", "Constant",
  "Special", "DiffAdd", "DiffDelete", "DiffText", "Search", "Visual",
}) do
  local ok, hl = pcall(vim.api.nvim_get_hl, 0, { name = g, link = false })
  if ok and not vim.tbl_isempty(hl) then
    out[g] = { fg = hex(hl.fg), bg = hex(hl.bg) }
  end
end
out.background = { fg = vim.o.background }
io.write(vim.json.encode(out))
"##;

/// The reader's own init, if they have one.
///
/// `XDG_CONFIG_HOME` first, because a reader who set it meant it.
///
/// Then both of the directories Windows keeps. nvim puts its init under
/// `%LOCALAPPDATA%`, which is not where a config directory is usually asked
/// for — so asking correctly and taking the first answer still read the wrong
/// place. Both are looked at, and whichever holds a file wins. On every other
/// platform the two are the same directory and the second look costs nothing.
pub(crate) fn init() -> Option<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();
    roots.extend(deck_core::home::dot_config());
    roots.extend(deck_core::home::local_config());
    roots.dedup();

    roots
        .into_iter()
        .flat_map(|at| [at.join("nvim/init.lua"), at.join("nvim/init.vim")])
        .find(|path| path.is_file())
}

/// One highlight group, as nvim reported it.
#[derive(Debug, Clone, Default, serde::Deserialize)]
struct Group {
    fg: Option<String>,
    bg: Option<String>,
}

/// How long [`read`] waits for nvim before giving up on it.
///
/// Generous, because this is the limit for `deck open` and the command line,
/// where the reader is waiting on deck anyway and a config that installs its
/// plugins on first start can take a while. A sheet that wants an answer
/// sooner asks [`read_within`] with its own.
pub const PATIENCE: Duration = Duration::from_secs(30);

/// Read the reader's Neovim colours, waiting at most [`PATIENCE`].
///
/// # Errors
///
/// As [`read_within`].
pub fn read() -> anyhow::Result<Imported> {
    read_within(PATIENCE)
}

/// Read the reader's Neovim colours, giving nvim `limit` to answer.
///
/// A config can hang — a plugin waiting on the network, a prompt nobody will
/// ever see in a headless editor — and an nvim that never answers used to be a
/// process that never ended and a thread that never came back. So it is
/// stopped when the time is up, and waited on, and the error says so in words
/// the reader can act on.
///
/// # Errors
///
/// When nvim is not installed, will not start, does not answer within `limit`,
/// or answers with something that is not the JSON this asked for.
pub fn read_within(limit: Duration) -> anyhow::Result<Imported> {
    // A file of its own each time. Two reads at once — a sheet asking while
    // the window is still starting, or two decks — would otherwise write the
    // same file and remove it from under each other.
    static ASKED: AtomicU64 = AtomicU64::new(0);
    let script = std::env::temp_dir().join(format!(
        "deck-ask-nvim-{}-{}.lua",
        std::process::id(),
        ASKED.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&script, ASK)?;

    // The config named outright, not left to nvim to find.
    //
    // `nvim -l` loads a *minimal* init of its own, not the reader's, and comes
    // back with the built-in scheme — which was how this first reported a dark
    // page for a reader whose editor is light. Naming the file is what makes it
    // the reader's colours rather than nvim's.
    let mut asking = Command::new("nvim");
    if let Some(init) = init() {
        asking.arg("-u").arg(init);
    }
    asking.args(["--headless", "-l"]).arg(&script);
    let answered = answer_within(asking, limit);
    let _ = std::fs::remove_file(&script);

    let stdout = match answered {
        Ok(Some(stdout)) => stdout,
        Ok(None) => anyhow::bail!("Neovim did not answer in {}", seconds(limit)),
        Err(err) => anyhow::bail!("cannot run nvim: {err}"),
    };

    // `-l` writes the script's own output to stdout and nvim's chatter to
    // stderr, so the two do not have to be told apart afterwards.
    let text = String::from_utf8_lossy(&stdout);
    let groups: HashMap<String, Group> = serde_json::from_str(text.trim())
        .map_err(|err| anyhow::anyhow!("nvim answered with something else: {err}"))?;

    from_groups(&groups)
}

/// `limit` the way a person says it: "5 seconds", "1 second".
fn seconds(limit: Duration) -> String {
    match limit.as_secs() {
        1 => "1 second".to_string(),
        whole => format!("{whole} seconds"),
    }
}

/// Run `command` and collect what it writes to stdout, giving it `limit` to
/// finish; `None` when it did not, and it has been stopped.
///
/// Stdout is read on a thread of its own while this one watches the clock.
/// Reading it here instead would mean the clock is not looked at until the
/// child writes or closes, which is exactly what a hung child never does; and
/// only waiting, without reading, lets a child that writes more than a pipe
/// holds block on the write and look hung when it is not.
///
/// The thread is not joined. A child can hand its stdout to something it
/// starts — a language server, a plugin's job — and that can outlive it, so
/// the pipe may stay open after the child has gone. The answer is taken from a
/// channel with what is left of the time instead, and the thread ends on its
/// own when the last holder of the pipe lets go.
///
/// On time running out the child is killed *and* waited on, so it does not
/// linger as a zombie for the rest of deck's life.
///
/// # Errors
///
/// When the command cannot be started.
fn answer_within(mut command: Command, limit: Duration) -> std::io::Result<Option<Vec<u8>>> {
    let deadline = Instant::now() + limit;
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;

    let (said, heard) = mpsc::channel();
    if let Some(mut stdout) = child.stdout.take() {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let _ = stdout.read_to_end(&mut bytes);
            let _ = said.send(bytes);
        });
    }

    // Short naps rather than one long wait: the standard library has no wait
    // with a deadline, and a tenth of the limit's worth of latency is not
    // something anyone waiting on an editor to start will notice.
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {}
            // Not knowing whether it has finished is no reason to leave it
            // running: stopped and waited on like one that ran over, so the
            // error does not leave a process or a zombie behind it.
            Err(err) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(err);
            }
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(None);
        }
        std::thread::sleep(Duration::from_millis(20));
    }

    // Finished in time; what it wrote, if the pipe closes before the time is
    // up. A pipe held open by something the child started counts as no answer.
    let left = deadline.saturating_duration_since(Instant::now());
    Ok(heard.recv_timeout(left).ok())
}

/// Turn nvim's highlight groups into what deck imports.
///
/// Free of the subprocess so it can be checked against a real theme's output
/// without one — which matters, because the mapping is the part with opinions
/// in it and the subprocess is the part that merely works.
fn from_groups(groups: &HashMap<String, Group>) -> anyhow::Result<Imported> {
    let colour = |name: &str, of: fn(&Group) -> &Option<String>| -> Option<Rgb> {
        groups
            .get(name)
            .and_then(|group| of(group).as_deref())
            .and_then(Rgb::from_hex)
    };
    let fg_of = |name: &str| colour(name, |group| &group.fg);
    let bg_of = |name: &str| colour(name, |group| &group.bg);

    // `Normal` is the one group every colour scheme sets, and the one deck
    // cannot do without: every surface is computed from the page.
    let bg = bg_of("Normal")
        .ok_or_else(|| anyhow::anyhow!("this colour scheme sets no Normal background"))?;
    let fg = fg_of("Normal")
        .ok_or_else(|| anyhow::anyhow!("this colour scheme sets no Normal foreground"))?;

    // Everything the scheme has that means *look here*, best first. Which of
    // them exists depends on the scheme, and whether any is usable depends on
    // the colours — `pick_accent` decides, because that rule is the same for
    // every editor.
    let accent = pick_accent(
        bg,
        fg,
        &[
            bg_of("Search"),
            bg_of("DiffText"),
            fg_of("Special"),
            fg_of("Type"),
            fg_of("Constant"),
            fg_of("Keyword"),
            bg_of("Visual"),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>(),
    );

    Ok(Imported {
        bg,
        fg,
        accent,
        comment: fg_of("Comment"),
        // The *mark*, not the ground: nvim's diff groups colour the whole line,
        // and deck makes its own ground from the page. What it wants here is
        // ink, so the foreground when the group has one.
        add: fg_of("DiffAdd"),
        del: fg_of("DiffDelete"),
        syntax: Syntax {
            keyword: fg_of("Keyword"),
            string: fg_of("String"),
            function: fg_of("Function"),
            type_: fg_of("Type"),
            literal: fg_of("Constant"),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn answered(json: &str) -> HashMap<String, Group> {
        serde_json::from_str(json).expect("this is the shape nvim answers in")
    }

    /// A real answer, from a real config: the light scheme this project was
    /// built against.
    const REAL: &str = r##"{
      "Normal": { "bg": "#dfdfdf", "fg": "#2d2d2d" },
      "Comment": { "fg": "#6e7781" },
      "Keyword": { "fg": "#0b0080" },
      "String": { "fg": "#800000" },
      "Function": { "fg": "#000000" },
      "Type": { "fg": "#ae6000" },
      "Constant": { "fg": "#800000" },
      "Visual": { "bg": "#c5c9d6" },
      "DiffAdd": { "bg": "#bce8bc" },
      "DiffDelete": { "bg": "#f2c2c2" }
    }"##;

    #[test]
    fn a_real_theme_reads() {
        let got = from_groups(&answered(REAL)).expect("Normal is set, so this imports");

        assert_eq!(got.bg, Rgb::new(0xdf, 0xdf, 0xdf));
        assert_eq!(got.fg, Rgb::new(0x2d, 0x2d, 0x2d));
        assert_eq!(got.comment, Rgb::from_hex("#6e7781"));
        assert_eq!(got.syntax.keyword, Rgb::from_hex("#0b0080"));
    }

    #[test]
    fn the_accent_is_the_first_colour_the_scheme_offers() {
        // This scheme's `Special` is pure black and its `Visual` is a pale
        // blue-grey; neither is an accent. Its `Type` is a burnt orange, and
        // that is what the reader's eye is trained on.
        let got = from_groups(&answered(REAL)).unwrap();
        assert_eq!(got.accent, Rgb::from_hex("#ae6000").unwrap());
    }

    #[test]
    fn a_search_hit_outranks_everything() {
        // It is the one colour in a scheme whose whole job is *look here*.
        let with_search = answered(
            r##"{
            "Normal": { "bg": "#000000", "fg": "#ffffff" },
            "Search": { "bg": "#ffcc00" },
            "Type": { "fg": "#8ec07c" }
        }"##,
        );
        assert_eq!(
            from_groups(&with_search).unwrap().accent,
            Rgb::from_hex("#ffcc00").unwrap()
        );
    }

    #[test]
    fn a_diff_group_with_only_a_ground_gives_no_mark() {
        // nvim colours a whole diff line, and deck makes its own ground from
        // the page. What it wants is ink, so a group with only a background
        // contributes nothing rather than a background pretending to be one.
        let got = from_groups(&answered(REAL)).unwrap();
        assert_eq!(got.add, None);
        assert_eq!(got.del, None);
    }

    #[cfg(unix)]
    #[test]
    fn a_command_that_does_not_finish_in_time_is_stopped() {
        // The hung nvim, stood in for by something that is certain to hang
        // for longer than it is given. It has to come back near the limit,
        // not after the full ten seconds — which is what waiting on it
        // unstopped would take.
        let started = Instant::now();
        let mut sleeping = Command::new("sleep");
        sleeping.arg("10");
        let got = answer_within(sleeping, Duration::from_millis(300)).expect("sleep starts");
        assert_eq!(got, None);
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "took {:?}",
            started.elapsed()
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_command_that_finishes_in_time_is_heard() {
        let mut saying = Command::new("echo");
        saying.arg("{}");
        let got = answer_within(saying, Duration::from_secs(5)).expect("echo starts");
        assert_eq!(got.as_deref(), Some(&b"{}\n"[..]));
    }

    #[test]
    fn the_limit_reads_the_way_a_person_says_it() {
        assert_eq!(seconds(Duration::from_secs(5)), "5 seconds");
        assert_eq!(seconds(Duration::from_secs(1)), "1 second");
    }

    #[test]
    fn a_scheme_without_a_page_is_refused() {
        // Not guessed at. Every surface deck paints is computed from the page,
        // and a guess there is a guess everywhere.
        let nothing = answered(r##"{ "Comment": { "fg": "#888888" } }"##);
        assert!(from_groups(&nothing).is_err());
    }
}
