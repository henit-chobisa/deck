//! Turning the derived palette into the theme every component paints with.
//!
//! `deck-core` computes colours as plain `Rgb` because it must not know what a
//! renderer is. This is the one place that crosses over — and it does not stop
//! at deck's own chrome.
//!
//! The component library keeps a theme as a global, and the code editor reads
//! its ground, its gutter and every syntax colour from there. Setting a
//! background on the wrapper around an editor therefore changes nothing about
//! the editor: it goes on painting its own default ground, in its own default
//! blues and oranges, inside our chrome. Which looks exactly like what it is.
//!
//! So the derived palette is installed *into* that global. After
//! [`install`] there is one palette in the window, not two.

use deck_core::theme::{Imported, Overrides, Palette, Paper, Rgb, Syntax, derive, tint};
use gpui_kit::component::{Theme, ThemeMode, highlighter::HighlightTheme};
use gpui_kit::{App, Hsla, Rgba, SharedString, px};

/// What the reader asked for, read once.
///
/// A global because it is wanted from every view that paints and settled before
/// any of them exist. Threading a preference that never changes through every
/// constructor would be carrying it a long way to say the same thing.
static CHOSEN: std::sync::OnceLock<Chosen> = std::sync::OnceLock::new();

/// Everything the reader said about how the deck should look.
#[derive(Debug, Clone, Copy, Default)]
pub struct Chosen {
    /// Which neutrals, when nothing is imported.
    pub paper: Paper,
    /// The editor's colours, when one was read.
    pub imported: Option<Imported>,
    /// Anything written by hand, which wins over both.
    pub colors: Overrides,
}

/// Say what this run is painted with. Ignored after the first call.
pub fn choose(chosen: Chosen) {
    let _ = CHOSEN.set(chosen);
}

/// What the reader asked for.
#[must_use]
pub fn asked() -> Chosen {
    CHOSEN.get().copied().unwrap_or_default()
}

/// The palette to draw this deck with, as this run was asked for.
///
/// Three layers, and the order is the whole of the theming rule: the deck's own
/// palette, then the editor's if one was imported, then whatever the reader
/// wrote by hand. Each is more specific than the last, and the last word is the
/// reader's.
#[must_use]
pub fn current(dark: bool) -> Palette {
    let chosen = asked();
    let mut palette = match chosen.imported {
        // Derived, not copied. The editor gives three colours and a few more;
        // the surfaces between them are deck's, or the deck stops looking like
        // a deck the moment somebody imports a theme.
        Some(imported) => derive(imported),
        None => current_on(dark, chosen.paper),
    };
    // Last, over everything. A colour written by hand is the reader saying they
    // know what they want better than the rules do, and the rules do not get a
    // second go at it.
    chosen.colors.apply(&mut palette);
    palette
}

/// The syntax colours to paint code with, as this run was asked for.
#[must_use]
pub fn syntax() -> Syntax {
    asked()
        .imported
        .map(|imported| imported.syntax)
        .unwrap_or_default()
}

/// The palette to draw this deck with.
///
/// These are the design's own values, written out rather than computed.
/// [`derive`](deck_core::theme::derive) exists to adapt to *somebody else's*
/// theme, and it is a good approximation — but an approximation of a palette
/// that already exists is a worse thing to ship than the palette. The two agree
/// to within a few units, which is what
/// `hairlines_and_the_desk_sit_below_every_surface` and its neighbours check.
#[must_use]
pub fn current_on(dark: bool, paper: Paper) -> Palette {
    let hex = |s: &str| Rgb::from_hex(s).unwrap_or(Rgb::new(0, 0, 0));

    /// How far a diff ground is tinted from the page it sits on.
    const DIFF: f32 = 0.17;

    let mut palette = match (dark, paper) {
        (true, Paper::Grey) => Palette {
            bg: hex("#282828"),
            fg: hex("#ebdbb2"),
            band: hex("#191b1b"),
            wash: hex("#1f1f1f"),
            focus: hex("#332b21"),
            accent: hex("#fe8019"),
            on_accent: hex("#282828"),
            add: hex("#b8bb26"),
            del: hex("#fb4934"),
            comment: hex("#a89984"),
            muted: hex("#a89984"),
            edge: hex("#3a3735"),
            ground: hex("#0d0e0e"),
            gone: Rgb::new(0, 0, 0),
            fresh: Rgb::new(0, 0, 0),
        },
        (true, Paper::Warm) => Palette {
            band: hex("#1c1a19"),
            wash: hex("#232120"),
            ground: hex("#121110"),
            ..current_on(true, Paper::Grey)
        },
        (false, Paper::Grey) => Palette {
            bg: hex("#dfdfdf"),
            fg: hex("#3c3836"),
            band: hex("#c6c6c6"),
            wash: hex("#d4d4d4"),
            focus: hex("#d7cbbb"),
            accent: hex("#af3a03"),
            on_accent: hex("#f2e5d5"),
            add: hex("#79740e"),
            del: hex("#9d0006"),
            comment: hex("#7c6f64"),
            muted: hex("#7c6f64"),
            edge: hex("#a9a7a3"),
            ground: hex("#b4b2ae"),
            gone: Rgb::new(0, 0, 0),
            fresh: Rgb::new(0, 0, 0),
        },
        (false, Paper::Warm) => Palette {
            bg: hex("#e2ded7"),
            band: hex("#cbc5bb"),
            wash: hex("#d8d3cb"),
            edge: hex("#aaa49a"),
            ground: hex("#b6afa5"),
            ..current_on(false, Paper::Grey)
        },
    };

    // Tinted from the page, and toward a brighter red and green than the ink
    // that marks the gutter.
    //
    // The ground and the mark are two different jobs. The mark is a glyph the
    // size of a hyphen and has to read on the ground, so it is dark; the ground
    // is a whole line of code's worth of paper and has to be told apart from
    // the page without becoming a colour the syntax has to fight. Tinting the
    // page toward the *dark* ink gave a muddy rose and a muddy olive — the
    // right hue at the wrong brightness.
    let (red, green) = if dark {
        (hex("#fb4934"), hex("#b8bb26"))
    } else {
        (hex("#cc241d"), hex("#98971a"))
    };
    palette.gone = tint(palette.bg, red, DIFF);
    palette.fresh = tint(palette.bg, green, DIFF);
    palette
}

/// A model colour as a GPUI one.
#[must_use]
pub fn paint(colour: Rgb) -> Hsla {
    Rgba {
        r: f32::from(colour.r) / 255.0,
        g: f32::from(colour.g) / 255.0,
        b: f32::from(colour.b) / 255.0,
        a: 1.0,
    }
    .into()
}

/// Make `palette` the theme the whole window paints with.
///
/// Called once, before any view is built. Everything a component reaches for —
/// the editor's ground, the gutter, the line numbers, the syntax — comes from
/// here afterwards.
pub fn install(palette: &Palette, dark: bool, cx: &mut App) {
    let theme = Theme::global_mut(cx);

    theme.mode = if dark {
        ThemeMode::Dark
    } else {
        ThemeMode::Light
    };
    theme.colors.background = paint(palette.bg);
    theme.colors.foreground = paint(palette.fg);
    theme.colors.muted_foreground = paint(palette.muted);
    theme.colors.border = paint(palette.edge);
    // The ground behind an inline `code` span in the narration. The library
    // calls it `accent` and means a quiet surface by it, not the deck's accent
    // — left unset it fell back to something near-white, which on the band
    // read as a hole rather than a chip.
    theme.colors.accent = paint(palette.wash);

    // Code wants to be dense. The component default is sized for prose, and at
    // that size a pane holds a dozen lines and the deck reads as a slideshow.
    theme.mono_font_size = px(12.);
    if let Some(mono) = first_installed(MONO, cx) {
        Theme::global_mut(cx).mono_font_family = mono;
    }
    let theme = Theme::global_mut(cx);
    theme.font_family = SharedString::from("IBM Plex Sans");
    theme.font_size = px(13.2);
    theme.shadow = true;
    theme.radius = px(5.);

    theme.highlight_theme = highlight_theme(palette, dark);

    // The base layer keeps its own copy for the parts it paints itself, and
    // writing to the public fields does not reach it.
    Theme::sync_base(cx);
}

/// The narration face, embedded rather than looked for.
///
/// The design was drawn in IBM Plex Sans, and the difference from the system
/// face is not a preference: Plex is darker at the same weight and tighter at
/// the same size, so prose set in the fallback reads washed out and loose no
/// matter what colour it is given. A deck must look the same on every machine,
/// so the font ships with it. IBM Plex is under the SIL Open Font License;
/// `fonts/LICENSE-IBM-Plex.txt` travels with the files.
const PLEX: &[(&[u8], &str)] = &[
    (include_bytes!("../fonts/IBMPlexSans-400.ttf"), "regular"),
    (include_bytes!("../fonts/IBMPlexSans-500.ttf"), "medium"),
    (include_bytes!("../fonts/IBMPlexSans-600.ttf"), "semibold"),
];

/// Load the embedded faces. Called once, before any window is opened.
pub fn embed_fonts(cx: &App) {
    let faces = PLEX
        .iter()
        .map(|(bytes, _)| std::borrow::Cow::Borrowed(*bytes))
        .collect();
    if let Err(err) = cx.text_system().add_fonts(faces) {
        // A missing face is a worse-looking deck, not a broken one.
        eprintln!("deck: could not load the narration font: {err}");
    }
}

/// Monospace faces to try, best first.
///
/// Named rather than left to the platform default, because a deck is mostly
/// code and the face it is set in is most of what the reader sees. Anything
/// missing is skipped, so a machine without any of them still gets whatever the
/// theme already had.
const MONO: &[&str] = &[
    "JetBrainsMono Nerd Font Mono",
    "JetBrains Mono",
    "Iosevka Fixed",
    "Berkeley Mono",
    "SF Mono",
    "Menlo",
];

/// The first of `names` the machine actually has.
fn first_installed(names: &[&str], cx: &App) -> Option<SharedString> {
    let installed = cx.text_system().all_font_names();
    names
        .iter()
        .find(|name| installed.iter().any(|have| have == *name))
        .map(|name| SharedString::from((*name).to_string()))
}

/// The editor's own theme, built from the palette.
///
/// Assembled as JSON and parsed back rather than constructed field by field.
/// `SyntaxColors` has some forty of them, most irrelevant here, and this is the
/// same shape a real editor theme arrives in — so importing one later is this
/// function with its middle replaced, not a rewrite.
fn highlight_theme(palette: &Palette, dark: bool) -> std::sync::Arc<HighlightTheme> {
    let hex = |c: Rgb| c.to_hex();

    // Syntax stays quiet. A review surface is read for its *shape* — what is
    // lit, what is washed back — and a rainbow underneath competes with the one
    // thing the deck is pointing at.
    let quiet = if dark {
        ("#d3869b", "#b8bb26", "#fabd2f", "#d3869b", "#8ec07c")
    } else {
        ("#8f3f71", "#79740e", "#b57614", "#8f3f71", "#427b58")
    };

    // The editor's own, where it had them, and deck's quiet set where it did
    // not. Mixed rather than all-or-nothing: a theme that names four of the
    // five should give four, not none.
    let imported = syntax();
    let taken = |had: Option<Rgb>, quiet: &str| had.map_or_else(|| quiet.to_string(), Rgb::to_hex);
    let keyword = taken(imported.keyword, quiet.0);
    let string = taken(imported.string, quiet.1);
    let function = taken(imported.function, quiet.2);
    let literal = taken(imported.literal, quiet.3);
    let type_ = taken(imported.type_, quiet.4);

    let json = format!(
        r#"{{
          "name": "deck",
          "appearance": "{mode}",
          "style": {{
            "editor.background": "{wash}",
            "editor.foreground": "{fg}",
            "editor.gutter.background": "{wash}",
            "editor.line_number": "{gutter}",
            "editor.active_line_number": "{muted}",
            "editor.active_line.background": "{wash}",
            "syntax": {{
              "comment":       {{ "color": "{comment}", "font_style": "italic" }},
              "comment.doc":   {{ "color": "{comment}", "font_style": "italic" }},
              "keyword":       {{ "color": "{keyword}" }},
              "string":        {{ "color": "{string}" }},
              "string.special":{{ "color": "{string}" }},
              "function":      {{ "color": "{function}" }},
              "function.method": {{ "color": "{function}" }},
              "constructor":   {{ "color": "{function}" }},
              "type":          {{ "color": "{type_}" }},
              "enum":          {{ "color": "{type_}" }},
              "number":        {{ "color": "{literal}" }},
              "boolean":       {{ "color": "{literal}" }},
              "constant":      {{ "color": "{literal}" }},
              "attribute":     {{ "color": "{muted}" }},
              "operator":      {{ "color": "{muted}" }},
              "punctuation":   {{ "color": "{muted}" }},
              "variable":      {{ "color": "{fg}" }},
              "property":      {{ "color": "{fg}" }}
            }}
          }}
        }}"#,
        mode = if dark { "dark" } else { "light" },
        wash = hex(palette.wash),
        fg = hex(palette.fg),
        // The gutter recedes further than any label: line numbers are there to
        // be referred to, not read.
        gutter = hex(palette.fg.mix(palette.wash, 0.62)),
        muted = hex(palette.muted),
        comment = hex(palette.fg.mix(palette.wash, 0.42)),
    );

    serde_json::from_str(&json).map_or_else(
        // A palette cannot make this JSON invalid, so a failure here is a typo
        // in the literal above — and a deck rendered in the library's default
        // colours is better than one that does not open.
        |_| {
            if dark {
                HighlightTheme::default_dark()
            } else {
                HighlightTheme::default_light()
            }
        },
        std::sync::Arc::new,
    )
}
