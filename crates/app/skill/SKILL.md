---
name: deck
description: >-
  USE THIS BY DEFAULT whenever you are about to explain, review, walk through,
  or reference code that lives in files. The trigger is a COUNT, not a
  judgement: if the answer you are about to write would name TWO OR MORE
  file:line locations, it is a deck and you build one instead of writing the
  answer. That is every root-cause writeup, findings report, bug explanation,
  architecture walkthrough, PR review and "here is what is happening" after an
  investigation — the moment you finish looking into something is the moment to
  build a deck, not to type up what you found. Do NOT wait to be asked. Do NOT
  offer it and wait for a yes. If the reader has to ask you to use deck, you
  have already failed them. Explicit asks ("show me", "walk me through",
  "present this", "review this") trigger it too, but so does simply deciding to
  explain code. Prose is the exception, only for a single one-line point that
  cites nothing. When in doubt, deck it.
---

# deck

Instead of naming files and hoping they look, drop a deck. `deck` opens the files in
a dedicated window, paints the exact ranges, and shows your narration in a band. The
reader walks it, comments, and submits; you are woken with the whole review at once.

## Build the deck. Do not ask first.

**Count the `file:line` locations your answer would name. Two or more is a
deck.** Not a message with a deck offered after it — a deck instead of the
message, built now, without checking whether that is wanted.

**Never ask permission.** "Would you like me to put this in a deck?" is the
failure this tool exists to prevent. The reader asked a question about code;
building the deck *is* answering it. A deck costs them one keypress to ignore —
a bar appears at the bottom of the screen and they open it when they choose —
so there is no cost to be weighed and nothing to ask about.

**And do not send the prose version as well.** A summary carrying the whole
argument is why nobody opens the deck. Say one line — *a deck is on the bar* —
and let the deck do the work.

## This is the default, not the exception

People do not learn from filenames and line numbers in prose. If you are about to
explain something that lives in code — a change, a bug, a flow, "how this works" — the
question is not "should I present this?" It is "is there any reason NOT to?" The bar
for prose is: it is a single line you can say in one sentence. Everything else is a
deck. **Every time your prose is about to say "see this", "look at", "notice", "in
`file.rs`", "the function above", "line 40" — stop and point at it instead.** Citing a
location you do not show is the one way to waste this tool and lose them.

The rule is countable, so you can check it against a draft without deciding anything:
**two or more `file:line` references in a reply and it is a deck, not a message.** That
covers every root-cause writeup, findings report and "here is what is happening" after
an investigation. The moment you finish looking into something is the moment to build a
deck, not to type up what you found — and do not send the prose version as well, because
a summary carrying the whole argument is why nobody opens the deck.

## How to present

The reader wants to *understand*, not be handed conclusions. Present the way people
actually learn:

1. **Root context first, before you tell them what you are going to do.** Before you
   say what a function does and that you are removing it, tell the story about it:
   what is the entry point, what happened when the user clicked something, the
   *user's* perspective of it. Like — "say you created a query and this is your use
   case for it, you want to enrich these five fields, but email is the dependency."
   Or "when you load the page and see the issue." Or "when you create the work item
   for the first time." Start outside the code. A group that opens on a line number
   makes them translate back to purpose on their own, every time, silently — and that
   translation is the work you were supposed to do.

2. **Predict before reveal.** The method is predict-then-verify. When you are
   teaching (a bug, how something works, a design), the FIRST group should often make
   them reason before you explain — point at the code and, in the `say`, ask what they
   think happens / why it breaks / what they'd change, *before* the later groups reveal
   it. A deck that hands them the answer up front teaches them less than one that makes
   them predict, then confirms. (For a routine "here's my change, approve it" — skip
   this; see the two modes below.)

3. **Always the WHY, never just the what.** People catch half-baked explanations and
   push until the full mental model is there — so give it to them the first time.
   Every group's `say` should carry the *reason*, not just the *fact*. "This column
   goes" is a shrug; "this column goes because the enum above is the only thing that
   names its valid values, and we're deleting the enum" is the model. If a group only
   says *what*, it is not done.

4. **Anchor to THEIR system and domain.** They think in their real codebase —
   schedulers, workflows, job-scheduling, durable execution — not toy examples. Use
   analogies from what is actually on screen and from their domain. "This is the
   straggler problem, like in a worker pool" lands harder than a generic metaphor.

5. **Is the claim PROVEN, or just plausible?** The one standing edge: people accept
   "done" by reading, not by running. A review deck is exactly where that bites — they
   can walk it, nod, and never notice the claim was never verified. So when a group
   asserts a change is correct/safe/fixed, the `say` should say HOW it's known — "tests
   green, shown below" / "this path is exercised by X" — or flag honestly that it is not
   yet proven. Never let a deck imply a green checkmark that does not exist. Point at
   the passing test as a ref when one exists.

6. **End on a question or a decision when teaching.** They want to drive, not spectate.
   The last group of a teaching deck should hand them the wheel: the open question, the
   trade-off to weigh, the thing to decide — not "and that's it." For a plain change
   deck, the natural close is "approve or comment", which the review loop already gives.

7. **Story, not a file dump.** Adhere to what you're actually saying. Present it like a
   story, group by group, point by point, each deck a part of the story. Keep language
   simple; do not over-complicate. Imagine you are walking them through it at a
   whiteboard, not reading them a changelog.

8. **Small decks, fast feedback.** Smaller deck, faster loop. Communicating a change
   across 5 files? Show it in small chunks (one removed function and its impact),
   not everything with context mixed in. They'd rather walk three tight decks than one
   sprawling one.

9. **One file per page when panes relate.** Don't show one file in multiple panes. If a
   page uses multiple panes, show one file per pane where a change in one impacts the
   other — e.g. you removed a function, now one callsite per pane per file.

## The two modes (do not be heavy-handed)

Not everything is a lesson. Read which one they are in and match it — getting this wrong
makes the tool annoying instead of helpful.

- **TEACHING mode** — they're learning: a bug they're chasing, a new area, "how does this
  work", "walk me through", anything where understanding is the goal. Use the full
  predict → why → decide shape above. Make them reason. This is the mode the enhancements
  are for.

- **SHIP mode** — they're working fast: "show me my change", "present this PR", a routine
  multi-file diff they want to approve and move on. Here they want to SEE the change
  clearly and act, not be Socratically quizzed on their own code. Point crisply, say the
  why in a line, let the review loop close it. Do NOT force predict-then-verify on a
  2-file refactor — that's friction, not help.

When unsure which mode, glance at what they asked: a question ("why does X…", "how does…")
is teaching; an imperative about their own work ("show my change", "present this") is ship.
When still unsure, ask in one line, or default to ship and let them pull you deeper.

## The loop

A deck is **streamed**, one group at a time. Writing a whole deck takes you tens of
seconds, and they should not spend them looking at nothing.

```
you  -> deck new       the header. THE BAR APPEARS NOW.
you  -> deck open      immediately, in the background, with nothing in the deck yet
you  -> deck group     they start reading THIS group
you  -> deck group…    written while they read the first
you  -> deck seal      no more coming
you  -> deck wait, in the background, TURN ENDS
they -> walk the deck, comment, submit
you  -> woken automatically when the command exits
```

Their first page arrives after ONE group, not after all of them. Their reading time and
your writing time overlap, which is the whole point.

You do not poll and you do not ask them to tell you when they're done. `deck wait`
exiting re-invokes you on its own.

**Do the reading first.** The bar goes up before the research; the groups go in after it,
back to back. Researching in silence and then writing four groups before showing anything
is two minutes of nothing from the other side of the screen. Opening and *then* going
back to read code is the same mistake at the other end — between groups the window shows
a pulsing dot, and that dot is the reader watching you think.

## 1. Open with the header

```bash
deck new --title "why the batch counter stalls" --total 4
```

It prints the deck's path. Everything after this takes it.

- `--cwd` is the project the deck is about, and defaults to the working directory.
  Every relative path in a ref resolves against it.
- `--total` is how many groups you intend to write. The band counts against it —
  `group 1/4 · writing 2…` — so they know how much deck is still coming. You are running
  this before the research, so it is a guess; give one anyway. Getting it slightly wrong
  is fine; deck trusts what actually arrives.
- `--title` is the first thing they read. Make it the QUESTION or the CLAIM the deck
  answers ("why the batch counter stalls"), not a label ("batch counter changes").
  A title that poses the question primes them to predict — which is how people learn best.

## 2. Put the bar up, before you have written anything

```bash
deck open <path>          # run this as a background command
```

Run it **second**, right after `deck new`, before the research — and then never again for
that deck. A bar appears at the bottom of the screen showing the title and `writing group
1…`. Everything after this point is them waiting on *less and less*.

The Open button stays dark until the first group lands, so opening this early is safe.

**You never open the deck itself.** There is no flag for it and there must be no attempt
at one. The bar is the whole invitation; when to read is theirs to choose, and it is
almost never the moment you happen to finish. Sealing a deck is not a cue to put a window
in front of somebody.

## 3. Write one group per tool call

```bash
deck group <path> \
  --say "Markdown. The thing you are actually saying — write it properly.

Both panes are ONE claim: the enum names the modes, the column stores one.
Delete a value and the column has nothing left to say." \
  --ref "apps/api/models/query.py:14-22 one of these two survives" \
  --ref "apps/api/models/query.py:88-91 ...so this column goes"
```

**One Bash call per group. Never batch them.** Putting two groups in the same call, or in
the same parallel block, means neither lands until you have generated both — which throws
away the entire reason this is a directory. Write the first, then think about the second.

They are on group 1 while you write group 2. If they walk past the last group you have
written, deck parks them there and opens the next one by itself the moment it lands, so
you never need to tell them to press anything.

- `--say` is Markdown, inline only: `**bold**`, `*emphasis*` (painted in the accent, so it
  means *look at this word*), `` `code` ``, and a blank line between paragraphs.
- `--ref` is `file:first-last`, a space, then a short note. A bare number is one line.
  `*starred*` words in the note are accented. Give it once per pane.
- `--diagram <file.json>` adds a picture. See §4.

Rules:

- One **group** = one thing you want to say. Refs inside a group are the evidence for
  that one claim, not a pile of related files.
- **The `say` should name what is on screen and why those things are there together.**
  "Both panes are ONE claim: the enum names the modes, the column stores one" tells
  them what they are looking at before they look. A `say` that never mentions its own
  panes leaves them to work out why two files are side by side, which is the thing you
  put them there to show.
- Ranges are 1-based and inclusive, and must be tight either way you name them. A
  200-line range is not a highlight, it is a shrug.
- **But a range must be a whole thing.** One that stops in the middle of the function it
  points at is worse than one that is too long — the reader sees an opening brace and no
  closing one. Smallest range that is still complete: a whole function, a whole block, a
  whole match arm, signature through closing brace. If the function is ninety lines and
  eight matter, point at the eight and say which function they are in. Count the closing
  line rather than guessing it.
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
- The grid is rows-biased and collapses to a single column on a narrow screen, so
  every ref stays readable. You do not lay anything out; just send the refs.

### Check before you write the first group

You write groups one at a time, but you plan the whole story before `deck new` — that is
where `--total` comes from. So do the count now, on the plan: **if every group has exactly
one ref, you have planned a list, not a presentation.** Go back and find what belongs
beside what. A deck of single-ref groups makes `next page` and `next group` do the same
thing and means they never see two pieces of evidence at once, which is the entire point.

And one more plan check, for teaching decks: **does the story make them reason, or just
receive?** If the deck reveals everything top-down with nothing for them to predict, you
have written a lecture. Find the spot where you can point at the code and ask them first.

## 4. When the answer is a picture

Some things are in no file at all. How a click reaches a controller, what calls what, the
order four services touch one request, the states a job moves through — that is structure
*between* files, and a code ref cannot point at it.

**A question about a flow is a diagram.** If answering means naming three or more places
and the order they run in, draw it. A deck that answers a flow question with six code refs
and no picture has made the reader assemble the diagram in their own head — which is the
work they asked you to do.

**Put the picture and the code in the same group.** The diagram says *where* in the flow
you are, the refs say *what* the code does there. Apart they are two things to hold;
together they are one.

```bash
deck group <path> \
  --say "The click never reaches the controller when the app is missing." \
  --diagram connect.json \
  --ref "web/silo/connect.tsx:40-52 the check that returns early"
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
- `role` — `step` (default), `decision`, `store`, `terminal`, `actor`. What the thing
  *is*. It decides the shape.
- `weight` — `normal` (default), `accent`, `muted`. How much it matters. One accented
  node is usually enough.
- `note` — one line under the label. A file path or a qualifier, not a sentence.
- `lane` — an integer pinning a node to a column, so a sequence keeps its tracks.
- `line` on an edge — `solid` (default), or `dashed` for a conditional one.
- Edge `label` is a word or two: `POST /connect`, `on submit`. Longer gets cut.
- `clusters` are containment, for a boundary worth seeing: browser against server.
- Cycles are fine. The returning edge is drawn as one that visibly comes back.
- `flows` are paths through the picture the reader can play, and they are how one
  diagram answers more than one question. Each is `{ "name", "steps" }` where the
  steps are node ids in the order they happen. A button per flow sits in the
  drawing's top-right; pressing it sends a current along the path while
  everything else recedes.

  Give a flow a `color` when there are two, so they can be told apart without
  pressing both. A step can be written `{ "node": "x", "color": "#c33" }` to
  light one stop differently from the rest — the place it goes wrong, in a path
  that is otherwise calm.

```json
"flows": [
  { "name": "cancel check", "steps": ["run", "worker", "cache"] },
  { "name": "re-run", "steps": ["run", "queued"] }
]
```

  Reach for them when a picture holds two paths that a reader would otherwise
  have to trace with a finger — the happy path and the one that stalls, the
  first run and the second. Two flows over one diagram beat two diagrams,
  because the thing being compared is what they share.

There are no colours, sizes or positions, on purpose. A node says what it is and how much
it matters; deck owns every pixel. **Six to ten nodes** — twenty is a wall. Every node
earns its place: cut the ones that only pass a value along.

Code refs are always laid out before diagrams whatever order you typed them. If the
picture has to be read first, it needs a group of its own.

## 5. Seal it

```bash
deck seal <path>
```

Until this lands, deck tells them groups are still coming and warns them if they try to
submit. Forget it and they get warned about groups that were never going to arrive.

## 6. Arm the waiter, then stop

```bash
deck wait <path>          # run this as a background command
```

Run it with `run_in_background: true`. Then **end your turn** and say nothing further —
they are reading, not waiting on you.

**Do not run `deck wait` in the foreground.** A review takes minutes, a foreground command
has a timeout, and a reader cannot be hurried.

### The waiter can end without a review, and you must be able to tell

A backgrounded waiter also ends when they close the deck without answering, when they
interrupt the turn, or when the session is killed. The exit code is how you tell them
apart:

| exit | means |
| ---- | ----- |
| 0 | they submitted; the review is on stdout |
| 4 | they closed the deck without answering |
| 3 | no review yet — only from `--timeout`, which you should not need |
| 1 | something was wrong; the message on stderr says what |

- Exit 0 with JSON on stdout → a real review. Proceed.
- **Anything else is NOT a review.** Nothing was submitted. They may not have seen the
  deck at all.

When there is no review: say plainly that the deck went unreviewed, and ask whether to
re-arm or to walk it in the terminal instead. Re-arm at most once without being asked.

**Never convert an absent review into agreement.** Do not write "review came back with no
comments", do not record verdicts, do not treat silence as sign-off, and do not move to
the next stage. An interrupted waiter means they were doing something else, not that they
agreed with you.

## 7. When you are woken

The payload is the review:

```jsonc
{ "v": 1, "deck": "d-1788265010-8842",
  "comments": [
    { "group": "g1", "ref": "g1r1", "file": "src/batch.ts",
      "range": [122, 124], "source": "diff", "kind": "must-fix",
      "quote": "if (--pending === 0) finish()",   // use this to relocate if lines moved
      "text": "why does this assume sorted input?" } ] }
```

`range` is where the comment sits *now* (deck tracked it through any edits made while they
were reading). `quote` is the text it was pinned to when they wrote it. If the two
disagree, trust `quote` — search for it. `source` says how far to trust `range`: `extmark`
and `diff` are exact, `fingerprint` is a good guess, `stale` means the line moved and could
not be found.

`kind` is `must-fix`, `question` or `nit`. Treat them differently: a `must-fix` blocks, a
`nit` does not. A comment with **no `ref` and no `range`** is about the group's claim
rather than any line — often the most important one in the review.

Act on every comment. Answer the questions, fix what they objected to. Their comments are
often the real "but why" — a question in a comment means the deck did not carry the model
far enough, so answer it fully, not thinly. If the review changes your plan materially,
present the revised plan as a new deck rather than describing the change in prose.

An empty `comments` array **that arrived with exit 0** means they walked the deck and
pressed submit with nothing to add. That is a deliberate act, and you may treat it as
approval.

Any other exit means no such thing. See above: there was no review, and there is nothing
to approve. If you find yourself about to summarise verdicts they never typed, stop — you
are inventing consent.

Deck also refuses to submit a walk with unopened pages unless they confirm, so a real
review means they saw every page or explicitly chose not to.

## When not to use this

- A single-file, single-line point you can just say in one sentence.
- As a substitute for actually making the change. A deck is for the argument, not the diff.
- To Socratically quiz them on a routine change they just want to approve (ship mode). Read
  the mode; do not force teaching where they want speed.
- For a one-line fix, or for something they watched you do.
