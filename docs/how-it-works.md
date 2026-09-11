# How deck works

The shape of the thing, for somebody about to change it. The code carries the
detail — every module here starts with a header explaining what it is for, and
those are the real documentation. This is the map that tells you which one to
open.

## The whole loop

```
agent                        disk                        you
─────                        ────                        ───
deck new        ────────►  deck.json
deck open       ────────►                      ────►  a bar appears
                                                      (Open stays dark)
deck group      ────────►  g1.json             ────►  the bar lights up
   "                       g2.json             ────►  you open it, and read
deck seal       ────────►  done
deck wait       ◄────────  <id>.review         ◄────  you comment and submit
```

Nothing runs in the background. There is no daemon and no server — `deck open`
is a process holding a window, `deck wait` is a process watching for a file, and
every other command writes one file and exits. That is the whole architecture,
and it is why deck works with any agent that has a shell.

## Four crates

| crate | knows about |
| --- | --- |
| `deck-core` | the model. No I/O, no UI, no idea what a screen is. |
| `deck-cli` | writing a deck to disk |
| `deck-theme` | reading a palette out of an editor |
| `deck-app` | the window, and the `deck` command |

The rule that keeps them apart: **share the model, not the view.** `deck-core`
has no view trait. `layout` returns cells; something else turns cells into
pixels. The moment that arithmetic knows what a pane widget is, a second client
cannot use it and the work is being done twice.

`deck-core` is also `#![forbid(unsafe_code)]`. The app is not — a window has to
talk to a platform — but the model has no business needing it.

## A deck is a directory

Not a file, and that is a design decision with consequences.

```
d-1788265010-8842.deck/
  deck.json      the header — title, project, how many groups are coming
  g1.json        one claim, and the code that shows it
  g2.json
  done           written last
d-1788265010-8842.review    your answer, written beside it
```

Writing a deck takes an agent tens of seconds, and there is no reason for you to
wait on the last group to see the first. So `crates/app/src/load.rs` does not
*read* a deck, it **tails** one: the header once, then whatever groups have
landed, again whenever asked, until `done` appears.

### The gap rule

Group files land in whatever order the agent finishes them. The story does not —
the prose of group 3 assumes group 2 has been read. So a group that arrives with
a gap in front of it is **held**: kept, counted, and not shown, until the gap
fills. What the window offers is the unbroken run from the beginning, which is
the most of the story that can honestly be read.

This is why you can start reading before the agent has finished, and why you
never read it out of order.

## Comments survive the file moving

You open a deck. It pins a comment to line 120. While reading you switch to your
editor, add three lines near the top, and save. Line 120 is now line 123 and
every range deck is holding is wrong.

Deck did not make those edits and cannot ride along with them. What it has is two
versions of the file — the snapshot taken when the deck opened, and whatever is
on disk now — and from those it works out where each pin went. Three answers, in
descending confidence, and **the answer always travels with how it was reached**:

| source | means |
| --- | --- |
| `diff` | the line was followed across a diff of the two versions. Exact. |
| `fingerprint` | the line was found again by its own text. A good guess. |
| `stale` | nobody knows. Search for `quote` instead of trusting the number. |

That last column is the point. An agent acting on a comment needs to know how far
to trust the line number, so the confidence is part of the protocol rather than
something the reader has to infer. `crates/core/src/relocate.rs`.

## The window

`deck-app` is [GPUI](https://www.gpui.rs/). Two things about it are worth knowing
before you touch `view.rs`.

**Bubble-phase mouse handlers run in reverse registration order.** An overlay
painted *after* a list answers *first*, and can stop propagation. Several
behaviours here depend on that ordering rather than on hit-testing.

**Never read the window during render.** A pane asking the entity that is drawing
it is a second borrow of something already borrowed, and it aborts the process
rather than failing. Worse, it usually aborts inside a mouse handler, which
cannot unwind. Everything a pane needs is handed down through `Slot` — the
palette, its share of the row, the playback speed. Handlers are fine: they run
later, when the borrow is released.

The main files:

- `view.rs` — the window, the selection, the review. The big one.
- `pane.rs` — a code pane: the rows, the highlighting, the picking.
- `chart.rs` — diagrams. Turns cells into geometry, then paints them.
- `pill.rs` — the bar that appears before the deck does.
- `prose.rs` — the narration band, and picking a sentence out of it.

## The bar, and who decides when to read

An agent finishes when it finishes. Being interrupted by a review is not the same
as being ready to give one — so what arrives is a bar at the bottom of the
screen, and the deck opens when *you* open it.

There is no flag that skips this. There was, and an agent used it to put a deck
on top of one the reader already had open. The skill told it not to and it did it
anyway, which settled the question of whether that belonged in the skill:
**a rule an agent can ignore is not a fix.** So the capability is gone rather
than documented against.

`deck open` also hands the window to a process of its own and returns in about
twenty milliseconds. It used to run the event loop in the process that was typed,
which meant an agent running it in the foreground hung there and never wrote a
group.

## Diagrams

Some things are in no file at all — how a click reaches a controller, the order
four services touch a request. `deck-core::diagram` is the model, `chart.rs`
turns it into geometry.

**The format exists so that decoration is not expressible.** There are no
colours, sizes or positions for the *shape* of a diagram: a node says what it is
(`role`) and how much it matters (`weight`), and deck owns every pixel. An agent
handed a blank canvas draws three rounded boxes and an arrow restating the
sentence above it. Two decks drawing the same idea come out looking the same.

A **flow** is a named path through the picture, and colour *is* expressible
there — per flow and per step — because two paths over one diagram are two
answers, and telling them apart by which button was last pressed means holding
the previous one in your head.

Playing one moves a *front* along the path continuously rather than stepping
frames: at `front: 2.4` the third box is 40 per cent lit and the arrow into it is
40 per cent across. Whole-number steps on a timer were the same information
delivered as a flick-book.

## Themes

Deck borrows your editor's colours rather than shipping a look you tolerate
beside your work. `deck-theme` reads nvim, Zed, VS Code, Cursor and Windsurf.

It takes the page, the text, an accent, and whatever syntax colours the theme
has — **never its chrome**. A theme designs its own status bar and tab strip, and
those are answers to questions deck is not asking. Everything else is derived, so
a deck always looks like a deck.

Two things that cost a day each and are not obvious:

- **Zed compiles One Dark and One Light into its binary.** They are not files on
  disk. The real JSON is extracted and shipped as constants.
- **Every Zed colour is 8-digit hex** (`#282c33ff`). A parser that only took 6
  would fail for every Zed user, silently.

Paths differ per platform and every one of them goes through
`deck-core::home` — one place that answers where a person's things live, because
spelling the macOS one out in eight places is how deck came to run on exactly one
kind of machine.

## Where to read next

- [`PROTOCOL.md`](../PROTOCOL.md) — the wire format, frozen at v1. Read this
  before changing anything a client can see.
- [`crates/core/src/lib.rs`](../crates/core/src/lib.rs) — the model, and a map of
  its modules.
- [`crates/app/skill/SKILL.md`](../crates/app/skill/SKILL.md) — what an agent is
  told about when and how to use deck.
- [`CONTRIBUTING.md`](../CONTRIBUTING.md) — conventions, and what needs doing.
