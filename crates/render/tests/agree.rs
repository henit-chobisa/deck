//! deck's line breaks and cuts against the window's own, on the platform's
//! real text system.
//!
//! The window lays a label out with `WindowTextSystem::shape_text` and breaks
//! it there; it cuts a note with its text element's `truncate()`, which keeps
//! the note whole when the shaped note fits and otherwise asks the line
//! wrapper. Both are run here, beside deck's [`boundaries`] and [`truncate`].
//!
//! Without the test harness, so `main` runs on the main thread: the Mac
//! platform, and the CoreText text system that comes with it, can only be made
//! there. A test context would not do — its text system measures nothing. On
//! other platforms this checks nothing and says so.

#[cfg(target_os = "macos")]
fn main() {
    use deck_render::text::{CORPUS, PLEX, Weight, boundaries, covers, truncate};
    use gpui_kit::{
        FontWeight, TextRun, TextSystem, TruncateFrom, WindowTextSystem, font, hsla, px,
    };

    let platform = gpui_kit::platform::current_platform(true);
    let system = std::sync::Arc::new(TextSystem::new(platform.text_system()));
    system
        .add_fonts(
            PLEX.iter()
                .map(|(bytes, _)| std::borrow::Cow::Borrowed(*bytes))
                .collect(),
        )
        .expect("Plex loads");
    let window = WindowTextSystem::new(system.clone());

    let (mut checked, mut wrong, mut fallback, mut ties) = (0, Vec::new(), 0, Vec::new());
    for (weight, gpui_weight) in [
        (Weight::Regular, FontWeight::NORMAL),
        (Weight::Medium, FontWeight::MEDIUM),
        (Weight::Semibold, FontWeight::SEMIBOLD),
    ] {
        for size in [10., 10.5, 12.] {
            let mut face = font("IBM Plex Sans");
            face.weight = gpui_weight;
            let mut wrapper = system.line_wrapper(face.clone(), px(size));
            for label in CORPUS {
                let run = TextRun {
                    len: label.len(),
                    font: face.clone(),
                    color: hsla(0., 0., 0., 1.),
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                };
                for width in [40., 60., 80., 100., 116., 136., 172.] {
                    // How the window breaks a label in a box this wide.
                    let lines = window
                        .shape_text(
                            (*label).to_string().into(),
                            px(size),
                            std::slice::from_ref(&run),
                            Some(px(width)),
                            None,
                        )
                        .expect("shapes");
                    let line = &lines[0];
                    let broken: Vec<usize> = line
                        .wrap_boundaries
                        .iter()
                        .map(|b| line.unwrapped_layout.runs[b.run_ix].glyphs[b.glyph_ix].index)
                        .collect();
                    // How its text element cuts a note this wide.
                    let whole = window
                        .shape_text(
                            (*label).to_string().into(),
                            px(size),
                            std::slice::from_ref(&run),
                            None,
                            None,
                        )
                        .expect("shapes");
                    let cut: String = if whole.iter().all(|l| l.unwrapped_layout.width <= px(width))
                    {
                        (*label).to_string()
                    } else {
                        wrapper
                            .truncate_line((*label).into(), px(width), "…", &[], TruncateFrom::End)
                            .0
                            .to_string()
                    };

                    let ours = boundaries(label, size, weight, width);
                    let ours_cut = truncate(label, size, weight, width);
                    let differs = ours != broken || ours_cut != cut;
                    // Characters Plex cannot draw are measured in a fallback
                    // face deck cannot see. Counted, not asserted.
                    if !covers(label) {
                        fallback += usize::from(differs);
                        continue;
                    }
                    // A line within a thousandth of a pixel of the box is a
                    // tie: CoreText keeps a few more bits than f32 can carry
                    // back out, and the last one decides it. Reported, not
                    // asserted, when nudging the box by that much agrees.
                    let tie = |nudge: f32| boundaries(label, size, weight, width + nudge) == broken;
                    if ours != broken && (tie(-0.001) || tie(0.001)) {
                        ties.push(format!("{label:?} at {width}px, {size}px {weight:?}"));
                    } else if ours != broken {
                        wrong.push(format!("{label:?} at {width}px, {size}px {weight:?}: deck breaks {ours:?}, window {broken:?}"));
                    }
                    if ours_cut != cut {
                        wrong.push(format!("{label:?} cut at {width}px, {size}px {weight:?}: deck {ours_cut:?}, window {cut:?}"));
                    }
                    checked += 1;
                }
            }
        }
    }
    assert!(checked > 1000, "only {checked} cases");
    assert!(
        wrong.is_empty(),
        "{} differ:\n{}",
        wrong.len(),
        wrong.join("\n")
    );
    println!(
        "deck breaks and cuts {checked} labels where the window does; {} sit on the edge of their box to a thousandth of a pixel and round the other way ({}); {fallback} more, in characters Plex alone does not draw, differ as expected",
        ties.len(),
        ties.join("; ")
    );
    assert!(
        ties.len() <= 2,
        "{} ties is more than rounding explains",
        ties.len()
    );
}

#[cfg(not(target_os = "macos"))]
fn main() {
    println!("the agreement check runs on macOS, where its text system is the window's");
}
