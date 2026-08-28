//! The palette, derived rather than configured.
//!
//! Four rules, in the order they matter. They are the visual identity, and
//! every one of them is here because the obvious alternative was tried first
//! and looked broken.
//!
//! **1. Derive, never link.** A theme designs its own chrome for its own
//! windows. Borrowing a colour it chose for a status bar paints a black slab
//! across a light page. So deck imports an editor's *code* colours — the
//! background, the foreground, the syntax — and computes its own chrome from
//! them.
//!
//! **2. The paper moves, not the ink.** Code is never recoloured to dim it.
//! Overriding foregrounds does work, and it collapses every syntax colour to
//! one flat grey, which reads as broken rather than quiet. Only backgrounds
//! change.
//!
//! **3. Recede means darker.** Receding *away from the text* fails on a light
//! theme: a `#dfdfdf` page is four units from white, so every surface lands on
//! top of every other. Down has room in both directions — until the ground is
//! already black, which is the one exception and is handled below.
//!
//! **4. Give the cue twice.** Tone alone depends on a theme having contrast
//! where you assumed it. Every lit range also carries an accent bar in the
//! gutter, so it is findable where the tint is too subtle to see. That rule
//! lives in the views; the accent it uses comes from here.

use serde::{Deserialize, Serialize};

/// An 8-bit-per-channel colour.
///
/// On the wire it is a string — `"#af3a03"` — because the only place a colour
/// is written by hand is a config file, and nobody writes `{ r = 175, g = 58 }`
/// there. A name this build does not understand is refused rather than guessed
/// at: a typo that silently painted black would be blamed on the design.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Rgb {
    /// Red.
    pub r: u8,
    /// Green.
    pub g: u8,
    /// Blue.
    pub b: u8,
}

impl Rgb {
    /// A colour from its channels.
    #[must_use]
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// A colour from `#rrggbb` or `#rgb`, with or without the `#`.
    #[must_use]
    pub fn from_hex(hex: &str) -> Option<Self> {
        let hex = hex.trim().trim_start_matches('#');
        let byte = |s: &str| u8::from_str_radix(s, 16).ok();

        match hex.len() {
            // Eight digits is six with an alpha on the end, and deck has no
            // use for the alpha: every colour here is opaque by the time it is
            // painted. Zed writes *every* colour this way, so without this a
            // real Zed theme parsed as no colours at all.
            6 | 8 => Some(Self::new(
                byte(&hex[0..2])?,
                byte(&hex[2..4])?,
                byte(&hex[4..6])?,
            )),
            // `#abc` means `#aabbcc`, so each digit is doubled rather than
            // shifted — `0xa` becomes `0xaa`, not `0xa0`. Four is the same with
            // an alpha.
            3 | 4 => {
                let digit = |i: usize| byte(&hex[i..=i]).map(|v| v * 17);
                Some(Self::new(digit(0)?, digit(1)?, digit(2)?))
            }
            _ => None,
        }
    }

    /// Perceived lightness, from 0 for black to 1 for white.
    ///
    /// Weighted for the eye, which reads green as far brighter than blue at the
    /// same numeric value.
    #[must_use]
    pub fn lightness(&self) -> f32 {
        let [r, g, b] = [self.r, self.g, self.b].map(f32::from);
        (0.2126 * r + 0.7152 * g + 0.0722 * b) / 255.0
    }

    /// How much colour there is in it, from 0 for a grey to 1 for a pure hue.
    #[must_use]
    pub fn saturation(&self) -> f32 {
        let [r, g, b] = [self.r, self.g, self.b].map(f32::from);
        let high = r.max(g).max(b);
        let low = r.min(g).min(b);
        if high <= f32::EPSILON {
            0.0
        } else {
            (high - low) / high
        }
    }

    /// Whether this colour is dark enough to want light text on it.
    #[must_use]
    pub fn is_dark(&self) -> bool {
        self.lightness() < 0.5
    }

    /// This colour blended `amount` of the way toward `other`, where 0 is
    /// unchanged and 1 is `other` exactly.
    ///
    /// Blending happens in plain sRGB. That is not perceptually even, but every
    /// blend here is small — twelve percent at the most — and at that size the
    /// difference from a perceptual space is not visible. If the palette ever
    /// needs large blends, this is the function to fix first.
    #[must_use]
    pub fn mix(self, other: Self, amount: f32) -> Self {
        let t = amount.clamp(0.0, 1.0);
        let blend = |a: u8, b: u8| {
            let mixed = f32::from(a) + (f32::from(b) - f32::from(a)) * t;
            // `clamp` before the cast: a saturating conversion is what keeps a
            // rounding error from wrapping 255.4 round to 0.
            saturate(mixed)
        };
        Self::new(
            blend(self.r, other.r),
            blend(self.g, other.g),
            blend(self.b, other.b),
        )
    }

    /// `#rrggbb`.
    #[must_use]
    pub fn to_hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

/// The colours the editor gave us, before deck computes anything.
///
/// This is rule 1's boundary: everything here is imported, and everything in
/// [`Palette`] that is not repeated from here is derived.
///
/// Three are required and the rest are not, because three is what any editor
/// can be counted on to have and the rest is what a good one also has. An
/// import that found only the three still produces a whole palette — deck's own
/// rules fill the gaps, which is what they were written for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Imported {
    /// The editor's background. Every surface is computed from this.
    pub bg: Rgb,
    /// The editor's foreground.
    pub fg: Rgb,
    /// The colour a lit range is tinted toward, and the gutter bar's colour.
    pub accent: Rgb,
    /// What the editor writes comments in.
    pub comment: Option<Rgb>,
    /// What it marks an added line in.
    pub add: Option<Rgb>,
    /// And a removed one.
    pub del: Option<Rgb>,
    /// What it writes code in. See [`Syntax`].
    pub syntax: Syntax,
}

/// The colours a code pane paints code with.
///
/// Deliberately five, where an editor theme has forty. A review surface is read
/// for its *shape* — what is lit, what is washed back — and a rainbow underneath
/// competes with the one thing the deck is pointing at. Five is enough that the
/// code looks like the reader's code, and few enough that it stays quiet.
///
/// All optional: an import that found none of them leaves the panes in deck's
/// own set, which is a worse likeness and not a worse deck.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Syntax {
    /// `fn`, `let`, `if`.
    pub keyword: Option<Rgb>,
    /// Anything in quotes.
    pub string: Option<Rgb>,
    /// A name being called or defined.
    pub function: Option<Rgb>,
    /// A named type.
    pub type_: Option<Rgb>,
    /// A number, a boolean, a constant.
    pub literal: Option<Rgb>,
}

impl Imported {
    /// The three every editor has, and nothing else.
    ///
    /// What an import starts from, and what a caller writes when it only has
    /// these — which is most of them, and every test.
    #[must_use]
    pub fn plain(bg: Rgb, fg: Rgb, accent: Rgb) -> Self {
        Self {
            bg,
            fg,
            accent,
            comment: None,
            add: None,
            del: None,
            syntax: Syntax::default(),
        }
    }
}

/// Every colour deck draws with.
///
/// Ten names, and it stays ten. A palette that needs an eleventh usually means
/// a view is inventing a distinction the design does not have.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    /// The page: true editor background, undimmed.
    pub bg: Rgb,
    /// Text.
    pub fg: Rgb,
    /// The narration band and the legend — a distinct panel.
    pub band: Rgb,
    /// A code pane at rest, scrimmed back.
    pub wash: Rgb,
    /// The lit range: the page again, tinted toward the accent.
    pub focus: Rgb,
    /// The gutter bar, and anything asking to be looked at.
    pub accent: Rgb,
    /// What is written *on* the accent, where the accent is a fill rather than
    /// a mark.
    ///
    /// A separate role because it cannot be worked out at the point of use.
    /// Deck's own accent is dark, so the page colour reads on it — and code
    /// that assumed the page colour would keep on assuming it after somebody
    /// imported a theme with a pale yellow accent, and put pale text on a pale
    /// button. Whichever of the page and the text has further to go from the
    /// accent is the one that goes on it, and that is a question about the
    /// palette, answered once, here.
    pub on_accent: Rgb,
    /// A proposed addition, as ink: the `+` in the gutter.
    pub add: Rgb,
    /// A proposed deletion, as ink: the `−` in the gutter.
    pub del: Rgb,
    /// The ground under a line that is going.
    ///
    /// Tinted paper, not a fill. The same rule the lit range follows — keep the
    /// page's brightness and shift only its hue — because a diff block is code
    /// the reader has to *read*, and a wash of red over it is a colour that
    /// competes with the syntax underneath rather than sitting beneath it.
    pub gone: Rgb,
    /// The ground under a line that is coming.
    pub fresh: Rgb,
    /// A comment marker and its text.
    pub comment: Rgb,
    /// Labels, line numbers, anything said quietly.
    pub muted: Rgb,
    /// Hairlines: the edge of the card, the rule between panes, the border of
    /// a key. Deeper than any surface, because it has to read against all of
    /// them.
    pub edge: Rgb,
    /// The desk the deck lies on. Not part of the deck, so it sits below
    /// everything.
    pub ground: Rgb,
}

impl TryFrom<String> for Rgb {
    type Error = String;

    fn try_from(hex: String) -> Result<Self, Self::Error> {
        Self::from_hex(&hex).ok_or_else(|| format!("`{hex}` is not a colour like `#af3a03`"))
    }
}

impl From<Rgb> for String {
    fn from(colour: Rgb) -> Self {
        colour.to_hex()
    }
}

/// Which set of neutrals the deck is painted on.
///
/// Two, and the difference between them is temperature and nothing else. The
/// steps between the surfaces, the accent, the ink — all the same. A reader
/// picks the one their screen and their room agree with, the way they would
/// pick a paper.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase", from = "String")]
pub enum Paper {
    /// Neutral grey. The design's own, and the default.
    #[default]
    Grey,
    /// The same page, biased toward the accent's hue.
    Warm,
}

impl From<String> for Paper {
    fn from(name: String) -> Self {
        match name.as_str() {
            "warm" => Self::Warm,
            _ => Self::Grey,
        }
    }
}

/// Whether the deck is painted light or dark.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase", from = "String")]
pub enum Mode {
    /// Whatever the machine is set to.
    #[default]
    Auto,
    /// Light, whatever the machine says.
    Light,
    /// Dark, whatever the machine says.
    Dark,
}

impl From<String> for Mode {
    fn from(name: String) -> Self {
        match name.as_str() {
            "light" => Self::Light,
            "dark" => Self::Dark,
            _ => Self::Auto,
        }
    }
}

impl Mode {
    /// Whether to paint dark, given what the machine says.
    #[must_use]
    pub fn is_dark(self, machine: bool) -> bool {
        match self {
            Self::Auto => machine,
            Self::Light => false,
            Self::Dark => true,
        }
    }
}

/// Colours written by hand, each one winning over what was derived.
///
/// One field per role, rather than a map of names to colours. A map would take
/// `[theme.colors] acent = "#af3a03"` without a word and paint the deck exactly
/// as before, leaving the reader to wonder why their override did nothing.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Overrides {
    /// See [`Palette::bg`].
    pub bg: Option<Rgb>,
    /// See [`Palette::fg`].
    pub fg: Option<Rgb>,
    /// See [`Palette::band`].
    pub band: Option<Rgb>,
    /// See [`Palette::wash`].
    pub wash: Option<Rgb>,
    /// See [`Palette::focus`].
    pub focus: Option<Rgb>,
    /// See [`Palette::accent`].
    pub accent: Option<Rgb>,
    /// See [`Palette::on_accent`].
    pub on_accent: Option<Rgb>,
    /// See [`Palette::add`].
    pub add: Option<Rgb>,
    /// See [`Palette::del`].
    pub del: Option<Rgb>,
    /// See [`Palette::gone`].
    pub gone: Option<Rgb>,
    /// See [`Palette::fresh`].
    pub fresh: Option<Rgb>,
    /// See [`Palette::comment`].
    pub comment: Option<Rgb>,
    /// See [`Palette::muted`].
    pub muted: Option<Rgb>,
    /// See [`Palette::edge`].
    pub edge: Option<Rgb>,
    /// See [`Palette::ground`].
    pub ground: Option<Rgb>,
}

impl Overrides {
    /// Write whatever was given over `palette`.
    pub fn apply(self, palette: &mut Palette) {
        let over = |slot: &mut Rgb, given: Option<Rgb>| {
            if let Some(colour) = given {
                *slot = colour;
            }
        };
        over(&mut palette.bg, self.bg);
        over(&mut palette.fg, self.fg);
        over(&mut palette.band, self.band);
        over(&mut palette.wash, self.wash);
        over(&mut palette.focus, self.focus);
        over(&mut palette.accent, self.accent);
        over(&mut palette.on_accent, self.on_accent);
        over(&mut palette.add, self.add);
        over(&mut palette.del, self.del);
        over(&mut palette.gone, self.gone);
        over(&mut palette.fresh, self.fresh);
        over(&mut palette.comment, self.comment);
        over(&mut palette.muted, self.muted);
        over(&mut palette.edge, self.edge);
        over(&mut palette.ground, self.ground);
    }
}

/// The best accent among `candidates`, for a page of `bg` and text of `fg`.
///
/// An editor has no field called "accent". What it has is a handful of colours
/// that mean *look here* — a search hit, a selection, a cursor — and which of
/// them exists depends on the theme. So an importer offers what it found, best
/// first, and this picks.
///
/// Two things disqualify a candidate, and both were learned from a real theme:
/// a **grey** is not an accent, however prominent (one scheme's `Special` is
/// pure black, which made the lit range a grey smudge and the gutter bar
/// invisible against the text); and a colour too close to the page cannot be
/// seen on it. A candidate that passes both wins outright, in the order the
/// importer offered them — the importer knows which of its own colours means
/// *look here* most strongly.
///
/// Nothing passing is not a failure. The text colour always reads on the page,
/// which is the one thing an accent must do.
#[must_use]
pub fn pick_accent(bg: Rgb, fg: Rgb, candidates: &[Rgb]) -> Rgb {
    /// Below this a colour is a grey with a hint in it.
    const COLOURFUL: f32 = 0.18;
    /// And below this it disappears into the page.
    const APART: f32 = 0.1;

    let seen = |colour: &Rgb| (colour.lightness() - bg.lightness()).abs() >= APART;

    candidates
        .iter()
        .find(|colour| colour.saturation() >= COLOURFUL && seen(colour))
        .or_else(|| candidates.iter().find(|colour| seen(colour)))
        .copied()
        .unwrap_or(fg)
}

/// How far each surface recedes from the page, and how far the lit range is
/// tinted. Small numbers on purpose: the page is meant to be quiet, not
/// striped.
const WASH: f32 = 0.05;
const BAND: f32 = 0.11;
const EDGE: f32 = 0.24;
const GROUND: f32 = 0.19;
const TINT: f32 = 0.16;
/// How far a diff ground is tinted. Further than the lit range, because a
/// change has to be told apart from the spotlight as well as from the page.
const DIFF: f32 = 0.115;
/// Below this lightness a page has no useful room left to darken into, and the
/// surfaces have to come up instead.
///
/// Raised from a hair above black once a real theme was imported. GitHub Dark's
/// page is 0.065, which cleared the old threshold and left every surface within
/// a hundredth of the page — a window with no seams, no band and no panes, just
/// one dark rectangle. Any page this dark has more room above it than below.
const NO_ROOM_BELOW: f32 = 0.12;
/// How far a hairline lifts off a dark page.
///
/// Further than [`BAND`], because on a page with no room below it the band
/// lifts too, and a hairline that only cleared the *page* would sit inside the
/// panel it is supposed to be the edge of.
const EDGE_UP: f32 = 0.18;

/// Compute the whole palette from what the editor gave us.
///
/// Callers override individual colours afterwards; nothing here is more
/// authoritative than a value the reader typed into `[theme.colors]`.
#[must_use]
pub fn derive(imported: Imported) -> Palette {
    let Imported {
        bg,
        fg,
        accent,
        comment,
        add,
        del,
        syntax: _,
    } = imported;

    // Warm and cool, taken far enough from the ground to read on it. The
    // editor's own, when it had them.
    let marks_add =
        add.unwrap_or_else(|| toward(bg, Rgb::new(0x79, 0x74, 0x0e), Rgb::new(0xb8, 0xbb, 0x26)));
    let marks_del =
        del.unwrap_or_else(|| toward(bg, Rgb::new(0x9d, 0x00, 0x06), Rgb::new(0xfb, 0x49, 0x34)));

    Palette {
        bg,
        fg,
        wash: recede(bg, WASH),
        band: recede(bg, BAND),
        // The lit range keeps the page's own brightness and only shifts hue, so
        // that it reads as un-dimmed rather than highlighted. It is the
        // surrounding wash that makes it stand out.
        focus: tint(bg, accent, TINT),
        accent,
        on_accent: readable_on(accent, bg, fg),
        add: marks_add,
        del: marks_del,
        // The ground under a changed line, made from the *page* rather than
        // from the mark. A mark is a glyph the size of a hyphen and has to read
        // on the page, so it is dark; a ground is a whole line's worth of paper
        // and has to be told from the page without becoming a colour the syntax
        // must fight. Tinted toward a brighter red and green for that reason.
        gone: tint(
            bg,
            toward(bg, Rgb::new(0xcc, 0x24, 0x1d), Rgb::new(0xfb, 0x49, 0x34)),
            DIFF,
        ),
        fresh: tint(
            bg,
            toward(bg, Rgb::new(0x98, 0x97, 0x1a), Rgb::new(0xb8, 0xbb, 0x26)),
            DIFF,
        ),
        comment: comment
            .unwrap_or_else(|| toward(bg, Rgb::new(0x42, 0x7b, 0x58), Rgb::new(0x8e, 0xc0, 0x7c))),
        // Text pulled back toward the page until it reads as an aside.
        muted: fg.mix(bg, 0.42),
        edge: hairline(bg),
        // The desk always goes down. It is what the deck lies *on*, and on a
        // page too dark to darken it simply stops moving — which is fine once
        // the surfaces above it have lifted.
        ground: bg.mix(Rgb::new(0, 0, 0), GROUND),
    }
}

/// Whichever of `a` and `b` reads better on `ground`.
///
/// Distance in lightness, which is what legibility mostly is at these sizes —
/// a hue difference that a reader can name is not a hue difference they can
/// read small text through.
fn readable_on(ground: Rgb, a: Rgb, b: Rgb) -> Rgb {
    let apart = |colour: Rgb| (colour.lightness() - ground.lightness()).abs();
    if apart(a) >= apart(b) { a } else { b }
}

/// How much of the lightness a tint loses is given back.
///
/// Not all of it. Putting every unit back leaves a pale wash of the accent —
/// pink, where the design asks for tan — because the hue arrives without the
/// weight that normally comes with it. Leaving a tenth of the darkening in
/// gives the lit range the warmth of paper rather than the look of a
/// highlighter pen, and lands on the `#d7cbbb` the design was drawn against.
const TINT_RESTORE: f32 = 0.1;

/// `base` shifted toward another colour's hue, keeping nearly all its lightness.
///
/// Public because it is how every tinted ground in the deck is made — the lit
/// range, and the two diff grounds — and a client that adds another one should
/// make it the same way rather than reaching for a mix.
///
/// `base` shifted toward the accent's hue, keeping nearly all its lightness.
///
/// Rule 2, taken literally. Mixing alone is not enough: a dark accent drags the
/// page down with it, and the lit range comes out *darker than the wash it is
/// supposed to stand out from* — the dimming, backwards. So the mix supplies
/// the hue and most of the original lightness is put back afterwards.
#[must_use]
pub fn tint(base: Rgb, accent: Rgb, amount: f32) -> Rgb {
    let mixed = base.mix(accent, amount);
    let (want, have) = (base.lightness(), mixed.lightness());
    if have <= f32::EPSILON {
        return base;
    }

    let scale = 1.0 + (want / have - 1.0) * TINT_RESTORE;
    let channel = |v: u8| saturate(f32::from(v) * scale);
    Rgb::new(channel(mixed.r), channel(mixed.g), channel(mixed.b))
}

/// A float rounded into a channel, with both ends pinned.
///
/// `as u8` on its own is a silent trap here: it truncates rather than rounds,
/// and anything out of range wraps instead of clamping. This is the one place
/// the conversion happens.
#[allow(clippy::cast_possible_truncation)]
fn saturate(v: f32) -> u8 {
    v.round().clamp(0.0, 255.0) as u8
}

/// One step away from the text: darker, unless the ground is already so dark
/// there is nowhere left to go.
fn recede(base: Rgb, amount: f32) -> Rgb {
    let target = if base.lightness() < NO_ROOM_BELOW {
        Rgb::new(255, 255, 255)
    } else {
        Rgb::new(0, 0, 0)
    };
    base.mix(target, amount)
}

/// A hairline that can be seen on `base`.
///
/// Not a surface, and the difference matters. A surface recedes — it goes away
/// from the reader, and on almost every page that is downward. A hairline has
/// the opposite job: it has to be *told apart* from the page it is drawn on, so
/// it goes whichever way there is room. Down on a light page, up on a dark one.
///
/// This was `recede` for a long time, and it was wrong the whole time — hidden
/// because deck's own dark palette writes its edge out by hand, lighter than
/// its page, and the derived one was only ever used for a light theme. The
/// first imported dark theme drew a window with no seams in it.
fn hairline(base: Rgb) -> Rgb {
    if base.is_dark() {
        base.mix(Rgb::new(255, 255, 255), EDGE_UP)
    } else {
        base.mix(Rgb::new(0, 0, 0), EDGE)
    }
}

/// Whichever of two colours will read on this ground.
fn toward(bg: Rgb, on_light: Rgb, on_dark: Rgb) -> Rgb {
    if bg.is_dark() { on_dark } else { on_light }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The light theme the design was drawn against.
    fn light() -> Imported {
        Imported::plain(
            Rgb::from_hex("#dfdfdf").unwrap(),
            Rgb::from_hex("#3c3836").unwrap(),
            Rgb::from_hex("#af3a03").unwrap(),
        )
    }

    fn dark() -> Imported {
        Imported::plain(
            Rgb::from_hex("#282828").unwrap(),
            Rgb::from_hex("#ebdbb2").unwrap(),
            Rgb::from_hex("#fe8019").unwrap(),
        )
    }

    #[test]
    fn an_alpha_on_the_end_is_ignored_rather_than_refused() {
        // Zed writes every colour with one. Refusing them meant a real Zed
        // theme came back as no colours at all — which read as "this theme
        // sets no editor background" and was true of nothing.
        assert_eq!(Rgb::from_hex("#282c33ff"), Some(Rgb::new(0x28, 0x2c, 0x33)));
        assert_eq!(Rgb::from_hex("#282c3300"), Some(Rgb::new(0x28, 0x2c, 0x33)));
        assert_eq!(Rgb::from_hex("#abcd"), Rgb::from_hex("#abc"));
    }

    #[test]
    fn hex_parses_in_both_lengths_and_survives_a_round_trip() {
        assert_eq!(Rgb::from_hex("#dfdfdf"), Some(Rgb::new(223, 223, 223)));
        assert_eq!(Rgb::from_hex("dfdfdf"), Some(Rgb::new(223, 223, 223)));
        // `#abc` is `#aabbcc`, not `#a0b0c0`.
        assert_eq!(Rgb::from_hex("#abc"), Some(Rgb::new(0xaa, 0xbb, 0xcc)));
        assert_eq!(Rgb::from_hex("#dfdfdf").unwrap().to_hex(), "#dfdfdf");

        assert_eq!(Rgb::from_hex("#gggggg"), None);
        assert_eq!(Rgb::from_hex("#dfdfd"), None, "five digits is not a colour");
    }

    #[test]
    fn a_hairline_can_be_seen_on_whatever_it_is_drawn_on() {
        // Whichever way there is room. A hairline is not a surface: a surface
        // recedes, and a hairline has to be told apart — which on a dark page
        // means going up, not down.
        for theme in [light(), dark()] {
            let p = derive(theme);
            let apart = (p.edge.lightness() - p.bg.lightness()).abs();
            assert!(apart > 0.03, "an edge that cannot be seen is not an edge");
        }

        assert!(derive(light()).edge.lightness() < derive(light()).bg.lightness());
        assert!(derive(dark()).edge.lightness() > derive(dark()).bg.lightness());
    }

    #[test]
    fn a_page_with_no_room_below_it_lifts_its_surfaces() {
        // GitHub Dark's page, which is very nearly black. Receding downward
        // left every surface within a hundredth of the page — one dark
        // rectangle with no seams, no band and no panes in it.
        let p = derive(Imported::plain(
            Rgb::new(0x0d, 0x11, 0x17),
            Rgb::new(0xe6, 0xed, 0xf3),
            Rgb::new(0x2f, 0x81, 0xf7),
        ));

        assert!(p.wash.lightness() > p.bg.lightness());
        assert!(
            p.band.lightness() - p.bg.lightness() > 0.04,
            "and far enough that the band is a panel rather than a smudge"
        );
        assert!(p.edge.lightness() > p.band.lightness());
    }

    #[test]
    fn the_desk_stays_under_the_page() {
        for theme in [light(), dark()] {
            let p = derive(theme);
            assert!(p.ground.lightness() <= p.bg.lightness());
        }
    }

    #[test]
    fn receding_is_darker_on_a_light_theme() {
        let p = derive(light());
        assert!(p.wash.lightness() < p.bg.lightness());
        assert!(p.band.lightness() < p.wash.lightness());
    }

    #[test]
    fn receding_is_darker_on_a_dark_theme_too() {
        // While there is room. Deck's own dark page has plenty; only a
        // near-black one has to lift instead.
        let p = derive(dark());
        assert!(p.wash.lightness() < p.bg.lightness());
        assert!(p.band.lightness() < p.wash.lightness());
    }

    #[test]
    fn a_black_ground_is_the_one_case_that_has_to_come_up() {
        let p = derive(Imported {
            bg: Rgb::new(0, 0, 0),
            ..dark()
        });
        assert!(p.wash.lightness() > 0.0, "black cannot darken any further");
        assert!(p.band.lightness() > p.wash.lightness());
    }

    #[test]
    fn the_lit_range_is_told_apart_by_warmth_rather_than_brightness() {
        let p = derive(light());

        // It stays near the page: the range is meant to read as un-dimmed, not
        // as something a highlighter pen was dragged across.
        assert!((p.focus.lightness() - p.bg.lightness()).abs() < 0.12);

        // It is not *brighter* than the wash, and it does not need to be. The
        // design's own values have the lit range a shade darker than the pane
        // around it — `#d7cbbb` against `#d4d4d4` — and it still reads as lit,
        // because the pane is neutral and the range is warm. Asserting the
        // brightness went up encodes a belief the palette never held.
        let warmth = |c: Rgb| f32::from(c.r) - f32::from(c.b);
        assert!(
            warmth(p.focus) > warmth(p.wash) + 12.,
            "the lit range has to be visibly warmer than the pane"
        );
    }

    #[test]
    fn the_lit_range_leans_toward_the_accent() {
        let p = derive(light());
        // The accent here is warm, so the tinted page must be warmer than the
        // neutral it started from.
        assert!(p.focus.r > p.focus.b);
        assert_eq!(p.bg.r, p.bg.b, "the page it came from was neutral");
    }

    #[test]
    fn a_grey_is_never_an_accent() {
        // Learned from a real colour scheme, whose `Special` is pure black. It
        // was the most prominent thing on offer and it made the lit range a
        // grey smudge — an accent is a colour, and black is the absence of one.
        let page = Rgb::new(0xdf, 0xdf, 0xdf);
        let text = Rgb::new(0x2d, 0x2d, 0x2d);
        let orange = Rgb::new(0xae, 0x60, 0x00);

        assert_eq!(
            pick_accent(page, text, &[Rgb::new(0, 0, 0), orange]),
            orange,
            "the colour wins over the grey, even offered second"
        );
    }

    #[test]
    fn an_accent_the_page_would_swallow_is_passed_over() {
        let page = Rgb::new(0xdf, 0xdf, 0xdf);
        let text = Rgb::new(0x2d, 0x2d, 0x2d);
        let nearly_the_page = Rgb::new(0xe2, 0xdd, 0xd8);
        let red = Rgb::new(0x9d, 0x00, 0x06);

        assert_eq!(pick_accent(page, text, &[nearly_the_page, red]), red);
    }

    #[test]
    fn a_theme_offering_nothing_usable_falls_back_on_its_own_text() {
        // Which always reads on the page, because the editor is legible.
        let page = Rgb::new(0xdf, 0xdf, 0xdf);
        let text = Rgb::new(0x2d, 0x2d, 0x2d);
        assert_eq!(pick_accent(page, text, &[]), text);
    }

    #[test]
    fn what_goes_on_the_accent_follows_the_accent() {
        // A dark accent takes the page's own colour, which is what deck's own
        // palette does. A pale one has to take the text colour instead, or the
        // button ends up pale on pale — which is the bug this role exists to
        // stop somebody writing at the point of use.
        let page = Rgb::new(0xdf, 0xdf, 0xdf);
        let text = Rgb::new(0x3c, 0x38, 0x36);

        let dark = derive(Imported::plain(page, text, Rgb::new(0xaf, 0x3a, 0x03)));
        assert_eq!(
            dark.on_accent, page,
            "a dark accent is written on in the page"
        );

        let pale = derive(Imported::plain(page, text, Rgb::new(0xfa, 0xbd, 0x2f)));
        assert_eq!(pale.on_accent, text, "a pale one is written on in the text");
    }

    #[test]
    fn muted_text_sits_between_the_text_and_the_page() {
        for theme in [light(), dark()] {
            let p = derive(theme);
            let (fg, bg, muted) = (p.fg.lightness(), p.bg.lightness(), p.muted.lightness());
            assert!(
                muted > fg.min(bg) && muted < fg.max(bg),
                "muted must be quieter than text without vanishing into the page"
            );
        }
    }

    #[test]
    fn the_diff_colours_flip_to_stay_legible_on_the_ground() {
        assert_ne!(derive(light()).add, derive(dark()).add);
        assert!(derive(dark()).add.lightness() > derive(light()).add.lightness());
    }

    #[test]
    fn mixing_is_bounded_at_both_ends() {
        let a = Rgb::new(0, 0, 0);
        let b = Rgb::new(255, 255, 255);
        assert_eq!(a.mix(b, 0.0), a);
        assert_eq!(a.mix(b, 1.0), b);
        // Out-of-range amounts are clamped rather than wrapping a channel.
        assert_eq!(a.mix(b, -1.0), a);
        assert_eq!(a.mix(b, 9.0), b);
    }
}
