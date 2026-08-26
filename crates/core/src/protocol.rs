//! The wire format: what an agent writes to disk, and what deck writes back.
//!
//! These types mirror the JSON exactly, field for field, and there is nothing
//! behind them. That is the point: every field means one thing, so there is no
//! second shape to convert into and no combination of fields to arbitrate
//! between.
//!
//! Unknown fields are accepted on purpose. A newer agent writing a field this
//! build has never heard of should still get its deck rendered, minus the part
//! we cannot show. A *missing* field is different — a ref with no `range` names
//! no location, and the parse fails rather than inventing one.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::diagram::Diagram;
use crate::line::LineRange;

/// The protocol version every document carries in its `v` field.
pub const VERSION: u32 = 1;

/// A deck header: the `deck.json` that appears with the directory.
///
/// The header alone is enough to open the window. Groups arrive after it, which
/// is the entire reason a deck is a directory instead of a file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Header {
    /// Protocol version. See [`VERSION`].
    pub v: u32,
    /// The deck's identity, which is also its directory and review filename.
    pub id: String,
    /// The project this deck was written against.
    ///
    /// `None` means unscoped: any project may render it. When set, a deck
    /// belongs to one checkout, so two projects cannot steal each other's
    /// decks.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<PathBuf>,
    /// The question or claim the deck answers. The first thing read.
    ///
    /// A heading, in sentence case: *The review path drops a comment*, not
    /// *review path bug*. It is read cold — on the bar that says a deck is
    /// ready, before anything else about it is on screen — so it has to make
    /// sense with no context in front of it. A client capitalises the first
    /// letter for display, because models are inconsistent about it and a
    /// heading that starts lower-case reads as a fragment; the rest is left
    /// alone, since a title may name code.
    pub title: String,
    /// How many groups the agent intends to write.
    ///
    /// Counts are shown against this, not against what has landed, so a deck
    /// does not appear to grow while it is being read. `None` for a deck that
    /// arrived whole and has nothing to stream.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<u32>,
}

/// One thing the agent wants to say, and the code that shows it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Group {
    /// Identifies the group within its deck, as `g1`, `g2`.
    pub id: String,
    /// Position in the story, counting from 1.
    ///
    /// Groups are applied in this order and held back when one is missing: the
    /// prose of group 3 assumes group 2 has been read.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ord: Option<u32>,
    /// The narration, as Markdown. May be long; the band scrolls.
    pub say: String,
    /// The evidence for this one claim. One pane each.
    #[serde(default)]
    pub refs: Vec<Ref>,
}

/// One pane's worth of evidence.
///
/// Code and a diagram are genuinely different kinds of thing, not one thing
/// with optional halves — a diagram has no file and a file has no nodes. An
/// enum says that, and leaves no shape for a ref that is somehow both.
///
/// On the wire they are told apart by what they carry: an entry with a `file`
/// is code, one with a `diagram` is a picture.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Ref {
    /// Lines of a real file.
    Code(RefSpec),
    /// A picture of something that is not in any one file.
    Diagram(DiagramRef),
}

impl Ref {
    /// Identifies the ref within its deck, whichever kind it is.
    #[must_use]
    pub fn id(&self) -> &str {
        match self {
            Self::Code(code) => &code.id,
            Self::Diagram(diagram) => &diagram.id,
        }
    }
}

/// A diagram, given a pane of its own.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiagramRef {
    /// Identifies the ref within its deck, as `g1d1`.
    pub id: String,
    /// What to draw. See [`crate::diagram`].
    pub diagram: Diagram,
    /// A short label for the band, as a code ref's `note` is.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// A pointer at code.
///
/// A ref names lines outright. There was once a second way — quote the text and
/// let deck find it — and it was dropped, because an agent writes a deck about
/// work it has just done, which means it has already read the file and the line
/// numbers are in front of it. The search was paying for a file read that had
/// already happened, and its `node` variant would have pulled a tree-sitter
/// grammar for every supported language into this crate to answer a question
/// the agent could already answer itself.
///
/// A range that has gone stale is not this type's problem. It is the same
/// problem as a comment that has gone stale, and [`relocate`](crate::relocate)
/// answers both.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefSpec {
    /// Identifies the ref within its deck, as `g1r1`.
    pub id: String,
    /// The file to open, relative to the deck's `cwd` or absolute.
    pub file: PathBuf,
    /// The lines to light up.
    ///
    /// Tight ranges only. A two hundred line range is not a highlight, it is a
    /// shrug.
    pub range: LineRange,
    /// A short label for the pane and the band.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// A proposed replacement for the range.
    ///
    /// When set, the ref is a change that has not been made: the range reads as
    /// a deletion and this text as the replacement. The file on disk is never
    /// touched.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
}

/// The review, written back when the reader submits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Review {
    /// Protocol version. See [`VERSION`].
    pub v: u32,
    /// The deck being answered, by id.
    pub deck: String,
    /// Every comment, in the order they were written.
    pub comments: Vec<Comment>,
}

/// One remark.
///
/// A comment is about *something*, and that something is either a line of code
/// or the group's claim itself. The second kind matters more than its size
/// here suggests: "this whole approach is wrong" is the most valuable thing a
/// reader can say, and with only line-pinned comments it had nowhere to go —
/// it had to be pinned to some arbitrary line and hope the agent understood
/// the remark was not about that line.
///
/// So `ref` and `range` are absent together for a remark about the group.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Comment {
    /// The group the comment was made in.
    pub group: String,
    /// The ref the comment was made on, or `None` for the group itself.
    ///
    /// `ref` is a Rust keyword, so the field is spelled out and renamed for the
    /// wire.
    #[serde(rename = "ref", skip_serializing_if = "Option::is_none")]
    pub ref_id: Option<String>,
    /// The file, repeated here so a comment is legible without the deck.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<PathBuf>,
    /// Where the comment sits now, after any edits made while reading, or
    /// `None` when the remark is about the group rather than a line.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<LineRange>,
    /// How much to trust `range`. See [`Source`].
    ///
    /// `None` means the client did not say, which is not the same as any of the
    /// four answers: search for `quote` rather than assume.
    pub source: Option<Source>,
    /// What kind of response the comment is asking for. See [`Kind`].
    #[serde(default)]
    pub kind: Kind,
    /// The text the comment was pinned to when it was written.
    ///
    /// The far side relocates by searching for this when `range` is stale. For
    /// a remark about the group it is the sentence being answered, or empty.
    #[serde(default)]
    pub quote: String,
    /// What the reader actually said.
    pub text: String,
}

/// How a comment's range was arrived at, and so how far to trust it.
///
/// Without this the agent cannot tell an exact range from a good guess, and
/// treats both as fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase", from = "String")]
pub enum Source {
    /// The editor tracked the range through every edit. Exact.
    Extmark,
    /// The range was replayed through a diff of the file. Reliable.
    Diff,
    /// The range was found again by matching surrounding lines. A good guess.
    Fingerprint,
    /// The range could not be confirmed. Search for `quote` instead.
    ///
    /// Also where an unrecognised value lands: a client that says something
    /// this version cannot read has said nothing it can be trusted on.
    #[default]
    Stale,
}

/// Read from the wire, forgivingly.
///
/// Anything this version does not know reads as the default rather than
/// failing. A later version may name a kind or a source this one has never
/// heard of, and a comment that arrives slightly flattened is worth more than a
/// review that will not parse.
impl From<String> for Source {
    fn from(name: String) -> Self {
        match name.as_str() {
            "extmark" => Self::Extmark,
            "diff" => Self::Diff,
            "fingerprint" => Self::Fingerprint,
            _ => Self::Stale,
        }
    }
}

/// What the reader wants done about a comment.
///
/// Terse remarks read as neutral, and an agent left to infer tone from prose
/// gets it wrong. Naming it costs the reader one keystroke.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", from = "String")]
pub enum Kind {
    /// Change this before going further.
    MustFix,
    /// Answer this. The deck did not carry the model far enough.
    #[default]
    Question,
    /// Worth saying, not worth blocking on.
    Nit,
}

/// Read from the wire, forgivingly.
///
/// Anything this version does not know reads as the default rather than
/// failing. A later version may name a kind or a source this one has never
/// heard of, and a comment that arrives slightly flattened is worth more than a
/// review that will not parse.
impl From<String> for Kind {
    fn from(name: String) -> Self {
        match name.as_str() {
            "must-fix" => Self::MustFix,
            "nit" => Self::Nit,
            _ => Self::Question,
        }
    }
}
