//! A document as one HTML file: the window, a group to a screen.
//!
//! Each group is a landscape stop laid out as the window lays it out — the
//! band with the group's number, its titles and its narration over the
//! panes, the contents down the side, the strip along the bottom — at the
//! window's own measurements. What was said about a group follows it as a
//! stop of its own, set across the width, because the side column of a page
//! is too narrow to hold an argument and a page that cannot scroll cuts one
//! off.
//!
//! The file is meant to be sent, so it is written to be safe to open:
//!
//! - **It asks for nothing.** The faces are embedded, there is no script, and
//!   a content security policy forbids anything the page might be talked into
//!   loading. It reads the same on a plane.
//! - **Every word from the deck is escaped.** A title, a note, a remark, a
//!   file name: all of it is text somebody else wrote.
//! - **A page pane runs nothing.** The agent's markup goes into a sandboxed
//!   frame with no scripts and its own opaque origin, under the same policy,
//!   so it is drawn and that is all it can do.
//! - **The deck travels with it.** The `.deck` file is embedded whole, as
//!   base64 in a script block no browser runs, so whoever it was sent to can
//!   take it out and open it in deck.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

use deck_core::document::{
    Author, Change, Code, Document, Group, Inks, Line, Marked, Message, Page, Pane, Picture, Row,
    RowKind, Run,
};
use deck_core::protocol::{Kind, Level};
use deck_core::theme::{LEGIBLE, Rgb, legible};

use super::{Options, face};
use crate::metrics::{
    BAND_BOTTOM, BAND_GAP, BAND_LEAST, BAND_MOST, BAND_SIDE, BAND_TOP, CHIP_SIZE, CONFIDENCE_SIZE,
    DECK_TITLE_SIZE, LINE_HEIGHT, MONO_SIZE, NUMBER_SIZE, NUMBER_SIZE_TENS, PANE_HEAD,
    PROSE_LEADING, PROSE_SIZE, PROSE_SPACING, RAIL, RAIL_LEADING, RAIL_SIZE, SANS_SIZE, STEM,
    STEM_GAP, STRIP_GAP, STRIP_ROW, STRIP_SIDE, STRIP_SIZE, TITLE_SIZE,
};
use crate::reading::{self, CHIP_PADDING};
use crate::svg::base64;
use crate::text::{MONO, PLEX, PLEX_ITALIC};

/// How many lines of context a landscape stop shows either side of a range:
/// the window opens a pane two lines above it, and a page has the room for
/// one more.
const CONTEXT_SHOWN: u32 = 3;

/// How far a context line's ink is taken toward its ground: back a step from
/// the range, never so far it stops being legible.
const CONTEXT_FADE: f32 = 0.35;

/// The policy the page is opened under: nothing from anywhere, styles and
/// faces only from the page itself. A sandboxed frame inherits it.
const POLICY: &str = "default-src 'none'; style-src 'unsafe-inline'; font-src data:; \
                      img-src data:; base-uri 'none'; form-action 'none'";

/// One screen of the export.
#[derive(Clone, Copy)]
enum Stop<'a> {
    /// A group, as the window shows it.
    Group(&'a Group),
    /// What was said about a group, after it.
    Argument(&'a Group),
    /// What was said about the deck as a whole, at the end.
    General(&'a [Message]),
}

impl Stop<'_> {
    fn id(&self) -> String {
        match self {
            Self::Group(group) => format!("g{}", group.n),
            Self::Argument(group) => format!("g{}-argument", group.n),
            Self::General(_) => "said".to_string(),
        }
    }

    /// The group this stop belongs to, for the contents and the strip.
    fn group(&self) -> Option<&Group> {
        match self {
            Self::Group(group) | Self::Argument(group) => Some(group),
            Self::General(_) => None,
        }
    }
}

/// `document` as one HTML file, carrying `bundle` — the `.deck` it was built
/// from — for its reader to open.
///
/// With [`Options::comments`] off the page holds none of what was said;
/// leaving the review out of `bundle` too is [`super::deck`]'s to do, which
/// is the way to call this from anything but a test.
#[must_use]
pub fn html(document: &Document, bundle: &[u8], options: &Options) -> String {
    let mut stops: Vec<Stop> = Vec::new();
    for group in document.groups.iter().filter(|g| options.shows(g.n)) {
        stops.push(Stop::Group(group));
        if options.comments && !group.argument.is_empty() {
            stops.push(Stop::Argument(group));
        }
    }
    // What was said about the whole deck may be about any of it, so it goes
    // only with the whole deck.
    let every = document.groups.iter().all(|g| options.shows(g.n));
    if options.comments && every && !document.general.is_empty() {
        stops.push(Stop::General(&document.general));
    }

    let mut writer = Writer {
        document,
        stops: &stops,
        inks: BTreeMap::new(),
        grounds: BTreeMap::new(),
        out: String::new(),
    };
    for ix in 0..stops.len() {
        writer.stop(ix);
    }
    if stops.is_empty() {
        let _ = write!(
            writer.out,
            r#"<section class="stop empty"><p>{}</p></section>"#,
            esc("None of the groups asked for are in this deck.")
        );
    }
    let body = std::mem::take(&mut writer.out);

    // The faces are cut to what the page sets, which is everything it says
    // — tags and all, which costs a few letters — and the printable ASCII,
    // so a number the browser draws itself still has its glyphs.
    let mut chars: BTreeSet<char> = (' '..='~').collect();
    chars.extend(body.chars().filter(|ch| !ch.is_control()));
    chars.insert('\u{a0}');

    let title = esc(&document.title);
    let mut out = String::with_capacity(body.len() + bundle.len() * 2);
    let _ = write!(
        out,
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta http-equiv=\"Content-Security-Policy\" content=\"{POLICY}\">\n\
         <meta name=\"referrer\" content=\"no-referrer\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <meta name=\"generator\" content=\"deck\">\n\
         <title>{title}</title>\n<style>\n"
    );
    faces(&mut out, &chars);
    style(&mut out, &document.inks, &writer, options.dark);
    out.push_str("</style>\n</head>\n<body>\n<main>\n");
    out.push_str(&body);
    out.push_str("</main>\n");
    // Data, not a script: a type no browser runs, holding base64, which has
    // no `<` in it to end the block early.
    let _ = writeln!(
        out,
        "<script type=\"application/x-deck\" id=\"deck\">{}</script>",
        base64(bundle)
    );
    out.push_str("</body>\n</html>\n");
    out
}

/// The six faces, each cut to `chars`.
fn faces(out: &mut String, chars: &BTreeSet<char>) {
    for ((bytes, _), weight) in PLEX.iter().zip([400, 500, 600]) {
        let _ = writeln!(
            out,
            "@font-face{{font-family:'IBM Plex Sans';font-weight:{weight};font-style:normal;\
             src:url(data:font/ttf;base64,{}) format('truetype')}}",
            base64(&face::subset(bytes, chars))
        );
    }
    // Emphasis is a true italic, as the window sets it. Left to the browser
    // it would be the regular slanted, which reads as a mistake beside the
    // real one in the window.
    let _ = writeln!(
        out,
        "@font-face{{font-family:'IBM Plex Sans';font-weight:400;font-style:italic;\
         src:url(data:font/ttf;base64,{}) format('truetype')}}",
        base64(&face::subset(PLEX_ITALIC, chars))
    );
    for ((bytes, _), style) in MONO.iter().zip(["normal", "italic"]) {
        let _ = writeln!(
            out,
            "@font-face{{font-family:'JetBrains Mono';font-weight:400;font-style:{style};\
             src:url(data:font/ttf;base64,{}) format('truetype')}}",
            base64(&face::subset(bytes, chars))
        );
    }
}

/// The stylesheet: the palette's inks as properties, then the window's
/// shapes at its own measurements, then one class per colour code is set in.
#[allow(clippy::too_many_lines)]
fn style(out: &mut String, inks: &Inks, writer: &Writer, dark: bool) {
    let hex = |rgb: Rgb| rgb.to_hex();
    // The prose a step off the band's ink and a chip's ground just off the
    // band, both by the rules the window reads by, and the group's number
    // faint in the accent.
    let prose = reading::prose(inks.on_band.text, inks.band);
    let chip_ground = reading::chip(inks.band);
    let number = inks.band.mix(inks.on_band.accent, 0.45);
    let name_ground = inks.on_band.accent.mix(inks.band, 0.88);
    let name = legible(inks.on_band.accent, name_ground, LEGIBLE);
    let name_edge = inks.on_band.accent.mix(inks.band, 0.7);
    let quiet_bar = inks.edge.mix(inks.on_band.text, 0.08);
    let stem = inks.edge.mix(inks.on_band.muted, 0.25);
    let row = (MONO_SIZE * LINE_HEIGHT).round();
    let _ = writeln!(
        out,
        ":root{{color-scheme:{scheme};\
         --page:{page};--band:{band};--pane:{pane};--edge:{edge};\
         --page-text:{page_text};--page-muted:{page_muted};--page-accent:{page_accent};\
         --band-text:{band_text};--band-muted:{band_muted};--band-accent:{band_accent};\
         --pane-text:{pane_text};--pane-muted:{pane_muted};--pane-accent:{pane_accent};\
         --prose:{prose};--number:{number};--chip-ground:{chip_ground};--name:{name};\
         --name-ground:{name_ground};--name-edge:{name_edge};--quiet:{quiet_bar};--stem:{stem};\
         --going:{going};--arriving:{arriving};\
         --high:{high};--medium:{medium};--low:{low};\
         --sans:'IBM Plex Sans',system-ui,sans-serif;\
         --mono:'JetBrains Mono',ui-monospace,monospace}}",
        scheme = if dark { "dark" } else { "light" },
        page = hex(inks.page),
        band = hex(inks.band),
        pane = hex(inks.pane),
        edge = hex(inks.edge),
        page_text = hex(inks.on_page.text),
        page_muted = hex(inks.on_page.muted),
        page_accent = hex(inks.on_page.accent),
        band_text = hex(inks.on_band.text),
        band_muted = hex(inks.on_band.muted),
        band_accent = hex(inks.on_band.accent),
        pane_text = hex(inks.on_pane.text),
        pane_muted = hex(inks.on_pane.muted),
        pane_accent = hex(inks.on_pane.accent),
        prose = hex(prose),
        number = hex(number),
        chip_ground = hex(chip_ground),
        name = hex(name),
        name_ground = hex(name_ground),
        name_edge = hex(name_edge),
        quiet_bar = hex(quiet_bar),
        stem = hex(stem),
        going = hex(inks.going),
        arriving = hex(inks.arriving),
        high = hex(inks.levels.high),
        medium = hex(inks.levels.medium),
        low = hex(inks.levels.low),
    );
    // The rail's column is the window's own `RAIL`, 400. The HTML export is
    // the window laid out in a browser, and a reader who opens it should find
    // the rail where the window keeps it. The paged PDF is the one place that
    // keeps the old 304 (see `landscape::PAGE_RAIL`), because a sheet of A4
    // has a fixed width to share and the code would pay for a wider rail.
    let _ = write!(
        out,
        r"*{{box-sizing:border-box}}
html{{scroll-snap-type:y mandatory}}
body{{margin:0;background:var(--page);color:var(--page-text);font:400 {SANS_SIZE}px/1.4 var(--sans);font-kerning:normal;-webkit-print-color-adjust:exact;print-color-adjust:exact}}
a{{color:inherit;text-decoration:none}}
a:focus-visible{{outline:2px solid var(--band-accent);outline-offset:2px;border-radius:2px}}
.stop{{position:relative;width:100%;height:100vh;min-height:560px;display:grid;grid-template-columns:minmax(0,1fr) {RAIL}px;grid-template-rows:auto minmax(0,1fr) auto;scroll-snap-align:start;overflow:hidden;background:var(--page);border-bottom:1px solid var(--edge)}}
.stop.empty{{display:grid;place-items:center;color:var(--page-muted)}}
.band{{grid-column:1;background:var(--band);color:var(--band-text);display:flex;flex-direction:column;gap:{BAND_GAP}px;padding-bottom:{BAND_BOTTOM}px;min-height:{BAND_LEAST}px;max-height:{BAND_MOST}px;border-bottom:1px solid var(--edge);min-width:0}}
.head{{display:flex;align-items:center;gap:{STEM_GAP}px;padding:{BAND_TOP}px {BAND_SIDE}px 0;min-width:0}}
.num{{flex:none;width:{STEM}px;text-align:center;font-weight:300;font-size:{NUMBER_SIZE}px;line-height:36px;color:var(--number)}}
.num.tens{{font-size:{NUMBER_SIZE_TENS}px}}
.titles{{display:flex;flex-direction:column;gap:2px;min-width:0}}
.deck-title{{font-size:{DECK_TITLE_SIZE}px;color:var(--band-muted);white-space:nowrap;overflow:hidden;text-overflow:ellipsis}}
.title-row{{display:flex;align-items:center;gap:10px;min-width:0}}
h2{{margin:0;font-size:{TITLE_SIZE}px;font-weight:600;color:var(--band-text);white-space:nowrap;overflow:hidden;text-overflow:ellipsis;min-width:0}}
.conf{{flex:none;display:inline-flex;align-items:center;gap:6px;font-size:{CONFIDENCE_SIZE}px}}
.conf .why{{color:var(--band-muted)}}
.bars{{display:inline-flex;align-items:flex-end;gap:2px;height:11px}}
.bars i{{width:3px;border-radius:1px;background:var(--quiet)}}
.bars i:nth-child(1){{height:5px}}.bars i:nth-child(2){{height:8px}}.bars i:nth-child(3){{height:11px}}
.high{{color:var(--high)}}.high .bars i{{background:var(--high)}}
.medium{{color:var(--medium)}}.medium .bars i:nth-child(-n+2){{background:var(--medium)}}
.low{{color:var(--low)}}.low .bars i:nth-child(1){{background:var(--low)}}
.prose{{margin:-4px {BAND_SIDE}px 0 {stem_at}px;padding-left:{prose_at}px;border-left:1px solid var(--stem);font-size:{PROSE_SIZE}px;line-height:{PROSE_LEADING}px;color:var(--prose);overflow-y:auto;min-height:0}}
.prose p{{margin:0 0 {PROSE_SPACING}px}}
.prose p:first-child{{color:var(--band-text)}}
.prose strong{{font-weight:600;color:var(--band-text)}}
.prose em{{font-style:italic;color:var(--band-text)}}
.chip{{font:400 {CHIP_SIZE}px var(--mono);padding:1px {CHIP_PADDING}px;border-radius:3px;background:var(--chip-ground)}}
.name{{padding:0 5px;border-radius:3px;background:var(--name-ground);border:1px solid var(--name-edge);color:var(--name)}}
.mk{{display:inline-grid;place-items:center;min-width:15px;height:15px;padding:0 3px;border-radius:8px;background:var(--band-accent);color:var(--band);font:600 9px/1 var(--sans);vertical-align:1px;margin:0 2px}}
.panes{{grid-column:1;display:flex;min-height:0;min-width:0;background:var(--pane)}}
.pane{{flex:1 1 0;display:flex;flex-direction:column;min-width:0;min-height:0;border-right:1px solid var(--edge);overflow:hidden}}
.pane.code{{flex-grow:1.3}}
.pane:last-child{{border-right:0}}
.ph{{flex:none;display:flex;gap:8px;align-items:baseline;padding:12px 16px 6px;font:400 {PANE_HEAD}px var(--mono);color:var(--pane-muted);white-space:nowrap;overflow:hidden}}
.ph b{{font-weight:400;color:var(--pane-accent)}}
.ph .file{{overflow:hidden;text-overflow:ellipsis}}
.ph a{{color:inherit;text-decoration:underline;text-decoration-color:var(--edge);text-underline-offset:3px}}
.ph a:hover{{color:var(--pane-text)}}
.note{{flex:none;padding:0 16px 6px;font-size:11px;color:var(--pane-muted)}}
.rows{{flex:1;min-height:0;overflow:auto;padding:4px 0 10px;font:400 {MONO_SIZE}px/{row}px var(--mono);font-variant-ligatures:none}}
.row{{display:grid;grid-template-columns:26px 12px 40px minmax(0,1fr);align-items:baseline;padding-right:16px;position:relative}}
.row .at{{display:flex;justify-content:flex-end;gap:1px}}
.row .at .mk{{margin:0;vertical-align:0}}
.row .bar{{align-self:stretch;position:relative;text-align:center}}
.row.spot .bar::before,.row.lit .bar::before{{content:'';position:absolute;left:3px;top:0;bottom:0;width:3px;background:var(--pane-muted);opacity:.5}}
.row.lit .bar::before{{background:var(--accent-of-row,var(--pane-accent));opacity:1}}
.row.going .bar,.row.arriving .bar{{font-weight:400}}
.row.going .bar{{color:var(--going)}}.row.arriving .bar{{color:var(--arriving)}}
.row.going .bar::before,.row.arriving .bar::before{{display:none}}
.row .no{{text-align:right;padding-right:12px;color:var(--pane-muted);font-variant-numeric:tabular-nums}}
.row .t{{white-space:pre-wrap;overflow-wrap:anywhere}}
.row .it{{font-style:italic}}
.val{{margin:0 6px 0 8px;font-size:11px;white-space:nowrap}}
.val.lit{{font-weight:400;text-decoration:underline;text-underline-offset:3px}}
.gap{{padding:1px 0 1px 78px;font-size:11px;color:var(--pane-muted)}}
.missing{{padding:12px 16px;font-size:12px;color:var(--pane-muted)}}
.dg{{flex:1;min-height:0;display:flex;align-items:flex-start;justify-content:center;padding:4px 18px 10px;overflow:auto}}
.dg svg{{max-width:100%;height:auto}}
.marks{{flex:none;display:flex;flex-wrap:wrap;gap:4px 12px;padding:0 16px 10px;font:400 11px var(--mono);color:var(--pane-muted)}}
iframe.page{{flex:1;min-height:0;width:100%;border:0;background:transparent}}
.side{{grid-column:2;grid-row:1/3;background:var(--band);border-left:1px solid var(--edge);padding:26px 22px;display:flex;flex-direction:column;gap:14px;overflow:hidden;color:var(--band-muted)}}
.side h4{{margin:0;display:flex;justify-content:space-between;font:600 10.5px var(--sans);text-transform:uppercase;color:var(--band-muted)}}
.toc{{margin:0;padding:0;list-style:none;display:grid;gap:9px;font-size:{RAIL_SIZE}px;line-height:{RAIL_LEADING}px}}
.toc a{{display:grid;grid-template-columns:18px minmax(0,1fr);gap:6px}}
.toc a:hover{{color:var(--band-text)}}
.toc b{{font-weight:500}}
.toc .on>a{{color:var(--band-text)}}.toc .on>a b{{color:var(--band-accent)}}
.toc .said{{display:block;margin:2px 0 0 24px;font-size:11px;color:var(--band-muted)}}
.toc .said.on{{color:var(--band-accent)}}
.strip{{grid-column:1/-1;display:flex;align-items:center;gap:{STRIP_GAP}px;padding:{STRIP_ROW}px {STRIP_SIDE}px;background:var(--band);border-top:1px solid var(--edge);font:400 {STRIP_SIZE}px var(--mono);color:var(--band-muted);white-space:nowrap;min-width:0}}
.strip .fill{{flex:1;min-width:0;overflow:hidden;text-overflow:ellipsis}}
.brand{{display:inline-flex;align-items:center;gap:6px;color:inherit}}
.brand:hover{{color:var(--band-text)}}
.brand i{{width:9px;height:9px;border-radius:2px;background:var(--band-accent)}}
.go{{display:inline-flex;gap:4px}}
.go a,.go span{{display:inline-grid;place-items:center;width:22px;height:18px;border:1px solid var(--edge);border-radius:4px;color:var(--band-text)}}
.go span{{opacity:.35}}
.thin{{grid-column:1;display:flex;align-items:baseline;gap:{STEM_GAP}px;padding:{BAND_TOP}px {BAND_SIDE}px {BAND_BOTTOM}px;background:var(--band);border-bottom:1px solid var(--edge);min-width:0}}
.thin .num{{font-size:30px;line-height:30px}}
.thin h2 .dim{{font-weight:400;color:var(--band-muted)}}
.thin .count{{margin-left:auto;flex:none;font:400 11px var(--mono);color:var(--band-muted)}}
.thread{{grid-column:1;min-height:0;overflow:auto;padding:24px 30px;background:var(--page)}}
.cols{{column-width:340px;column-count:2;column-gap:34px;column-rule:1px solid var(--edge)}}
.msg{{break-inside:avoid;display:grid;grid-template-columns:10px minmax(0,1fr);gap:8px;margin:0 0 16px;font-size:{RAIL_SIZE}px;line-height:{RAIL_LEADING}px;color:var(--page-text)}}
.msg .dot{{width:8px;height:8px;border-radius:50%;margin-top:6px;border:1.5px solid var(--page-muted)}}
.msg.reader .dot{{background:var(--page-accent);border-color:var(--page-accent)}}
.who{{display:flex;gap:6px;font-size:10.5px;color:var(--page-muted)}}
.who b{{font-weight:500}}
.who .n{{margin-left:auto;font-family:var(--mono)}}
.msg p{{margin:2px 0 0;white-space:pre-wrap;overflow-wrap:anywhere}}
.q{{margin-top:7px;border-left:3px solid var(--page-accent);border-radius:0 4px 4px 0;font:400 11.5px/1.7 var(--mono);overflow:hidden}}
.q .row{{grid-template-columns:44px minmax(0,1fr);padding-right:10px}}
.q .quote{{padding:2px 10px;white-space:pre-wrap;background:var(--pane);color:var(--pane-text)}}
@page{{size:297mm 210mm;margin:0}}
@media print{{html{{scroll-snap-type:none}}.stop{{width:297mm;height:210mm;min-height:0;break-before:page;border:0}}.stop:first-child{{break-before:auto}}.go{{display:none}}.prose,.rows,.dg,.thread{{overflow:hidden}}}}
",
        stem_at = BAND_SIDE + STEM / 2.,
        prose_at = STEM / 2. + STEM_GAP - 1.,
    );
    for (colour, class) in &writer.inks {
        let _ = writeln!(out, ".i{class}{{color:{colour}}}");
    }
    for (colour, class) in &writer.grounds {
        let _ = writeln!(out, ".b{class}{{background:{colour}}}");
    }
}

/// What is being written, and the colours it has asked for so far.
struct Writer<'a> {
    document: &'a Document,
    stops: &'a [Stop<'a>],
    /// Every ink code is set in, by hex, and the class that sets it: a few
    /// dozen colours, where an attribute on every run would repeat them by
    /// the thousand.
    inks: BTreeMap<String, usize>,
    /// The same, for the grounds rows are set on.
    grounds: BTreeMap<String, usize>,
    out: String,
}

impl Writer<'_> {
    fn ink(&mut self, colour: Rgb) -> String {
        let next = self.inks.len();
        format!("i{}", self.inks.entry(colour.to_hex()).or_insert(next))
    }

    fn ground(&mut self, colour: Rgb) -> String {
        let next = self.grounds.len();
        format!("b{}", self.grounds.entry(colour.to_hex()).or_insert(next))
    }

    fn stop(&mut self, ix: usize) {
        let stop = self.stops[ix];
        let id = stop.id();
        match stop {
            Stop::Group(group) => {
                let _ = writeln!(
                    self.out,
                    "<section class=\"stop\" id=\"{id}\" aria-label=\"{}\">",
                    esc(&format!("Group {}: {}", group.n, group.title))
                );
                self.band(group);
                self.panes(group);
            }
            Stop::Argument(group) => {
                let _ = writeln!(
                    self.out,
                    "<section class=\"stop argument\" id=\"{id}\" aria-label=\"{}\">",
                    esc(&format!("What was said about group {}", group.n))
                );
                self.thin(Some(group), &group.title, group.argument.len());
                self.thread(&group.argument, Some(group));
            }
            Stop::General(said) => {
                let _ = writeln!(
                    self.out,
                    "<section class=\"stop argument\" id=\"{id}\" aria-label=\"{}\">",
                    esc("What was said about the whole deck")
                );
                self.thin(None, &self.document.title, said.len());
                self.thread(said, None);
            }
        }
        self.side(ix);
        self.strip(ix);
        self.out.push_str("</section>\n");
    }

    fn band(&mut self, group: &Group) {
        let tens = if group.n >= 10 { " tens" } else { "" };
        let _ = write!(
            self.out,
            "<header class=\"band\">\n<div class=\"head\"><div class=\"num{tens}\">{}</div>\
             <div class=\"titles\"><div class=\"deck-title\">{}</div>\
             <div class=\"title-row\"><h2>{}</h2>",
            group.n,
            esc(&self.document.title),
            esc(&group.title),
        );
        if let Some(confidence) = &group.confidence {
            let (class, name) = match confidence.level {
                Level::High => ("high", "High"),
                Level::Medium => ("medium", "Medium"),
                Level::Low => ("low", "Low"),
            };
            let _ = write!(
                self.out,
                "<span class=\"conf {class}\"><span class=\"bars\"><i></i><i></i><i></i></span>{name}"
            );
            if let Some(why) = &confidence.why {
                let _ = write!(self.out, "<span class=\"why\">· {}</span>", esc(why));
            }
            self.out.push_str("</span>");
        }
        self.out
            .push_str("</div></div></div>\n<div class=\"prose\">\n");
        for paragraph in &group.prose {
            self.out.push_str("<p>");
            for run in paragraph {
                self.run(run);
            }
            self.out.push_str("</p>\n");
        }
        self.out.push_str("</div>\n</header>\n");
    }

    fn run(&mut self, run: &Run) {
        match run {
            Run::Point { numeral } => {
                let _ = write!(self.out, "<span class=\"mk\">{numeral}</span>");
            }
            Run::Text { text, mark, pane } => {
                // A chip's trailing space stays outside it, as the window
                // draws it.
                let (word, after) = match mark {
                    Some(Marked::Code | Marked::Pane) => {
                        let word = text.trim_end();
                        (word, &text[word.len()..])
                    }
                    _ => (text.as_str(), ""),
                };
                match (mark, pane) {
                    (Some(Marked::Pane), Some(pane)) => {
                        let _ = write!(
                            self.out,
                            "<a class=\"name\" href=\"#{}\">{}</a>",
                            anchor(pane),
                            esc(word)
                        );
                    }
                    (Some(Marked::Pane), None) => {
                        let _ = write!(self.out, "[{}]", esc(word));
                    }
                    (Some(Marked::Code), _) => {
                        let _ = write!(self.out, "<code class=\"chip\">{}</code>", esc(word));
                    }
                    (Some(Marked::Strong), _) => {
                        let _ = write!(self.out, "<strong>{}</strong>", esc(word));
                    }
                    (Some(Marked::Emphasis), _) => {
                        let _ = write!(self.out, "<em>{}</em>", esc(word));
                    }
                    (None, _) => self.out.push_str(&esc(word)),
                }
                self.out.push_str(&esc(after));
            }
        }
    }

    fn panes(&mut self, group: &Group) {
        self.out.push_str("<div class=\"panes\">\n");
        for pane in &group.panes {
            match pane {
                Pane::Code(code) => self.code(code),
                Pane::Diagram(picture) => self.picture(picture),
                Pane::Page(page) => self.page(page),
            }
        }
        self.out.push_str("</div>\n");
    }

    /// A pane's header: its name, what it shows, and its note. `url`, for
    /// a code pane, takes the file and its range to those lines on the
    /// repo's host.
    fn header(&mut self, name: Option<&str>, what: &str, note: Option<&str>, url: Option<&str>) {
        let what = match url {
            Some(url) => format!(
                "<a href=\"{}\" rel=\"noopener noreferrer\" target=\"_blank\">{}</a>",
                esc(url),
                esc(what)
            ),
            None => esc(what),
        };
        let _ = write!(
            self.out,
            "<div class=\"ph\">{}<span class=\"file\">{what}</span></div>",
            name.map(|name| format!("<b>{}</b>", esc(name)))
                .unwrap_or_default(),
        );
        if let Some(note) = note {
            let _ = write!(self.out, "<div class=\"note\">{}</div>", esc(note));
        }
    }

    fn code(&mut self, code: &Code) {
        let _ = write!(
            self.out,
            "<section class=\"pane code\" id=\"{}\">",
            anchor(&code.id)
        );
        let range = if code.range.first == code.range.last {
            format!("{}:{}", code.file, code.range.first)
        } else {
            format!("{}:{}-{}", code.file, code.range.first, code.range.last)
        };
        let what = match code.change {
            Some(Change::Proposed) => format!("{range} · proposed"),
            Some(Change::Made) => format!("{range} · changed"),
            None => range,
        };
        let url = super::links::source(self.document.repo.as_ref(), code);
        self.header(
            code.name.as_deref(),
            &what,
            code.note.as_deref(),
            url.as_deref(),
        );
        if code.missing {
            let _ = writeln!(
                self.out,
                "<div class=\"missing\">{}</div></section>",
                esc("Not in this deck: these lines were not packed with it.")
            );
            return;
        }
        self.out.push_str("<div class=\"rows\">\n");
        for line in &code.lines {
            match line {
                Line::Gap { lines } => {
                    let _ = writeln!(
                        self.out,
                        "<div class=\"gap\">⋯ {lines} {}</div>",
                        if *lines == 1 { "line" } else { "lines" }
                    );
                }
                Line::Code(row) => {
                    // Far context goes, but never a row a point lights or
                    // starts on: a point can start well above its range, and
                    // its numeral and its lit lines are the point.
                    let far = matches!(row.kind, RowKind::Context { distance } if distance > CONTEXT_SHOWN);
                    if far && row.numerals.is_empty() && row.lit_by.is_empty() {
                        continue;
                    }
                    self.row(row, true);
                    self.out.push('\n');
                }
            }
        }
        self.out.push_str("</div></section>\n");
    }

    /// One row of code: its numerals in the margin, its bar, its number, its
    /// runs and what it held. `margin` off is a row quoted in the argument,
    /// which has no gutter of its own.
    fn row(&mut self, row: &Row, margin: bool) {
        let kind = match row.kind {
            RowKind::Context { .. } => "ctx",
            RowKind::Spot => "spot",
            RowKind::Going => "going",
            RowKind::Arriving => "arriving",
        };
        let lit = if row.lit_by.is_empty() { "" } else { " lit" };
        let ground = self.ground(row.ground);
        let accent = self.ink(row.accent);
        let _ = write!(self.out, "<div class=\"row {kind}{lit} {ground}\">");
        if margin {
            self.out.push_str("<span class=\"at\">");
            for numeral in &row.numerals {
                let _ = write!(
                    self.out,
                    "<span class=\"mk\" style=\"background:{};color:{}\">{numeral}</span>",
                    row.accent.to_hex(),
                    row.ground.to_hex()
                );
            }
            let sign = match row.kind {
                RowKind::Going => "−",
                RowKind::Arriving => "+",
                _ => "",
            };
            let bar = if lit.is_empty() {
                String::new()
            } else {
                format!(" style=\"--accent-of-row:{}\"", row.accent.to_hex())
            };
            let _ = write!(self.out, "</span><span class=\"bar\"{bar}>{sign}</span>");
        }
        let number = if lit.is_empty() {
            String::new()
        } else {
            format!(" {accent}")
        };
        let _ = write!(
            self.out,
            "<span class=\"no{number}\">{}</span><span class=\"t\">",
            row.no
        );
        let context = matches!(row.kind, RowKind::Context { .. });
        // A single value goes inline, right after the first time its name
        // is on the line, or at the line's end when it is not, as the window
        // places it; a value with keys gets a row of its own under the line.
        let text: String = row.spans.iter().map(|span| span.text.as_str()).collect();
        let mut inlays: Vec<(usize, &deck_core::protocol::Value)> = row
            .values
            .iter()
            .filter(|value| value.keys.is_empty())
            .map(|value| (crate::after_name(&text, &value.at), value))
            .collect();
        inlays.sort_by_key(|(at, _)| *at);
        let mut inlays = inlays.into_iter().peekable();
        let mut at = 0;
        for span in &row.spans {
            // Context a step back from the range, as the window washes it,
            // and never past legible.
            let ink = if context {
                legible(span.ink.mix(row.ground, CONTEXT_FADE), row.ground, LEGIBLE)
            } else {
                span.ink
            };
            let class = self.ink(ink);
            let italic = if span.italic { " it" } else { "" };
            let end = at + span.text.len();
            let mut from = at;
            // A value lands inside a run wherever its name ends there; the
            // run is cut around it and goes on after.
            while let Some(&(place, value)) = inlays.peek()
                && place < end
            {
                let cut = place.max(from) - at;
                if cut > from - at {
                    let _ = write!(
                        self.out,
                        "<span class=\"{class}{italic}\">{}</span>",
                        esc(&span.text[from - at..cut])
                    );
                }
                self.value(value, &accent);
                from = at + cut;
                inlays.next();
            }
            if from < end {
                let _ = write!(
                    self.out,
                    "<span class=\"{class}{italic}\">{}</span>",
                    esc(&span.text[from - at..])
                );
            }
            at = end;
        }
        for (_, value) in inlays {
            self.value(value, &accent);
        }
        self.out.push_str("</span></div>");
        let indent = text.len() - text.trim_start().len();
        for value in row.values.iter().filter(|value| !value.keys.is_empty()) {
            let _ = write!(
                self.out,
                "<div class=\"row preview {ground}\">{}<span class=\"no\"></span>\
                 <span class=\"t\">{}<span class=\"{accent}\">↳ {} ▸ {}</span></span></div>",
                if margin {
                    "<span class=\"at\"></span><span class=\"bar\"></span>"
                } else {
                    ""
                },
                " ".repeat(indent),
                esc(&value.at),
                esc(&value.is)
            );
        }
    }

    /// One value, inline on its line, in the row's accent.
    fn value(&mut self, value: &deck_core::protocol::Value, accent: &str) {
        let lit = if value.lit { " lit" } else { "" };
        let _ = write!(
            self.out,
            "<span class=\"val{lit} {accent}\">{}</span>",
            esc(&value.is)
        );
    }

    fn picture(&mut self, picture: &Picture) {
        let _ = write!(
            self.out,
            "<section class=\"pane diagram\" id=\"{}\">",
            anchor(&picture.id)
        );
        self.header(
            picture.name.as_deref(),
            picture.note.as_deref().unwrap_or("diagram"),
            None,
            None,
        );
        // Written by deck's own SVG writer, which escapes every word it
        // draws: markup to place, not text to escape again.
        let _ = write!(self.out, "<div class=\"dg\">{}</div>", picture.svg);
        if !picture.marks.is_empty() {
            self.out.push_str("<div class=\"marks\">");
            for mark in &picture.marks {
                let _ = write!(
                    self.out,
                    "<span><span class=\"mk\">{}</span>{}</span>",
                    mark.numeral,
                    esc(&mark.block)
                );
            }
            self.out.push_str("</div>");
        }
        self.out.push_str("</section>\n");
    }

    /// A page pane: the agent's markup, in a frame that can run nothing.
    ///
    /// `sandbox` with no permissions at all: no scripts, no forms, no popups,
    /// no navigating the export away, and an origin of its own, so even
    /// markup that tried could not reach this document or its `.deck`. The
    /// page's policy reaches into the frame as well, so it cannot load
    /// anything either. What it loses is its motion — the window drives a
    /// page's steps with a script — and it is drawn as it is at rest, which
    /// is how the window opens one.
    fn page(&mut self, page: &Page) {
        let _ = write!(
            self.out,
            "<section class=\"pane page\" id=\"{}\">",
            anchor(&page.id)
        );
        let label = page.note.as_deref().unwrap_or("page");
        self.header(page.name.as_deref(), label, None, None);
        let _ = writeln!(
            self.out,
            "<iframe class=\"page\" sandbox=\"\" referrerpolicy=\"no-referrer\" title=\"{}\" \
             srcdoc=\"{}\"></iframe></section>",
            esc(page.name.as_deref().unwrap_or(label)),
            esc(&dressed(&page.html, &self.document.inks))
        );
    }

    fn thin(&mut self, group: Option<&Group>, title: &str, said: usize) {
        self.out.push_str("<header class=\"thin\">");
        if let Some(group) = group {
            let _ = write!(self.out, "<div class=\"num\">{}</div>", group.n);
        }
        let what = if group.is_some() {
            "the argument"
        } else {
            "what was said about the deck"
        };
        let _ = writeln!(
            self.out,
            "<h2>{} <span class=\"dim\">— {what}</span></h2><span class=\"count\">{said} said</span></header>",
            esc(title)
        );
    }

    fn thread(&mut self, said: &[Message], group: Option<&Group>) {
        self.out
            .push_str("<div class=\"thread\"><div class=\"cols\">\n");
        let all = said.len();
        for (ix, message) in said.iter().enumerate() {
            let (class, who) = match message.author {
                Author::Reader => ("reader", "Reader"),
                Author::Agent => ("agent", "Agent"),
            };
            let _ = write!(
                self.out,
                "<article class=\"msg {class}\"><span class=\"dot\"></span><div><div class=\"who\"><b>{who}</b>"
            );
            if message.author == Author::Reader
                && let Some(kind) = message.kind
            {
                let kind = match kind {
                    Kind::MustFix => "must fix",
                    Kind::Question => "question",
                    Kind::Nit => "nit",
                };
                let _ = write!(self.out, "<span>· {kind}</span>");
            }
            let about = match (&message.pane, message.range) {
                (Some(pane), Some(range)) if range.first == range.last => {
                    Some(format!("{pane}, line {}", range.first))
                }
                (Some(pane), Some(range)) => {
                    Some(format!("{pane}, lines {}–{}", range.first, range.last))
                }
                (Some(pane), None) => Some(pane.clone()),
                (None, _) => None,
            };
            if let Some(about) = about {
                let _ = write!(self.out, "<span>· {}</span>", esc(&about));
            }
            let _ = write!(
                self.out,
                "<span class=\"n\">{}/{all}</span></div><p>{}</p>",
                ix + 1,
                esc(&message.text)
            );
            self.quote(message, group);
            self.out.push_str("</div></article>\n");
        }
        self.out.push_str("</div></div>\n");
    }

    /// The lines a remark was about, carried with it so it reads without
    /// turning back: the rows themselves when the group printed them, and
    /// what the reader had selected when it did not.
    fn quote(&mut self, message: &Message, group: Option<&Group>) {
        let rows: Vec<Row> = match (group, &message.ref_id, message.range) {
            (Some(group), Some(id), Some(range)) => group
                .panes
                .iter()
                .filter_map(|pane| match pane {
                    Pane::Code(code) if &code.id == id => Some(code),
                    _ => None,
                })
                .flat_map(|code| {
                    // The remark is about the lines in the file, as the window
                    // takes it: for a change already made those are what
                    // arrived, and for anything else what is there now.
                    let other = if code.change == Some(Change::Made) {
                        RowKind::Going
                    } else {
                        RowKind::Arriving
                    };
                    code.lines.iter().filter_map(move |line| match line {
                        Line::Code(row) if range.contains(row.no) && row.kind != other => {
                            Some(row.clone())
                        }
                        _ => None,
                    })
                })
                .take(3)
                .collect(),
            _ => Vec::new(),
        };
        if !rows.is_empty() {
            self.out.push_str("<div class=\"q\">");
            for row in &rows {
                self.row(row, false);
            }
            self.out.push_str("</div>");
        } else if !message.quote.trim().is_empty() {
            let _ = write!(
                self.out,
                "<div class=\"q\"><div class=\"quote\">{}</div></div>",
                esc(message.quote.trim())
            );
        }
    }

    /// The contents down the side: every group shown, the one this stop
    /// belongs to lit, and under each with an argument a way to it.
    fn side(&mut self, ix: usize) {
        let here = self.stops[ix];
        let shown: Vec<&Group> = self
            .stops
            .iter()
            .filter_map(|stop| match stop {
                Stop::Group(group) => Some(*group),
                _ => None,
            })
            .collect();
        let at = here
            .group()
            .and_then(|group| shown.iter().position(|g| g.n == group.n));
        let _ = write!(
            self.out,
            "<nav class=\"side\" aria-label=\"Contents\"><h4><span>Contents</span><span>{}</span></h4><ol class=\"toc\">",
            match at {
                Some(at) => format!("{} of {}", at + 1, shown.len()),
                None => format!("{}", shown.len()),
            }
        );
        for group in &shown {
            let on = here.group().is_some_and(|g| g.n == group.n);
            let _ = write!(
                self.out,
                "<li{}><a href=\"#g{n}\"><b>{n}</b><span>{}</span></a>",
                if on { " class=\"on\"" } else { "" },
                esc(&group.title),
                n = group.n
            );
            let argued = self
                .stops
                .iter()
                .any(|stop| matches!(stop, Stop::Argument(g) if g.n == group.n));
            if argued {
                let lit = matches!(here, Stop::Argument(g) if g.n == group.n);
                let _ = write!(
                    self.out,
                    "<a class=\"said{}\" href=\"#g{}-argument\">the argument · {}</a>",
                    if lit { " on" } else { "" },
                    group.n,
                    group.argument.len()
                );
            }
            self.out.push_str("</li>");
        }
        self.out.push_str("</ol>");
        if self
            .stops
            .iter()
            .any(|stop| matches!(stop, Stop::General(_)))
        {
            let _ = write!(
                self.out,
                "<ol class=\"toc\"><li{}><a href=\"#said\"><b>·</b><span>{}</span></a></li></ol>",
                if matches!(here, Stop::General(_)) {
                    " class=\"on\""
                } else {
                    ""
                },
                esc("What was said about the deck")
            );
        }
        self.out.push_str("</nav>\n");
    }

    /// The strip along the bottom: who made it, where this is in the deck,
    /// which code it is about and when, and the way to the stops either side.
    fn strip(&mut self, ix: usize) {
        let here = self.stops[ix];
        let total = self.document.groups.len();
        let place = match here {
            Stop::Group(group) => format!("group {}/{total}", group.n),
            Stop::Argument(group) => format!("group {}/{total} · the argument", group.n),
            Stop::General(_) => "the deck".to_string(),
        };
        let _ = write!(
            self.out,
            "<footer class=\"strip\"><a class=\"brand\" href=\"{deck}\" rel=\"noopener noreferrer\" \
             target=\"_blank\"><i></i>Powered by Deck</a>\
             <span>{}</span><span class=\"fill\">{}</span>",
            esc(&place),
            esc(&self.document.title),
            deck = super::links::DECK,
        );
        if let Some(repo) = repo(self.document) {
            let _ = write!(self.out, "<span>{}</span>", esc(&repo));
        }
        if let Some(day) = day(&self.document.created) {
            let _ = write!(self.out, "<span>{}</span>", esc(&day));
        }
        let _ = write!(
            self.out,
            "<span>page {} of {}</span><span class=\"go\">",
            ix + 1,
            self.stops.len()
        );
        for (to, glyph, label) in [
            (ix.checked_sub(1), "←", "Previous"),
            (Some(ix + 1).filter(|&n| n < self.stops.len()), "→", "Next"),
        ] {
            match to {
                Some(to) => {
                    let _ = write!(
                        self.out,
                        "<a href=\"#{}\" aria-label=\"{label}\">{glyph}</a>",
                        self.stops[to].id()
                    );
                }
                None => {
                    let _ = write!(self.out, "<span aria-hidden=\"true\">{glyph}</span>");
                }
            }
        }
        self.out.push_str("</span></footer>\n");
    }
}

/// The repo as the strip names it: `acme/ingest@a1b2c3d`.
fn repo(document: &Document) -> Option<String> {
    let repo = document.repo.as_ref()?;
    let name = repo.remote.as_deref().map(|remote| {
        let remote = remote.trim_end_matches('/').trim_end_matches(".git");
        let parts: Vec<&str> = remote.rsplit(['/', ':']).take(2).collect();
        parts.into_iter().rev().collect::<Vec<_>>().join("/")
    });
    let commit = repo
        .commit
        .as_deref()
        .map(|commit| commit.chars().take(7).collect::<String>());
    match (name, commit) {
        (Some(name), Some(commit)) => Some(format!("{name}@{commit}")),
        (Some(name), None) => Some(name),
        (None, Some(commit)) => Some(commit),
        (None, None) => None,
    }
}

/// The day a deck was packed, as `20 Oct 2026`: a date and never a time,
/// since the time it was packed is nothing its reader needs and something
/// they may read as when the review happened.
fn day(created: &str) -> Option<String> {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let year = created.get(..4)?;
    let month: usize = created.get(5..7)?.parse().ok()?;
    let date: u32 = created.get(8..10)?.parse().ok()?;
    Some(format!(
        "{date} {} {year}",
        MONTHS.get(month.checked_sub(1)?)?
    ))
}

/// The page's markup, wearing the export's colours as the window dresses it:
/// the same custom properties, so a page that asks for `var(--deck-accent)`
/// gets this palette's. Without a script, nothing a page shows step by step
/// is shown, which is the still first frame the window opens it on.
fn dressed(html: &str, inks: &Inks) -> String {
    format!(
        "<!doctype html><meta charset=\"utf-8\"><style>:root{{\
           --deck-bg:{bg};--deck-fg:{fg};--deck-accent:{accent};\
           --deck-on-accent:{bg};--deck-muted:{muted};--deck-edge:{edge};\
           --deck-wash:{wash};--deck-add:{add};--deck-del:{del};\
           --deck-comment:{muted};\
         }}\
         html,body{{background:transparent;color:var(--deck-fg);\
           font:12px ui-monospace,SFMono-Regular,Menlo,monospace;}}\
         [data-show]:not(.deck-seen),[data-from]:not(.deck-seen)\
           {{opacity:0!important;pointer-events:none!important}}\
         </style>{html}",
        bg = inks.page.to_hex(),
        fg = inks.on_pane.text.to_hex(),
        accent = inks.on_pane.accent.to_hex(),
        muted = inks.on_pane.muted.to_hex(),
        edge = inks.edge.to_hex(),
        wash = inks.pane.to_hex(),
        add = inks.arriving.to_hex(),
        del = inks.going.to_hex(),
    )
}

/// A ref's id as a fragment: letters, digits and dashes, which every id deck
/// writes already is, and a dash for anything else.
fn anchor(id: &str) -> String {
    let safe: String = id
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect();
    format!("pane-{safe}")
}

/// `text` as HTML that says exactly it, in text or in a quoted attribute.
pub(super) fn esc(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}
