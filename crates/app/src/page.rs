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
use gpui_kit::prelude::FluentBuilder as _;
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
    /// Whether the agent is talking about this pane. See [`Self::heed`].
    heeded: crate::pane::Fade,
    /// The last name the page was told, which it is still showing.
    ///
    /// Apart from `pointed`, which is whether the narration is in this page
    /// right now and so whether its pane wears the frame.
    told: Option<SharedString>,
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
            told: None,
            heeded: crate::pane::Fade::default(),
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
        self.told = None;
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
    ///
    /// Only ever forward. The page is told a name of its own, and never that
    /// the finger left: when the narration moves into another pane, or the
    /// voice stops, the page keeps the moment it was showing. Told *nothing*
    /// each time, it snapped back to its first frame and played its way up
    /// again on the next point — a walk that went between code and a page
    /// shuddered from rest to the step and back, sentence after sentence.
    /// `again` is the one way back to the start.
    pub fn point_at(&mut self, ids: Vec<SharedString>) {
        self.pointed = ids;
        let Some(first) = self.pointed.first().cloned() else {
            return;
        };
        if self.told.as_ref() == Some(&first) {
            return;
        }
        self.told = Some(first.clone());
        self.tell("point", quoted(&first));
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
        // The hole is the last word on whether this page is on screen. The
        // window keeps the view away while its pane is folding or folded; this
        // catches whatever else leaves the pane with no room to stand in.
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
        // Whole device pixels: a flex share puts the hole on a fraction, and
        // a native view standing on a fraction is drawn soft.
        let at = snapped(at, window.scale_factor());
        if self.at != Some(at) {
            let _ = view.set_bounds(rect(at));
            self.at = Some(at);
        }
        if !self.shown {
            let _ = view.set_visible(true);
            self.shown = true;
        }
    }

    /// The page folded down to its spine: its name, and what it is of.
    pub fn render_folded(&self, slot: &Slot, cx: &App) -> AnyElement {
        let (title, under) =
            crate::pane::spine_words(self.name.as_ref(), &self.label, self.note.as_ref());
        let said = under.as_ref().map_or_else(
            || title.clone(),
            |under| format!("{title} — {under}").into(),
        );
        crate::pane::spine(slot, &title, under.as_ref(), said, cx)
    }

    /// The pane, header and all.
    ///
    /// Deck draws the chrome and leaves a hole. The hole is measured as it is
    /// painted and the native view is moved to sit in it, which is the only way
    /// two rendering worlds can share a window.
    /// Outline this pane, or stop: the agent is talking about it, or is not.
    pub fn heed(&mut self, on: bool) {
        self.heeded.set(on);
    }

    /// Whether the outline is still coming up or going out.
    #[must_use]
    pub fn fading(&self) -> bool {
        self.heeded.moving()
    }

    pub fn render(&self, slot: &Slot, cx: &App) -> impl IntoElement {
        let palette = slot.palette;
        let hole = Rc::clone(&self.hole);
        let heeded = self.heeded.level();

        div()
            .v_flex()
            // Folding is a width, as it is for a file: the pane gives up its
            // share of the row a frame at a time and its spine takes the edge.
            .flex_grow(slot.share * (1. - slot.fold))
            // And fades as it goes, rather than being squeezed in full view.
            .opacity(1. - slot.fold)
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
                crate::pane::controls(slot, None),
                palette,
                cx,
            ))
            .relative()
            .children(self.render_again(slot.ix, palette, slot.view))
            // Two points in from the pane's edge, always. The page is a
            // native view laid over this hole, and it covers anything deck
            // draws under it — so the outline below needs a margin of the
            // pane's own to be seen in.
            .child(
                div().flex_1().min_h_0().p(px(crate::pane::FRAME)).child(
                    canvas(
                        move |at, window, _cx| {
                            // Only recorded. The window reads this at the top of
                            // the next frame and moves the view then, because
                            // nothing may change the window while it is drawing.
                            //
                            // A hole that moved asks for one more frame, so the
                            // view is never left where the last-but-one frame had
                            // it when a movement stops.
                            if hole.replace(Some(at)) != Some(at) {
                                window.request_animation_frame();
                            }
                        },
                        |_, (), _, _| {},
                    )
                    .size_full(),
                ),
            )
            // The pane being talked about, outlined — as a code pane is. A
            // point lit the block inside the picture and left the pane itself
            // unmarked, so with two panes side by side there was nothing to
            // say which one the sentence meant (#49).
            // Not round a pane that has folded away: a name hovered in the
            // prose can ask for one, and there is nothing there to frame.
            .when(heeded > 0. && slot.fold < 1., |pane| {
                pane.child(crate::pane::outline(palette, heeded))
            })
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

/// The names a document answers to, as the command line reads them — see
/// [`deck_cli::answered_by`]. Read from the markup rather than asked of the
/// document, because the answer is needed before there is a view to ask.
fn declared(html: &str) -> Vec<SharedString> {
    deck_cli::answered_by(html)
        .into_iter()
        .map(SharedString::from)
        .collect()
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
         [data-show],[data-from]{{transition:opacity .35s}}\
         [data-show]:not(.deck-seen),[data-from]:not(.deck-seen)\
           {{opacity:0!important;pointer-events:none!important}}\
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
///
/// And the declarative half, so most pages need no script at all: an element
/// with `data-on="hop settle"` wears `on` while the point is either of those;
/// `data-show="hop"` is there only then; `data-from="hop"` arrives at `hop`
/// and stays for every step after it, in the order `deck-points` lists them —
/// a point at a plain element in between does not take it away.
/// At rest — no point — nothing is on and nothing shown is showing, which is
/// the still first frame a page must open on.
const SHIM: &str = "window.deck=window.deck||{at:null};\
    (function(){\
    var step=null;\
    function apply(at){\
      var meta=document.querySelector('meta[name=deck-points]');\
      var order=meta?meta.content.split(/\\s+/):[];\
      if(at===null){step=null;}else if(order.indexOf(at)>=0){step=at;}\
      var here=order.indexOf(step);\
      var names=function(e,k){var v=e.getAttribute(k);return v===null?null:v.split(/\\s+/);};\
      document.querySelectorAll('[data-on]').forEach(function(e){\
        e.classList.toggle('on',at!==null&&names(e,'data-on').indexOf(at)>=0);});\
      document.querySelectorAll('[data-show],[data-from]').forEach(function(e){\
        var show=names(e,'data-show'),from=names(e,'data-from'),seen=true;\
        if(show){seen=at!==null&&show.indexOf(at)>=0;}\
        if(from){var f=order.indexOf(from[0]);seen=seen&&here>=0&&f>=0&&here>=f;}\
        e.classList.toggle('deck-seen',seen);});\
    }\
    addEventListener('deck:point',function(e){apply(e.detail);});\
    addEventListener('DOMContentLoaded',function(){\
      apply(window.deck.at);\
      if(window.deck.at!==null){\
        window.dispatchEvent(new CustomEvent('deck:point',{detail:window.deck.at}));\
      }\
    });\
    })();";

/// `at`, moved to the nearest whole device pixels on every edge.
#[cfg(not(target_os = "windows"))]
fn snapped(at: Bounds<Pixels>, scale: f32) -> Bounds<Pixels> {
    let whole = |v: Pixels| px((f32::from(v) * scale).round() / scale);
    let (left, top) = (whole(at.origin.x), whole(at.origin.y));
    let right = whole(at.origin.x + at.size.width);
    let bottom = whole(at.origin.y + at.size.height);
    Bounds {
        origin: point(left, top),
        size: size(right - left, bottom - top),
    }
}

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

#[cfg(test)]
mod tests {
    use super::{SHIM, dressed};

    /// What `data-show` and `data-from` switch has to look like something.
    /// The script toggled a class nothing styled, and both did nothing.
    #[test]
    fn what_the_shim_hides_is_hidden() {
        let page = dressed(
            "<p>x</p>",
            &crate::palette::current_on(true, Default::default()),
        );
        // Hidden by deck's own stylesheet from the first paint, not by the
        // script once the document has loaded — so nothing flashes, and a
        // page rule cannot quietly show it again.
        assert!(page.contains(":not(.deck-seen)"));
        assert!(page.contains("opacity:0!important"));
        assert!(SHIM.contains("'deck-seen'"));
        assert!(
            SHIM.contains("(function(){"),
            "and the script keeps its names to itself"
        );
    }
}
