//! The `deck` command, and the window it opens.
//!
//! `deck new`, `deck group` and `deck seal` write a deck; `deck open` puts it
//! on screen; `deck wait` blocks until the answer comes back. Nothing runs in
//! the background: the agent opens the window, so there is no drop directory to
//! watch, no deck to claim, and no question of which client wins.
//!
//! What arrives first is a bar, not the deck. An agent finishes when it
//! finishes, and being interrupted by a review is not the same as being ready
//! to give one — so the reader is told a deck is ready and opens it when they
//! want it.
//!
//! There is no flag that skips the bar. There was, and an agent used it to put
//! a deck on screen over one the reader had already opened. Whoever writes a
//! deck does not get to decide when it is read.

mod chart;
mod cli;
mod config;
mod load;
mod palette;
mod pane;
mod pill;
mod prose;
mod queue;
mod setup;
mod sheet;
mod showing;
mod skill;
mod state;
mod view;
mod waiting;

use std::process::ExitCode;

use clap::Parser as _;
use gpui_kit::component::*;
use gpui_kit::*;

use crate::cli::Cli;
use crate::load::Deck;
use crate::pill::Pill;
use crate::view::{DeckView, Session};

fn main() -> ExitCode {
    // Everything but `open` finishes without a window, and says so in its exit
    // code. `open` needs the main thread, so it comes back here to be run.
    let opening = match Cli::parse().run() {
        Ok(Some(opening)) => opening,
        Ok(None) => return ExitCode::SUCCESS,
        Err(code) => return code,
    };

    // Read before opening anything. A window that appears and then admits it
    // could not read its own deck is worse than a command that never opened
    // one — and the agent, which is what ran this, can act on an exit code.
    //
    // All of them, and any that will not read is fatal. A queue that quietly
    // dropped one would leave an agent waiting for a review of a deck nobody
    // was ever shown.
    let mut decks = Vec::with_capacity(opening.deck.len());
    for path in &opening.deck {
        match Deck::read(path) {
            Ok(deck) => decks.push(deck),
            Err(err) => {
                eprintln!("deck: cannot read {}: {err}", path.display());
                return ExitCode::FAILURE;
            }
        }
    }

    show(decks, &opening);

    if !opening.wait {
        return ExitCode::SUCCESS;
    }
    // The window has gone. Either the reader submitted or they did not, and
    // the agent needs to be able to tell which without guessing.
    //
    // The first deck named, because `--wait` is one agent asking about the deck
    // it just wrote. A queue is the reader's shape, not the agent's: an agent
    // that opened five would be waiting on five answers and needs to ask for
    // them one at a time.
    let asked_about = &opening.deck[0];
    match deck_cli::review(asked_about) {
        Ok(Some(review)) => match serde_json::to_string_pretty(&review) {
            Ok(json) => {
                println!("{json}");
                ExitCode::SUCCESS
            }
            Err(err) => {
                eprintln!("deck: the review will not print: {err}");
                ExitCode::FAILURE
            }
        },
        // Four, not three. Three is `deck wait` giving up on a timeout — *no
        // answer yet* — and this is *no answer coming*. An agent that could not
        // tell them apart would either keep waiting on a deck the reader has
        // closed, or give up on one they are still reading.
        Ok(None) => {
            eprintln!("deck: closed without a review");
            ExitCode::from(4)
        }
        Err(err) => {
            eprintln!("deck: the review will not parse: {err}");
            ExitCode::FAILURE
        }
    }
}

/// Open the window and run it until it closes.
///
/// Blocks: this is the whole of the process's life while a deck is on screen.
fn show(decks: Vec<Deck>, opening: &cli::Opening) {
    let (dark, layout) = (opening.dark, opening.layout);
    let (editor, paper, colors) = (opening.editor, opening.paper, opening.colors);
    let named = opening.named.clone();
    // With the asset source, or every icon is an empty box: the component
    // library draws them from SVGs it ships, and nothing serves those unless
    // the application is told where they live.
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(move |cx: &mut App| {
            gpui_kit::init(cx);
            palette::embed_fonts(cx);

            cx.bind_keys(view::bindings());
            cx.bind_keys(pill::bindings());

            // A deck is one window at a time. Closing the last one ends the
            // command rather than leaving a headless process behind for the
            // agent to wonder about.
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();

            // The theme is read here rather than while the arguments were
            // being parsed, because an editor with no theme set has one for
            // light and one for dark — and which it would show is the
            // *machine's* answer, which only the platform can give and only
            // once it is up.
            //
            // Said out loud when it fails, then carried on without. A deck
            // quietly in the wrong colours looks like deck being wrong rather
            // than the import having failed; a review nobody can read because
            // their theme moved is worse.
            // The *platform*, not the component library's theme.
            //
            // `cx.theme().mode` is whatever the library was last set to, which
            // at this point is its own default — light. Asking it what the
            // machine is set to gives the answer to a different question, and
            // the answer is always light. A reader in dark mode with an
            // unthemed editor got a white deck.
            let system_dark = matches!(
                cx.window_appearance(),
                WindowAppearance::Dark | WindowAppearance::VibrantDark
            );
            let imported = match deck_theme::read(editor, named.as_deref(), system_dark) {
                Ok(imported) => imported,
                Err(err) => {
                    eprintln!("deck: cannot read the theme: {err}");
                    None
                }
            };
            palette::choose(palette::Chosen {
                paper,
                imported,
                colors,
            });

            // Before any view is built: a view reads the mode the moment it is
            // constructed, and the palette it paints with follows from it.
            //
            // An imported theme decides this outright. Deck asked for light and
            // the editor's page is nearly black would be two answers to one
            // question, and the editor's is the one with a colour behind it.
            let machine = imported.map_or(system_dark, |imported| imported.bg.is_dark());
            Theme::change(
                if dark.is_dark(machine) {
                    ThemeMode::Dark
                } else {
                    ThemeMode::Light
                },
                None,
                cx,
            );

            // Anything another window is already showing is dropped here.
            // Running `deck open` twice on one deck used to give the reader a
            // second window over the one they were reading.
            let waiting: Vec<Session> = decks
                .into_iter()
                .filter(|deck| !crate::showing::taken(&deck.header.id))
                .map(|deck| {
                    crate::showing::take(&deck.header.id);
                    Session::fresh(deck).arranged(layout)
                })
                .collect();

            if waiting.is_empty() {
                // Every one of them is already on screen somewhere. Nothing to
                // add, and a window with nothing in it is worse than none.
                cx.quit();
                return;
            }

            // The bar, always. There is no way for whoever opened this to put
            // the deck itself on screen, and that is deliberate: an agent that
            // could would use it, and a deck arriving across somebody's work
            // uninvited is the thing this bar was built to replace. Reading is
            // the reader's to start.
            //
            // The one exception is for taking pictures of the window, and it
            // exists only in a debug build: the screenshots and the recordings
            // in the README have to come from somewhere, and the alternative is
            // a person opening the same deck in five themes by hand. An
            // installed deck does not contain this branch at all, so the
            // guarantee above is a guarantee and not a convention.
            let mut waiting = waiting;
            if cfg!(debug_assertions)
                && std::env::var_os("DECK_SHOW_ME").is_some()
                && waiting.len() == 1
            {
                cx.activate(true);
                open_deck(waiting.remove(0), cx);
                return;
            }

            open_pill_over(waiting, cx);
        });
}

/// Put the bar on screen: a deck is ready, opened when the reader wants it.
///
/// It does not take focus. The whole point is that the reader's hands stay
/// where they were until they decide otherwise, and a bar that stole the
/// keyboard to announce itself would be the interruption it exists to avoid.
pub fn open_pill(session: Session, cx: &mut App) {
    open_pill_over(vec![session], cx);
}

/// The bar, while one is on screen.
///
/// A global because there is at most one and anything may need to reach it: a
/// deck being put away goes back into the bar it came from, and it may be put
/// away from a window that knows nothing about how it was opened.
struct Bar(WindowHandle<Pill>);

impl Global for Bar {}

/// Put the bar on screen over every deck waiting.
///
/// One bar, however many decks. Five bars fighting over the bottom of the
/// screen is not five times as useful as one — it is less useful, because the
/// reader has to work out which is which before they can start on any of them.
///
/// So a deck arriving while the bar is up joins the queue rather than opening a
/// second bar. That is the ordinary case, not an edge one: it is what putting a
/// deck away does.
pub fn open_pill_over(waiting: Vec<Session>, cx: &mut App) {
    queue::extend(waiting, cx);
    let pending = queue::len(cx);
    if pending == 0 {
        return;
    }

    if let Some(Bar(bar)) = cx.try_global::<Bar>() {
        let bar = *bar;
        // `update` fails once the window has gone, which is how a stale handle
        // is told from a live one without asking.
        let joined = bar.update(cx, |pill, _window, cx| pill.show_last(cx));
        if joined.is_ok() {
            cx.activate(true);
            return;
        }
        cx.remove_global::<Bar>();
    }
    let mut origin = point(px(120.), px(120.));
    if let Some(display) = cx.primary_display() {
        let screen = display.bounds();
        // Centred on the width it will end up at, not the width it opens at:
        // the bar grows rightward from where it starts, so starting it at the
        // finished left edge is what leaves it centred when it stops.
        origin.x = screen.origin.x + (screen.size.width - px(pill::WIDE)) / 2.;
        origin.y = screen.origin.y + screen.size.height - pill::WINDOW.height - px(pill::ABOVE);
    }

    let options = WindowOptions {
        // No titlebar and no client decorations, unlike the deck window.
        //
        // The deck asks for both because it has to be resizable, and a
        // frameless window has no edges for the platform to offer. This one is
        // a fixed bar, and asking for the frame got a frame: a rounded panel
        // in the theme's own ground, drawn around the bar with its own border
        // and its own shadow. Against a dark desktop it was invisible and
        // against a light one it was a bezel.
        titlebar: None,
        window_bounds: Some(WindowBounds::Windowed(Bounds {
            origin,
            size: pill::WINDOW,
        })),
        kind: WindowKind::PopUp,
        // Clear, so the bar's rounded corners are corners rather than notches
        // cut out of a rectangle.
        window_background: WindowBackgroundAppearance::Transparent,
        is_movable: true,
        is_resizable: false,
        focus: true,
        ..Default::default()
    };

    // Without the component library's `Root`, which the deck window does use.
    // Root paints the theme's ground across the whole window, and the whole
    // point of this one is that everything outside the bar is not painted at
    // all. The bar needs none of what Root provides.
    let handle = match cx.open_window(options, |window, cx| {
        // Cleared here, in the window's own builder, and not from the handle
        // afterwards.
        //
        // Asking for it in the options does not reach the platform: GPUI's
        // macOS window keeps `window_background` as a field it never reads at
        // creation, so the window opens opaque whatever was requested. Asking
        // again once the handle came back worked, but not until the update had
        // run — and the window painted a frame or two before that, each one a
        // rounded panel of the window's own ground with the bar sitting inside
        // it. The bezel was not gone; it was brief.
        window.set_background_appearance(WindowBackgroundAppearance::Transparent);
        cx.new(|cx| Pill::new(pending - 1, cx))
    }) {
        Ok(handle) => handle,
        Err(err) => {
            eprintln!("deck: could not open: {err}");
            return;
        }
    };
    cx.set_global(Bar(handle));
}

/// Put the deck itself on screen.
pub fn open_deck(session: Session, cx: &mut App) {
    // Sized to a deck, not to the screen. A review is read, so the window wants
    // the shape of a page rather than every pixel the display has. Placed
    // explicitly rather than centred: a deck is read from the top down, so it
    // sits high on the screen, and centring a panel put it most of the way off
    // the bottom edge. Where it was last left, if it has been anywhere.
    let bounds = state::remembered().unwrap_or_else(|| {
        let mut wanted = size(px(1180.), px(860.));
        let mut origin = point(px(120.), px(80.));
        if let Some(display) = cx.primary_display() {
            let screen = display.bounds();
            wanted.width = wanted.width.min(screen.size.width * 0.92);
            wanted.height = wanted.height.min(screen.size.height * 0.86);
            origin.x = screen.origin.x + (screen.size.width - wanted.width) / 2.;
            origin.y = screen.origin.y + px(80.);
        }
        Bounds {
            origin,
            size: wanted,
        }
    });

    let options = WindowOptions {
        // A real titlebar, made invisible — not no titlebar.
        //
        // `titlebar: None` gives a window with no frame, and a window with no
        // frame has no edges for the platform to offer: `is_resizable` had
        // nothing to act on, and dragging anywhere moved the window instead.
        // GPUI's own `start_window_resize` is no help either — it is an empty
        // default in the platform trait, implemented for Wayland and doing
        // nothing here.
        //
        // `appears_transparent` keeps the frame, and with it the resizable
        // edges, while hiding the chrome. The traffic lights are pushed
        // off-screen rather than left sitting on the deck's own header.
        titlebar: Some(TitlebarOptions {
            title: None,
            appears_transparent: true,
            traffic_light_position: Some(point(px(-64.), px(-64.))),
        }),
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        // A panel rather than a document window, so a tiling window manager
        // leaves it alone and the deck keeps the size it asked for — which is
        // the shape the whole design assumes.
        kind: WindowKind::PopUp,
        window_decorations: Some(WindowDecorations::Client),
        is_movable: true,
        // A borderless window has no frame to grab, so the ability to resize
        // has to be asked for outright.
        is_resizable: true,
        window_min_size: Some(size(px(560.), px(360.))),
        ..Default::default()
    };

    let handle = match cx.open_window(options, |window, cx| {
        let view = cx.new(|cx| DeckView::resume(session, cx));
        cx.new(|cx| Root::new(view, window, cx))
    }) {
        Ok(handle) => handle,
        Err(err) => {
            eprintln!("deck: could not open: {err}");
            return;
        }
    };

    cx.activate(true);

    // Closing by any route the platform knows about. `q` writes it too, on its
    // way out — see `DeckView::on_close` — because that path quits the
    // application without closing the window first.
    let _ = handle.update(cx, |_, window, cx| {
        window.on_window_should_close(cx, |window, _| {
            if let WindowBounds::Windowed(bounds) = window.window_bounds() {
                state::remember_bounds(bounds);
            }
            true
        });
    });
}
