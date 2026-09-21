//! A page: HTML the agent wrote, living inside the deck's own window.
//!
//! # Why a native view and not something deck draws
//!
//! The other two kinds of pane are deck's own pixels — a list of highlighted
//! rows, a canvas of boxes and arrows — so folding and every animation come
//! for free. A page cannot be: HTML with a script in it needs a
//! browser engine, and the only ones available are the platform's own.
//!
//! So this is a child view inside the deck's window, not a second window.
//! [`Window`] hands out the native handle — the same one gpui-base uses
//! to install IME behaviour — and the platform webview is added as a subview of
//! it, positioned to the pane's rectangle every frame.
//!
//! # What that costs, said plainly
//!
//! A native subview composites **above** everything deck paints, whatever order
//! deck would like. It cannot slide under the shade, be clipped by a folding
//! neighbour, or scroll with the room. So a page is hidden while any of that is
//! happening and shown again when the room is still. That is the whole trade,
//! and it is the reason a picture is still the right answer for anything a
//! picture can say.

use std::cell::Cell;
use std::rc::Rc;

use deck_core::protocol::PageRef;
use deck_core::theme::{Palette, Rgb};
use gpui_kit::component::*;
use gpui_kit::*;

use crate::palette::paint;
use crate::sheet::Slot;

/// A page pane: the agent's markup, and the native view showing it.
pub struct Paper {
    /// The ref this pane shows, so a remark can name it.
    pub ref_id: SharedString,
    /// What the prose calls it.
    pub name: Option<SharedString>,
    /// The label along its header.
    label: SharedString,
    /// The note beside the label.
    note: Option<SharedString>,
    /// The markup, handed to the view once.
    html: SharedString,
    /// The document as the view was given it, kept for putting it back.
    dressed: Option<String>,
    /// The ids the markup declares, so a point can find its pane.
    ids: Vec<SharedString>,
    /// The elements a point in the narration is holding a light on.
    pub pointed: Vec<SharedString>,
    /// What the reader has picked inside the page, if anything.
    ///
    /// A page has no lines, so a remark about one is pinned to whichever region
    /// the page said was chosen — and to nothing at all when none is, which
    /// makes it a remark about the whole thing.
    pub selected: Option<SharedString>,
    /// The view, once the window has been able to make one.
    view: Option<wry::WebView>,
    /// Where it was last put, so it is only moved when it has moved.
    at: Option<Bounds<Pixels>>,
    /// Where the pane's hole was drawn, written while painting and read on the
    /// next frame.
    ///
    /// A cell rather than a call back into the window: the pane is being drawn
    /// from inside the window's own render, so asking the window to change
    /// itself there is a borrow it cannot give. This was the whole reason the
    /// first page came up empty.
    hole: Rc<Cell<Option<Bounds<Pixels>>>>,
    /// Whether it is on screen right now.
    shown: bool,
}

impl Paper {
    /// Build a pane for `spec`.
    #[must_use]
    pub fn new(spec: &PageRef) -> Self {
        Self {
            ref_id: spec.id.clone().into(),
            name: spec.name.clone().map(SharedString::from),
            label: spec
                .name
                .clone()
                .unwrap_or_else(|| "page".to_string())
                .into(),
            note: spec.note.clone().map(SharedString::from),
            ids: declared(&spec.page),
            html: spec.page.clone().into(),
            dressed: None,
            pointed: Vec::new(),
            selected: None,
            view: None,
            at: None,
            hole: Rc::new(Cell::new(None)),
            shown: false,
        }
    }

    /// Whether this page declares an element by that id.
    ///
    /// How a point finds its pane, as a picture answers with its blocks. Read
    /// from the markup rather than asked of the document, because the answer is
    /// needed before there is a view to ask.
    #[must_use]
    pub fn has_block(&self, id: &str) -> bool {
        self.ids.iter().any(|known| known == id)
    }

    /// Put the page back to the state it was opened in.
    ///
    /// A page is the one pane that keeps going after you have looked away. A
    /// picture is the same picture every time you come back to it and a file
    /// does not move at all, but an animation runs once and leaves the reader
    /// looking at the end of it — which is the least useful frame, because it
    /// is the one they can already see the consequence of.
    ///
    /// So: the same offer a diagram makes with its flows. Watch it again.
    pub fn again(&mut self) {
        let (Some(view), Some(document)) = (self.view.as_ref(), self.dressed.as_ref()) else {
            return;
        };
        let _ = view.load_html(document);
        // Back to the first frame, and the finger with it: the fresh document
        // has never heard of the point that was held on the old one.
        let _ = view.evaluate_script("window.deck && (window.deck.at = null)");
        self.pointed.clear();
    }

    /// Light these elements, and tell the page it happened.
    ///
    /// The light itself is the page's business: deck cannot reach inside a
    /// document and highlight something without knowing what the document
    /// meant by it. What deck can do is say *the reader is on this now* — and
    /// that event is the difference between a page and a picture of a page. It
    /// arrives as `deck:point` with the id, or with `null` when the finger has
    /// lifted, and the page decides whether that means a glow, an animation, or
    /// a simulation stepping forward.
    pub fn point_at(&mut self, ids: Vec<SharedString>) {
        if self.pointed == ids {
            return;
        }
        self.pointed = ids;
        self.tell(
            "point",
            self.pointed
                .first()
                .map_or_else(|| "null".to_string(), |id| quoted(id)),
        );
    }

    /// Say that a remark was pressed, so the page can move with the thread.
    ///
    /// The reader clicking a comment in the rail is the other half of the
    /// to-and-fro: it is them saying *this part*, in a review, about something
    /// that moves. No artifact can hear that, because no artifact knows the
    /// conversation is happening.
    pub fn remarked(&self, quote: &str) {
        self.tell("remark", quoted(quote));
    }

    /// Hand one event to the page.
    ///
    /// The point is remembered on `window.deck.at` as well as dispatched,
    /// because a page that is still loading has no listeners yet and an event
    /// sent into that gap is simply lost. The shim replays it once the document
    /// is ready — which is how a page opened halfway through a walk comes up
    /// showing the step the reader is actually on.
    fn tell(&self, what: &str, detail: String) {
        let Some(view) = self.view.as_ref() else {
            return;
        };
        let remember = if what == "point" {
            format!("window.deck.at={detail};")
        } else {
            String::new()
        };
        let _ = view.evaluate_script(&format!(
            "{remember}window.dispatchEvent(new CustomEvent('deck:{what}',{{detail:{detail}}}))"
        ));
    }

    /// What a remark about this page quotes.
    ///
    /// The region the reader picked, or the pane's own label when they picked
    /// nothing — which makes the remark one about the whole page.
    #[must_use]
    pub fn quote(&self) -> String {
        self.selected
            .as_ref()
            .map_or_else(|| self.label.to_string(), ToString::to_string)
    }

    /// Take it off the screen, without throwing away what it is showing.
    ///
    /// Called for every movement a native view cannot join in with: a fold, the
    /// shade going down, a turn. Hiding is cheap, and keeping the view alive
    /// means the page does not lose the state it was animating.
    pub fn hide(&mut self) {
        if !self.shown {
            return;
        }
        if let Some(view) = self.view.as_ref() {
            let _ = view.set_visible(false);
        }
        self.shown = false;
    }

    /// Put the view where the pane was drawn, making it if there is none yet.
    pub(crate) fn settle(&mut self, window: &Window, palette: &Palette) {
        let Some(at) = self.hole.get() else {
            return; // not drawn yet, so there is nowhere to stand
        };
        // The hole is the only honest answer to whether this page is on screen.
        //
        // Asking the fold instead was wrong in the one case that matters: a
        // fold is a request, and the row can refuse it — the last pane with
        // anything in it keeps its width, because there is nobody to give it
        // to. Deck then believed it was folded and hid the view, while the
        // layout carried on drawing the pane. An empty box, half the row wide.
        if f32::from(at.size.width) < 8. || f32::from(at.size.height) < 8. {
            self.hide();
            return;
        }
        if self.view.is_none() {
            let document = dressed(&self.html, palette);
            self.view = build(window, &document, at);
            self.dressed = Some(document);
        }
        let Some(view) = self.view.as_ref() else {
            return;
        };
        if self.at != Some(at) {
            let _ = view.set_bounds(rect(at));
            self.at = Some(at);
        }
        if !self.shown {
            let _ = view.set_visible(true);
            self.shown = true;
        }
    }

    /// The pane, header and all.
    ///
    /// Deck draws the chrome and leaves a hole. The hole is measured as it is
    /// painted and the native view is moved to sit in it, which is the only way
    /// two rendering worlds can share a window.
    pub fn render(&self, slot: &Slot, cx: &App) -> impl IntoElement {
        let palette = slot.palette;
        let hole = Rc::clone(&self.hole);

        div()
            .v_flex()
            .flex_grow(slot.share)
            .flex_shrink(1.)
            .flex_basis(px(0.))
            .h_full()
            .min_w_0()
            .min_h_0()
            .overflow_hidden()
            .bg(paint(palette.wash))
            .child(crate::pane::chrome(
                None,
                self.label.clone(),
                self.note.clone(),
                crate::pane::Controls::default(),
                palette,
                cx,
            ))
            .relative()
            .children(self.render_again(slot.ix, palette, slot.view))
            .child(
                canvas(
                    move |at, _window, _cx| {
                        // Only recorded. The window reads this at the top of
                        // the next frame and moves the view then, because
                        // nothing may change the window while it is drawing.
                        hole.set(Some(at));
                    },
                    |_, (), _, _| {},
                )
                .size_full(),
            )
    }
}

impl Paper {
    /// The button that plays the page again.
    ///
    /// Anchored where a diagram's flow buttons sit, because it is the same
    /// offer in the same place: this pane can be watched more than once.
    fn render_again(
        &self,
        pane_ix: usize,
        palette: &Palette,
        view: &WeakEntity<crate::view::DeckView>,
    ) -> Option<impl IntoElement> {
        let view = view.clone();
        Some(
            div()
                .absolute()
                .top(px(35.))
                .right(px(10.))
                .id(("again", pane_ix))
                .h_flex()
                .items_center()
                .gap(px(6.))
                .pl(px(9.))
                .pr(px(7.))
                .py(px(3.))
                .rounded(px(11.))
                .border_1()
                .border_color(paint(palette.edge))
                .bg(paint(palette.bg))
                .text_size(px(10.5))
                .text_color(paint(palette.muted))
                .cursor_pointer()
                .hover(move |this| this.border_color(paint(palette.accent)))
                .child("again")
                .child(
                    div()
                        .flex_none()
                        .w(px(10.))
                        .flex()
                        .justify_center()
                        .text_size(px(8.))
                        .child("\u{21ba}"),
                )
                .on_click(move |_, _window, cx| {
                    view.update(cx, |deck, cx| deck.play_page_again(pane_ix, cx))
                        .ok();
                }),
        )
    }
}

/// The names a document answers to, in the order they appear.
///
/// A scan rather than a parse. Deck needs to know which page owns `[point
/// rows]` before anything has been rendered, so it reads two things out of the
/// markup: every `id`, and anything listed in
///
/// ```html
/// <meta name="deck-points" content="make link attach wrong">
/// ```
///
/// The meta tag exists because the interesting names are usually *states* and
/// not elements. A page about putting a box into a chain answers to `attach`,
/// and there is no element called that — there is a row of boxes that arranges
/// itself differently when the reader gets to that sentence. Without a way to
/// say so, the point found no pane, the page never heard, and the code lit up
/// beside a picture that had not moved.
fn declared(html: &str) -> Vec<SharedString> {
    let mut out = Vec::new();
    if let Some(at) = html.find("name=\"deck-points\"") {
        let after = &html[at..];
        if let Some(from) = after.find("content=\"")
            && let Some(end) = after[from + 9..].find('"')
        {
            for name in after[from + 9..from + 9 + end].split_whitespace() {
                out.push(SharedString::from(name.to_string()));
            }
        }
    }
    let mut rest = html;
    while let Some(at) = rest.find("id=") {
        rest = &rest[at + 3..];
        let Some(quote) = rest.chars().next().filter(|ch| *ch == '"' || *ch == '\'') else {
            continue;
        };
        let Some(end) = rest[1..].find(quote) else {
            break;
        };
        let id = &rest[1..=end];
        if !id.is_empty()
            && id
                .chars()
                .all(|ch| ch.is_alphanumeric() || ch == '-' || ch == '_')
        {
            out.push(SharedString::from(id.to_string()));
        }
        rest = &rest[end + 2..];
    }
    out
}

/// The page, wearing deck's colours.
///
/// A page does not choose how it looks. Deck hands it the palette the rest of
/// the window is painted in — as custom properties, so the markup asks for
/// `var(--deck-accent)` and gets the reader's theme, light or dark, borrowed
/// from their editor or not.
///
/// This is most of what separates a page from an artifact somebody embedded. A
/// document that brings its own colours is a website sitting in the middle of a
/// review, announcing that it came from somewhere else.
fn dressed(html: &str, palette: &Palette) -> String {
    let hex = |colour: Rgb| format!("#{:02x}{:02x}{:02x}", colour.r, colour.g, colour.b);
    format!(
        "<style>:root{{\
           --deck-bg:{bg};--deck-fg:{fg};--deck-accent:{accent};\
           --deck-on-accent:{on_accent};--deck-muted:{muted};--deck-edge:{edge};\
           --deck-wash:{wash};--deck-add:{add};--deck-del:{del};\
           --deck-comment:{comment};\
         }}\
         html,body{{background:transparent;color:var(--deck-fg);\
           font:12px ui-monospace,SFMono-Regular,Menlo,monospace;}}\
         </style>{html}",
        bg = hex(palette.bg),
        fg = hex(palette.fg),
        accent = hex(palette.accent),
        on_accent = hex(palette.on_accent),
        muted = hex(palette.muted),
        edge = hex(palette.edge),
        wash = hex(palette.wash),
        add = hex(palette.add),
        del = hex(palette.del),
        comment = hex(palette.comment),
    )
}

/// What deck puts in every page before the page's own script runs.
///
/// Two lines of contract. `window.deck.at` is the point the reader is on right
/// now, and `deck:point` fires whenever that changes — including once more when
/// the document finishes loading, so a page that arrives mid-walk is not left
/// showing its first frame while the narration is three sentences in.
const SHIM: &str = "window.deck=window.deck||{at:null};\
    addEventListener('DOMContentLoaded',function(){\
      if(window.deck.at!==null){\
        window.dispatchEvent(new CustomEvent('deck:point',{detail:window.deck.at}));\
      }\
    });";

/// Where a page sits, in the coordinates a webview wants.
fn rect(at: Bounds<Pixels>) -> wry::Rect {
    wry::Rect {
        position: wry::dpi::LogicalPosition::new(f64::from(at.origin.x), f64::from(at.origin.y))
            .into(),
        size: wry::dpi::LogicalSize::new(f64::from(at.size.width), f64::from(at.size.height))
            .into(),
    }
}

/// Make the view, as a child of the window deck is already drawing in.
fn build(window: &Window, html: &str, at: Bounds<Pixels>) -> Option<wry::WebView> {
    wry::WebViewBuilder::new()
        .with_html(html)
        // Nothing is loaded from anywhere. A page is markup the agent wrote,
        // and a review surface that phones out while somebody reads their own
        // code is not a trade deck makes on their behalf.
        //
        // The page's own document is not a navigation away from anything —
        // refusing everything refused that too, which is a webview that builds
        // cleanly and then shows an empty rectangle.
        // Installed before anything in the document runs, so a page can read
        // where the reader is before it draws its first frame.
        .with_initialization_script(SHIM)
        .with_navigation_handler(|url| {
            let url = url.trim().to_ascii_lowercase();
            url.is_empty() || url.starts_with("about:") || url.starts_with("data:")
        })
        .with_transparent(true)
        .with_bounds(rect(at))
        .build_as_child(&window)
        .inspect_err(|err| {
            // Said once, out loud. A pane that is silently empty is worse than
            // one that explains itself: the reader would be looking at a hole
            // wondering whether the agent forgot to draw anything.
            eprintln!("deck: this window would not take a page: {err}");
        })
        .ok()
}

/// A string, as a script may safely receive it.
fn quoted(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' | '\r' => out.push(' '),
            ch if (ch as u32) < 0x20 => out.push(' '),
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
}
