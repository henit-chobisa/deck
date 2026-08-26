//! The deck model.
//!
//! This crate is the part of deck that has no opinion about screens. It knows
//! what a deck is, where a comment has drifted to, how many columns a grid
//! should have, and which colours follow from a background. It does not know
//! what a window is, it does not read files, and it never draws.
//!
//! That is deliberate, and it is the one rule to keep: **the model is shared,
//! the views are not.** The native window and, later, the web renderer each
//! write their own presentation against these types. The moment layout maths
//! knows what a pane widget is, the second client cannot use it and the work is
//! being done twice.
//!
//! # Layout of the crate
//!
//! - [`line`] — line coordinates, and the single sanctioned exit from 1-based
//!   counting.
//! - [`protocol`] — the wire format, mirroring the JSON field for field.
//! - [`relocate`] — following a pinned line while the file moves under it.
//! - [`layout`] — how many panes fit beside each other.
//! - [`diagram`] — pictures of things that are not in any one file.
//! - [`theme`] — the palette, derived from the editor's own colours.

#![forbid(unsafe_code)]

pub mod config;
pub mod diagram;
pub mod layout;
pub mod line;
pub mod protocol;
pub mod relocate;
pub mod theme;

// The names a caller reaches for constantly are re-exported here, so the
// internal file layout can change without moving anyone's imports.
pub use diagram::{Diagram, Role, Weight};
pub use layout::{Arrange, Grid, Layout};
pub use line::LineRange;
pub use protocol::{
    Comment, DiagramRef, Group, Header, Kind, Ref, RefSpec, Review, Source, VERSION,
};
pub use relocate::{Relocated, relocate};
pub use theme::{Imported, Palette, Rgb, derive};
