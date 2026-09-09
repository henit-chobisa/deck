---
name: deck
description: >-
  Point at code instead of describing it. Use whenever you are about to explain,
  review, or walk through code that lives in files — a change, a bug, an
  architecture, "how X works", "where Y happens". Reach for it on your own
  initiative: the moment your prose is about to say "see this", "look at",
  "notice", "in file X", "the function above", "line 40" — stop and show it
  instead. Explicit asks ("show me", "walk me through", "review this") trigger it
  too. Prose is the exception, for a single one-line point. When in doubt, deck it.
---

# deck

A deck is a short presentation of code. You point at real lines; the user walks
them, comments where they disagree, and submits. Every comment comes back at
once, pinned to the lines it was about.

Four commands and a wait.

## This is the default, not the exception

Nobody learns from filenames and line numbers in prose. If you are about to
explain something that lives in code, the question is not *should I present
this?* It is *is there any reason not to?*

The bar for prose is: it is one line you can say in one sentence. Everything else
is a deck.

**If your prose names a location, that location must be a ref.** Writing "line
116" or "the enum above" and not showing it is the single most common way to
waste this tool — the reader is looking at a screen that does not contain the
thing you just cited. The rule is mechanical: cite it, show it.

**Do not** use a deck for a one-line fix, for something the user watched you do,
or when they asked a question rather than for work. A deck for a typo is a window
in somebody's face for no reason. If you are unsure, ask in one line.

## Read the mode before you write

Getting this wrong makes the tool annoying instead of useful.

- **Ship** — they are working fast: *show me my change*, *review this PR*, a
  routine diff they want to approve and move on from. Point crisply, give the
  reason in a line, let the review loop close it.
- **Teaching** — they are learning: a bug being chased, a new area, *how does
  this work*, *walk me through*. Here the deck can make them reason before it
  explains: point at the code and, in the `say`, ask what they think happens —
  then reveal in a later group. End on the open question or the decision, not on
  *and that's it*.

A question (*why does X…*) is teaching. An imperative about their own work
(*show my change*) is ship. When unsure, default to ship and let them pull you
deeper — do not Socratically quiz somebody on a two-file refactor.

## Writing one

```
deck new --title "The batch counter stalls at 63" --total 3
```

Prints the deck's path. Everything after this takes it.

`--title` is the **question or the claim the deck answers**, in sentence case —
not a label. *The retry backoff is unbounded*, not *backoff changes*. It is read
cold, on a bar at the bottom of the screen, before anything else about the deck
is visible.

`--total` is how many groups you intend to write, so the window can say *2 of 3*
while the rest are on their way. Say it even if you are not certain.

```
deck group <path> \
  --say "The counter is decremented on the **error path** too." \
  --ref "src/batch.ts:140-148 decremented *twice* when the write fails" \
  --ref "src/queue.ts:88 the only caller"
```

Once per claim.

- `--say` is Markdown, inline only: `**bold**`, `*emphasis*` (painted in the
  accent, so it means *look at this word*), `` `code` ``, and a blank line
  between paragraphs. No headings, no lists.
- `--ref` is `file:first-last`, a space, then a short note. A bare number is one
  line. `*starred*` words in the note are accented.
- `--diagram <file.json>` draws a picture instead, for structure that is in no
  single file. See **When the answer is a picture** below — do not skip it, a
  flow question answered without one is a deck that did half the job.

```
deck seal <path>
```

Says there is no more coming.

## What makes a deck good

**One group is one thing you are saying.** If the prose has an *and* in it, it is
two groups.

**The `say` carries the why, not just the what.** *This column goes* is a shrug.
*This column goes because the enum above is the only thing that names its valid
values, and we are deleting the enum* is the model. A group that only says what
is not finished.

**A group with one ref should be a decision, not a default.** One ref is right
when the point *is* that one thing. Before writing, look for the pairs: an enum
and the column that stores it, a writer and the reader that consumes it, a
function and its only caller. Those belong on screen together — that is what the
grid is for, and the relationship is the thing being taught.

Plan the whole story before `deck new` — that is where `--total` comes from — and
count then: **if every group has exactly one ref, you have planned a list, not a
presentation.** Go back and find what belongs beside what.

**Tight ranges.** A two hundred line range is not a highlight, it is a shrug.

**Four refs to a group at most.** More is allowed and simply splits into pages to
walk, but four panes is already a lot to hold in your head, and a fifth ref is
usually a second group wearing a disguise.

**One file per pane.** Do not show one file in two panes. When a group has
several, they should be several files whose relationship is the point.

**Say whether a claim is proven.** When a group asserts something is correct,
safe or fixed, say *how it is known* — *tests green, shown below*, *this path is
exercised by X* — or say honestly that it is not verified yet. Never let a deck
imply a green checkmark that does not exist. Point at the passing test as a ref
when there is one.

**Small decks.** Three tight decks beat one sprawling one. A change across five
files is several decks, not one with everything in it.

## When the answer is a picture

Some things are in no file at all. How a click reaches a controller, what calls
what, the order four services touch one request, the states a job moves through
— that is structure *between* files, and a code ref cannot point at it.

**A question about a flow is a diagram.** *How does X work under the hood*, *what
happens when I press this*, *walk me through the request*. If answering means
naming three or more places and the order they run in, draw it. A deck that
answers a flow question with six code refs and no picture has made the reader
assemble the diagram in their own head — which is the work they asked you to do.

Then do both. The picture says *where*, the code says *what*. The usual shape is
one group holding the diagram, then a group per step that matters, each pointing
at real lines.

```
deck group <path> --say "..." --diagram flow.json
```

```json
{
  "title": "connect to controller",
  "flow": "down",
  "nodes": [
    { "id": "b", "label": "Connect", "role": "actor" },
    { "id": "h", "label": "onConnect", "note": "web/silo/connect.tsx" },
    { "id": "q", "label": "installed?", "role": "decision", "weight": "accent" },
    { "id": "c", "label": "JiraController", "note": "apps/api" },
    { "id": "s", "label": "workspace_credentials", "role": "store" }
  ],
  "edges": [
    { "from": "b", "to": "h" },
    { "from": "h", "to": "q", "label": "POST /connect" },
    { "from": "q", "to": "c", "label": "yes" },
    { "from": "c", "to": "s", "label": "upsert", "line": "dashed" }
  ],
  "clusters": [{ "label": "browser", "nodes": ["b", "h"] }]
}
```

Only `nodes` is required.

- `flow` — `down` or `right`. Default `down`.
- `role` — `step` (default), `decision`, `store`, `terminal`, `actor`. What the
  thing *is*. It decides the shape.
- `weight` — `normal` (default), `accent`, `muted`. How much it matters. It
  decides the colour, and one accented node is usually enough.
- `note` — one line under the label. A file path or a qualifier, not a sentence.
- `lane` — an integer pinning a node to a column, so a sequence keeps its tracks.
- `line` on an edge — `solid` (default), or `dashed` for a conditional one.
- Edge `label` is a word or two: `POST /connect`, `on submit`. Longer gets cut.
- `clusters` are containment, for a boundary worth seeing: browser against
  server, one crate against another.
- Cycles are fine. The returning edge is drawn as one that visibly comes back.

`--diagram` can be given more than once, and mixes with `--ref` — but the code
refs are always laid out first, whatever order you typed them in. When the
picture should come first, give it a group of its own.

**There are no colours, sizes or positions, on purpose.** A node says what it is
and how much it matters; deck owns every pixel. You cannot make it prettier, only
clearer, so spend the effort on what goes in it.

**Six to ten nodes.** Twenty is a wall, and a reader hunting for the entry point
has lost the one thing a picture is for. If it will not fit, it is two diagrams.

**Every node earns its place.** Cut the ones that only pass a value along. A
diagram is the *shape* of the flow, not a call graph.

## Write it so nobody wants to leave

This is the part that decides whether a deck is worth opening twice. The
commands are easy; the writing is the product.

**Plain words, always.** Say *runs twice* rather than *is invoked on multiple
code paths*. Say *this number is wrong* rather than *an incorrect value is
propagated*. If a sentence needs reading twice, it is your sentence that is
wrong, not the reader.

**One idea per sentence, one claim per group.** Long sentences hide the joins.
When you catch yourself writing *and* or *which* or *, so that*, put a full stop
in and start again.

**Never explain the code — explain the thing the code is doing.** The reader can
see `pending -= 1`; they have it on screen. What they cannot see is that it also
runs when the write fails, so the counter drifts down by one every error and the
batch finishes early. Point at the line, and say the consequence.

**Open at the surprise.** The first group is the one that makes them want the
second. Start where the story turns — the line that does the wrong thing, the
assumption that does not hold — not with three groups of throat-clearing about
where the file lives.

**Make it inevitable.** By the last group the reader should feel they could have
reached the same conclusion. Each group is a step they can take with you, not a
fact handed down. If a group needs something you have not shown yet, it is in
the wrong place.

**Name things the way they are.** *The counter*, *the retry*, *the queue*. Not
*the aforementioned variable*. Use the codebase's own words for its own things,
and one word per thing throughout — a concept renamed halfway is a concept the
reader loses.

**Be light, never clever.** A dry line is welcome; a pun that has to be worked
out is a speed bump. If a joke makes the point land faster, keep it. If it makes
the reader stop to admire it, cut it. Warmth is in the plainness, not in the
wordplay: *this one is my fault* reads better than any pun.

**Say the awkward part out loud.** *I am not sure this is right.* *This is the
ugly bit.* *This was a bad idea and here is the better one.* A deck that admits
its own weak spot is one the reader trusts everywhere else — and it is the
sentence that makes them comment, which is the whole point.

**End with something for them to do.** A question, a decision, a choice between
two shapes. A deck that ends in *and that is it* is a lecture, and lectures do
not get comments.

**Then read it back as if you had not written it.** Every group: does it say the
why, would a stranger follow it, is there a shorter word. That pass is the
difference between a deck people walk and a deck people close.

## Showing it, and waiting

```
deck open <path>
```

Returns immediately. A small bar appears saying a deck is ready; the reader opens
it when *they* are ready, which is not the same moment as you being ready to ask.

Then wait for the answer — **in the background, and then end your turn**:

```
deck wait <path>          # run this as a background command
```

It blocks until they submit, then prints the review as JSON on stdout. Your turn
ends; they read at their own pace; when they submit, the command exits and you
are woken with every comment at once. Say nothing further in the meantime — they
are reading, not waiting on you.

**Do not run `deck wait` in the foreground.** A review takes minutes, a
foreground command has a timeout, and a reader cannot be hurried.

| exit | means |
| ---- | ----- |
| 0 | they submitted; the review is on stdout |
| 3 | no review yet — only from `--timeout`, which you should not need |
| 4 | they closed the deck without answering |
| 1 | something was wrong; the message on stderr says what |

### Do not open a deck you are still working out

Write groups **one command at a time**, and open once a group or two is in it —
but only when you already know what the rest of them say. Streaming is for the
seconds it takes to run the commands. It is not for the minutes it takes to read
the code.

While the deck waits for the next group the window shows a pulsing dot and the
words *writing group 2*. That is the reader watching you think. A group takes
about half a minute to read, so if the next one is further away than that they
are sitting in front of a spinner — which is the one way this tool costs
somebody time instead of saving it.

So do the reading first. Decide every group and every ref, and only then start
writing. **If you open a file to work out what group three says after the deck
is already on screen, you opened too early.**

Batching every group before opening is the other mistake, and the smaller one. A
deck is a directory that fills up while it is read, and the shape to aim for is:
plan fully, write group one, open, then the rest with nothing in between. If the
story is too big to hold in your head like that, it is several decks and not one
— see **Small decks** above.

`deck open <path> --now` skips the bar and puts the deck on screen at once. Use
it only when they have asked to be shown something *now*.

## Reading the answer

```json
{ "v": 1, "deck": "d-…", "comments": [
  { "group": "g1", "ref": "g1r1", "file": "src/batch.ts",
    "range": [143, 143], "source": "diff", "kind": "must-fix",
    "quote": "  pending -= 1;", "text": "This runs on the error path too." } ] }
```

- `kind` is `must-fix`, `question` or `nit`. Treat them differently: a
  `must-fix` blocks, a `nit` does not.
- `source` says how far to trust `range` — `extmark` and `diff` are exact,
  `fingerprint` is a good guess, `stale` means the line moved and could not be
  found. On `stale`, search for `quote` instead of trusting the number.
- A comment with **no `ref` and no `range`** is about the group's claim rather
  than any line — often the most important one in the review. `quote` is the
  sentence it answers.

Answer every comment: fix what they objected to, and answer the questions fully.
A question in a comment usually means the deck did not carry the model far
enough. If the review changes your plan materially, present the revised plan as a
new deck rather than describing the change in prose.

**An exit code of 0 with no comments is approval** — they walked the deck and
submitted with nothing to add, which is a deliberate act.

**Any other exit is not.** Exit 4 means they closed the deck without answering;
they may not have read it at all. Never turn that into agreement — do not say the
review came back clean, do not record a verdict, do not move on to the next
thing. Say plainly that the deck went unreviewed and ask whether to reopen it.
