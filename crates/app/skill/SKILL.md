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

## Read the mode before you write

Getting this wrong makes the tool annoying instead of useful.

- **Ship** — they are working fast: *show me my change*, *review this PR*, a
  routine diff they want to approve and move on from. Point crisply, give the
  reason in a line, let the review loop close it.
- **Teaching** — they are learning: a bug being chased, a new area, *how does
  this work*, *walk me through*. **Make them predict before you reveal.** Point
  at the code and, in the `say`, ask what they think happens — what breaks, what
  they would change — and answer it in a later group. Somebody handed the
  conclusion up front learns less than somebody who guessed first and found out
  they were right. End on the open question or the decision, not on *and that's
  it*.

  One check on a teaching deck before you write it: **does the story make them
  reason, or only receive?** If every group reveals top-down with nothing to
  predict, you have written a lecture. Find the place where you can ask first.

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

**Tight ranges, but whole ones.** A two hundred line range is not a highlight,
it is a shrug. A range that stops in the middle of the function it is pointing at
is worse — the reader sees an opening brace and no closing one, and spends their
first seconds working out what they are looking at instead of reading it.

So: the smallest range that is still a **complete thing**. A whole function, a
whole block, a whole match arm — opening line through closing brace, including
the signature, because a body without its signature is a body without its name.
If the function is ninety lines and only eight matter, that is a sign the eight
are the thing: point at them and say which function they are in. Never split the
difference by stopping halfway.

Count the closing line rather than guessing it. `140-148` that should have been
`140-151` is the single most common way a pane comes out looking careless.

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

## Write it so nobody wants to leave

The commands are the easy half. The writing is the product, and one worked
rewrite teaches it faster than a list of rules:

> The `pending` counter is decremented within the catch block in addition to
> the success path, resulting in an incorrect decrement when write operations
> fail.

> `pending -= 1` runs on the error path too. So every failed write takes the
> counter down twice, and the batch calls itself finished while eight rows are
> still in the air. **That is the 63.**

Same fact. The second one is shorter, it names the thing the reader came for,
and it ends where they will want to argue. Four habits get you there.

**Plain words, and one idea per sentence.** *Runs twice*, not *is invoked on
multiple code paths*. When you catch yourself writing *and*, *which*, or *, so
that*, put in a full stop and start again. If a sentence needs reading twice, it
is your sentence that is wrong, not the reader.

**Never explain the code — explain what the code is doing.** They can see
`pending -= 1`; it is on screen beside your words. What they cannot see is the
consequence. Point at the line and say what it costs.

**Explain it in terms of what is on their screen.** The comparison that lands is
the one from their own codebase — the other place this pattern already appears,
the sibling that does it correctly, the thing they built last month that works
the same way. A generic metaphor makes them translate; a reference to the queue
they already know makes them recognise. Reach for the domain they work in before
you reach for cars, restaurants or plumbing.

**Tell it like a story at a whiteboard, not like a changelog.** Group by group,
point by point, each one carrying the last one forward. You are walking somebody
through something, not reading them a list of what changed. Keep the words
simple; the thing being explained is complicated enough.

**Name things the way the codebase does.** *The counter*, *the retry*, *the
queue* — one word per thing, all the way through. A concept renamed halfway is a
concept the reader loses.

**Be light, never clever.** A dry line is welcome. A pun that has to be worked
out is a speed bump. Warmth is in the plainness: *this one is my fault* reads
better than any wordplay.

And three things about the shape of the whole deck.

**Open at the surprise.** The first group makes them want the second. Start
where the story turns — the line that does the wrong thing, the assumption that
does not hold — not with three groups about where the file lives.

**Make it inevitable.** By the last group they should feel they could have got
there themselves. Each group is a step they take with you. If a group needs
something you have not shown yet, it is in the wrong place.

**Say the awkward part out loud, and end with something to do.** *I am not sure
this is right.* *This is the ugly bit.* A deck that admits its own weak spot is
trusted everywhere else, and it is the sentence that makes them comment. Then
finish on a question or a decision — a deck that ends in *and that is it* is a
lecture, and lectures do not get comments.

Then **read it back** as if you had not written it. Does each group say the why,
would a stranger follow it, is there a shorter word. That pass is the difference
between a deck people walk and a deck people close.

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
