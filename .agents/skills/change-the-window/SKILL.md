---
name: change-the-window
description: >-
  Work on deck's GPUI window — crates/app. Use before touching view.rs, pane.rs,
  chart.rs, pill.rs or prose.rs, or when a change makes the window crash, makes
  a picture vanish, or makes something land in the wrong place. Four mistakes
  account for nearly every bug this window has had, and three of them abort the
  process rather than failing.
---

# Changing the window

`deck-app` is [GPUI](https://www.gpui.rs/). It has a few sharp edges that do not
announce themselves, and the same four have caused nearly every bug in here.

## Never read the window during render

A pane asking the entity that is drawing it is a **second borrow of something
already borrowed**, and it aborts the process rather than failing. It usually
aborts inside a mouse handler, which is an `extern "C"` callback that cannot
unwind — so nothing catches it and deck dies.

```rust
// This killed every deck with a diagram in it.
let pace = view.upgrade().map_or(Pace::Normal, |deck| deck.read(cx).pace());
```

Everything a pane needs is handed down through `Slot` — the palette, its share
of the row, the playback speed. Add a field there instead.

**Handlers are fine.** `on_click` calling `deck.update(...)` runs later, when the
borrow has been released. It is only reading during `render` that is fatal.

If you see `panic_cannot_unwind` in a crash report, this is what happened.

## `div()` is display: block

This has eaten a diagram twice.

A block div is **only as wide as its content** and gives its children no height
to fill. So a child asking for `size_full` gets nothing, and a child that was
being centred stops being centred. Wrapping something in a plain `div()` to hang
a position off is the exact move that breaks it:

```rust
// The drawing inside this had no height and disappeared.
div().relative().child(self.render_drawing(...))

// It needs to be a flex row, and at least as big as the pane.
div()
    .relative()
    .flex()
    .min_w(relative(1.))
    .min_h(relative(1.))
    .child(self.render_drawing(...))
```

Before adding a wrapper, ask whether you can hang the position off something
already there. The flow buttons ended up anchored to the pane itself for exactly
this reason.

## Bubble-phase handlers run in reverse registration order

An overlay painted **after** a list answers **first**, and can call
`stop_propagation`. Several behaviours here depend on that ordering rather than
on hit-testing, and `HitboxBehavior::Normal` does not occlude anything.

If a click is being answered by the wrong element, the order things were added
in is the first place to look.

## A list whose length changes between frames

`with_animations` keeps the index of the running animation in element state and
indexes the list with it on the next frame. A list that gets **shorter** between
frames is an index past the end, and deck aborts.

```rust
// Two while writing, one when finished. It crashed the moment a deck
// was sealed, because that is when the flag flipped.
if writing { vec![arrive, pulse] } else { vec![arrive] }
```

Keep the length fixed and vary the contents. A zero-length animation with a
constant easing is a held frame and asks for nothing further.

## Verifying a change

You cannot screenshot the window from a script, and synthetic clicks do not
reach it. There is no flag that opens a deck without a person pressing Open, and
adding one is not the answer — that was removed on purpose.

So:

```sh
D=$(cargo run -q -p deck-app -- new --title "Checking a change" --total 1)
cargo run -q -p deck-app -- group "$D" --say "Here." --ref "src/main.rs:1-10 the change"
cargo run -q -p deck-app -- seal "$D"
cargo run -q -p deck-app -- open "$D"
```

A bar appears. Ask the person you are working with to press Open and say what
they see. **Do not claim a visual change works because it compiled.** If you
could not see it, say so.

One thing you *can* check from a script — that the window survived being opened,
which catches the aborts above:

```sh
sleep 3 && ps -Ao args | grep -c "[d]eck open $D"
```

## Crash reports

macOS writes them, and they name the failure precisely:

```sh
F=$(ls -t ~/Library/Logs/DiagnosticReports/deck-*.ips | head -1)
```

The first line is JSON metadata and the rest is a second JSON document. The
faulting thread's frames are usually enough: `panic_bounds_check` is an index
past the end, `panic_cannot_unwind` is a borrow inside a handler.
