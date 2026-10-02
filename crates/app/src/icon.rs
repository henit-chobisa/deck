//! The mark, on the Dock and in the switcher.
//!
//! deck installs as a binary on `PATH`, not as a `.app` bundle — `brew install`
//! and `install.sh` both put one Mach-O file in `bin` and stop. A bare
//! executable has no `Info.plist`, and an icon is something the platform reads
//! out of one, so there was nowhere to put the mark and macOS drew the generic
//! blank page instead.
//!
//! Bundling deck would fix it and cost more than it is worth: a `.app` is a
//! directory, it does not go in `bin`, and every install path and the upgrade
//! that is coming would have to learn about it.
//!
//! AppKit will take an icon at runtime instead. One call, no bundle, and the
//! binary stays a binary.

/// Wear it.
///
/// Quiet about failure on purpose: a window with the wrong icon is a window,
/// and refusing to open a deck over it would be the worse trade.
pub fn wear_the_mark() {
    mac::wear_the_mark();
}

#[cfg(target_os = "macos")]
mod mac {
    use objc2::{AnyThread, MainThreadMarker};
    use objc2_app_kit::{NSApplication, NSImage};
    use objc2_foundation::NSData;

    /// The mark, rendered from `assets/mark-dark.svg` at the size the Dock
    /// asks for, with the margin macOS icons are drawn inside — without it the
    /// squircle sits visibly larger than everything beside it.
    const MARK: &[u8] = include_bytes!("../../../assets/icon.png");

    pub fn wear_the_mark() {
        // AppKit, so the main thread or nothing. Called from the app's own
        // startup, which is already there.
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        let data = NSData::with_bytes(MARK);
        let Some(mark) = NSImage::initWithData(NSImage::alloc(), &data) else {
            return;
        };
        let app = NSApplication::sharedApplication(mtm);
        unsafe { app.setApplicationIconImage(Some(&mark)) };
    }
}

#[cfg(not(target_os = "macos"))]
mod mac {
    /// Nothing to do: every other platform reads an icon from a file beside the
    /// binary or from a desktop entry, and neither is ours to write from here.
    pub fn wear_the_mark() {}
}
