//! deck's line breaks against GPUI's own wrapper, on the platform's real text
//! system.
//!
//! Without the test harness, so `main` runs on the main thread: the Mac
//! platform, and the CoreText text system that comes with it, can only be made
//! there. A test context would not do — its text system measures nothing.
//! On other platforms this checks nothing and says so.

#[cfg(target_os = "macos")]
fn main() {
    use deck_render::text::{CORPUS, PLEX, Weight, boundaries, covers, truncate};
    use gpui_kit::{FontWeight, LineFragment, TextSystem, TruncateFrom, font, px};

    let platform = gpui_kit::platform::current_platform(true);
    let system = std::sync::Arc::new(TextSystem::new(platform.text_system()));
    system
        .add_fonts(
            PLEX.iter()
                .map(|(bytes, _)| std::borrow::Cow::Borrowed(*bytes))
                .collect(),
        )
        .expect("Plex loads");
    let (mut checked, mut wrong, mut fallback) = (0, Vec::new(), 0);
    for (weight, gpui_weight) in [
        (Weight::Regular, FontWeight::NORMAL),
        (Weight::Medium, FontWeight::MEDIUM),
        (Weight::Semibold, FontWeight::SEMIBOLD),
    ] {
        for size in [10., 10.5, 12.] {
            let mut face = font("IBM Plex Sans");
            face.weight = gpui_weight;
            let mut theirs = system.line_wrapper(face, px(size));
            for label in CORPUS {
                for width in [40., 60., 80., 100., 116., 136., 172.] {
                    // A note is cut short rather than wrapped.
                    if covers(label) {
                        let (cut, _) = theirs.truncate_line(
                            (*label).into(),
                            px(width),
                            "…",
                            &[],
                            TruncateFrom::End,
                        );
                        let ours = truncate(label, size, weight, width);
                        if ours != cut.as_ref() {
                            wrong.push(format!(
                                "{label:?} cut at {width}px, {size}px {weight:?}: deck {ours:?}, window {cut:?}"
                            ));
                        }
                    }
                    let fragments = [LineFragment::text(label)];
                    let window: Vec<(usize, u32)> = theirs
                        .wrap_line(&fragments, px(width))
                        .map(|b| (b.ix, b.next_indent))
                        .collect();
                    let ours = boundaries(label, size, weight, width);
                    // Characters Plex cannot draw are measured in a fallback
                    // face deck cannot see. Counted, not asserted.
                    if !covers(label) {
                        fallback += usize::from(ours != window);
                        continue;
                    }
                    if ours != window {
                        wrong.push(format!(
                            "{label:?} at {width}px, {size}px {weight:?}: deck {ours:?}, window {window:?}"
                        ));
                    }
                    checked += 1;
                }
            }
        }
    }
    assert!(checked > 800, "only {checked} cases");
    assert!(
        wrong.is_empty(),
        "{} of {checked} break differently:\n{}",
        wrong.len(),
        wrong.join("\n")
    );
    println!(
        "deck breaks and cuts {checked} lines where the window does; {fallback} more, in characters Plex has no glyph for, differ as expected"
    );
}

#[cfg(not(target_os = "macos"))]
fn main() {
    println!("the agreement check runs on macOS, where its text system is the window's");
}
