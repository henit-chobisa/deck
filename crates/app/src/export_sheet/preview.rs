//! A laid-out page, painted in the window.
//!
//! Not a picture of the PDF. The export lays a deck out once, as pages of
//! placed boxes ([`deck_render::export::layout`]), and the PDF writer only
//! draws what that hands back; this draws the same boxes with GPUI, scaled
//! to the room it is given. So the page in the sheet is the page in the
//! file, box for box, and it changes the moment an option does, with no file
//! written and no PDF read back.
//!
//! Words are set in the faces the PDF embeds — Plex for the narration,
//! JetBrains Mono for code — shaped by the platform rather than by the PDF's
//! own shaper, so a line can come out a hair wider or narrower than on
//! paper; where it starts, and what it says, are the layout's. Diagrams are
//! the SVG the page carries, drawn through GPUI's own SVG support at twice
//! their size and scaled down, placed and cut exactly as the PDF places and
//! cuts them.

use std::collections::HashMap;
use std::sync::Arc;

use deck_core::theme::Rgb;
use deck_render::export::{Face, Item, Page};
use gpui_kit::*;

use crate::palette::paint;

/// The family JetBrains Mono goes by once it is loaded.
const MONO: &str = "JetBrains Mono";
/// And Plex.
const SANS: &str = "IBM Plex Sans";

/// The faces the PDF embeds, loaded into the window once so the preview is
/// set in them too. The window's own code faces are chosen from what the
/// machine has installed; a page is set in the one deck carries.
pub fn embed_faces(cx: &App) {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let faces = deck_render::text::MONO
            .iter()
            .map(|(bytes, _)| std::borrow::Cow::Borrowed(*bytes))
            .collect();
        if let Err(err) = cx.text_system().add_fonts(faces) {
            // The preview in another mono face is a worse likeness of the
            // page, not a wrong one.
            eprintln!("deck: could not load the code font for the preview: {err}");
        }
    });
}

/// Each diagram on a set of pages, drawn once, by the drawing it is.
pub type Drawings = HashMap<u64, Arc<RenderImage>>;

/// What a diagram is known by in [`Drawings`].
#[must_use]
pub fn key(svg: &str) -> u64 {
    use std::hash::{Hash as _, Hasher as _};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    svg.hash(&mut hasher);
    hasher.finish()
}

/// How far a drawing is drawn past its own size, so a page shown larger
/// than its points still has sharp lines.
const OVERSAMPLE: f32 = 2.;

/// Every diagram on `pages`, drawn, that `known` does not have yet.
pub fn draw_diagrams(pages: &[Page], known: &Drawings, renderer: &SvgRenderer) -> Drawings {
    let mut out = Drawings::new();
    for page in pages {
        for item in &page.items {
            let Item::Svg { svg, .. } = item else {
                continue;
            };
            let at = key(svg);
            if known.contains_key(&at) || out.contains_key(&at) {
                continue;
            }
            if let Ok(image) = renderer.render_single_frame(svg.as_bytes(), OVERSAMPLE) {
                out.insert(at, image);
            }
        }
    }
    out
}

/// A drawing's own width and height, from its root element: what the PDF
/// writer reads as the tree's size.
fn own_size(svg: &str) -> Option<(f32, f32)> {
    let head = &svg[..svg.find('>')?];
    let number = |name: &str| -> Option<f32> {
        let at = head.find(&format!(" {name}=\""))? + name.len() + 3;
        let rest = &head[at..];
        rest[..rest.find('"')?].trim_end_matches("px").parse().ok()
    };
    Some((number("width")?, number("height")?))
}

/// `page`, painted to fill as much of the canvas as its shape allows,
/// centred in it.
pub fn page(page: Arc<Page>, drawings: Arc<Drawings>) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, (), window, cx| {
            let room = bounds.size;
            let k = (f32::from(room.width) / page.width).min(f32::from(room.height) / page.height);
            if k <= 0. {
                return;
            }
            let origin = point(
                bounds.origin.x + (room.width - px(page.width * k)) / 2.,
                bounds.origin.y + (room.height - px(page.height * k)) / 2.,
            );
            paint_page(&page, &drawings, origin, k, window, cx);
        },
    )
    .size_full()
}

/// Every item of `page`, in the order it is drawn, at `k` pixels a unit
/// from `origin`.
fn paint_page(
    page: &Page,
    drawings: &Drawings,
    origin: Point<Pixels>,
    k: f32,
    window: &mut Window,
    cx: &mut App,
) {
    let at = |x: f32, y: f32| point(origin.x + px(x * k), origin.y + px(y * k));
    let rect = |r: deck_render::export::Rect| Bounds {
        origin: at(r.x, r.y),
        size: size(px(r.w * k), px(r.h * k)),
    };
    let whole = Bounds {
        origin,
        size: size(px(page.width * k), px(page.height * k)),
    };
    // A hairline is never thinner than a pixel can show.
    let hair = |w: f32| px((w * k).max(0.5));
    window.with_content_mask(Some(ContentMask { bounds: whole }), |window| {
        for item in &page.items {
            match item {
                Item::Ground { colour } => window.paint_quad(fill(whole, paint(*colour))),
                Item::Fill { rect: r, colour } | Item::Chip { rect: r, colour } => {
                    window.paint_quad(fill(rect(*r), paint(*colour)));
                }
                Item::Frame { rect: r, colour } => {
                    window.paint_quad(outline(rect(*r), paint(*colour), BorderStyle::Solid))
                }
                Item::Rule { x, y, w, colour } => window.paint_quad(fill(
                    Bounds {
                        origin: at(*x, *y - 0.25),
                        size: size(px(w * k), hair(0.5)),
                    },
                    paint(*colour),
                )),
                Item::Text {
                    x,
                    baseline,
                    text,
                    face,
                    size: points,
                    colour,
                } => words(
                    text,
                    *face,
                    points * k,
                    *colour,
                    at(*x, *baseline),
                    window,
                    cx,
                ),
                Item::Badge {
                    cx: x,
                    cy: y,
                    r,
                    numeral,
                    fill: disc,
                    ink,
                } => {
                    window.paint_quad(circle(at(*x, *y), r * k, paint(*disc), None));
                    let points = r * 1.25;
                    let numeral = numeral.to_string();
                    let width = deck_render::text::shaped_width(
                        &numeral,
                        points,
                        deck_render::text::Weight::Semibold,
                    );
                    words(
                        &numeral,
                        Face::Semibold,
                        points * k,
                        *ink,
                        at(x - width / 2., y + points * 0.36),
                        window,
                        cx,
                    );
                }
                Item::Dot {
                    cx: x,
                    cy: y,
                    r,
                    colour,
                    filled,
                } => {
                    let ink = paint(*colour);
                    window.paint_quad(if *filled {
                        circle(at(*x, *y), r * k, ink, None)
                    } else {
                        circle(
                            at(*x, *y),
                            r * k,
                            transparent_black(),
                            Some((hair(1.), ink)),
                        )
                    });
                }
                Item::Svg {
                    rect: r,
                    svg,
                    scale,
                    band,
                } => {
                    let (Some(image), Some((w, h))) = (drawings.get(&key(svg)), own_size(svg))
                    else {
                        continue;
                    };
                    let (w, h) = (w * scale, h * scale);
                    // As the PDF places it: centred, or from the left edge
                    // when wider than its room, and a band of a tall drawing
                    // moved up so the band is what shows.
                    let x = r.x + ((r.w - w) / 2.).max(0.);
                    let y = match band {
                        Some([from, _]) => r.y - from * scale,
                        None => r.y + ((r.h - h) / 2.).max(0.),
                    };
                    let drawn = Bounds {
                        origin: at(x, y),
                        size: size(px(w * k), px(h * k)),
                    };
                    let _ = window.paint_image(
                        rect(*r),
                        drawn,
                        Corners::default(),
                        image.clone(),
                        0,
                        false,
                    );
                }
                Item::Dots {
                    rect: r,
                    colour,
                    pitch,
                } => {
                    // Too fine to see at a thumbnail's size, and thousands of
                    // quads to paint nothing.
                    if pitch * k < 3. {
                        continue;
                    }
                    let ink = paint(*colour);
                    let mut y = r.y + pitch / 2.;
                    while y < r.y + r.h {
                        let mut x = r.x + pitch / 2.;
                        while x < r.x + r.w {
                            window.paint_quad(circle(at(x, y), (0.6 * k).max(0.5), ink, None));
                            x += pitch;
                        }
                        y += pitch;
                    }
                }
                Item::Url { .. } | Item::Link { .. } => {}
            }
        }
    });
}

/// A disc at `centre`, `r` pixels across from it, filled, with an edge.
fn circle(centre: Point<Pixels>, r: f32, ink: Hsla, edge: Option<(Pixels, Hsla)>) -> PaintQuad {
    let bounds = Bounds {
        origin: point(centre.x - px(r), centre.y - px(r)),
        size: size(px(2. * r), px(2. * r)),
    };
    let (width, colour) = edge.unwrap_or((px(0.), transparent_black()));
    quad(
        bounds,
        Corners::all(px(r)),
        ink,
        Edges::all(width),
        colour,
        BorderStyle::Solid,
    )
}

/// One run of `text`, its baseline at `baseline`'s height from its x.
fn words(
    text: &str,
    face: Face,
    size: f32,
    colour: Rgb,
    baseline: Point<Pixels>,
    window: &mut Window,
    cx: &mut App,
) {
    if text.is_empty() || size < 0.5 {
        return;
    }
    let (family, weight, style) = match face {
        Face::Regular => (SANS, FontWeight::NORMAL, FontStyle::Normal),
        Face::Medium => (SANS, FontWeight::MEDIUM, FontStyle::Normal),
        Face::Semibold => (SANS, FontWeight::SEMIBOLD, FontStyle::Normal),
        Face::Mono => (MONO, FontWeight::NORMAL, FontStyle::Normal),
        Face::MonoItalic => (MONO, FontWeight::NORMAL, FontStyle::Italic),
    };
    let mut font = font(family);
    font.weight = weight;
    font.style = style;
    // Code without ligatures, as the PDF sets it: `!=` is two characters
    // in the file, and in a review of the file.
    if matches!(face, Face::Mono | Face::MonoItalic) {
        font.features = FontFeatures::disable_ligatures();
    }
    let run = TextRun {
        len: text.len(),
        font,
        color: paint(colour),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    let line = window
        .text_system()
        .shape_line(text.to_string().into(), px(size), &[run], None);
    let ascent = line.ascent;
    let height = line.ascent + line.descent;
    let _ = line.paint(
        point(baseline.x, baseline.y - ascent),
        height,
        TextAlign::Left,
        None,
        window,
        cx,
    );
}
