//! An embedded face cut down to the characters a document uses, as a font a
//! browser will load.
//!
//! Five faces travel with every export — three cuts of Plex, two of JetBrains
//! Mono — and whole, they are over a megabyte before base64 makes them a
//! third larger again: more than the deck they carry, for the few hundred
//! characters a deck is written in. So each is subset to those characters.
//!
//! `subsetter` does the cutting: it keeps every glyph a kept glyph is built
//! from, and renumbers them. It is written for PDF, where the document
//! carries its own character map, so what it hands back has no `cmap` and no
//! `OS/2` — and a browser refuses a font without either, silently, and sets
//! the page in whatever it has instead. Those two are put back here: a `cmap`
//! written fresh for the renumbered glyphs, and the original `OS/2`, which
//! holds the face's metrics and weight and names no glyph. Its first and last
//! character fields go on describing the whole face, which nothing reads.
//!
//! What does not come back is `GPOS` and `GSUB`. Plex loses its kerning, a
//! fraction of a pixel between some pairs; JetBrains Mono loses its
//! ligatures, which code in a review should not have anyway, since `!=` drawn
//! as one glyph is not what the file says.

use std::collections::BTreeSet;

use rustybuzz::ttf_parser;

/// `font` cut down to `chars`, as a TrueType font with a character map, or
/// the whole face when it cannot be cut.
///
/// The whole face is the answer to anything going wrong, because a document
/// set in the wrong font is worse than a larger one.
pub fn subset(font: &[u8], chars: &BTreeSet<char>) -> Vec<u8> {
    cut(font, chars).unwrap_or_else(|| font.to_vec())
}

fn cut(font: &[u8], chars: &BTreeSet<char>) -> Option<Vec<u8>> {
    let face = ttf_parser::Face::parse(font, 0).ok()?;
    let mut remapper = subsetter::GlyphRemapper::new();
    let mut map: Vec<(u32, u16)> = Vec::new();
    for &ch in chars {
        if let Some(glyph) = face.glyph_index(ch) {
            map.push((ch as u32, remapper.remap(glyph.0)));
        }
    }
    let cut = subsetter::subset(font, 0, &remapper).ok()?;
    let os2 = face
        .raw_face()
        .table(ttf_parser::Tag::from_bytes(b"OS/2"))?;

    let mut tables = tables_of(&cut)?;
    tables.retain(|(tag, _)| tag != b"cmap" && tag != b"OS/2");
    tables.push((*b"cmap", cmap(&map)));
    tables.push((*b"OS/2", os2.to_vec()));
    Some(sfnt(&cut[..4], tables))
}

/// Every table in an sfnt, by tag, as bytes.
fn tables_of(font: &[u8]) -> Option<Vec<([u8; 4], Vec<u8>)>> {
    let be16 = |at: usize| Some(u16::from_be_bytes(font.get(at..at + 2)?.try_into().ok()?));
    let be32 = |at: usize| Some(u32::from_be_bytes(font.get(at..at + 4)?.try_into().ok()?));
    let count = usize::from(be16(4)?);
    let mut tables = Vec::with_capacity(count + 2);
    for ix in 0..count {
        let record = 12 + ix * 16;
        let tag: [u8; 4] = font.get(record..record + 4)?.try_into().ok()?;
        let offset = usize::try_from(be32(record + 8)?).ok()?;
        let length = usize::try_from(be32(record + 12)?).ok()?;
        tables.push((tag, font.get(offset..offset + length)?.to_vec()));
    }
    Some(tables)
}

/// A character map for `map`, sorted by character: a format 4 subtable for
/// the characters in the Basic Multilingual Plane, which every reader of
/// fonts understands, and a format 12 for all of them, which is the only one
/// that can hold an emoji.
fn cmap(map: &[(u32, u16)]) -> Vec<u8> {
    // Runs of characters whose glyphs follow on from each other, as
    // (first, last, first glyph): one segment each, rather than one per
    // character.
    let mut runs: Vec<(u32, u32, u16)> = Vec::new();
    for &(ch, glyph) in map {
        match runs.last_mut() {
            Some((first, last, start))
                if ch == *last + 1 && u32::from(glyph) == u32::from(*start) + (ch - *first) =>
            {
                *last = ch;
            }
            _ => runs.push((ch, ch, glyph)),
        }
    }

    let mut four = Vec::new();
    // A segment can only describe characters below 0x10000, and the table
    // ends with one for 0xFFFF that maps it to nothing.
    let mut segments: Vec<(u16, u16, u16)> = runs
        .iter()
        .filter_map(|&(first, last, glyph)| {
            let first = u16::try_from(first).ok().filter(|&first| first < 0xFFFF)?;
            let last = u16::try_from(last.min(0xFFFE)).ok()?;
            Some((first, last, glyph.wrapping_sub(first)))
        })
        .collect();
    // A format 4 table is at most 64K long, which is eight thousand
    // segments: far more than a deck is written in, but cut there rather
    // than written wrong. The format 12 table still has them all.
    segments.truncate(8000);
    segments.push((0xFFFF, 0xFFFF, 1));
    let count = u16::try_from(segments.len()).unwrap_or(u16::MAX);
    // The binary-search fields: the largest power of two that fits, twice.
    let floor = 15 - count.leading_zeros();
    let search = 2u16 << floor;
    let length = 16 + 8 * count;
    for word in [
        4,
        length,
        0,
        count * 2,
        search,
        u16::try_from(floor).unwrap_or(0),
        (count * 2).saturating_sub(search),
    ] {
        four.extend(word.to_be_bytes());
    }
    for &(_, last, _) in &segments {
        four.extend(last.to_be_bytes());
    }
    four.extend(0u16.to_be_bytes());
    for &(first, _, _) in &segments {
        four.extend(first.to_be_bytes());
    }
    for &(_, _, delta) in &segments {
        four.extend(delta.to_be_bytes());
    }
    for _ in &segments {
        four.extend(0u16.to_be_bytes());
    }

    let mut twelve = Vec::new();
    let groups = u32::try_from(runs.len()).unwrap_or(u32::MAX);
    twelve.extend(12u16.to_be_bytes());
    twelve.extend(0u16.to_be_bytes());
    twelve.extend((16 + 12 * groups).to_be_bytes());
    twelve.extend(0u32.to_be_bytes());
    twelve.extend(groups.to_be_bytes());
    for &(first, last, glyph) in &runs {
        twelve.extend(first.to_be_bytes());
        twelve.extend(last.to_be_bytes());
        twelve.extend(u32::from(glyph).to_be_bytes());
    }

    // The header and two records — Windows Unicode BMP, then full — with
    // their subtables after.
    let mut out = Vec::new();
    out.extend(0u16.to_be_bytes());
    out.extend(2u16.to_be_bytes());
    let four_at = 4u32 + 2 * 8;
    let twelve_at = four_at + u32::try_from(four.len()).unwrap_or(u32::MAX);
    for (encoding, at) in [(1u16, four_at), (10, twelve_at)] {
        out.extend(3u16.to_be_bytes());
        out.extend(encoding.to_be_bytes());
        out.extend(at.to_be_bytes());
    }
    out.extend(four);
    out.extend(twelve);
    out
}

/// `tables` as one sfnt with the version `flavor`, its directory sorted, every
/// table padded to four bytes and checksummed, and the whole file's checksum
/// put in `head` as the format asks.
fn sfnt(flavor: &[u8], mut tables: Vec<([u8; 4], Vec<u8>)>) -> Vec<u8> {
    tables.sort_by_key(|(tag, _)| *tag);
    let count = u16::try_from(tables.len()).unwrap_or(u16::MAX);
    let selector = 15 - count.leading_zeros().min(15);
    let search = (1u16 << selector) * 16;
    let mut out = Vec::new();
    out.extend(flavor);
    for word in [
        count,
        search,
        u16::try_from(selector).unwrap_or(0),
        (count * 16).saturating_sub(search),
    ] {
        out.extend(word.to_be_bytes());
    }
    let mut at = 12 + 16 * tables.len();
    let mut adjust = None;
    for (tag, data) in &mut tables {
        if tag == b"head" && data.len() >= 12 {
            data[8..12].fill(0);
            adjust = Some(at + 8);
        }
        out.extend(*tag);
        out.extend(checksum(data).to_be_bytes());
        out.extend(u32::try_from(at).unwrap_or(u32::MAX).to_be_bytes());
        out.extend(u32::try_from(data.len()).unwrap_or(u32::MAX).to_be_bytes());
        at += data.len().next_multiple_of(4);
    }
    for (_, data) in &tables {
        out.extend(data);
        out.resize(out.len().next_multiple_of(4), 0);
    }
    if let Some(at) = adjust {
        let sum = 0xB1B0_AFBA_u32.wrapping_sub(checksum(&out));
        out[at..at + 4].copy_from_slice(&sum.to_be_bytes());
    }
    out
}

/// The sfnt checksum: the bytes as big-endian words, zero-padded, summed.
fn checksum(data: &[u8]) -> u32 {
    data.chunks(4).fold(0u32, |sum, chunk| {
        let mut word = [0; 4];
        word[..chunk.len()].copy_from_slice(chunk);
        sum.wrapping_add(u32::from_be_bytes(word))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::{MONO, PLEX};

    /// What a browser does with the cut face, as near as a test can ask: it
    /// parses, every character asked for has a glyph and the same advance it
    /// had whole, and a character left out has none.
    #[test]
    fn a_cut_face_still_maps_its_characters() {
        let chars: BTreeSet<char> = "pending -= 1 ⋯ — é".chars().collect();
        for (font, _) in PLEX.iter().chain(MONO) {
            let cut = subset(font, &chars);
            assert!(
                cut.len() < font.len() / 4,
                "{} of {}",
                cut.len(),
                font.len()
            );
            let whole = ttf_parser::Face::parse(font, 0).unwrap();
            let face = ttf_parser::Face::parse(&cut, 0).expect("the cut face parses");
            for &ch in &chars {
                let Some(was) = whole.glyph_index(ch) else {
                    continue;
                };
                let now = face.glyph_index(ch).unwrap_or_else(|| panic!("{ch:?}"));
                assert_eq!(
                    face.glyph_hor_advance(now),
                    whole.glyph_hor_advance(was),
                    "{ch:?}"
                );
            }
            assert!(face.glyph_index('Z').is_none());
            assert!(
                face.raw_face()
                    .table(ttf_parser::Tag::from_bytes(b"OS/2"))
                    .is_some()
            );
        }
    }

    #[test]
    fn the_file_checksums_to_the_magic_number() {
        let chars: BTreeSet<char> = "abc".chars().collect();
        let cut = subset(PLEX[0].0, &chars);
        assert_eq!(checksum(&cut), 0xB1B0_AFBA);
    }
}
