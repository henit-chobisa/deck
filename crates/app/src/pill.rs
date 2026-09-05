//! The knock before the door opens.
//!
//! A deck used to arrive as a window across the middle of the screen, whether
//! or not the reader had a hand free. That is the wrong shape for the thing:
//! the agent finishes when it finishes, and being interrupted by a review is
//! not the same as being ready to give one.
//!
//! So the agent gets a knock, not a door. A small bar says a deck is ready and
//! what it is about; the reader opens it when they are ready, and nothing takes
//! the screen or the keyboard until they do.
//!
//! It is the same window in spirit as the one it opens — the deck's own
//! palette, its own face, the same breathing dot the footer uses while a deck
//! is still being written — so that opening it feels like the same thing
//! getting bigger rather than a second application starting.

use std::time::Duration;

use deck_core::theme::Palette;
use gpui_kit::component::{ActiveTheme as _, Icon, IconName, StyledExt as _};
use gpui_kit::*;

use crate::palette::{current, paint};

gpui_kit::actions!(deck, [OpenDeck, Later, NextDeck]);

/// The bar's own keys.
///
/// It can take them: a non-activating panel becomes key without its
/// application becoming active, so the bar is reachable from the keyboard
/// without the reader's editor losing the menu bar.
#[must_use]
pub fn bindings() -> Vec<KeyBinding> {
    vec![
        KeyBinding::new("enter", OpenDeck, Some("Pill")),
        KeyBinding::new("o", OpenDeck, Some("Pill")),
        KeyBinding::new("escape", Later, Some("Pill")),
        KeyBinding::new("q", Later, Some("Pill")),
        // The same key the window uses to walk groups, doing the same job one
        // level up.
        KeyBinding::new("n", NextDeck, Some("Pill")),
        KeyBinding::new("tab", NextDeck, Some("Pill")),
    ]
}

/// How big the window holding the bar is.
///
/// Exactly the bar, with nothing to spare.
///
/// A window larger than what is painted in it paints the rest itself — on this
/// platform as a rounded panel with its own hairline and its own shadow, a
/// bezel around the bar. It is only drawn while the window is *opaque*, which
/// it is until told otherwise, and it is told otherwise in its own builder now.
/// But a window the size of the bar cannot draw one at all, which is a better
/// guarantee than a correctly ordered call.
///
/// The cost is that the bar is as wide as this and no wider, so a long title is
/// cut off at the end of it. That is the trade: a bar that grew to fit its
/// title could not stay centred, because a window cannot be moved after it
/// opens — GPUI offers no way — and a bar that wanders along the bottom of the
/// screen as titles change is worse than one that shortens a heading.
pub const WINDOW: Size<Pixels> = Size {
    width: px(NARROW),
    height: px(BAR_H),
};
/// How wide the bar is while it is only saying whose it is and what it is
/// about.
const NARROW: f32 = 292.;
/// And how wide once everything is on it.
pub const WIDE: f32 = 560.;
/// How long the bar holds at its narrow width before opening out, in seconds.
///
/// Long enough to read six words, which is what is on it.
const HOLD: f32 = 0.5;
/// How long it takes to open out.
const GROW: f32 = 0.55;
/// How far the bar floats above the bottom of the screen.
pub const ABOVE: f32 = 68.;
/// How tall the bar itself is.
const BAR_H: f32 = 48.;

/// The bar that says a deck is ready.
pub struct Pill {
    /// Which of the decks waiting the bar is showing, and would open.
    ///
    /// The decks themselves are not here: they are in [`crate::queue`], because
    /// they outlive this window and belong to it no more than they belong to
    /// the deck window. The bar is a view onto the queue.
    at: usize,
    palette: Palette,
    focus: FocusHandle,
    /// When the bar appeared, so the opening can be timed from it.
    ///
    /// Hand-rolled rather than an [`Animation`], because what is being animated
    /// is the *window*, and GPUI's animator is handed an element. The window is
    /// only reachable from `render`, so the clock is here and the frames are
    /// asked for by hand.
    opened: std::time::Instant,
    /// Whether the bar has finished opening out.
    ///
    /// The reveal resizes the window from `render`, and every later repaint — a
    /// hover, a group landing — runs `render` too. Without this the bar would
    /// ask the platform to resize itself to the size it already is, forever,
    /// once per frame.
    settled: bool,
    /// The loop watching the deck fill, while there is more coming.
    tailing: Task<()>,
}

impl Pill {
    /// Build the bar over whatever is waiting.
    pub fn new(showing: usize, cx: &mut Context<Self>) -> Self {
        // The deck's own theme, not a dark bar for everyone.
        //
        // It was dark whichever way the deck was themed, on the reasoning that
        // this floats over someone else's screen. That was the wrong reading:
        // a bar in a palette the window it opens does not share is a second
        // application announcing itself. This is the same product at a smaller
        // size, and it should look like it.
        let dark = cx.theme().mode.is_dark();
        let palette = current(dark);
        crate::palette::install(&palette, dark, cx);

        let mut pill = Self {
            at: showing,
            palette,
            focus: cx.focus_handle(),
            opened: std::time::Instant::now(),
            settled: false,
            tailing: Task::ready(()),
        };
        pill.tail(cx);
        pill
    }

    /// Keep the count honest while the agent is still writing.
    ///
    /// The same poll the window does, for the same reason and at the same
    /// rate: a bar that said *2 of 4* until it was clicked would be lying for
    /// as long as the reader left it alone.
    fn tail(&mut self, cx: &mut Context<Self>) {
        const EVERY: Duration = Duration::from_millis(250);

        // Only while something is still being written. A queue of finished
        // decks polls nothing.
        let (_, writing) = crate::queue::refresh(cx);
        if !writing {
            self.tailing = Task::ready(());
            return;
        }
        self.tailing = cx.spawn(async move |pill, cx| {
            loop {
                cx.background_executor().timer(EVERY).await;
                let more = pill.update(cx, |_, cx| {
                    let (changed, writing) = crate::queue::refresh(cx);
                    if changed {
                        cx.notify();
                    }
                    writing
                });
                if !matches!(more, Ok(true)) {
                    return;
                }
            }
        });
    }

    /// Open the deck the bar is showing.
    ///
    /// The bar always goes, whether or not anything is left waiting. One window
    /// at a time: a bar floating over the deck it just opened is the
    /// interruption the bar exists to avoid. What is left is still in the
    /// queue, and comes back the moment the deck is put away.
    ///
    /// The deck's window opens *before* this one goes, because closing the last
    /// window ends the command — taking the bar away first left a moment with
    /// no window at all, which quit the program on its way to opening a deck.
    fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(session) = crate::queue::take(self.at, cx) else {
            return;
        };
        crate::open_deck(session, cx);
        window.remove_window();
    }

    /// Say which deck to show when the bar comes back.
    ///
    /// The one that just arrived: it is what the reader is thinking about, and
    /// this is how a deck they put away comes back to them.
    pub fn show_last(&mut self, cx: &mut Context<Self>) {
        self.at = crate::queue::len(cx).saturating_sub(1);
        // A deck arriving while the bar is up may still be being written, and
        // the poll stops when everything on the queue is sealed.
        self.tail(cx);
        cx.notify();
    }

    fn on_open(&mut self, _: &OpenDeck, window: &mut Window, cx: &mut Context<Self>) {
        self.open(window, cx);
    }

    /// Put this one away for good. The rest keep waiting.
    fn on_later(&mut self, _: &Later, window: &mut Window, cx: &mut Context<Self>) {
        let _ = crate::queue::take(self.at, cx);
        if crate::queue::len(cx) == 0 {
            window.remove_window();
        } else {
            self.at = self.at.min(crate::queue::len(cx) - 1);
            cx.notify();
        }
    }

    /// Walk to the next deck waiting, wrapping.
    ///
    /// Wrapping, unlike walking a deck's groups: a queue has no story in it and
    /// no beginning to have lost your place in, so three decks that stopped
    /// dead at the third would need a second key to get back.
    fn on_walk(&mut self, _: &NextDeck, _window: &mut Window, cx: &mut Context<Self>) {
        let pending = crate::queue::len(cx);
        if pending > 0 {
            self.at = (self.at + 1) % pending;
            cx.notify();
        }
    }

    /// Where the reader is up to, in as few words as it takes.
    fn count(&self, cx: &App) -> String {
        let Some(said) = crate::queue::about(self.at, cx, |session| {
            (
                session.deck.title(),
                session.deck.groups().len(),
                session.deck.header.total,
                session.deck.sealed(),
                session.said(),
            )
        }) else {
            return String::new();
        };
        let (_, here, claimed, sealed, remarks) = said;
        let total = claimed.map_or(here, |total| (total as usize).max(here));

        // What it is doing, said the way the reader would ask it. A count on
        // its own answers a question nobody has: they want to know whether
        // there is anything to read yet, and whether it is finished.
        let where_at = if sealed {
            format!("{total} group{}", if total == 1 { "" } else { "s" })
        } else {
            format!("Getting deck ready… {here}/{total}")
        };

        // What has already been said, when there is any. A bar that came back
        // from a deck someone was halfway through has to say so, or putting
        // the window away looks like losing the work in it.
        match remarks {
            0 => where_at,
            1 => format!("{where_at} · 1 comment"),
            said => format!("{where_at} · {said} comments"),
        }
    }
}

/// An entrance that waits its turn.
///
/// Nothing for the first `after` of the run and then the whole of it, eased
/// out. GPUI's animations all start when the element first draws, so a delay
/// has to live in the curve — which is also where it belongs: the bar is one
/// movement with parts that arrive in order, not four animations that happen
/// to overlap.
fn arrive(after: f32) -> impl Fn(f32) -> f32 {
    move |turn| {
        let turn = ((turn - after) / (1. - after)).clamp(0., 1.);
        1. - (1. - turn).powi(5)
    }
}

impl Render for Pill {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.focus.focus(window, cx);

        // The bar arrives saying whose it is and what it is about, holds long
        // enough to be read, and then opens out — the state and the button
        // arriving with the room to put them in.
        //
        // The window itself is what widens, and the bar fills it. It has to be
        // the window: a bar narrower than its window leaves the rest of the
        // window showing, and what shows is a rounded panel of the window's own
        // ground with the bar sitting inside it.
        let age = self.opened.elapsed().as_secs_f32();
        let out = {
            let turn = ((age - HOLD) / GROW).clamp(0., 1.);
            1. - (1. - turn).powi(5)
        };
        if !self.settled {
            window.resize(size(px(NARROW + (WIDE - NARROW) * out), px(BAR_H)));
            if out < 1. {
                window.request_animation_frame();
            } else {
                self.settled = true;
            }
        }
        // The tail fades in behind the widening rather than with it, so the
        // words appear in room that is already there.
        let tail = (((age - HOLD - 0.16) / 0.42).clamp(0., 1.)).powf(0.7);

        let palette = self.palette;
        let writing =
            crate::queue::about(self.at, cx, |session| !session.deck.sealed()).unwrap_or(false);
        let title =
            crate::queue::about(self.at, cx, |session| session.deck.title()).unwrap_or_default();
        let pending = crate::queue::len(cx);

        // Everything inside the bar arrives after it does, in reading order.
        // Generic over what it is styling: some of these are plain elements
        // and some carry an id, which is a different type.
        fn after<E: Styled + AnimationExt + 'static>(
            id: &'static str,
            wait: f32,
            element: E,
        ) -> AnimationElement<E> {
            element.with_animation(
                id,
                Animation::new(Duration::from_millis(760)).with_easing(arrive(wait)),
                |this, here| this.opacity(here),
            )
        }

        // Whose bar this is, before whatever it is about.
        //
        // The title is the agent's own words — the question the deck answers —
        // and on its own it says nothing about where it came from. A line of
        // prose appearing at the bottom of somebody's screen has to name
        // itself before it says anything else.
        let mark = div()
            .flex_none()
            .size(px(18.))
            .v_flex()
            .items_center()
            .justify_center()
            .gap(px(3.))
            .rounded(px(5.))
            .bg(paint(palette.wash))
            .border_1()
            .border_color(paint(palette.edge))
            .child(
                div()
                    .w(px(8.))
                    .h(px(2.))
                    .rounded_full()
                    .bg(paint(palette.muted.mix(palette.wash, 0.4))),
            )
            // The lit line sweeps out as the bar arrives, and then — only
            // while the agent is still writing — keeps moving, a little.
            //
            // The mark is the one thing on the bar that is deck rather than
            // this deck, so it is the right place for a sign of life. It is
            // width, not opacity: the accent already fades on the status dot,
            // and two things fading in time with each other read as one.
            .child(
                div()
                    .h(px(2.))
                    .rounded_full()
                    .bg(paint(palette.accent))
                    .with_animations(
                        "pill-mark",
                        if writing {
                            vec![
                                Animation::new(Duration::from_millis(760))
                                    .with_easing(arrive(0.22)),
                                Animation::new(Duration::from_millis(2400))
                                    .repeat()
                                    .with_easing(pulsating_between(0.42, 1.0)),
                            ]
                        } else {
                            vec![
                                Animation::new(Duration::from_millis(760))
                                    .with_easing(arrive(0.22)),
                            ]
                        },
                        |this, _, out| this.w(px(8. * out)),
                    ),
            );

        let brand = div()
            .flex_none()
            .h_flex()
            .items_center()
            .gap(px(7.))
            .child(mark)
            .child(
                div()
                    .font_family(cx.theme().mono_font_family.clone())
                    .text_size(px(11.5))
                    .text_color(paint(palette.fg))
                    .child("deck"),
            )
            // A rule, not a dot. The bar is one line of text and a dot in the
            // middle of it reads as punctuation belonging to the words.
            .child(
                div()
                    .w(px(1.))
                    .h(px(15.))
                    .ml(px(4.))
                    .bg(paint(palette.edge)),
            );

        // The title at one end, where it is read, and where the deck has got
        // to at the other, next to the button that acts on it.
        // The title takes what is left, and is cut short when that is not
        // enough. The bar is as wide as its window and the window is a fixed
        // size, so there is a limit and a long heading meets it.
        let said = div()
            .flex_1()
            .min_w_0()
            .h_flex()
            .items_center()
            .gap(px(10.))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .font_medium()
                    .text_size(px(13.))
                    .text_color(paint(palette.fg))
                    .child(title),
            )
            .children(writing.then(|| {
                div()
                    .flex_none()
                    .size(px(5.))
                    .rounded_full()
                    .opacity(tail)
                    .bg(paint(palette.accent))
                    .with_animation(
                        "pill-writing",
                        Animation::new(Duration::from_millis(1900))
                            .repeat()
                            .with_easing(pulsating_between(0.25, 1.0)),
                        move |this, breath| this.opacity(breath * tail),
                    )
            }))
            .child(
                div()
                    .flex_none()
                    .opacity(tail)
                    .font_family(cx.theme().mono_font_family.clone())
                    .text_size(px(10.5))
                    .text_color(paint(palette.muted))
                    .child(self.count(cx)),
            );

        // Filled, not outlined. There is one thing to do here, and an outline
        // is what you give the other one.
        let open = div()
            .id("open")
            .flex_none()
            .px(px(14.))
            .py(px(6.))
            .rounded(px(7.))
            .bg(paint(palette.accent))
            .text_size(px(11.5))
            .font_medium()
            .text_color(paint(palette.on_accent))
            .opacity(tail)
            .cursor_pointer()
            .hover(|this| this.bg(paint(palette.accent.mix(palette.fg, 0.16))))
            .on_click(cx.listener(|pill, _, window, cx| pill.open(window, cx)))
            .child("Open");

        let later = div()
            .id("later")
            .flex_none()
            .size(px(24.))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(6.))
            .opacity(tail)
            .text_color(paint(palette.muted.mix(palette.bg, 0.35)))
            .hover(|this| this.bg(paint(palette.wash)).text_color(paint(palette.fg)))
            // Stopped here, or the bar underneath would take the same click
            // and open the very thing being put away.
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(|pill, _, window, cx| pill.on_later(&Later, window, cx)))
            .child(Icon::new(IconName::Close).size(px(12.)));

        // How many are waiting, when more than one is. It goes before the
        // title, where the reader's eye already is, because *2 of 3* changes
        // what the title means: it stops being the deck and becomes the first
        // of several.
        let queued = (pending > 1).then(|| {
            div()
                .flex_none()
                .px(px(6.))
                .py(px(2.))
                .rounded_full()
                .bg(paint(palette.focus))
                .font_family(cx.theme().mono_font_family.clone())
                .text_size(px(10.))
                .text_color(paint(palette.accent))
                .child(format!("{} of {pending}", self.at + 1))
        });

        let bar = div()
            .id("pill")
            .relative()
            .w_full()
            .h(px(BAR_H))
            .h_flex()
            .items_center()
            .gap(px(12.))
            .pl(px(15.))
            .pr(px(9.))
            .rounded(px(12.))
            .overflow_hidden()
            .bg(paint(palette.bg))
            .border_1()
            .border_color(paint(palette.edge))
            .shadow_lg()
            // The bar itself does nothing when clicked.
            //
            // It was the whole target once, on the reasoning that a button the
            // size of a word is a button you have to aim at. But the bar is
            // also the only thing there is to take hold of when moving it out
            // of the way, and a drag ends in a click — so putting it somewhere
            // else opened the deck. One of the two had to go, and being able
            // to move it matters more than the larger target.
            .child(after("pill-brand", 0.18, brand))
            .children(queued.map(|queued| after("pill-queued", 0.22, queued)))
            .child(after("pill-said", 0.28, said))
            .child(open)
            .child(later);

        div()
            .size_full()
            .flex()
            .items_center()
            .track_focus(&self.focus)
            .key_context("Pill")
            .on_action(cx.listener(Self::on_open))
            .on_action(cx.listener(Self::on_later))
            .on_action(cx.listener(Self::on_walk))
            // It rises into place rather than appearing. This is the first
            // thing anyone sees of deck, and a bar that blinks into existence
            // over your work reads as an alert; one that comes up reads as
            // something arriving.
            .child(bar.with_animation(
                "pill-arrive",
                Animation::new(Duration::from_millis(420)).with_easing(ease_out_quint()),
                |this, up| this.opacity(up).mt(px(14. * (1. - up))),
            ))
    }
}
