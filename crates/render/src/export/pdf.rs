//! Laid-out pages as a PDF, written with krilla.
//!
//! Nothing is decided here: every box and every line of text is where
//! [`super::layout`] put it, and this only draws it. The text is real text in
//! the embedded faces — Plex for words, JetBrains Mono for code — which krilla
//! subsets to the glyphs a page uses, so a reader can select it, search it
//! and copy code out of it. Diagrams go through krilla-svg with Plex in its
//! font database, their labels kept as text too.
//!
//! The file is the same file every time for the same deck: its creation date
//! is the day the deck was packed, krilla names a document by a hash of what
//! is in it, and no clock is read. The one exception is text the embedded
//! faces cannot set — CJK, emoji, Arabic — which is drawn from a face the
//! machine has: a PDF made on another machine may set those runs, and only
//! those, in another face. It carries the `.deck` it was made from, as
//! an attachment a reader's PDF viewer lists, so whoever it was sent to can
//! take it out and open it in deck.

use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, OnceLock};

use deck_core::document::Document;
use deck_core::theme::Rgb;
use krilla::color::rgb;
use krilla::destination::{Destination, XyzDestination};
use krilla::embed::{AssociationKind, EmbeddedFile, MimeType};
use krilla::geom::{PathBuilder, Point, Size, Transform};
use krilla::metadata::{DateTime, Metadata};
use krilla::num::NormalizedF32;
use krilla::outline::{Outline, OutlineNode};
use krilla::page::PageSettings;
use krilla::paint::{Fill, Stroke};
use krilla::surface::Surface;
use krilla::text::{Font, GlyphId, KrillaGlyph};
use krilla::{Data, Document as Pdf};
use krilla_svg::{SurfaceExt as _, SvgSettings};

use super::layout::{Face, Item, Kind, Page, Rect};
use crate::text::{MONO, PLEX};

/// How a PDF is to be written, beyond its pages.
#[derive(Debug, Clone, Default)]
pub struct Writing {
    /// What the embedded `.deck` is called.
    pub name: String,
    /// Draw the 4-point grid and every box's outline over the pages, to
    /// check the spacing by eye.
    pub grid: bool,
}

/// Where a run of characters is drawn from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Source {
    /// The face the layout chose.
    Own,
    /// A face from the machine, for what the embedded faces lack.
    Machine(usvg::fontdb::ID),
}

/// The faces from the machine, found only once something needs them.
struct Machine {
    db: Arc<usvg::fontdb::Database>,
    chosen: HashMap<char, Option<usvg::fontdb::ID>>,
    loaded: HashMap<usvg::fontdb::ID, (Font, Arc<Vec<u8>>, u32)>,
}

/// The families tried first for a character the embedded faces lack, in
/// order: the Mac's own CJK, emoji and Arabic faces, then Noto, then a face
/// that holds nearly everything. Past these, any face on the machine that
/// has the character.
const FALLBACKS: &[&str] = &[
    "PingFang SC",
    "Hiragino Sans",
    "Hiragino Kaku Gothic ProN",
    "Apple SD Gothic Neo",
    "Apple Color Emoji",
    "Geeza Pro",
    "Arial Hebrew",
    "Noto Sans CJK SC",
    "Noto Sans CJK JP",
    "Noto Color Emoji",
    "Noto Sans Arabic",
    "Noto Sans Hebrew",
    "Noto Sans",
    "Arial Unicode MS",
];

impl Machine {
    fn load() -> Self {
        let mut db = usvg::fontdb::Database::new();
        db.load_system_fonts();
        // Plex, so a diagram read with this database is still set in it.
        for (bytes, _) in PLEX {
            db.load_font_data(bytes.to_vec());
        }
        db.set_sans_serif_family("IBM Plex Sans");
        Self {
            db: Arc::new(db),
            chosen: HashMap::new(),
            loaded: HashMap::new(),
        }
    }

    fn has(&self, id: usvg::fontdb::ID, ch: char) -> bool {
        self.db
            .with_face_data(id, |data, index| {
                rustybuzz::ttf_parser::Face::parse(data, index)
                    .ok()
                    .and_then(|face| face.glyph_index(ch))
                    .is_some()
            })
            .unwrap_or(false)
    }

    /// A face on the machine with a glyph for `ch`, if there is one.
    fn face_for(&mut self, ch: char) -> Option<usvg::fontdb::ID> {
        if let Some(chosen) = self.chosen.get(&ch) {
            return *chosen;
        }
        let named = FALLBACKS.iter().flat_map(|family| {
            self.db
                .faces()
                .filter(move |face| face.families.iter().any(|(name, _)| name == family))
                .map(|face| face.id)
        });
        let any = self.db.faces().map(|face| face.id);
        let found = named.chain(any).find(|&id| self.has(id, ch));
        self.chosen.insert(ch, found);
        found
    }

    fn font(&mut self, id: usvg::fontdb::ID) -> Option<(Font, Arc<Vec<u8>>, u32)> {
        if let Some(loaded) = self.loaded.get(&id) {
            return Some(loaded.clone());
        }
        let (bytes, index) = self
            .db
            .with_face_data(id, |data, index| (Arc::new(data.to_vec()), index))?;
        let font = Font::new(Data::from(bytes.clone()), index)?;
        self.loaded.insert(id, (font.clone(), bytes.clone(), index));
        Some((font, bytes, index))
    }
}

/// What text is drawn with: the embedded faces, the machine's when they
/// lack a character, and a record of what nothing could draw.
struct Pen {
    faces: Faces,
    machine: Option<Machine>,
    missing: BTreeSet<char>,
}

/// The embedded face `face` is, parsed once, for shaping and coverage.
fn own(face: Face) -> Option<&'static rustybuzz::Face<'static>> {
    static OWN: [OnceLock<Option<rustybuzz::Face<'static>>>; 5] = [const { OnceLock::new() }; 5];
    let (ix, bytes) = match face {
        Face::Regular => (0, PLEX[0].0),
        Face::Medium => (1, PLEX[1].0),
        Face::Semibold => (2, PLEX[2].0),
        Face::Mono => (3, MONO[0].0),
        Face::MonoItalic => (4, MONO[1].0),
    };
    OWN[ix]
        .get_or_init(|| rustybuzz::Face::from_slice(bytes, 0))
        .as_ref()
}

/// A character that belongs with the one before it, whatever face that one
/// came from: a joiner, a variation selector, a skin tone, a combining mark.
fn rides(ch: char) -> bool {
    matches!(
        ch,
        '\u{200D}' | '\u{FE00}'..='\u{FE0F}' | '\u{1F3FB}'..='\u{1F3FF}'
            | '\u{0300}'..='\u{036F}' | '\u{E0020}'..='\u{E007F}'
    )
}

impl Pen {
    fn machine(&mut self) -> &mut Machine {
        self.machine.get_or_insert_with(Machine::load)
    }

    /// `text` cut into runs by where each is drawn from.
    fn runs(&mut self, face: Face, text: &str) -> Vec<(Source, std::ops::Range<usize>)> {
        let mine = own(face);
        let mut out: Vec<(Source, std::ops::Range<usize>)> = Vec::new();
        for (at, ch) in text.char_indices() {
            let source = if ch.is_whitespace() || rides(ch) {
                out.last().map_or(Source::Own, |(source, _)| *source)
            } else if mine.is_some_and(|face| face.glyph_index(ch).is_some()) {
                Source::Own
            } else if let Some(id) = self.machine().face_for(ch) {
                Source::Machine(id)
            } else {
                self.missing.insert(ch);
                Source::Own
            };
            let end = at + ch.len_utf8();
            match out.last_mut() {
                Some((was, range)) if *was == source => range.end = end,
                _ => out.push((source, at..end)),
            }
        }
        out
    }

    /// Whether a diagram needs the machine's faces, noting what not even
    /// they can draw.
    fn diagram_needs_machine(&mut self, svg: &str) -> bool {
        let Some(plex) = own(Face::Regular) else {
            return false;
        };
        let lacking: Vec<char> = svg
            .chars()
            .filter(|ch| !ch.is_ascii() && !ch.is_whitespace() && !rides(*ch))
            .filter(|ch| plex.glyph_index(*ch).is_none())
            .collect();
        if lacking.is_empty() {
            return false;
        }
        for ch in lacking {
            if self.machine().face_for(ch).is_none() {
                self.missing.insert(ch);
            }
        }
        true
    }
}

/// `text` in `face` at `size` from `at`, shaped here rather than by krilla.
///
/// krilla's own shaping turns on every feature a face has, and JetBrains
/// Mono's ligatures draw `===` as one long bar and `!=` as `≠`: not what the
/// file says, in a review of the file. Code is shaped with ligatures and
/// contextual alternates off; Plex keeps its kerning.
///
/// What the embedded faces have no glyph for — CJK, emoji, Arabic — is drawn
/// from a face on the machine, still as real text, and a right-to-left run
/// is shaped right to left. Those runs are the one part of a PDF that can
/// differ between machines: another machine may have another face, or none.
fn write(surface: &mut Surface, pen: &mut Pen, face: Face, size: f32, at: Point, text: &str) {
    let features: &[&str] = match face {
        Face::Mono | Face::MonoItalic => &["-liga", "-calt", "-dlig"],
        _ => &[],
    };
    let features: Vec<rustybuzz::Feature> = features
        .iter()
        .filter_map(|feature| feature.parse().ok())
        .collect();
    let mut x = at.x;
    for (source, range) in pen.runs(face, text) {
        let piece = &text[range];
        let drawn = match source {
            Source::Own => {
                let Some(shaper) = own(face) else { continue };
                let advance = draw_run(
                    surface,
                    shaper,
                    &features,
                    pen.faces.get(face),
                    size,
                    x,
                    at.y,
                    piece,
                );
                Some(advance)
            }
            Source::Machine(id) => pen.machine().font(id).and_then(|(font, bytes, index)| {
                let shaper = rustybuzz::Face::from_slice(&bytes, index)?;
                Some(draw_run(surface, &shaper, &[], font, size, x, at.y, piece))
            }),
        };
        x += drawn.unwrap_or(0.);
    }
}

/// One run shaped with `shaper` and drawn in `font` at `x`, `y`; returns how
/// far it moved the pen.
#[allow(clippy::too_many_arguments)]
fn draw_run(
    surface: &mut Surface,
    shaper: &rustybuzz::Face,
    features: &[rustybuzz::Feature],
    font: Font,
    size: f32,
    x: f32,
    y: f32,
    text: &str,
) -> f32 {
    #[allow(clippy::cast_precision_loss)]
    let units = shaper.units_per_em() as f32;
    let mut buffer = rustybuzz::UnicodeBuffer::new();
    buffer.push_str(text);
    // The script, and so the direction: Arabic and Hebrew come out right to
    // left, in the order they are seen.
    buffer.guess_segment_properties();
    let shaped = rustybuzz::shape(shaper, features, buffer);
    let infos = shaped.glyph_infos();
    // Each glyph's text runs from its cluster to the next cluster after it,
    // which in a right-to-left run is not the next glyph's.
    let mut clusters: Vec<usize> = infos.iter().map(|info| info.cluster as usize).collect();
    clusters.sort_unstable();
    clusters.dedup();
    let mut advance = 0.;
    let glyphs: Vec<KrillaGlyph> = infos
        .iter()
        .zip(shaped.glyph_positions())
        .map(|(info, at)| {
            let start = info.cluster as usize;
            let end = clusters
                .iter()
                .find(|&&next| next > start)
                .copied()
                .unwrap_or(text.len());
            #[allow(clippy::cast_precision_loss)]
            let glyph = KrillaGlyph::new(
                GlyphId::new(info.glyph_id),
                at.x_advance as f32 / units,
                at.x_offset as f32 / units,
                at.y_offset as f32 / units,
                at.y_advance as f32 / units,
                start..end,
                None,
            );
            #[allow(clippy::cast_precision_loss)]
            {
                advance += at.x_advance as f32 / units * size;
            }
            glyph
        })
        .collect();
    surface.draw_glyphs(Point::from_xy(x, y), &glyphs, font, text, size, false);
    advance
}

/// The faces, parsed once.
struct Faces {
    regular: Font,
    medium: Font,
    semibold: Font,
    mono: Font,
    mono_italic: Font,
}

impl Faces {
    fn get(&self, face: Face) -> Font {
        match face {
            Face::Regular => self.regular.clone(),
            Face::Medium => self.medium.clone(),
            Face::Semibold => self.semibold.clone(),
            Face::Mono => self.mono.clone(),
            Face::MonoItalic => self.mono_italic.clone(),
        }
    }
}

fn faces() -> Option<Faces> {
    let font = |bytes: &'static [u8]| Font::new(Data::from(bytes), 0);
    Some(Faces {
        regular: font(PLEX[0].0)?,
        medium: font(PLEX[1].0)?,
        semibold: font(PLEX[2].0)?,
        mono: font(MONO[0].0)?,
        mono_italic: font(MONO[1].0)?,
    })
}

/// The font database diagrams are read with: Plex, and nothing from the
/// machine, so a diagram is set in the face it was measured in wherever the
/// PDF is made.
fn fontdb() -> Arc<usvg::fontdb::Database> {
    static DB: OnceLock<Arc<usvg::fontdb::Database>> = OnceLock::new();
    DB.get_or_init(|| {
        let mut db = usvg::fontdb::Database::new();
        for (bytes, _) in PLEX {
            db.load_font_data(bytes.to_vec());
        }
        db.set_sans_serif_family("IBM Plex Sans");
        Arc::new(db)
    })
    .clone()
}

/// `pages` as a PDF of `document`, carrying `bundle`, and what the person
/// exporting should hear: characters no face on this machine could draw.
///
/// # Errors
///
/// When krilla cannot write it, which for pages laid out here means a face
/// would not load.
pub fn pdf(
    document: &Document,
    pages: &[Page],
    bundle: &[u8],
    writing: &Writing,
) -> Result<(Vec<u8>, Vec<String>), String> {
    let faces = faces().ok_or("the embedded faces would not load")?;
    let mut pen = Pen {
        faces,
        machine: None,
        missing: BTreeSet::new(),
    };
    let mut pdf = Pdf::new();
    let date = date(&document.created);
    let mut metadata = Metadata::new()
        .title(document.title.clone())
        .creator("deck".to_string())
        .producer(format!("deck {}", env!("CARGO_PKG_VERSION")))
        .language("en".to_string());
    if let Some(date) = date {
        metadata = metadata.creation_date(date);
    }
    pdf.set_metadata(metadata);

    for page in pages {
        let size = (page.width * page.scale, page.height * page.scale);
        let settings = PageSettings::from_wh(size.0, size.1).ok_or("a page with no size")?;
        let mut sheet = pdf.start_page_with(settings);
        let mut links = Vec::new();
        {
            let mut surface = sheet.surface();
            surface.push_transform(&Transform::from_scale(page.scale, page.scale));
            for item in &page.items {
                draw(&mut surface, &mut pen, (page.width, page.height), item);
                match item {
                    Item::Link { rect, page: to } => links.push((
                        *rect,
                        krilla::annotation::Target::Destination(Destination::Xyz(
                            XyzDestination::new(*to, Point::from_xy(0., 0.)),
                        )),
                    )),
                    Item::Url { rect, url } => links.push((
                        *rect,
                        krilla::annotation::Target::Action(krilla::action::Action::Link(
                            krilla::action::LinkAction::new(url.clone()),
                        )),
                    )),
                    _ => {}
                }
            }
            if writing.grid {
                grid(&mut surface, page);
            }
            surface.pop();
            surface.finish();
        }
        for (rect, target) in links {
            let Some(at) = krilla::geom::Rect::from_xywh(
                rect.x * page.scale,
                rect.y * page.scale,
                rect.w * page.scale,
                rect.h * page.scale,
            ) else {
                continue;
            };
            sheet.add_annotation(krilla::annotation::Annotation::new_link(
                krilla::annotation::LinkAnnotation::new(at, target),
                None,
            ));
        }
        sheet.finish();
    }

    // Bookmarks: one per group, at its first page.
    let mut outline = Outline::new();
    for (ix, page) in pages.iter().enumerate() {
        let title = match page.kind {
            Kind::Cover => Some("Contents".to_string()),
            Kind::Group { n, part: 1, .. } => document
                .groups
                .iter()
                .find(|group| group.n == n)
                .map(|group| format!("{n} · {}", group.title)),
            Kind::Argument { n, part: 1, .. } => Some(format!("{n} · the argument")),
            Kind::Said
                if !matches!(
                    pages.get(ix.wrapping_sub(1)).map(|p| p.kind),
                    Some(Kind::Said)
                ) =>
            {
                Some("What was said about the deck".to_string())
            }
            _ => None,
        };
        if let Some(title) = title {
            outline.push_child(OutlineNode::new(
                title,
                XyzDestination::new(ix, Point::from_xy(0., 0.)),
            ));
        }
    }
    pdf.set_outline(outline);

    let mut file = EmbeddedFile {
        path: writing.name.clone(),
        mime_type: MimeType::new("application/vnd.deck+zip"),
        description: Some("The deck this was exported from: open it in deck".to_string()),
        // Written out as `/AFRelationship /Source` only in PDF/A-3 and later,
        // which krilla holds to rules this file does not keep (no
        // transparency, every colour profiled). Outside it the file is still
        // attached and listed; only the relationship goes unsaid.
        association_kind: AssociationKind::Source,
        data: bundle.to_vec().into(),
        modification_date: None,
        // A `.deck` is a zip already.
        compress: Some(false),
        location: None,
    };
    if let Some(date) = date {
        file.modification_date = Some(date);
    }
    pdf.embed_file(file)
        .ok_or("the deck could not be attached")?;
    let bytes = pdf.finish().map_err(|err| format!("{err:?}"))?;
    let mut warnings = Vec::new();
    if !pen.missing.is_empty() {
        let listed: Vec<String> = pen
            .missing
            .iter()
            .map(|ch| format!("{ch} (U+{:04X})", u32::from(*ch)))
            .collect();
        warnings.push(format!(
            "no face on this machine can draw {}; they show as boxes",
            listed.join(", ")
        ));
    }
    Ok((bytes, warnings))
}

/// The packing day and time, `2026-10-20T09:30:00Z`, as a PDF date.
fn date(created: &str) -> Option<DateTime> {
    let number = |at: std::ops::Range<usize>| created.get(at)?.parse::<u16>().ok();
    let byte = |at: std::ops::Range<usize>| u8::try_from(number(at)?).ok();
    Some(
        DateTime::new(number(0..4)?)
            .month(byte(5..7)?)
            .day(byte(8..10)?)
            .hour(byte(11..13).unwrap_or(0))
            .minute(byte(14..16).unwrap_or(0))
            .second(byte(17..19).unwrap_or(0))
            .utc_offset_hour(0)
            .utc_offset_minute(0),
    )
}

fn colour(rgb: Rgb) -> rgb::Color {
    rgb::Color::new(rgb.r, rgb.g, rgb.b)
}

fn fill_of(rgb: Rgb) -> Fill {
    Fill {
        paint: colour(rgb).into(),
        opacity: NormalizedF32::ONE,
        rule: krilla::paint::FillRule::default(),
    }
}

fn stroke_of(rgb: Rgb, width: f32) -> Stroke {
    Stroke {
        paint: colour(rgb).into(),
        width,
        ..Stroke::default()
    }
}

fn rect_path(rect: Rect) -> Option<krilla::geom::Path> {
    let mut path = PathBuilder::new();
    path.push_rect(krilla::geom::Rect::from_xywh(
        rect.x, rect.y, rect.w, rect.h,
    )?);
    path.finish()
}

/// A circle, as four cubic arcs.
fn circle(cx: f32, cy: f32, r: f32) -> Option<krilla::geom::Path> {
    const K: f32 = 0.552_284_8;
    let k = r * K;
    let mut path = PathBuilder::new();
    path.move_to(cx + r, cy);
    path.cubic_to(cx + r, cy + k, cx + k, cy + r, cx, cy + r);
    path.cubic_to(cx - k, cy + r, cx - r, cy + k, cx - r, cy);
    path.cubic_to(cx - r, cy - k, cx - k, cy - r, cx, cy - r);
    path.cubic_to(cx + k, cy - r, cx + r, cy - k, cx + r, cy);
    path.close();
    path.finish()
}

fn draw(surface: &mut Surface, pen: &mut Pen, size: (f32, f32), item: &Item) {
    match item {
        Item::Fill { rect, colour } | Item::Chip { rect, colour } => {
            if let Some(path) = rect_path(*rect) {
                surface.set_fill(Some(fill_of(*colour)));
                surface.set_stroke(None);
                surface.draw_path(&path);
            }
        }
        Item::Frame { rect, colour } => {
            if let Some(path) = rect_path(*rect) {
                surface.set_fill(None);
                surface.set_stroke(Some(stroke_of(*colour, 0.75)));
                surface.draw_path(&path);
            }
        }
        Item::Rule { x, y, w, colour } => {
            let mut path = PathBuilder::new();
            path.move_to(*x, *y);
            path.line_to(x + w, *y);
            if let Some(path) = path.finish() {
                surface.set_fill(None);
                surface.set_stroke(Some(stroke_of(*colour, 0.5)));
                surface.draw_path(&path);
            }
        }
        Item::Text {
            x,
            baseline,
            text,
            face,
            size,
            colour,
        } => {
            surface.set_stroke(None);
            surface.set_fill(Some(fill_of(*colour)));
            write(
                surface,
                pen,
                *face,
                *size,
                Point::from_xy(*x, *baseline),
                text,
            );
        }
        Item::Badge {
            cx,
            cy,
            r,
            numeral,
            fill,
            ink,
        } => {
            if let Some(path) = circle(*cx, *cy, *r) {
                surface.set_stroke(None);
                surface.set_fill(Some(fill_of(*fill)));
                surface.draw_path(&path);
            }
            let size = r * 1.25;
            let words = numeral.to_string();
            let width = super::layout::measure(&words, Face::Semibold, size);
            surface.set_fill(Some(fill_of(*ink)));
            write(
                surface,
                pen,
                Face::Semibold,
                size,
                Point::from_xy(cx - width / 2., cy + size * 0.36),
                &words,
            );
        }
        Item::Dot {
            cx,
            cy,
            r,
            colour,
            filled,
        } => {
            if let Some(path) = circle(*cx, *cy, *r) {
                if *filled {
                    surface.set_stroke(None);
                    surface.set_fill(Some(fill_of(*colour)));
                } else {
                    surface.set_fill(None);
                    surface.set_stroke(Some(stroke_of(*colour, 1.)));
                }
                surface.draw_path(&path);
            }
        }
        Item::Svg {
            rect,
            svg,
            scale,
            band,
        } => {
            // Plex alone, unless the drawing has words Plex cannot set.
            let fontdb = if pen.diagram_needs_machine(svg) {
                pen.machine().db.clone()
            } else {
                fontdb()
            };
            let options = usvg::Options {
                fontdb,
                ..usvg::Options::default()
            };
            let Ok(tree) = usvg::Tree::from_str(svg, &options) else {
                return;
            };
            let (w, h) = (tree.size().width() * scale, tree.size().height() * scale);
            let Some(size) = Size::from_wh(w, h) else {
                return;
            };
            // Centred, or from the left edge when wider than its room; a
            // band of a tall drawing is moved up so the band is in view.
            let x = rect.x + ((rect.w - w) / 2.).max(0.);
            let y = match band {
                Some([from, _]) => rect.y - from * scale,
                None => rect.y + ((rect.h - h) / 2.).max(0.),
            };
            let Some(clip) = rect_path(*rect) else {
                return;
            };
            surface.push_clip_path(&clip, &krilla::paint::FillRule::NonZero);
            surface.push_transform(&Transform::from_translate(x, y));
            surface.draw_svg(
                &tree,
                size,
                SvgSettings {
                    embed_text: true,
                    ..SvgSettings::default()
                },
            );
            surface.pop();
            surface.pop();
        }
        Item::Dots {
            rect,
            colour,
            pitch,
        } => {
            surface.set_stroke(None);
            surface.set_fill(Some(fill_of(*colour)));
            let mut y = rect.y + pitch / 2.;
            while y < rect.y + rect.h {
                let mut x = rect.x + pitch / 2.;
                while x < rect.x + rect.w {
                    if let Some(path) = circle(x, y, 0.6) {
                        surface.draw_path(&path);
                    }
                    x += pitch;
                }
                y += pitch;
            }
        }
        Item::Ground { colour } => {
            let rect = Rect::new(0., 0., size.0, size.1);
            if let Some(path) = rect_path(rect) {
                surface.set_stroke(None);
                surface.set_fill(Some(fill_of(*colour)));
                surface.draw_path(&path);
            }
        }
        Item::Link { .. } | Item::Url { .. } => {}
    }
}

/// The 4-point grid over the page, and every box's outline, for a person
/// checking the spacing.
fn grid(surface: &mut Surface, page: &Page) {
    let faint = Rgb::new(64, 160, 255);
    let step = 4. / page.scale;
    surface.set_fill(None);
    surface.set_stroke(Some(Stroke {
        opacity: NormalizedF32::new(0.18).unwrap_or(NormalizedF32::ONE),
        ..stroke_of(faint, 0.25)
    }));
    let mut path = PathBuilder::new();
    let mut x = 0.;
    while x <= page.width {
        path.move_to(x, 0.);
        path.line_to(x, page.height);
        x += step;
    }
    let mut y = 0.;
    while y <= page.height {
        path.move_to(0., y);
        path.line_to(page.width, y);
        y += step;
    }
    if let Some(path) = path.finish() {
        surface.draw_path(&path);
    }
    let edge = Rgb::new(255, 64, 160);
    surface.set_stroke(Some(Stroke {
        opacity: NormalizedF32::new(0.7).unwrap_or(NormalizedF32::ONE),
        ..stroke_of(edge, 0.4)
    }));
    for item in &page.items {
        if let Item::Fill { rect, .. }
        | Item::Frame { rect, .. }
        | Item::Svg { rect, .. }
        | Item::Dots { rect, .. } = item
            && let Some(path) = rect_path(*rect)
        {
            surface.draw_path(&path);
        }
    }
}
