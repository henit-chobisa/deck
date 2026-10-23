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

    /// Relative luminance, as WCAG defines it: each channel taken out of its
    /// sRGB curve and weighted for the eye, from 0 for black to 1 for white.
    ///
    /// Not [`Self::lightness`], which weights the channels as they are stored
    /// and is good for deciding which way is up. Contrast is measured in light,
    /// and light is what this is.
    ///
    /// Read from [`LINEAR`], a table worked out once, rather than from `powf`
    /// at run time: each platform's maths library may round a power a hair
    /// differently, and a colour that sits on the 4.5 line would then pass on
    /// one machine and fail on another — the same export, in two colours.
    #[must_use]
    pub fn luminance(&self) -> f64 {
        let linear = |channel: u8| LINEAR[usize::from(channel)];
        // Plain multiplies and adds, never fused: each is rounded the same
        // way on every machine.
        0.2126 * linear(self.r) + 0.7152 * linear(self.g) + 0.0722 * linear(self.b)
    }

    /// The contrast between this colour and `other`, as WCAG measures it:
    /// from 1, for two colours that are the same, to 21, for black on white.
    /// Body text wants at least 4.5.
    #[must_use]
    pub fn contrast(&self, other: Self) -> f64 {
        ratio(self.luminance(), other.luminance())
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
    /// The same two grounds, deeper, for when the narration is pointing at one.
    ///
    /// A point used to wash the accent over whatever row it landed on, which
    /// works on ordinary paper and ruins a diff: orange mixed into the arriving
    /// green gives a muddy brown, and the one thing the row was saying — *this
    /// is the half that arrives* — is the thing the light takes away. A pointed
    /// red line should be a darker red and a pointed green line a darker green,
    /// so the emphasis deepens what the row already means instead of arguing
    /// with it.
    pub gone_deep: Rgb,
    /// The arriving ground, deeper. See [`gone_deep`](Self::gone_deep).
    pub fresh_deep: Rgb,
    /// A comment marker and its text.
    pub comment: Rgb,
    /// Labels, line numbers, anything said quietly.
    pub muted: Rgb,
    /// The colour code's punctuation, operators and attributes are drawn in:
    /// muted as the theme gave it. It is kept apart because muted is lifted
    /// for reading, and code must keep the colours it was drawn in.
    pub punctuation: Rgb,
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
        over(&mut palette.punctuation, self.muted);
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
///
/// Halved from 0.115, for the reason deck's own palettes give: a change is a
/// quiet ground under its lines, never the loudest thing in the window.
const DIFF: f32 = 0.055;
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

    let mut palette = Palette {
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
        gone: tint(bg, toward(bg, RED_DIM, RED_BRIGHT), DIFF),
        fresh: tint(bg, toward(bg, GREEN_DIM, GREEN_BRIGHT), DIFF),
        // Twice the distance from the page, so a pointed line reads as more of
        // what it already is rather than as a different colour.
        gone_deep: tint(bg, toward(bg, RED_DIM, RED_BRIGHT), DIFF * 1.9),
        fresh_deep: tint(bg, toward(bg, GREEN_DIM, GREEN_BRIGHT), DIFF * 3.4),
        comment: comment
            .unwrap_or_else(|| toward(bg, Rgb::new(0x42, 0x7b, 0x58), Rgb::new(0x8e, 0xc0, 0x7c))),
        // Text pulled back toward the page until it reads as an aside.
        muted: fg.mix(bg, 0.42),
        punctuation: fg.mix(bg, 0.42),
        edge: hairline(bg),
        // The desk always goes down. It is what the deck lies *on*, and on a
        // page too dark to darken it simply stops moving — which is fine once
        // the surfaces above it have lifted.
        ground: bg.mix(Rgb::new(0, 0, 0), GROUND),
    };
    readable(&mut palette);
    palette
}

/// Muted text lifted until it reads on every surface it is drawn on.
///
/// Muted is the ink stepped toward the page, a fixed step, and on many themes
/// that step went too far: the strip, the review column, the line numbers and
/// the sheet's labels came out near 3:1 on a dark theme and under it on a
/// light one. A secondary line is still a line somebody reads. So it keeps its
/// hue and is moved by the least that reads at [`LEGIBLE`] on the page, the
/// pane and the band. A colour written by hand is applied after this and is
/// left as the reader wrote it.
pub fn readable(palette: &mut Palette) {
    for ground in [palette.bg, palette.wash, palette.band] {
        palette.muted = legible(palette.muted, ground, LEGIBLE);
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

/// The inks a diff ground is tinted toward: dim for a light page, bright for a
/// dark one.
const RED_DIM: Rgb = Rgb::new(0xcc, 0x24, 0x1d);
const RED_BRIGHT: Rgb = Rgb::new(0xfb, 0x49, 0x34);
const GREEN_DIM: Rgb = Rgb::new(0x98, 0x97, 0x1a);
const GREEN_BRIGHT: Rgb = Rgb::new(0xb8, 0xbb, 0x26);

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

/// The contrast body text needs on its ground: WCAG's AA for normal text.
pub const LEGIBLE: f64 = 4.5;

/// Each sRGB channel value as linear light, worked out exactly once and kept:
/// `c / 12.92` up to 0.04045, `((c + 0.055) / 1.055)^2.4` above it, for
/// `c = value / 255`. See [`Rgb::luminance`] for why this is a table.
#[allow(clippy::unreadable_literal, clippy::excessive_precision)]
pub const LINEAR: [f64; 256] = [
    0.0,
    0.0003035269835488375,
    0.000607053967097675,
    0.0009105809506465125,
    0.00121410793419535,
    0.0015176349177441874,
    0.001821161901293025,
    0.0021246888848418626,
    0.0024282158683907,
    0.0027317428519395373,
    0.003035269835488375,
    0.0033465357638991586,
    0.003676507324047435,
    0.004024717018496304,
    0.0043914420374102925,
    0.0047769534806937275,
    0.005181516702338385,
    0.005605391624202721,
    0.006048833022857052,
    0.006512090792594472,
    0.006995410187265385,
    0.00749903204322617,
    0.008023192985384993,
    0.008568125618069302,
    0.009134058702220785,
    0.009721217320237844,
    0.010329823029626936,
    0.01096009400648824,
    0.011612245179743881,
    0.012286488356915867,
    0.012983032342173007,
    0.013702083047289683,
    0.014443843596092541,
    0.015208514422912706,
    0.015996293365509628,
    0.016807375752887377,
    0.017641954488384078,
    0.01850022012837969,
    0.019382360956935723,
    0.02028856305665239,
    0.021219010376003555,
    0.022173884793387375,
    0.023153366178110403,
    0.02415763244850475,
    0.025186859627361623,
    0.02624122189484989,
    0.02732089163907489,
    0.028426039504420787,
    0.029556834437808797,
    0.03071344373299362,
    0.03189603307301152,
    0.03310476657088505,
    0.03433980680868216,
    0.03560131487502032,
    0.03688945040110002,
    0.03820437159534648,
    0.03954623527673283,
    0.04091519690685317,
    0.042311410620809654,
    0.04373502925697345,
    0.04518620438567554,
    0.04666508633688008,
    0.048171824226889405,
    0.04970656598412722,
    0.051269458374043224,
    0.05286064702318025,
    0.054480276442442355,
    0.05612849004960008,
    0.05780543019106721,
    0.059511238162981185,
    0.061246054231617594,
    0.06301001765316766,
    0.06480326669290576,
    0.06662593864377288,
    0.06847816984440015,
    0.07036009569659588,
    0.07227185068231748,
    0.07421356838014961,
    0.07618538148130781,
    0.07818742180518633,
    0.08021982031446831,
    0.0822827071298148,
    0.08437621154414877,
    0.08650046203654974,
    0.08865558628577294,
    0.09084171118340768,
    0.09305896284668742,
    0.09530746663096466,
    0.09758734714186242,
    0.09989872824711389,
    0.10224173308810128,
    0.10461648409110416,
    0.10702310297826759,
    0.10946171077829933,
    0.11193242783690557,
    0.11443537382697372,
    0.11697066775851081,
    0.1195384279883456,
    0.12213877222960184,
    0.12477181756095046,
    0.12743768043564743,
    0.13013647669036427,
    0.13286832155381792,
    0.13563332965520564,
    0.13843161503245183,
    0.14126329114027164,
    0.14412847085805772,
    0.14702726649759498,
    0.14995978981060853,
    0.15292615199615014,
    0.15592646370782734,
    0.15896083506088035,
    0.162029375639111,
    0.1651321945016676,
    0.1682694001896907,
    0.17144110073282254,
    0.17464740365558498,
    0.17788841598362912,
    0.18116424424986013,
    0.1844749945004409,
    0.18782077230067779,
    0.19120168274079136,
    0.1946178304415757,
    0.1980693195599488,
    0.20155625379439707,
    0.2050787363903169,
    0.20863687014525567,
    0.2122307574140551,
    0.21586050011389915,
    0.21952619972926918,
    0.22322795731680842,
    0.22696587351009834,
    0.23074004852434896,
    0.23455058216100508,
    0.23839757381227095,
    0.24228112246555472,
    0.2462013267078354,
    0.2501582847299533,
    0.2541520943308267,
    0.25818285292159576,
    0.262250657529696,
    0.2663556048028623,
    0.27049779101306576,
    0.27467731206038454,
    0.27889426347681034,
    0.28314874042999194,
    0.2874408377269174,
    0.29177064981753587,
    0.29613827079832095,
    0.3005437944157764,
    0.3049873140698861,
    0.30946892281750843,
    0.3139887133757175,
    0.31854677812509175,
    0.3231432091129507,
    0.32777809805654207,
    0.3324515363461792,
    0.33716361504833026,
    0.34191442490866075,
    0.3467040563550295,
    0.3515325995004392,
    0.3564001441459434,
    0.36130677978350945,
    0.3662525955988394,
    0.37123768047414896,
    0.3762621229909062,
    0.38132601143253,
    0.3864294337870489,
    0.3915724777497231,
    0.3967552307256268,
    0.4019777798321956,
    0.40724021190173665,
    0.4125426134839036,
    0.41788507084813725,
    0.4232676699860715,
    0.42869049661390657,
    0.4341536361747488,
    0.43965717384091874,
    0.44520119451622775,
    0.45078578283822335,
    0.4564110231804045,
    0.46207699965440685,
    0.4677837961121588,
    0.4735314961480093,
    0.47932018310082664,
    0.4851499400560704,
    0.49102084984783545,
    0.49693299506087035,
    0.5028864580325684,
    0.5088813208549335,
    0.5149176653765213,
    0.5209955732043541,
    0.527115125705813,
    0.533276404010505,
    0.539479489012107,
    0.5457244613701866,
    0.5520114015119999,
    0.5583403896342677,
    0.5647115057049289,
    0.5711248294648729,
    0.5775804404296505,
    0.584078417891164,
    0.5906188409193368,
    0.5972017883637631,
    0.6038273388553375,
    0.6104955708078647,
    0.6172065624196509,
    0.6239603916750759,
    0.6307571363461467,
    0.6375968739940324,
    0.644479681970582,
    0.6514056374198239,
    0.6583748172794482,
    0.6653872982822719,
    0.6724431569576873,
    0.6795424696330937,
    0.6866853124353132,
    0.6938717612919898,
    0.7011018919329731,
    0.7083757798916867,
    0.7156935005064805,
    0.7230551289219689,
    0.7304607400903533,
    0.7379104087727307,
    0.7454042095403872,
    0.7529422167760778,
    0.7605245046752922,
    0.7681511472475069,
    0.7758222183174234,
    0.7835377915261932,
    0.79129794033263,
    0.7991027380144087,
    0.8069522576692514,
    0.8148465722161011,
    0.8227857543962833,
    0.8307698767746545,
    0.8387990117407399,
    0.8468732315098577,
    0.8549926081242336,
    0.863157213454102,
    0.8713671191987971,
    0.8796223968878317,
    0.8879231178819664,
    0.8962693533742666,
    0.9046611743911491,
    0.9130986517934189,
    0.9215818562772945,
    0.9301108583754234,
    0.9386857284578878,
    0.9473065367331996,
    0.955973353249286,
    0.9646862478944651,
    0.9734452903984123,
    0.982250550333117,
    0.9911020971138297,
    1.0,
];

/// WCAG's contrast ratio between two luminances.
fn ratio(a: f64, b: f64) -> f64 {
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

/// `ink` made to read on `ground` at `ratio` or better, changing only how
/// light it is.
///
/// An export is printed and read by people who never chose its theme, and a
/// theme drawn for a screen in a dark room often sets comments, numbers or a
/// muted label a shade too close to the page. Swapping such a colour would
/// lose the theme; this keeps its hue and its saturation and changes its
/// lightness by the least that reaches `ratio` — lighter or darker, whichever
/// is the smaller step. A colour that already reads is returned as it is.
///
/// Every ground reaches 4.5 against white or against black, so at [`LEGIBLE`]
/// there is always an answer. Above about 4.58 some grounds have none: then
/// this returns whichever of black and white reads better, which is as close
/// as any colour comes, and short of `ratio`.
#[must_use]
pub fn legible(ink: Rgb, ground: Rgb, ratio: f64) -> Rgb {
    let floor = ground.luminance();
    let reads = |colour: Rgb| self::ratio(colour.luminance(), floor) >= ratio;
    if reads(ink) {
        return ink;
    }
    let (hue, saturation, lightness) = hsl(ink);
    let at = |l: f32| from_hsl(hue, saturation, l);
    // Both ways, keeping the nearer: the side the ink is already on is
    // usually the short way, but near a middling ground both reach and only
    // measuring says which is closer.
    let nearest = [1.0f32, 0.0]
        .into_iter()
        .filter(|&end| reads(at(end)))
        .map(|end| {
            // The least change that reads: halve the distance between a
            // lightness that does not and one that does.
            let (mut near, mut far) = (lightness, end);
            for _ in 0..24 {
                let mid = f32::midpoint(near, far);
                if reads(at(mid)) {
                    far = mid;
                } else {
                    near = mid;
                }
            }
            far
        })
        .min_by(|a, b| (a - lightness).abs().total_cmp(&(b - lightness).abs()));
    if let Some(found) = nearest {
        return at(found);
    }
    let (white, black) = (Rgb::new(255, 255, 255), Rgb::new(0, 0, 0));
    if white.contrast(ground) >= black.contrast(ground) {
        white
    } else {
        black
    }
}

/// `colour` as hue (0 to 1), saturation and lightness, the way HSL has them.
fn hsl(colour: Rgb) -> (f32, f32, f32) {
    let [r, g, b] = [colour.r, colour.g, colour.b].map(|c| f32::from(c) / 255.0);
    let high = r.max(g).max(b);
    let low = r.min(g).min(b);
    let lightness = f32::midpoint(high, low);
    let span = high - low;
    // Two steps or less apart is grey. HSL calls a colour one step off grey
    // near black or white fully saturated, and lifting `#000001` off a black
    // page would otherwise come back vivid blue.
    if span <= 2.0 / 255.0 + f32::EPSILON {
        return (0.0, 0.0, lightness);
    }
    let saturation = span / (1.0 - (2.0 * lightness - 1.0).abs());
    let hue = if (high - r).abs() <= f32::EPSILON {
        ((g - b) / span).rem_euclid(6.0)
    } else if (high - g).abs() <= f32::EPSILON {
        (b - r) / span + 2.0
    } else {
        (r - g) / span + 4.0
    };
    (hue / 6.0, saturation.min(1.0), lightness)
}

/// A colour from hue (0 to 1), saturation and lightness.
fn from_hsl(hue: f32, saturation: f32, lightness: f32) -> Rgb {
    let chroma = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
    let h = hue * 6.0;
    let x = chroma * (1.0 - (h.rem_euclid(2.0) - 1.0).abs());
    let (r, g, b) = match h {
        h if h < 1.0 => (chroma, x, 0.0),
        h if h < 2.0 => (x, chroma, 0.0),
        h if h < 3.0 => (0.0, chroma, x),
        h if h < 4.0 => (0.0, x, chroma),
        h if h < 5.0 => (x, 0.0, chroma),
        _ => (chroma, 0.0, x),
    };
    let m = lightness - chroma / 2.0;
    Rgb::new(
        saturate((r + m) * 255.0),
        saturate((g + m) * 255.0),
        saturate((b + m) * 255.0),
    )
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
    fn contrast_is_measured_as_wcag_measures_it() {
        let (white, black) = (Rgb::new(255, 255, 255), Rgb::new(0, 0, 0));
        assert!((white.contrast(black) - 21.0).abs() < 0.01);
        assert!((white.contrast(white) - 1.0).abs() < 1e-6);
        // WCAG's own worked figure: #767676 on white is 4.54.
        let grey = Rgb::from_hex("#767676").unwrap();
        assert!((grey.contrast(white) - 4.54).abs() < 0.01);
    }

    #[test]
    fn a_colour_that_reads_is_left_alone() {
        let p = derive(dark());
        assert_eq!(legible(p.fg, p.bg, LEGIBLE), p.fg);
    }

    #[test]
    fn a_faint_colour_is_lifted_until_it_reads_and_keeps_its_hue() {
        // A comment a theme set too close to its dark page.
        let page = Rgb::from_hex("#282828").unwrap();
        let faint = Rgb::from_hex("#5c6370").unwrap();
        assert!(faint.contrast(page) < LEGIBLE);
        let lifted = legible(faint, page, LEGIBLE);
        assert!(lifted.contrast(page) >= LEGIBLE);
        assert!(lifted.lightness() > faint.lightness(), "up, on a dark page");
        let ((h0, s0, _), (h1, s1, _)) = (hsl(faint), hsl(lifted));
        assert!(
            (h0 - h1).abs() < 0.02 && (s0 - s1).abs() < 0.06,
            "the same colour, lighter"
        );
        // The least that reaches it, not all the way to white.
        assert!(
            lifted.contrast(page) < LEGIBLE + 0.3,
            "the least that reaches it"
        );

        // And down, on a light page.
        let paper = Rgb::from_hex("#dfdfdf").unwrap();
        let pale = Rgb::from_hex("#b57614").unwrap();
        let darkened = legible(pale, paper, LEGIBLE);
        assert!(darkened.contrast(paper) >= LEGIBLE);
        assert!(darkened.lightness() < pale.lightness());
    }

    /// The smallest change of lightness, in steps of 1/4096, that makes
    /// `ink` read on `ground`, either way: what [`legible`] should find.
    fn least(ink: Rgb, ground: Rgb) -> Option<f32> {
        let (hue, saturation, lightness) = hsl(ink);
        (0..=4096)
            .map(|n| n as f32 / 4096.0)
            .filter(|&l| from_hsl(hue, saturation, l).contrast(ground) >= LEGIBLE)
            .map(|l| (l - lightness).abs())
            .min_by(f32::total_cmp)
    }

    #[test]
    fn legible_finds_the_least_change_either_way() {
        // Against a brute-force search, on inks and grounds across the cube:
        // never further than the search's own step from the least change.
        let steps = [0u8, 40, 80, 117, 118, 119, 160, 200, 255];
        for &r in &steps {
            for &g in &steps {
                for &b in &[0u8, 120, 255] {
                    let ink = Rgb::new(r, g, b);
                    for level in [0u8, 30, 100, 117, 118, 119, 120, 160, 230, 255] {
                        let ground = Rgb::new(level, level, level);
                        let made = legible(ink, ground, LEGIBLE);
                        assert!(made.contrast(ground) >= LEGIBLE);
                        if ink.contrast(ground) >= LEGIBLE {
                            assert_eq!(made, ink);
                            continue;
                        }
                        let Some(best) = least(ink, ground) else {
                            continue;
                        };
                        let moved = (hsl(made).2 - hsl(ink).2).abs();
                        assert!(
                            // Half a channel step for rounding to 8 bits,
                            // and one step of the search.
                            moved <= best + 0.5 / 255.0 + 1.0 / 4096.0 + 1e-6,
                            "{} on {}: moved {moved}, least {best}",
                            ink.to_hex(),
                            ground.to_hex()
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn near_black_on_a_middling_ground_goes_down_not_to_white() {
        // #757575 reaches 4.5 both ways; the ink is already dark, so the short
        // way is down.
        let ground = Rgb::from_hex("#757575").unwrap();
        let made = legible(Rgb::from_hex("#080808").unwrap(), ground, LEGIBLE);
        assert!(made.lightness() < 0.1, "{}", made.to_hex());
    }

    #[test]
    fn a_colour_a_step_off_grey_stays_grey() {
        let white = Rgb::new(255, 255, 255);
        let made = legible(Rgb::from_hex("#fffffe").unwrap(), white, LEGIBLE);
        assert!(hsl(made).1 < 0.05, "{}", made.to_hex());
        let black = Rgb::new(0, 0, 0);
        let made = legible(Rgb::from_hex("#000001").unwrap(), black, LEGIBLE);
        assert!(hsl(made).1 < 0.05, "{}", made.to_hex());
    }

    #[test]
    fn the_table_is_the_curve() {
        // The literals against the formula they were worked out from, to
        // within what f64 rounding allows.
        for (value, &kept) in LINEAR.iter().enumerate() {
            let c = f64::from(u8::try_from(value).unwrap()) / 255.0;
            let exact = if c <= 0.040_45 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            };
            assert!((exact - kept).abs() < 1e-12, "{value}");
        }
    }

    #[test]
    fn a_ratio_out_of_reach_comes_back_as_close_as_it_can() {
        let ground = Rgb::from_hex("#777777").unwrap();
        let made = legible(Rgb::from_hex("#802020").unwrap(), ground, 7.0);
        assert!(made == Rgb::new(0, 0, 0) || made == Rgb::new(255, 255, 255));
    }

    #[test]
    fn muted_text_reads_on_every_surface_without_lifting() {
        // The window draws muted as it is, with no lift at paint time, so the
        // palette itself has to read: on the page, the pane and the band.
        let themes = [
            light(),
            dark(),
            Imported::plain(
                Rgb::new(0x0d, 0x11, 0x17),
                Rgb::new(0xe6, 0xed, 0xf3),
                Rgb::new(0x2f, 0x81, 0xf7),
            ),
            Imported::plain(
                Rgb::new(0xfd, 0xf6, 0xe3),
                Rgb::new(0x65, 0x7b, 0x83),
                Rgb::new(0xb5, 0x89, 0x00),
            ),
        ];
        for theme in themes {
            let p = derive(theme);
            for ground in [p.bg, p.wash, p.band] {
                assert!(
                    p.muted.contrast(ground) >= LEGIBLE - 0.01,
                    "muted {:?} on {:?} is {:.2}",
                    p.muted,
                    ground,
                    p.muted.contrast(ground)
                );
            }
        }
    }

    #[test]
    fn every_token_an_export_writes_reads_on_every_ground_it_sits_on() {
        // Both of deck's palettes and two imported themes: text, muted text,
        // the accent as text, and a comment at the window's own blend, on the
        // page, the pane, the band and the lit range.
        let themes = [
            light(),
            dark(),
            Imported::plain(
                Rgb::new(0x0d, 0x11, 0x17),
                Rgb::new(0xe6, 0xed, 0xf3),
                Rgb::new(0x2f, 0x81, 0xf7),
            ),
            Imported::plain(
                Rgb::new(0xfd, 0xf6, 0xe3),
                Rgb::new(0x65, 0x7b, 0x83),
                Rgb::new(0xb5, 0x89, 0x00),
            ),
        ];
        for theme in themes {
            let p = derive(theme);
            let inks = [
                p.fg,
                p.muted,
                p.accent,
                p.fg.mix(p.wash, 0.42),
                p.add,
                p.del,
            ];
            for ground in [p.bg, p.wash, p.band, p.focus] {
                for ink in inks {
                    assert!(legible(ink, ground, LEGIBLE).contrast(ground) >= LEGIBLE);
                }
            }
        }
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
