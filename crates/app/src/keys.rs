//! The keys, as the reader meets them: three in the strip, every one in the
//! help dock that `?` brings up, and a card over each saying what it does and
//! when to reach for it.
//!
//! They used to stand in a column beside the narration. Ten keys tall, that
//! column set the band's height whatever the prose needed, and a short
//! narration sat over a strip of empty band.

use std::time::Duration;

use deck_core::protocol::{Confidence, Level};
use deck_core::theme::Palette;
use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;

use crate::palette::paint;

/// How long the pointer rests on a key before its card comes up, and how long
/// the card stays once the pointer has left. The question pills' pace, so the
/// two kinds of hover in the window behave as one.
pub(crate) const REST: Duration = Duration::from_millis(150);
pub(crate) const LEAVE: Duration = Duration::from_millis(150);

/// How long a key's card takes to come in, and to go. A card is a surface, and
/// surfaces come in at 200 and go at 160 wherever they are in the window.
pub(crate) const TIP_RISE: Duration = Duration::from_millis(200);
pub(crate) const TIP_FALL: Duration = Duration::from_millis(160);

/// How long the help dock takes to come up, and to go. The dock is a surface
/// like the card, so it keeps the card's pace rather than a slower one of its
/// own.
pub(crate) const DOCK_RISE: Duration = Duration::from_millis(200);
pub(crate) const DOCK_FALL: Duration = Duration::from_millis(160);

/// How long the help dock stays once nobody is looking at it. Never while the
/// pointer is on it: it is help that stays beside you while you try, and it
/// goes only after the reader has looked away for this long.
pub(crate) const DOCK_STAYS: Duration = Duration::from_secs(15);

/// The key that brings the dock up, and the name its card goes by.
pub(crate) const HELP: &str = "?";
/// The dock's close button, which has a card like a key.
pub(crate) const CLOSE: &str = "×";

/// What each key does, and when to reach for it.
///
/// Written from the reader's side: the moment they are in, not the mechanism.
pub(crate) const TIPS: &[(&str, &str, &str, &str)] = &[
    (
        "n",
        "next",
        "Moves to the next group of the deck.",
        "you have read this step and want the next part of the answer.",
    ),
    (
        "p",
        "previous",
        "Goes back one group.",
        "something here leans on a step you want to see again.",
    ),
    (
        "w",
        "walk",
        "The agent reads the deck aloud and lights the code as it talks.",
        "you would rather listen and watch than read.",
    ),
    (
        "c",
        "comment",
        "Writes a comment on what you picked: lines of code, or words in the narration.",
        "you disagree, have a question, or want something changed.",
    ),
    (
        "t",
        "turn",
        "Turns the layout a quarter: side by side becomes stacked, and back.",
        "the panes feel cramped, or a long file needs the full width.",
    ),
    (
        "⎋",
        "clear",
        "Puts out the lights you made: picked lines and sentences.",
        "the window is lit up and you want to see what the agent is pointing at.",
    ),
    (
        "f",
        "follow",
        "Hands the deck back to the agent, so it moves you along as it shows things.",
        "you stopped the agent by scrolling or picking, and want to follow it again.",
    ),
    (
        "z",
        "lights",
        "Dims everything on screen except the deck.",
        "you want to read without the rest of your desktop pulling at you.",
    ),
    (
        "h",
        "hide",
        "Puts the deck away to the bar; nothing is lost.",
        "you need your screen for a moment and will come back.",
    ),
    (
        "s",
        "submit",
        "Sends your comments to the agent as one review.",
        "you have said everything you want to say about this deck.",
    ),
    (
        "q",
        "close",
        "Closes the deck. A comment you were writing is kept as a draft.",
        "you are done with this deck for now.",
    ),
    (
        HELP,
        "keys",
        "Brings up every key in a dock over the code.",
        "you forgot a key, or want to try them out.",
    ),
    (
        CLOSE,
        "close the dock",
        "Puts the keys away. They also go on their own, 15 seconds after you look away.",
        "you have what you needed.",
    ),
];

/// The card's entry for a key.
pub(crate) fn tip(
    key: &str,
) -> Option<&'static (&'static str, &'static str, &'static str, &'static str)> {
    TIPS.iter().find(|(k, ..)| *k == key)
}

/// The key a keystroke lights on the dock, if it is one of deck's.
pub(crate) fn pressed(stroke: &Keystroke) -> Option<&'static str> {
    if stroke.key == "escape" {
        return Some("⎋");
    }
    let typed = stroke.key_char.as_deref().unwrap_or(&stroke.key);
    TIPS.iter()
        .map(|(key, ..)| *key)
        .find(|key| *key == typed && *key != CLOSE)
}

/// A key drawn as one: a cap with a thicker bottom edge, the way a key catches
/// light.
pub(crate) fn cap(key: &str, palette: &Palette, mono: SharedString) -> Div {
    div()
        .min_w(px(18.))
        .flex_none()
        .px(px(4.))
        .py(px(3.))
        .text_center()
        .rounded(px(4.))
        .border_1()
        .border_b_2()
        .border_color(paint(palette.edge))
        .bg(paint(palette.wash))
        .font_family(mono)
        .text_size(px(10.))
        .line_height(px(10.))
        .text_color(paint(palette.fg))
        .child(key.to_string())
}

/// The card over a key: its cap and name, what it does, and when to use it.
///
/// `level` is how far in it has come, nought to one: it fades and rises the
/// last few points into place.
pub(crate) fn card(
    key: &str,
    level: f32,
    palette: &Palette,
    sans: SharedString,
    mono: SharedString,
) -> Option<Div> {
    let (key, name, does, when) = tip(key)?;
    Some(
        div()
            .w(px(260.))
            // Its own type, whatever it is laid over: the strip it rises from
            // is set in mono, on one line, and the card took both.
            .font_family(sans)
            .whitespace_normal()
            .mb(px(10. + 4. * (1. - level)))
            .opacity(level)
            .px(px(13.))
            .pt(px(11.))
            .pb(px(12.))
            .rounded(px(12.))
            .bg(paint(palette.band.mix(palette.fg, 0.05)))
            .border_1()
            .border_color(paint(palette.edge.mix(palette.fg, 0.06)))
            .shadow_lg()
            .text_size(px(12.))
            .line_height(px(18.))
            .text_color(paint(palette.fg.mix(palette.band, 0.22)))
            .child(
                div()
                    .h_flex()
                    .items_center()
                    .gap(px(8.))
                    .mb(px(5.))
                    .when(*key != CLOSE, |this| this.child(cap(key, palette, mono)))
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_size(px(12.5))
                            .text_color(paint(palette.fg))
                            .child(*name),
                    ),
            )
            .child(div().child(*does))
            .child(
                div()
                    .mt(px(7.))
                    .pt(px(7.))
                    .border_t_1()
                    .border_color(paint(palette.edge))
                    .text_color(paint(palette.muted))
                    .child({
                        // One run of text, so the words wrap as a sentence;
                        // the lead is coloured rather than set apart.
                        const LEAD: &str = "Use it when ";
                        StyledText::new(format!("{LEAD}{when}")).with_highlights([(
                            0..LEAD.len(),
                            HighlightStyle {
                                color: Some(paint(palette.accent.mix(palette.fg, 0.25))),
                                font_weight: Some(FontWeight::SEMIBOLD),
                                ..HighlightStyle::default()
                            },
                        )])
                    }),
            ),
    )
}

/// The confidence on a group's claim answers to the pointer like a key, with
/// a card of its own.
pub(crate) const CONFIDENCE: &str = "confidence";

/// A level's colour: the palette's own green and red at the ends, the accent
/// leaning toward the text between them.
pub(crate) fn level_colour(level: Level, palette: &Palette) -> deck_core::theme::Rgb {
    match level {
        Level::High => palette.add,
        Level::Medium => palette.accent.mix(palette.fg, 0.3),
        Level::Low => palette.del,
    }
}

/// The level's name and what it means, said the way the skill says it.
pub(crate) fn level_words(level: Level) -> (&'static str, &'static str) {
    match level {
        Level::High => (
            "High",
            "The agent watched it behave: ran it, reproduced it, ran a test that covers it, or read a trace from the real system, with nothing to mark it down.",
        ),
        Level::Medium => (
            "Medium",
            "The agent read every line this rests on without running it, or watched it behave with one thing marking it down.",
        ),
        Level::Low => (
            "Low",
            "The agent is inferring, from names, part of the code, or a pattern elsewhere, or its evidence was marked down to here.",
        ),
    }
}

/// Three bars, filled to the level, like signal strength.
pub(crate) fn bars(level: Level, palette: &Palette) -> Div {
    let filled = match level {
        Level::High => 3,
        Level::Medium => 2,
        Level::Low => 1,
    };
    let on = paint(level_colour(level, palette));
    let off = paint(palette.edge.mix(palette.fg, 0.08));
    div()
        .flex()
        .items_end()
        .gap(px(2.))
        .h(px(11.))
        .children((1..=3).map(|n| {
            div()
                .w(px(3.))
                .h(px(2. + 3. * n as f32))
                .rounded(px(1.))
                .bg(if n <= filled { on } else { off })
        }))
}

/// The card over a group's confidence: the level, what it means, and the
/// agent's reason when there is one.
pub(crate) fn confidence_card(
    confidence: &Confidence,
    level: f32,
    palette: &Palette,
    sans: SharedString,
) -> Div {
    let (name, means) = level_words(confidence.level);
    let colour = paint(level_colour(confidence.level, palette));
    div()
        .w(px(270.))
        .font_family(sans)
        .whitespace_normal()
        .mt(px(10. - 4. * (1. - level)))
        .opacity(level)
        .px(px(13.))
        .pt(px(11.))
        .pb(px(12.))
        .rounded(px(12.))
        .bg(paint(palette.band.mix(palette.fg, 0.05)))
        .border_1()
        .border_color(paint(palette.edge.mix(palette.fg, 0.06)))
        .shadow_lg()
        .text_size(px(12.))
        .line_height(px(18.))
        .text_color(paint(palette.fg.mix(palette.band, 0.22)))
        .child(
            div()
                .h_flex()
                .items_center()
                .gap(px(8.))
                .mb(px(5.))
                .child(bars(confidence.level, palette))
                .child(
                    div()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_size(px(12.5))
                        .text_color(paint(palette.fg))
                        .child(format!("{name} confidence")),
                ),
        )
        .child(div().child(means))
        .when_some(confidence.why.clone(), |this, why| {
            this.child(
                div()
                    .mt(px(7.))
                    .pt(px(7.))
                    .border_t_1()
                    .border_color(paint(palette.edge))
                    .text_color(paint(palette.muted))
                    .child({
                        const LEAD: &str = "Basis ";
                        StyledText::new(format!("{LEAD}{why}")).with_highlights([(
                            0..LEAD.len(),
                            HighlightStyle {
                                color: Some(colour),
                                font_weight: Some(FontWeight::SEMIBOLD),
                                ..HighlightStyle::default()
                            },
                        )])
                    }),
            )
        })
}
