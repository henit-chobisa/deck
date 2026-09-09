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

## Do these in order

The order is the difference between a deck that feels instant and one nobody
waits for. It is **not** the order the commands are explained in below.

1. `deck new` — the title comes from the question you were asked, so this runs
   before you have read a line of code.
2. `deck open` — **straight away, with nothing in the deck yet, and as a
   background command.** It holds the window for as long as the window is up,
   so run in the foreground it never returns and you never write a group. A bar
   appears at the bottom of their screen: your title, and that something is
   being written. Open stays dark until there is a group to read, so opening
   this early is safe.
3. **Now** go and read the code.
4. `deck group`, one command per claim, as each becomes ready. The bar counts
   up and the Open button comes up to colour when the first one lands.
5. `deck seal` after the last one.
6. `deck wait`, in the background, and end your turn.

**The mistake this prevents** is researching in silence and then writing four
groups before showing anything. From the other side of the screen that is two
minutes of nothing followed by a finished deck. A tool that makes people wait is
a tool they stop reaching for — they should see the bar within seconds of
asking, and every group should land the moment it is written.

**The mistake at the other end** is opening and then going back to read code, so
the deck sits on *writing group 2* while you think. Between groups the window
shows a pulsing dot, and that dot is the reader watching you think.

Both come from the same confusion, and one rule settles it: **research, then
write.** The bar goes up before the research. The groups go in after it, back to
back, because by then you know what they all say.

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
or for an answer that is not about code at all — how to install something, what
a command does, whether to ship on Friday. A deck for a typo is a window in
somebody's face for no reason. If you are unsure, ask in one line.

A *question* is not a reason to skip it. "How does X work" is the best thing a
deck is for. What disqualifies a question is having no code in the answer, not
its being a question.

## The two modes (do not be heavy-handed)

Not everything is a lesson. Read which one they are in and match it — getting this
wrong makes the tool annoying instead of helpful.

- **TEACHING mode** — they are learning: a bug they are chasing, a new area, "how does
  this work", "walk me through", anything where understanding is the goal. Use the full
  predict → why → decide shape below. Make them reason. This is the mode the enhancements
  are for.

- **SHIP mode** — they are working fast: "show me my change", "present this PR", a
  routine multi-file diff they want to approve and move on. Here they want to SEE the
  change clearly and act, not be Socratically quizzed on their own code. Point crisply,
  say the why in a line, let the review loop close it. Do NOT force predict-then-verify
  on a 2-file refactor — that's friction, not help.

When unsure which mode, glance at what they asked: a question ("why does X…", "how does…")
is teaching; an imperative about their own work ("show my change", "present this") is ship.
When still unsure, ask in one line, or default to ship and let them pull you deeper.

## Writing one

```
deck new --title "The batch counter stalls at 63" --total 3
```

Prints the deck's path. Everything after this takes it.

`--title` is the **question or the claim the deck answers**, in sentence case —
not a label. *The retry backoff is unbounded*, not *backoff changes*. It is read
cold, on a bar at the bottom of the screen, before anything else about the deck
is visible.

`--total` is how many groups you mean to write, so the bar can say *2 of 3*
while the rest are on their way. You are running this before the research, so it
is a guess — give one anyway. It is a progress hint, not a promise, and writing
a fourth group to a deck that said three is fine.

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
- `--diagram <file.json>` adds a picture, for structure that is in no single
  file. **Not instead of the refs** — one group takes both, and for anything
  shaped like a flow that is the normal thing to do:

  ```
  deck group <path> \
    --say "The click never reaches the controller when the app is missing." \
    --diagram connect.json \
    --ref "web/silo/connect.tsx:40-52 the check that returns early"
  ```

  See **When the answer is a picture** below — do not skip it. A flow question
  answered with refs alone is a deck that did half the job.

```
deck seal <path>
```

Says there is no more coming.

## What a group is

One **group** = one thing you want to say. Refs inside a group are the evidence for
that one claim, not a pile of related files.

Rules:

- **If your prose names a location, that location must be a ref.** Writing "line 116"
  or "the enum above" and not showing it is the single most common way to waste this
  tool — they are looking at a screen that does not contain the thing you just cited.
  This rule is mechanical: cite it, show it.
- **The `say` carries the WHY.** A group that only states what-changed, with no reason,
  will make them ask "but why" — so answer it in the group. Fact + reason, every time.
- **A group with one ref should be a decision, not a default.** One ref is right when
  the point *is* that one thing. It is wrong when the refs that prove your claim exist
  and you just listed them in separate groups. Before writing, look for the pairs:
  an enum and the column that stores it, a writer and the reader that consumes it, a
  function and its only caller. Those belong on screen together — that is what the
  grid is for. They learn from seeing the RELATIONSHIP, so put the two related things
  on screen at once.
- Keep a group to **4 refs or fewer**. More is allowed — the group just splits into
  pages they have to walk — but four panes is already a lot to hold in your head,
  and a fifth ref is usually a second group wearing a disguise.
- One file per pane. Do not show one file in two panes. When a group has several, they
  should be several files whose relationship is the point.
- The grid is rows-biased and collapses to a single column on a narrow screen, so
  every ref stays readable. You do not lay anything out; just send the refs.

**Ranges are 1-based and inclusive, and must be tight either way you name them.** A
200-line range is not a highlight, it is a shrug.

But a range that stops in the middle of the function it is pointing at is worse — the
reader sees an opening brace and no closing one, and spends their first seconds working
out what they are looking at instead of reading it. So: the smallest range that is still
a **complete thing**. A whole function, a whole block, a whole match arm — opening line
through closing brace, including the signature, because a body without its signature is
a body without its name. If the function is ninety lines and only eight matter, that is
a sign the eight are the thing: point at them and say which function they are in. Never
split the difference by stopping halfway. Count the closing line rather than guessing it.

**Say whether a claim is proven.** When a group asserts something is correct, safe or
fixed, say *how it is known* — *tests green, shown below*, *this path is exercised by
X* — or say honestly that it is not verified yet. Never let a deck imply a green
checkmark that does not exist. Point at the passing test as a ref when there is one.

**Small decks.** Three tight decks beat one sprawling one. A change across five files
is several decks, not one with everything in it.

### Check before you write the first group

You write groups one at a time, but you plan the whole story before `deck new` — that
is where `--total` comes from. So do the count now, on the plan: **if every group has
exactly one ref, you have planned a list, not a presentation.** Go back and find what
belongs beside what. A deck of single-ref groups makes `next page` and `next group` do
the same thing and means they never see two pieces of evidence at once, which is the
entire point.

And one more plan check, for teaching decks: **does the story make them reason, or just
receive?** If the deck reveals everything top-down with nothing for them to predict, you
have written a lecture. Find the spot where you can point at the code and ask them first.

## When the answer is a picture

Some things are in no file at all. How a click reaches a controller, what calls
what, the order four services touch one request, the states a job moves through
— that is structure *between* files, and a code ref cannot point at it.

**A question about a flow is a diagram.** *How does X work under the hood*, *what
happens when I press this*, *walk me through the request*. If answering means
naming three or more places and the order they run in, draw it. A deck that
answers a flow question with six code refs and no picture has made the reader
assemble the diagram in their own head — which is the work they asked you to do.

**Put the picture and the code in the same group.** The diagram says *where* in
the flow you are, the refs say *what* the code does there — apart they are two
things to hold, together they are one. Give the diagram a group of its own only
when it maps a whole story that later groups then walk.

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

`--diagram` can be given more than once and mixes freely with `--ref`. The one
thing you do not control is the arrangement: refs are laid out before diagrams
whatever order you typed them. If the picture has to be read first, it needs a
group of its own.

**There are no colours, sizes or positions, on purpose.** A node says what it is
and how much it matters; deck owns every pixel. You cannot make it prettier, only
clearer, so spend the effort on what goes in it.

**Six to ten nodes.** Twenty is a wall, and a reader hunting for the entry point
has lost the one thing a picture is for. If it will not fit, it is two diagrams.

**Every node earns its place.** Cut the ones that only pass a value along. A
diagram is the *shape* of the flow, not a call graph.

## How to explain

The reader wants to *understand*, not be handed conclusions. Present the way people
actually learn:

1. **Predict before reveal.** The method is predict-then-verify. When you are
   teaching (a bug, how something works, a design), the FIRST group should often make
   them reason before you explain — point at the code and, in the `say`, ask what they
   think happens / why it breaks / what they'd change, *before* the later groups reveal
   it. A deck that hands them the answer up front teaches them less than one that makes
   them predict, then confirms. (For a routine "here's my change, approve it" — skip
   this; see the two modes above.)

2. **Always the WHY, never just the what.** People catch half-baked explanations and
   push until the full mental model is there — so give it to them the first time.
   Every group's `say` should carry the *reason*, not just the *fact*. "This column
   goes" is a shrug; "this column goes because the enum above is the only thing that
   names its valid values, and we're deleting the enum" is the model. If a group only
   says *what*, it is not done.

3. **Anchor to THEIR system and domain.** They think in their real codebase — the
   schedulers, workflows and services they actually work on — not toy examples. Use
   analogies from what is actually on screen and from their domain. "This is the
   straggler problem, like in a worker pool" lands harder than a generic metaphor.

4. **Is the claim PROVEN, or just plausible?** The standing edge: people accept "done"
   by reading, not by running. A review deck is exactly where that bites — they can walk
   it, nod, and never notice the claim was never verified. So when a group asserts a
   change is correct/safe/fixed, the `say` should say HOW it's known — "tests green,
   shown below" / "this path is exercised by X" — or flag honestly that it is not yet
   proven. Never let a deck imply a green checkmark that does not exist. Point at the
   passing test as a ref when one exists.

5. **End on a question or a decision when teaching.** They want to drive, not spectate.
   The last group of a teaching deck should hand them the wheel: the open question, the
   trade-off to weigh, the thing to decide — not "and that's it." For a plain change
   deck, the natural close is "approve or comment", which the review loop already gives.

6. **Story, not a file dump.** Adhere to what you're actually saying. Present it like a
   story, group by group, point by point, each deck a part of the story. Keep language
   simple; do not over-complicate. Imagine you are walking them through it at a
   whiteboard, not reading them a changelog.

7. **Small decks, fast feedback.** Smaller deck, faster loop. Communicating a change
   across 5 files? Show it in small chunks (one removed function and its impact),
   not everything with context mixed in. They'd rather walk three tight decks than one
   sprawling one.

8. **One file per page when panes relate.** Don't show one file in multiple panes. If a
   page uses multiple panes, show one file per pane where a change in one impacts the
   other — e.g. you removed a function, now one callsite per pane per file.

## The two commands that do not return

```
deck open <path>          # run this as a background command
```

**This one blocks.** It is the window: it runs for as long as the bar or the
deck is on screen, which is minutes, and in the foreground it will simply never
come back. Run it in the background the same way you run `deck wait` — those are
the two commands here that do not return, and they are the two you background.

It does not need the deck to have anything in it. A bar appears at the bottom of
the screen with your title on it, and the reader opens it when *they* are ready,
which is not the same moment as you being ready to ask.

Run it **second**, right after `deck new`, before the research — and then **never
again for that deck**. The bar tells them something is coming; its Open button
stays dark until the first group lands and then comes up to colour, so an empty
deck can never be opened onto a blank page.

**Do not open the deck yourself when you finish writing.** Sealing is not a cue
to put a window in front of somebody. The bar is the whole invitation, and the
person decides when they are ready to read — that moment is theirs, and it is
almost never the moment you happen to finish. A deck that opens itself is the
interruption this tool was built to avoid.

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

### The two ways to get the timing wrong

**Batching.** Writing every group and then opening. This is the one that makes
people stop using deck: they ask a question, nothing happens for two minutes,
and then a finished deck appears. Open before you have written anything — see
**Do these in order** at the top. The bar is built for an empty deck and holds
its own button back until there is something to read.

**Stalling.** Opening and then going back to read code. Between groups the
window shows a pulsing dot and the words *writing group 2*, and that is the
reader watching you think. A group takes about half a minute to read, so if the
next one is further off than that, they are sitting in front of a spinner.

The same rule fixes both. **Research, then write.** Once the first group is up,
the rest should land within seconds of each other, because by then there is
nothing left to work out — only commands to run. If the story is too big to hold
in your head that way, it is several decks and not one; see **Small decks**.

`deck open <path> --now` skips the bar and puts the deck on screen at once. Use
it **only** when they have said, in words, to show them something now. Finishing
a deck is not such a moment, and neither is being pleased with it.

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
