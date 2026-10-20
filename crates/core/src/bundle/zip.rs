//! The smallest zip that a `.deck` file needs, and a reader that trusts none
//! of it.
//!
//! Every entry is stored, never compressed. A deck is JSON and the text of a
//! few files, so compression would save little, and a stored entry cannot
//! hold more than the file it arrives in: the size it declares is the size of
//! the bytes it covers, so a reader is never asked to inflate a few kilobytes
//! into gigabytes. Writing it by hand rather than through a zip library keeps
//! the output the same, byte for byte, from one build to the next, which is
//! what lets a test compare a packed deck with the one it packed last time.
//!
//! The writer emits one shape and the reader accepts only that shape. A
//! `.deck` file arrives from somebody else, so every field the reader looks at
//! is checked against the one it was paired with — the local header against
//! the central directory, the checksum against the bytes, every name against
//! the rules in [`valid_name`] — and anything it cannot account for is a
//! refusal rather than a guess.

use std::collections::HashSet;

/// The date every entry carries: 1980-01-01, the zip format's own zero, so
/// the bytes do not depend on when or where a deck was packed.
const DOS_DATE: u16 = (1 << 5) | 1;
/// Version 2.0: stored entries and UTF-8 names, nothing that needs more.
const VERSION_NEEDED: u16 = 20;
/// Made on Unix, so the permission bits below mean what they say.
const VERSION_MADE_BY: u16 = (3 << 8) | VERSION_NEEDED;
/// Bit 11: the names are UTF-8. The only flag deck sets or accepts.
const FLAG_UTF8: u16 = 1 << 11;
/// An ordinary file, readable by everyone and writable by its owner.
const MODE: u32 = 0o100_644 << 16;

const LOCAL: u32 = 0x0403_4b50;
const CENTRAL: u32 = 0x0201_4b50;
const END: u32 = 0x0605_4b50;
const LOCAL_LEN: usize = 30;
const CENTRAL_LEN: usize = 46;
const END_LEN: usize = 22;

/// How much a reader will take from one file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// The most entries.
    pub entries: usize,
    /// The most bytes, all entries together.
    pub bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            entries: 4096,
            bytes: 64 * 1024 * 1024,
        }
    }
}

/// Why a file could not be read as a deck's zip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// It ended before something it promised.
    Truncated,
    /// It is not a zip, or not one this writer could have made: a comment,
    /// several disks, zip64, or a directory that does not sit where it says.
    NotThisZip(&'static str),
    /// An entry is compressed or encrypted.
    Method(String),
    /// An entry's local header disagrees with its central directory entry.
    Mismatch(String),
    /// An entry's bytes do not match its checksum.
    Checksum(String),
    /// Two entries share a name, ignoring case.
    Duplicate(String),
    /// A name that could escape the folder it is unpacked into, or that no
    /// file system could hold. See [`valid_name`].
    Name(String),
    /// More entries or more bytes than [`Limits`] allows.
    TooBig,
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Truncated => write!(f, "the file ends early"),
            Self::NotThisZip(why) => write!(f, "not a deck's zip: {why}"),
            Self::Method(name) => write!(f, "{name} is compressed or encrypted"),
            Self::Mismatch(name) => write!(f, "{name} says two different things about itself"),
            Self::Checksum(name) => write!(f, "{name} does not match its checksum"),
            Self::Duplicate(name) => write!(f, "{name} is in the file twice"),
            Self::Name(name) => write!(f, "{name:?} is not a name a deck can hold"),
            Self::TooBig => write!(f, "the file holds more than a deck can"),
        }
    }
}

impl std::error::Error for Refusal {}

/// Whether `name` is a name an entry may have.
///
/// Relative, with forward slashes, made of lowercase letters, digits, `.`,
/// `_` and `-`, with no empty segment, no `.` or `..` segment, and short. That
/// is narrower than zip allows, and it is meant to be: every name a deck uses
/// fits, and nothing that fits can climb out of the folder it is unpacked
/// into, name a device, or collide with another entry on a file system that
/// ignores case.
#[must_use]
pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && name.split('/').all(|segment| {
            !segment.is_empty()
                && segment != "."
                && segment != ".."
                && segment.bytes().all(|b| {
                    b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b'-')
                })
        })
}

/// `entries` as a zip, in the order given, every one stored.
///
/// # Panics
///
/// If a name is not a [`valid_name`], or the zip would pass four gigabytes.
/// Both are mistakes in the caller: a deck's names are chosen by deck, and a
/// deck is far smaller.
#[must_use]
pub fn write(entries: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data) in entries {
        assert!(valid_name(name), "{name:?} is not a valid entry name");
        let offset = small(out.len());
        let crc = crc32fast::hash(data);
        let size = small(data.len());
        let name_len = u16::try_from(name.len()).expect("a name under 255 bytes");

        put32(&mut out, LOCAL);
        put16(&mut out, VERSION_NEEDED);
        put16(&mut out, FLAG_UTF8);
        put16(&mut out, 0); // stored
        put16(&mut out, 0); // time
        put16(&mut out, DOS_DATE);
        put32(&mut out, crc);
        put32(&mut out, size);
        put32(&mut out, size);
        put16(&mut out, name_len);
        put16(&mut out, 0); // no extra field
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(data);

        put32(&mut central, CENTRAL);
        put16(&mut central, VERSION_MADE_BY);
        put16(&mut central, VERSION_NEEDED);
        put16(&mut central, FLAG_UTF8);
        put16(&mut central, 0);
        put16(&mut central, 0);
        put16(&mut central, DOS_DATE);
        put32(&mut central, crc);
        put32(&mut central, size);
        put32(&mut central, size);
        put16(&mut central, name_len);
        put16(&mut central, 0); // extra
        put16(&mut central, 0); // comment
        put16(&mut central, 0); // disk
        put16(&mut central, 0); // internal attributes
        put32(&mut central, MODE);
        put32(&mut central, offset);
        central.extend_from_slice(name.as_bytes());
    }
    let count = u16::try_from(entries.len()).expect("fewer than 65536 entries");
    let central_at = small(out.len());
    let central_len = small(central.len());
    out.extend_from_slice(&central);
    put32(&mut out, END);
    put16(&mut out, 0);
    put16(&mut out, 0);
    put16(&mut out, count);
    put16(&mut out, count);
    put32(&mut out, central_len);
    put32(&mut out, central_at);
    put16(&mut out, 0); // no comment
    out
}

/// The entries of `bytes`, in the order they are stored, or why not.
///
/// # Errors
///
/// Any [`Refusal`]: the file is read only if every check passes.
pub fn read(bytes: &[u8], limits: Limits) -> Result<Vec<(String, Vec<u8>)>, Refusal> {
    // The end record is the last 22 bytes exactly: deck writes no comment,
    // so there is nothing to search backwards through.
    let end_at = bytes.len().checked_sub(END_LEN).ok_or(Refusal::Truncated)?;
    let end = &bytes[end_at..];
    if get32(end, 0) != END {
        return Err(Refusal::NotThisZip("no end record where it belongs"));
    }
    if get16(end, 4) != 0 || get16(end, 6) != 0 {
        return Err(Refusal::NotThisZip("spread over several disks"));
    }
    let count = usize::from(get16(end, 10));
    if usize::from(get16(end, 8)) != count {
        return Err(Refusal::NotThisZip("two counts of its entries"));
    }
    if count > limits.entries {
        return Err(Refusal::TooBig);
    }
    let central_len = wide(get32(end, 12));
    let central_at = wide(get32(end, 16));
    if get16(end, 20) != 0 {
        return Err(Refusal::NotThisZip("a comment"));
    }
    if central_at.checked_add(central_len) != Some(end_at) {
        return Err(Refusal::NotThisZip("a directory that is not where it says"));
    }

    let mut entries = Vec::with_capacity(count);
    let mut seen = HashSet::new();
    let mut total = 0usize;
    let mut next_local = 0usize;
    let mut at = central_at;
    for _ in 0..count {
        let header = bytes.get(at..at + CENTRAL_LEN).ok_or(Refusal::Truncated)?;
        if get32(header, 0) != CENTRAL {
            return Err(Refusal::NotThisZip("a broken directory"));
        }
        let flags = get16(header, 8);
        let method = get16(header, 10);
        let crc = get32(header, 16);
        let packed = get32(header, 20);
        let size = get32(header, 24);
        let name_len = usize::from(get16(header, 28));
        let extra_len = usize::from(get16(header, 30));
        let comment_len = usize::from(get16(header, 32));
        let disk = get16(header, 34);
        let local_at = wide(get32(header, 42));
        let name_bytes = bytes
            .get(at + CENTRAL_LEN..at + CENTRAL_LEN + name_len)
            .ok_or(Refusal::Truncated)?;
        let name = std::str::from_utf8(name_bytes)
            .map_err(|_| Refusal::Name(String::from_utf8_lossy(name_bytes).into_owned()))?
            .to_string();
        at += CENTRAL_LEN + name_len + extra_len + comment_len;
        if at > end_at {
            return Err(Refusal::Truncated);
        }

        if get16(header, 6) > VERSION_NEEDED || packed == u32::MAX || size == u32::MAX {
            return Err(Refusal::NotThisZip("zip64"));
        }
        if disk != 0 {
            return Err(Refusal::NotThisZip("spread over several disks"));
        }
        if method != 0 || flags & !FLAG_UTF8 != 0 || packed != size {
            return Err(Refusal::Method(name));
        }
        if !valid_name(&name) {
            return Err(Refusal::Name(name));
        }
        if !seen.insert(name.to_ascii_lowercase()) {
            return Err(Refusal::Duplicate(name));
        }

        // Entries are laid end to end, in directory order, from the start of
        // the file. One that starts anywhere else overlaps another or hides
        // bytes between them, and both are ways to make a small file claim to
        // hold more than it does.
        if local_at != next_local {
            return Err(Refusal::Mismatch(name));
        }
        let local = bytes
            .get(local_at..local_at + LOCAL_LEN)
            .ok_or(Refusal::Truncated)?;
        if get32(local, 0) != LOCAL
            || get16(local, 6) != flags
            || get16(local, 8) != method
            || get32(local, 14) != crc
            || get32(local, 18) != packed
            || get32(local, 22) != size
            || usize::from(get16(local, 26)) != name_len
            || get16(local, 28) != 0
            || bytes.get(local_at + LOCAL_LEN..local_at + LOCAL_LEN + name_len) != Some(name_bytes)
        {
            return Err(Refusal::Mismatch(name));
        }
        let data_at = local_at + LOCAL_LEN + name_len;
        let data_end = data_at + wide(size);
        if data_end > central_at {
            return Err(Refusal::Truncated);
        }
        total += wide(size);
        if total > limits.bytes {
            return Err(Refusal::TooBig);
        }
        let data = &bytes[data_at..data_end];
        if crc32fast::hash(data) != crc {
            return Err(Refusal::Checksum(name));
        }
        next_local = data_end;
        entries.push((name, data.to_vec()));
    }
    if at != end_at || next_local != central_at {
        return Err(Refusal::NotThisZip("bytes nothing accounts for"));
    }
    Ok(entries)
}

fn small(n: usize) -> u32 {
    u32::try_from(n).expect("a deck under four gigabytes")
}

fn wide(n: u32) -> usize {
    usize::try_from(n).expect("a usize holds a u32")
}

fn put16(out: &mut Vec<u8>, n: u16) {
    out.extend_from_slice(&n.to_le_bytes());
}

fn put32(out: &mut Vec<u8>, n: u32) {
    out.extend_from_slice(&n.to_le_bytes());
}

fn get16(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([bytes[at], bytes[at + 1]])
}

fn get32(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn two() -> Vec<(String, Vec<u8>)> {
        vec![
            ("mimetype".to_string(), b"application/vnd.deck+zip".to_vec()),
            ("deck/deck.json".to_string(), br#"{"v":1}"#.to_vec()),
        ]
    }

    fn refused(bytes: &[u8]) -> Refusal {
        read(bytes, Limits::default()).expect_err("refused")
    }

    /// The offset of the `n`th central directory header.
    fn central(bytes: &[u8], n: usize) -> usize {
        let end = bytes.len() - END_LEN;
        let mut at = wide(get32(bytes, end + 16));
        for _ in 0..n {
            at += CENTRAL_LEN + usize::from(get16(bytes, at + 28));
        }
        at
    }

    #[test]
    fn what_is_written_reads_back() {
        let entries = two();
        assert_eq!(read(&write(&entries), Limits::default()), Ok(entries));
        assert_eq!(read(&write(&[]), Limits::default()), Ok(vec![]));
    }

    #[test]
    fn the_same_entries_make_the_same_bytes() {
        assert_eq!(write(&two()), write(&two()));
    }

    #[test]
    fn the_mimetype_sits_where_a_sniffer_looks_for_it() {
        // As EPUB does it: the first entry, stored, no extra field, so its
        // name starts at byte 30 and its contents at byte 38.
        let bytes = write(&two());
        assert_eq!(&bytes[30..38], b"mimetype");
        assert_eq!(&bytes[38..62], b"application/vnd.deck+zip");
    }

    #[test]
    fn another_tool_can_list_it() {
        // `unzip -l` reads the format the way everyone else does; a file it
        // cannot list is not a zip, whatever this reader thinks.
        let Ok(unzip) = std::process::Command::new("unzip").arg("-v").output() else {
            return;
        };
        if !unzip.status.success() {
            return;
        }
        let dir = std::env::temp_dir().join(format!("deck-zip-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("a temp dir");
        let at = dir.join("t.deck");
        std::fs::write(&at, write(&two())).expect("writable");
        let listed = std::process::Command::new("unzip")
            .arg("-t")
            .arg(&at)
            .output()
            .expect("unzip runs");
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            listed.status.success(),
            "{}",
            String::from_utf8_lossy(&listed.stdout)
        );
    }

    #[test]
    fn names_that_could_escape_are_refused() {
        for bad in [
            "",
            "/etc/passwd",
            "../up",
            "deck/../../up",
            "a//b",
            "./a",
            "C:/x",
            "a\\b",
            "Deck/deck.json",
            "a\0b",
            "snap/ü.txt",
        ] {
            assert!(!valid_name(bad), "{bad:?}");
        }
        for good in ["mimetype", "deck/g12.json", "snap/3.txt", "review.json"] {
            assert!(valid_name(good), "{good:?}");
        }
    }

    #[test]
    fn a_name_that_could_escape_is_refused_on_reading() {
        let mut bytes = write(&[("deck/aa".to_string(), vec![])]);
        // Rewrite the name in both headers to `../aa`, keeping its length.
        for at in [LOCAL_LEN, central(&bytes, 0) + CENTRAL_LEN] {
            bytes[at..at + 3].copy_from_slice(b"../");
            bytes[at + 3..at + 7].copy_from_slice(b"aa\0\0");
        }
        assert!(matches!(refused(&bytes), Refusal::Name(_)));
    }

    #[test]
    fn a_name_twice_is_refused_whatever_its_case() {
        let mut bytes = write(&[
            ("deck/a".to_string(), vec![1]),
            ("deck/b".to_string(), vec![2]),
        ]);
        // Rename the second entry to the first, in both its headers.
        let local = LOCAL_LEN + "deck/a".len() + 1;
        bytes[local + LOCAL_LEN + 5] = b'a';
        let second = central(&bytes, 1);
        bytes[second + CENTRAL_LEN + 5] = b'a';
        assert!(matches!(refused(&bytes), Refusal::Duplicate(_)));
    }

    #[test]
    fn a_wrong_checksum_is_refused() {
        let mut bytes = write(&two());
        bytes[38] ^= 1;
        assert!(matches!(refused(&bytes), Refusal::Checksum(_)));
    }

    #[test]
    fn a_compressed_entry_is_refused() {
        let mut bytes = write(&two());
        let at = central(&bytes, 0);
        bytes[at + 10] = 8; // deflate
        bytes[8] = 8;
        assert!(matches!(refused(&bytes), Refusal::Method(_)));
    }

    #[test]
    fn an_encrypted_entry_is_refused() {
        let mut bytes = write(&two());
        let at = central(&bytes, 0);
        bytes[at + 8] |= 1;
        assert!(matches!(refused(&bytes), Refusal::Method(_)));
    }

    #[test]
    fn a_size_that_lies_is_refused() {
        // The directory says the first entry is a byte longer than it is.
        let mut bytes = write(&two());
        let at = central(&bytes, 0);
        for field in [20, 24] {
            let size = get32(&bytes, at + field) + 1;
            bytes[at + field..at + field + 4].copy_from_slice(&size.to_le_bytes());
        }
        assert!(matches!(refused(&bytes), Refusal::Mismatch(_)));
    }

    #[test]
    fn overlapping_entries_are_refused() {
        // Both directory entries point at the first local header: the
        // classic way to make one file's bytes count many times over.
        let mut bytes = write(&[
            ("deck/a".to_string(), vec![1; 10]),
            ("deck/b".to_string(), vec![1; 10]),
        ]);
        let second = central(&bytes, 1);
        bytes[second + 42..second + 46].copy_from_slice(&0u32.to_le_bytes());
        assert!(matches!(refused(&bytes), Refusal::Mismatch(_)));
    }

    #[test]
    fn a_comment_or_trailing_bytes_are_refused() {
        let mut bytes = write(&two());
        bytes.push(0);
        assert!(matches!(refused(&bytes), Refusal::NotThisZip(_)));
        assert_eq!(refused(&[0; 10]), Refusal::Truncated);
    }

    #[test]
    fn too_many_entries_or_bytes_are_refused() {
        let bytes = write(&two());
        let few = Limits {
            entries: 1,
            bytes: 1024,
        };
        assert_eq!(read(&bytes, few), Err(Refusal::TooBig));
        let small = Limits {
            entries: 10,
            bytes: 10,
        };
        assert_eq!(read(&bytes, small), Err(Refusal::TooBig));
    }

    #[test]
    fn nothing_short_of_a_whole_file_reads() {
        // Every prefix of a good file is refused, never read in part and
        // never a panic.
        let bytes = write(&two());
        for len in 0..bytes.len() {
            assert!(read(&bytes[..len], Limits::default()).is_err(), "{len}");
        }
    }

    #[test]
    fn no_single_flipped_byte_reads_as_something_else() {
        // A flipped byte is refused, or — where it lands somewhere nothing
        // reads, such as the time field — changes nothing. It never panics
        // and never yields different entries.
        let entries = two();
        let bytes = write(&entries);
        for at in 0..bytes.len() {
            for bit in [1u8, 0x80] {
                let mut bent = bytes.clone();
                bent[at] ^= bit;
                if let Ok(read) = read(&bent, Limits::default()) {
                    assert_eq!(read, entries, "byte {at} bit {bit}");
                }
            }
        }
    }
}
