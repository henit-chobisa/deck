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
use std::path::PathBuf;

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
pub(crate) fn init() -> Option<PathBuf> {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;

    [config.join("nvim/init.lua"), config.join("nvim/init.vim")]
        .into_iter()
        .find(|path| path.is_file())
}

/// One highlight group, as nvim reported it.
#[derive(Debug, Clone, Default, serde::Deserialize)]
struct Group {
    fg: Option<String>,
    bg: Option<String>,
}

/// Read the reader's Neovim colours.
///
/// # Errors
///
/// When nvim is not installed, will not start, or answers with something that
/// is not the JSON this asked for.
pub fn read() -> anyhow::Result<Imported> {
    let script = std::env::temp_dir().join("deck-ask-nvim.lua");
    std::fs::write(&script, ASK)?;

    // The config named outright, not left to nvim to find.
    //
    // `nvim -l` loads a *minimal* init of its own, not the reader's, and comes
    // back with the built-in scheme — which was how this first reported a dark
    // page for a reader whose editor is light. Naming the file is what makes it
    // the reader's colours rather than nvim's.
    let mut asking = std::process::Command::new("nvim");
    if let Some(init) = init() {
        asking.arg("-u").arg(init);
    }
    let asked = asking
        .args(["--headless", "-l"])
        .arg(&script)
        .output()
        .map_err(|err| anyhow::anyhow!("cannot run nvim: {err}"))?;
    let _ = std::fs::remove_file(&script);

    // `-l` writes the script's own output to stdout and nvim's chatter to
    // stderr, so the two do not have to be told apart afterwards.
    let text = String::from_utf8_lossy(&asked.stdout);
    let groups: HashMap<String, Group> = serde_json::from_str(text.trim())
        .map_err(|err| anyhow::anyhow!("nvim answered with something else: {err}"))?;

    from_groups(&groups)
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

    #[test]
    fn a_scheme_without_a_page_is_refused() {
        // Not guessed at. Every surface deck paints is computed from the page,
        // and a guess there is a guess everywhere.
        let nothing = answered(r##"{ "Comment": { "fg": "#888888" } }"##);
        assert!(from_groups(&nothing).is_err());
    }
}
