//! Faces from the machine, for the characters deck's own faces cannot set.
//!
//! Plex and JetBrains Mono set Latin, Greek, Cyrillic and code. A deck about
//! code written in Japanese, a remark with an emoji, an Arabic string in a
//! test: those characters come from a face the machine has, found here once
//! and then asked for by both halves of a paged export — the layout, which
//! measures a run in the face it will be drawn in, and the PDF writer, which
//! draws it there — so the two agree by construction rather than by luck.
//!
//! A face is chosen for a character by a short list of families every Mac
//! (and most Linux machines with Noto) has, then by any face that has a
//! glyph for it, and among a family's faces by the weight and slant nearest
//! the face it stands in for: regular prose in a regular face, a title in a
//! semibold one. A machine without a face for a character draws it as the
//! box Plex has for what it lacks, and the export says so.
//!
//! These runs are the one part of an export that can differ between
//! machines: another machine may have another face, or none.

use std::collections::{BTreeSet, HashMap};
use std::ops::Range;
use std::sync::{Arc, Mutex, OnceLock};

use usvg::fontdb;

use super::layout::Face;
use crate::text::{MONO, PLEX};

/// Where a run of characters is drawn from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Source {
    /// The face the layout chose.
    Own,
    /// A face from the machine.
    Machine(fontdb::ID),
}

/// The families tried first, in order: the Mac's own CJK, emoji, Arabic and
/// Hebrew faces, then Noto's, then a face that holds nearly everything. Past
/// these, any face on the machine that has the character.
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

/// The machine's faces, loaded the first time a character needs one.
struct Machine {
    db: Arc<fontdb::Database>,
    chosen: HashMap<(char, u16, bool), Option<fontdb::ID>>,
    data: HashMap<fontdb::ID, (Arc<Vec<u8>>, u32)>,
}

impl Machine {
    fn load() -> Self {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        // Plex too, so a diagram read with this database is still set in it.
        for (bytes, _) in PLEX {
            db.load_font_data(bytes.to_vec());
        }
        db.set_sans_serif_family("IBM Plex Sans");
        Self {
            db: Arc::new(db),
            chosen: HashMap::new(),
            data: HashMap::new(),
        }
    }

    fn has(&self, id: fontdb::ID, ch: char) -> bool {
        self.db
            .with_face_data(id, |data, index| {
                rustybuzz::ttf_parser::Face::parse(data, index)
                    .ok()
                    .and_then(|face| face.glyph_index(ch))
                    .is_some()
            })
            .unwrap_or(false)
    }

    /// Of `ids`, the one nearest `weight` and the slant asked for.
    fn nearest(&self, ids: &[fontdb::ID], weight: u16, italic: bool) -> Option<fontdb::ID> {
        ids.iter().copied().min_by_key(|&id| {
            self.db.face(id).map_or(u32::MAX, |face| {
                let slanted = face.style != fontdb::Style::Normal;
                u32::from(face.weight.0.abs_diff(weight)) + if slanted == italic { 0 } else { 1000 }
            })
        })
    }

    fn face_for(&mut self, ch: char, weight: u16, italic: bool) -> Option<fontdb::ID> {
        if let Some(chosen) = self.chosen.get(&(ch, weight, italic)) {
            return *chosen;
        }
        let mut found = None;
        for family in FALLBACKS {
            let ids: Vec<fontdb::ID> = self
                .db
                .faces()
                .filter(|face| face.families.iter().any(|(name, _)| name == family))
                .map(|face| face.id)
                .filter(|&id| self.has(id, ch))
                .collect();
            if !ids.is_empty() {
                found = self.nearest(&ids, weight, italic);
                break;
            }
        }
        if found.is_none() {
            let ids: Vec<fontdb::ID> = self
                .db
                .faces()
                .map(|face| face.id)
                .filter(|&id| self.has(id, ch))
                .collect();
            found = self.nearest(&ids, weight, italic);
        }
        self.chosen.insert((ch, weight, italic), found);
        found
    }
}

fn machine<R>(f: impl FnOnce(&mut Machine) -> R) -> R {
    static MACHINE: OnceLock<Mutex<Machine>> = OnceLock::new();
    let lock = MACHINE.get_or_init(|| Mutex::new(Machine::load()));
    let mut machine = lock
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    f(&mut machine)
}

/// The weight and slant `face` is set at.
fn style_of(face: Face) -> (u16, bool) {
    match face {
        Face::Regular | Face::Mono => (400, false),
        Face::Medium => (500, false),
        Face::Semibold => (600, false),
        Face::MonoItalic => (400, true),
    }
}

/// The embedded face `face` is, parsed once, for shaping and coverage.
pub(super) fn own(face: Face) -> Option<&'static rustybuzz::Face<'static>> {
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
pub(super) fn rides(ch: char) -> bool {
    matches!(
        ch,
        '\u{200D}'
            | '\u{FE00}'..='\u{FE0F}'
            | '\u{1F3FB}'..='\u{1F3FF}'
            | '\u{0300}'..='\u{036F}'
            | '\u{E0020}'..='\u{E007F}'
    )
}

/// Whether `face` sets every character of `text` itself.
pub(super) fn covered(face: Face, text: &str) -> bool {
    if text.is_ascii() {
        return true;
    }
    let Some(mine) = own(face) else {
        return true;
    };
    text.chars()
        .all(|ch| ch.is_whitespace() || rides(ch) || mine.glyph_index(ch).is_some())
}

/// `text` cut into runs by where each is drawn from, and what nothing on
/// this machine can draw.
pub(super) fn runs(
    face: Face,
    text: &str,
    missing: &mut BTreeSet<char>,
) -> Vec<(Source, Range<usize>)> {
    if covered(face, text) {
        return vec![(Source::Own, 0..text.len())];
    }
    let mine = own(face);
    let (weight, italic) = style_of(face);
    let mut out: Vec<(Source, Range<usize>)> = Vec::new();
    for (at, ch) in text.char_indices() {
        let source = if ch.is_whitespace() || rides(ch) {
            out.last().map_or(Source::Own, |(source, _)| *source)
        } else if mine.is_some_and(|face| face.glyph_index(ch).is_some()) {
            Source::Own
        } else if let Some(id) = machine(|m| m.face_for(ch, weight, italic)) {
            Source::Machine(id)
        } else {
            missing.insert(ch);
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

/// The bytes of a machine face, and its index in its file.
pub(super) fn data(id: fontdb::ID) -> Option<(Arc<Vec<u8>>, u32)> {
    machine(|m| {
        if let Some(found) = m.data.get(&id) {
            return Some(found.clone());
        }
        let found =
            m.db.with_face_data(id, |data, index| (Arc::new(data.to_vec()), index))?;
        m.data.insert(id, found.clone());
        Some(found)
    })
}

/// The machine's font database, Plex in it, for diagrams with words Plex
/// cannot set.
pub(super) fn database() -> Arc<fontdb::Database> {
    machine(|m| m.db.clone())
}

/// A machine face's weight.
#[cfg(test)]
pub(super) fn weight(id: fontdb::ID) -> Option<u16> {
    machine(|m| m.db.face(id).map(|face| face.weight.0))
}

/// How wide `text` is at `size` once shaped by `shaper`, as the PDF draws it.
pub(super) fn shaped(shaper: &rustybuzz::Face, text: &str, size: f32) -> f32 {
    #[allow(clippy::cast_precision_loss)]
    let units = shaper.units_per_em() as f32;
    let mut buffer = rustybuzz::UnicodeBuffer::new();
    buffer.push_str(text);
    buffer.guess_segment_properties();
    let shaped = rustybuzz::shape(shaper, &[], buffer);
    #[allow(clippy::cast_precision_loss)]
    let advance: f32 = shaped
        .glyph_positions()
        .iter()
        .map(|at| at.x_advance as f32)
        .sum();
    advance / units * size
}

/// How wide a machine face sets `text` at `size`.
pub(super) fn machine_width(id: fontdb::ID, text: &str, size: f32) -> f32 {
    data(id)
        .and_then(|(bytes, index)| {
            rustybuzz::Face::from_slice(&bytes, index).map(|face| shaped(&face, text, size))
        })
        .unwrap_or(0.)
}
