//! The four corner radii every surface in the app takes one of.
//!
//! A corner is proportional to the size of the thing it rounds: the window
//! is the roundest, a key cap the least. Nothing paints a radius of its own;
//! it names a step here, so the whole app curves in one family.

/// The deck's own window corner.
///
/// Near enough to what macOS gives a window of this kind that the two agree,
/// and the window is transparent behind it so a disagreement shows as a
/// slightly rounder corner rather than as a square one.
///
/// On Windows it is Windows 11's own 8, because there the platform draws a
/// rounded shadow around the window too, and a deck rounder than its shadow
/// showed a sliver of it at every corner (see `win::fit_frame`).
pub const WINDOW: f32 = if cfg!(target_os = "windows") { 8. } else { 14. };

/// Cards: sheets, popovers, dock cards, key cards, the export sheet, the help
/// dock, and the question pills with their arrows. On Windows, where the
/// window itself is 8, the whole scale steps down one so nothing inside the
/// window is rounder than the window.
pub const CARD: f32 = if cfg!(target_os = "windows") { 8. } else { 10. };

/// Controls: buttons, review rows, chips and other small interactive
/// surfaces.
pub const CONTROL: f32 = if cfg!(target_os = "windows") { 6. } else { 7. };

/// Caps: key caps and other tiny inline marks, such as inline code chips and
/// keycap-like tags.
pub const CAP: f32 = if cfg!(target_os = "windows") { 4. } else { 5. };
