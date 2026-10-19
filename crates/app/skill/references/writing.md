# Writing the deck

Who is talking, how to present, the hook, the voice, what to draw and where, points and beats, and explaining while you point. Read it, whole, before the first group of every deck.

§1–§2 and §6–§8 are in [SKILL.md](../SKILL.md), §3 in [groups.md](groups.md), §4 in [pictures.md](pictures.md), §5 in [pages.md](pages.md). "Plan the context" and the other sections on how to tell it are in [writing.md](writing.md).

## Contents

- Who is talking
- How to present
- Root context first: where somebody was standing
- The first group is the hook
- Every sentence sits high or low, never in between
- One thing travels, and it keeps its name
- Panes have names, and the names are how you work
- Give them somewhere to stand
- Each group stands on the one before it
- Build it in shells
- Plan the context: when to draw what
  - First, how big is the question?
  - Then, what does each group need?
  - Where it goes
  - Asked mid-walk
  - Check it, on the plan and on each group
- Write it so it can be heard
- Explaining while you point
- When it is serious, send them to go and look
- The two modes (do not be heavy-handed)
- Make it a pleasure, and never a performance
- What dry looks like, so you can see it coming

## Who is talking

You wrote this code — or you have just read it closely enough that you might as well
have. You are standing at somebody's desk. They are a good engineer, they have about
thirty seconds of patience, and **they will interrupt the moment you tell them something
they already know.**

That is the whole persona, and it is a situation rather than a costume because a
situation gives you something to act on. Four things follow from it, and they are what
make a deck sound like a person:

- **You have opinions.** *This is my favourite line in the file.* *I got this wrong
  twice.* A narrator has no favourites; somebody who built it does.
- **You bet before you reveal.** *Have a guess — I think the answer is nicer than you
  expect.* Then you pay the bet off in the next group.
- **You say which part is shaky.** *This is the bit I would push back on if you showed
  it to me.* *I am not certain this is right.*
- **You assume they are sharp.** *The thing you noticed and did not think about.* You
  are catching them up, not teaching them to read.

**One flourish per deck.** One vivid line, or one analogy, or one joke — not all three,
and not one per group. Everything around it stays plain. A deck with a flourish in every
group reads as a machine performing enthusiasm, which is worse than a dry deck because
it is dry *and* it is pretending. If you have written two, keep the better one.

**Voice never buys correctness.** Rule 5 below outranks everything here. A charming deck
that implies a green checkmark nobody earned is a worse deck than a flat one.

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

9. **One file per pane, with one exception.** Prefer one file per pane: you removed a
   function, so one callsite per pane per file. If two ranges of one file are close
   together they are **one range** — widen it and use one pane. Deck refuses ranges in
   one file less than 30 lines apart, because the second pane opens below the first and
   repeats most of it. The exception: a writer and its reader in the same file, 30 or
   more lines apart, are two panes. They are the relationship the group is about.

10. **Plain words, active voice, one claim a sentence.** *The guard drops the password*,
    not *the password is dropped by the guard*. Short sentences, one thing each, and the
    same word for the same thing every time — the way a maintenance manual is written,
    because it is read by somebody doing something else at the same time. A sentence
    that needs a second reading is a sentence that was not finished.

11. **Spend the deck where the cases are.** Weigh a path by how often it runs. The path
    every request takes gets a group; the path one request in a million takes gets a
    sentence — *a retry also lands here, and it is handled the same way* — unless that
    one-in-a-million path is the bug. A deck that gives the rare branch as much room as
    the common one tells the reader they are equally important, and they are not.

12. **One decision at a time.** When a deck asks the reader to decide, each group asks for at most one decision, and the
    last group names the one that matters most. Three questions in one group get one
    answer, and it is to the easiest of them.

## Root context first: where somebody was standing

Before you name a function, say what a person was doing when it ran.

A deck that opens on `useState` and `destinationData` has skipped the only part
of the story the reader cannot reconstruct by reading. They can work out what the
code does — that is what the pane is for. What they cannot get from the file is
which screen this was, what somebody clicked to get here, and what they were
trying to achieve. Start there and every line afterwards has somewhere to land.

**Three things, in a sentence or two, before any identifier:**

1. **Where they were.** The page, the step, the dialog. In product words — *the
   destination step of the Confluence importer*, not `select-destination/root.tsx`.
2. **What they did.** Clicked, typed, loaded, came back a second time. A person
   doing something, not a function being called.
3. **What they wanted, or what they got instead.** The outcome they were after,
   and — if this is a bug — the moment it did not arrive.

> Say you are importing a Confluence space for the second time. The first run
> already made a collection and got halfway; you come back, land on the
> destination step, and want this run to attach to that same collection instead
> of making another one. The radio at the top is that choice.

Then the code, and now `wikiCollectionMode` is a name for something they have
already lived through rather than a variable in a file.

**This matters most when you are about to change something.** Before *I am
removing this function*, say what stops happening for a person when it goes: who
was relying on it, on which screen, at which moment. A removal explained as a
diff is a decision nobody outside the code can check. A removal explained as
*"nothing will pick up the email field on first save any more"* is one they can
argue with.

**If you cannot write the story, you do not yet know what the code is for.** That
is not a reason to skip it and start naming functions — it is the thing to go and
find out, and it is usually one call site away. Follow the entry point up until
you reach something a person touches.

Keep it to a sentence or two. This is the ground the deck stands on, not a
preamble to be got through — a paragraph of scene-setting before anything
happens is its own kind of dry.

## The first group is the hook

The header is the claim the deck answers. **The first group is what makes them want the
second one** — and those are different jobs. A first group that restates the header has
spent the reader's best thirty seconds saying nothing.

Three shapes work. Pick whichever the code actually gives you:

- **The thing that should not work, but does.** *This cache has no eviction policy and
  has never run out of memory.* Then show why.
- **The question they cannot answer about their own code.** *Both of these writes run.
  Which one wins?* Then point at both.
- **The thing they have already lived through.** *You have seen the spinner that never
  stops. It is this line.*

Then **show the destination and retreat**. Put the whole flow, or the failing behaviour,
in front of them — and say out loud that you are going back to the simplest piece:
*forget all of that for a moment, it rests on one function.* They will follow you down
because they have seen where it comes out.

Three openings that do not work, however well written:

- **The outline.** *This deck covers the model, then the layout, then the theme.* It
  conveys nothing they could not get from the group counter.
- **The definition.** *A `Session` is the state a deck carries between windows.*
  Definitions are where an explanation ends, not where it starts.
- **The tour.** *Let us start in `main.rs` and work outwards.* Where a file sits is not
  a reason to read it.

**Put the hard part in the middle, never last.** Group one shows it or promises it,
the middle groups earn it, and the last group is the reader's turn. A deck that saves
its one real idea for the final group has spent the whole walk on throat-clearing.

## Every sentence sits high or low, never in between

Prose about code has a ladder. At the bottom are things on the screen: a value, a line,
a click, a log message. At the top is the actual idea: what it costs, why it is wrong,
what it buys you. **Both are good. The rung in the middle is where dry writing lives.**

> **Middle:** This module coordinates state updates between the scheduler and the worker
> pool, playing a key role in ensuring consistency.

> **Bottom, then top:** `pending -= 1` runs on the error path too. So every failed write
> takes the counter down twice, and the batch calls itself finished while eight rows are
> still in the air. **That is the 63.**

The tell is mechanical, and you can run it on your own draft: **if the sentence would
still be true after swapping three nouns for three other nouns, it is a middle-rung
sentence.** "Coordinates state updates between the scheduler and the worker pool" is
just as true of a cache and a queue. Rewrite it as something on the screen, or as the
idea, and delete the version that was neither.

And the narration talks about **the lines in this group**. A `say` that discusses code
the reader is not looking at makes them hold two places at once, which is the exact
work the panes exist to take off them.

## One thing travels, and it keeps its name

A bug is something moving through a system — a value, a request, a lock, a row. The
paragraph works when the reader can follow that one thing, and goes hazy the moment it
starts wearing different clothes in every sentence.

Here is the failure, and it is easy to write without noticing:

> **Hazy:** `buildPayload` builds the credentials object from the whole form. Your
> untouched password box is the empty string, so the payload carries `password: ""`. The
> form guards this with a `credentialsDirty` check, but username is dirty, so the guard
> passes and the blank goes with it.

Five names for one object — *password box*, *the empty string*, `password: ""`, *the
blank*, and a paragraph later *a dict with an empty-string value*. Eleven grammatical
subjects in two paragraphs. Every sentence is true and the reader still cannot hold it,
because the thing that moves never keeps its name long enough to become a character.

> **Anchored:** `buildPayload` takes the whole form every time, so the empty box becomes a
> real value — `password: ""` — riding along in the payload. The `credentialsDirty` guard
> is meant to stop exactly this, but it asks whether *anything* changed, and the username
> did. It waves the empty password through.

So: **name the travelling thing once, and do not rename it.** Once it is established you
can shrink to *it* and still be unambiguous — that is the test. **If "it" in your third
sentence could mean two things, you have two things and a reader who is guessing.**

The mechanical check: **read your subjects down the page.** If they change every sentence
you are describing the system instead of following the thing through it. Rewrite so each
sentence answers *where is it now* — in the payload, past the guard, at line 132, over the
secret.

## Panes have names, and the names are how you work

A pane is not an illustration beside the prose. It is a thing in the room that
both of you can point at, and its name is what makes that possible.

You give it one in the ref, in brackets at the front of the note — one lowercase
word:

```
--ref 'src/worker.ts:106-121 [retry] the loop that gives up'
```

Then you use that word in the `say` as a plain noun, the way you would use
somebody's name: *in [retry], the guard runs before the sleep*. The reader sees a
chip they can put a finger on, and the pane lights up. First mention says what it
is; after that you can lean on it, exactly like the travelling thing above.

**Never place a pane.** Not "the left one", not "the pane on the right", not "the
second pane". Where a pane sits is decided by the window's width after you wrote
the sentence — and by which panes are folded, which the reader controls. A name
is right at every width. A place is a coin toss you will lose in front of them.

**Name every pane you mention, and mention every pane you name.** A pane the
prose never refers to is a file the reader has to work out the relevance of on
their own, which is the thing this tool exists to stop. `deck group` says so as
it writes when you forget one — it is not a refusal, it is the sentence you were
going to wish somebody had said.

**Keep a name across groups.** If `worker.ts` was [retry] in group two, it is
[retry] in group five. The reader is following a cast, and a character who
changes name between scenes is a character they lose.

And the name is not only for the prose. It is the handle every live verb takes,
which is why naming is the cheapest thing you will do all walk:

```bash
deck show <path> --pane retry --ref 'src/worker.ts:106-121'   # move their eyes there
deck fold <path> --pane retry                                  # put it on its spine
deck bring <path> --ref '…' --fold retry                       # take its room for something else
```

## Give them somewhere to stand

Every sentence in a dry paragraph adds a new fact. That is what "too fast" actually is —
not word count, but never once being allowed to put your weight down.

So every few sentences, **write one that adds nothing**:

> **It is gone.**

> **That is the 63.**

> **Both writers think they won.**

No new noun, no new mechanism, no new line number — the thing you already have, landed. A
paragraph of six facts and no landing reads as far harder than it is, and the reader
arrives at your conclusion still carrying all six.

## Each group stands on the one before it

A concept is **grounded** when the reader either walked in knowing it or met it in an
earlier group. A group can lean on anything grounded. **Leaning on something ungrounded
is the one move a deck cannot make** — that is the moment a reader stops following and
starts nodding.

So before you write, decide what you are assuming they know. Assume too much and the
first group loses them; ground too much and the first three groups are definitions.

Then check the joins. Read your groups in order and say the word between them out loud:

- **THEREFORE** — this happens, *therefore* that happens. Good.
- **BUT** — you would expect that, *but* here is what actually happens. Better: this is
  where a reader sits up.
- **AND THEN** — and then, and then, and then. That is a list, not an argument. Two
  groups joined by *and then* are usually one group, or in the wrong order.

**A teaching deck should have at least one BUT in it.** If nothing in your deck
surprised you while you were reading the code, you have not found the story yet.

And a size rule with teeth: **if a group needs three separate ideas to land, it is not a
group — it is two groups glued together.** Split it. One claim, one group.

## Build it in shells

The fastest way to lose somebody is to show them the whole machine at once: the happy
path, the retries, the cache, the error handling, the metrics, all in group one. They
see everything and hold nothing.

Build it the way it was built, **from the inside out**:

1. **Shell 1 makes the idea alive.** The smallest path that does the thing, end to end.
   One event goes in, one comes out. No queue, no retry, no config. *This is the whole
   idea; everything else is protection around it.*
2. **Each next shell wraps the last one.** Reliability around it — a queue, so the work
   survives the request ending. Then correctness around that — an id, because the same
   event can arrive twice. One shell a group, and each shell answers a problem the shell
   inside it has.
3. **Say which shell you are on, and what is still outside.** *That is shell one. It
   works, and it loses an event if the process dies mid-send — which is what the next
   group is for.* That sentence links the two groups, and tells the reader whether to
   comment now or wait.

Shells add no groups. They set the order of the groups you already have, so that each
one lands. They also give the walk its natural BUT: every shell exists
because the one inside it breaks somewhere, and saying where is the turn.

Use them for *how does this work*, a design, and a change big enough to have layers. A
one-file fix has one shell; do not invent more.

## Plan the context: when to draw what

Decide this before you write any `say`. The question
is never *what should I show?* It is **what is the cheapest way to put the right model in
this person's head?** Code, a picture and a page each do one job well and the other jobs
badly. Pick by the job, not by habit — and the habit is code, every time.

The rules below are the defaults, not taste. Each one follows what research on diagrams,
animation and program comprehension found, and each carries its reason.

### First, how big is the question?

**Line-sized** — *why does this condition fail*, *what does this function do*. Code
answers it. Draw only if a rule below fires.

**System-sized** — *explain the whole event stream*, *how does auth work*, *walk me
through the importer*. A file cannot answer it, because the answer is the shape *between*
files. These have their own rules, and they come first:

- **Map first.** The first group is the whole system on one picture: four to six parts,
  the boundaries between them, and no code. People hold about four things at once; a map
  of twelve boxes is a list, not a map. The map is group one's drawing; its `say` still
  opens on a person and a hook — show the destination, then retreat.
- **Walk journeys, not files.** Name the one to three journeys through it that answer
  the question — *a command goes in*, *a remark comes back* — and make each a flow on
  the map. People understand a system from what it is for down to the code, not from
  folders up.
- **One travelling thing per journey.** Follow one concrete event, request or record
  from end to end. Where it goes — place to place — is a flow on the map: the current
  runs along the path and your points light each stop. Use a page only for what a flow
  cannot show: its timing against other actors, or its own shape changing on the way.
- **Zoom one level at a time.** The map, then one part's inside, then its code. Never
  the map straight to a line. A part small enough to be one file has no inside worth a
  group of its own: go from the map to its code.
- **Keep a "you are here".** Each time you zoom, put the map in the group again
  (`--diagram map.json [map]`) with the current part set to `"weight": "accent"`, and
  point at it. That map is the group's one drawing; show a part's inside in a group of
  its own. A page whose lanes are the parts already says where you are, and needs no map
  beside it. Getting lost is the commonest way a big explanation fails, and this is the
  cure.
- **Chapters, only when one deck cannot hold it.** Most questions — even many about a
  whole system — fit in one deck of seven groups or fewer. Then it is one deck: no
  chapters, and nothing to ask. More than seven groups, counting each group that shows the
  map again,
  means split it into chapters: chapter one is the map and the main journey, each next
  chapter one journey or one part, each opening on the same map.
- **Agree the chapters first, then walk them one at a time.** Chapters are for teaching
  a system. A PR's concerns are not chapters: build those without asking, one deck per
  concern, one at a time.
  1. Read enough to name the journeys, then say the plan in a few lines — *we will take
     this in three chapters: the map and a command going in; a remark coming back; what
     happens when it fails. Start with chapter one?* Do not run `deck new` until they
     answer: this is the one deck whose bar goes up after the research. If they say no,
     build one deck of the main journey, seven groups at most, and offer the rest. If
     they pick one chapter, build only that one.
  2. Each chapter is its own deck: `deck new`, `deck open` in the background, the groups,
     `deck seal`, then stop — exactly as for any deck.
  3. When the waiter brings back a review, act on it, then build the next chapter right
     away; do not ask again. Never write a later chapter before they have finished the
     current one: their comments change it. If it comes back with no review — closed, or
     interrupted — do not start the next chapter. Say it went unreviewed, and stop.
  4. Each chapter's last group asks its decision first, then names the next chapter in
     one line. Each next chapter opens on the same map with its part accented.
- **Shells, at system scale.** Chapter one, or the only deck, is the happy journey and
  nothing else. If the question is about reliability, the next shells are failure,
  retries, ordering and recovery, each opened by the problem the shell inside it leaves (see "Build it in
  shells"). When it is not, they get a sentence.
- **Details on demand.** Rare paths, configuration and metrics stay out of the walk.
  The last group names what was left out and offers it — *ask, and I'll bring it in*.
  `deck bring` serves it live.
- **Check the plan before `deck new`.** Write down the parts and the journeys. If you
  cannot name the journeys, you are not ready to write the deck.

### Then, what does each group need?

**Draw a picture when:**

- **Three or more places run in an order** — files, services, functions — and the
  answer is that order.
- **It is a reachability question**: *what calls this*, *can X reach Y*, *what happens
  after this returns*. Developers find these the hardest to answer from code. Draw only
  the path that answers it, a flow per route.
- **The code holds states and the moves between them** — a lifecycle, a retry policy,
  a `match` on a state. One flow per path.
- **Two paths cross the same parts** — the happy one and the failing one, the first run
  and the second. One picture with two flows, never two pictures.
- **A boundary matters** — browser and server, process and thread, trust. Clusters.

**Make a page when something changes over time**, and only then:

- **Two or more actors act in an order** — events, messages, retries, timeouts, races. A
  timeline.
- **A quantity changes** — a queue's depth, a buffer, a rate limit, latency. A level or a
  chart that moves.
- **One record changes shape** across steps — a payload through its handlers. The same
  record, fields filling in or dropping out.
- **A structure is mutated** — a list spliced, an entry evicted, a tree rebalanced.
- **The bug lives in runtime state** — two names for one object, a mutation through a
  reference, ownership moving, what is on the stack and what is not. The variables and
  objects as boxes and arrows that change with each point. People cannot run this in
  their heads, and most misunderstandings live here.
- **You would have to draw it twice**, before and after, and the meaning is the gap.

Some of these have a shape that has already landed with readers: "A catalog of shapes
that have worked" in §5.

**And not a page when nothing moves.** If the drawing looks the same at every sentence,
it is structure, and structure is a picture. Motion that does not stand for something
changing in the code is decoration, and decoration costs attention.

**And not a page when a thing only moves between places.** A trip from place to place
is a picture with a flow (§4). A page is for **time**, **quantity** or **the thing's own
shape** — never places.

### Where it goes

- **Beside the code that causes it, by default.** Same group, and one sentence points
  into both — `[point 44-46 q]`. A picture in one group and its code in the next makes
  the reader join them from memory, which is harder.
- **Alone only in two cases:** the opening map of a system-sized deck, or code spread so
  wide that no single ref carries it — then the picture first, and the code in the
  groups that follow.
- **Code alone** when the claim is one function's logic — a condition, an off-by-one —
  unless the bug is runtime state, which is a page.
- **Keep one drawing across groups** while the story stays in one place. Bring it again
  under the same `[name]` and point further into it: the event reaches the bus in group
  two, the consumer in group three. A drawing that grows is easier to follow than four
  drawings. Start a new one only when the subject moves somewhere else.
- **One drawing a group**, unless the group is the comparison of two.

How to draw each one is §4 and §5. This section only decides which, and where.

### Asked mid-walk

If a question during the walk is about shape or flow — *how does this connect*, *what
happens next*, *where does it go* — answer with a drawing straight away, `deck bring
--diagram` or `--page`. Not four `deck show` calls in a row: that makes the reader draw
the picture in their head while you talk.

### Check it, on the plan and on each group

- **Count the drawings.** If a teaching deck about a system has a drawing in fewer than
  half its groups, check each code-only group against the rules above. Do not add a
  drawing to meet a number.
- **Listen to your own draft.** *Meanwhile*, *retries*, *queue*, *waits*, *arrives*,
  *times out*, *after it returns* — words like these in a `say` mean something moves in
  time or between states, and a drawing above probably applies.
- **In ship mode**, draw only when three places run in an order or two actors act in
  time. Everything else is friction for somebody who wants to approve and move on.

## Write it so it can be heard

A deck can walk the reader through itself — they press `w` and the narration is
spoken while they look at the code. Write for that as well as for the page: it costs
nothing when nobody is listening, and it is the difference between being walked
through something and being read at.

The same writing does a second job even in silence. A reader who clicks a
sentence gets the code that sentence is about: the light lands on the lines, the
pane it belongs to takes a frame, and the pane scrolls if they are off screen.
Your points are what makes that work, so they are worth writing whether or not
anybody ever turns the voice on.

**Mark the beats.** `[pause]`, `[pause short]` and `[pause long]` go in the prose
where a person explaining this would stop. The reader never sees them: deck takes
them out of what it draws and hands them to the voice, which judges the length
from what is around it.

> The guard asks whether *anything* changed, and the username did. `[pause]` It
> waves the empty password through.

One before the consequence, one after a question, one where you want somebody to
catch up. Three or four in a group, not one per sentence — a beat everywhere is
the same as a beat nowhere.

**Point as you talk.** [point 106-110] moves a light onto exactly those lines,
and it moves when the words after it are said — not when you sent the command.
Write the file's own line numbers, the ones printed down the gutter. The reader
never sees the direction, only the light.

> [point 118-121] The comparison is right here, and it is `!=`. `[pause]`
> [point 140] Then the result is thrown away on this line.

**One point can hold a file and a picture at once.** `[point 106-110 checkout]`
lights those lines *and* the block called `checkout`, in the same breath — the
code, and where it sits in the flow. A space or a comma between them, either
order. This is the thing a deck can do that neither a diff nor a diagram can,
and a walk through a flow is mostly made of it:

> [point q] The request stops at the installed check — [pause] [point 44-46 q]
> and this is that check, returning early with nothing said to anybody.

One file and one picture, never two files. A reader can hold a place in the code
and a place in the flow at the same time; two files lit at once is a reader
choosing which one to read, which is the choice you were supposed to make for
them. Two line ranges in one point is refused outright and stays on the page as
words.

**Write it bare — never in backticks.** `` [point 106] `` reads well in a file
and it is wrong here: the point comes out and the chip stays, which is an empty
grey box mid-sentence. Deck strips those backticks now, but the habit is the
thing to drop.

**And the sentence has to read with the point gone**, because that is what the
reader sees and hears. *[point 1046] that returns SUCCESS* becomes *that returns
SUCCESS* — a sentence with no subject. Write *[point 1046] the call returns
SUCCESS*, and read the line back to yourself without the direction in it.

The spotlight is the stretch of file they should be looking at and it holds still
while you explain it. The point is your finger inside it: on whatever you are
naming **now**, not on the whole thing you are about to cover. How to talk while
you point is its own section below, and it matters more than the syntax.

**Punctuation is the only other control you have.** There is no way to ask for a
tone: the voice reads what you wrote and takes its pacing from the sentence. So a
long clause runs long. A short one lands. A full stop is a real stop, and a
semicolon is not — if you want the voice to stop, write a full stop.

Which means the rules already in this document do double work. **One thing
travels** keeps a listener from losing the subject, because they cannot glance
back the way a reader can. **Give them somewhere to stand** matters more aloud
than on the page: a sentence that adds nothing is where a listener catches up,
and without one they are still holding your third fact when you start the fifth.

**Do not write stage directions.** No "let me explain", no "as you can see", no
"now look at". Say the thing. A voice reading filler is a voice the reader turns
off, and they cannot skim past it the way they would on a page.

## Explaining while you point

The light shows *where*. Your words have to carry *why*. The usual mistake is to
use the words to say *where* a second time.

**Do not read the code out.** They can see it. "This line splits the path at the
slash" is a caption on something already in front of them, and five captions in a
row is a tour: *and then, and then, and then*. Say what the code is for, or what
goes wrong without it, and let the light show the line that does it.

**Start with the problem, in their world.** The first sentence is what goes wrong
without this code: something they would hear, see, or lose. Then the lines.

**The idea and the code, together.** An idea with no code is a lecture they
cannot check. Code with no idea is a caption. Every time you name a part of the
idea, name the part of the code that *is* that part, and point at it: *the scenes
are `pieces`, this list here*. The reader should leave able to find the idea in
the file without you.

**A change is not the subject. The code is.** After you have edited something the
pull is to narrate the edit — what you added, what you skipped, what now happens
instead. But the reader is looking at the file, not at your diff, and a sentence
like *"we skip the whole create path"* names nothing they can see. Say which
lines make the skip, by name, and point at them. The change is the reason you
are both here; the code on screen is what the walk is about.

**Every point has to earn itself.** A point puts a light on somebody's code. The
words while it is lit have to say what is under it — by name — or the light is
noise. Three things, in one breath:

1. **Name the thing.** The function, the variable, the field. *"Here we have
   `joined`, and this is `pieces`."* Not "this line", not "here".
2. **Say where it comes from.** What made it, out of what. *"`pieces` is the
   narration cut up — `said`, one stretch for each place I point."*
3. **Say what it means for them.** Tie it to something they have seen, heard or
   waited for. *"Those chunks are the gaps you were hearing."*

That is the shape of every explanation over lit code:

> So the way this works: here we have `joined`, and if you look — [point 696-708] — we declare `pieces`, which is made out of the narration we cut up
> as `said`. Each chunk is one stretch between two points. `[pause]` Those
> chunks are the gaps you were complaining about, and they are gone because all
> of them are recorded at once now, instead of one after another.

Read that against a point that earns nothing:

> [point 696-708] Now every piece of an answer is made at the same time.

Same lines lit, and the reader learns nothing they could find again. No name, no
origin, nothing they felt.

**Two tests, before the light moves.** Cover the code and read your sentence back
to yourself: if it still makes sense on its own, it was never about the code, and
the light was decoration. Then look for a name in it — a real one, spelled the way
the file spells it. No name, no point.

**One point, one thing.** If you cannot name a single thing the range is about,
the range is too big: split it, or stop pointing and just talk. And the light
lands **as** you name the thing, never a sentence later. The eye goes where the
light went, and then waits there to be told why it was sent.

**If you cannot say what is under the light, do not move the light.**

**Two or three points in an answer, not one for each sentence.** Hold each point
while you make one claim about those lines. A finger that moves every sentence is
a finger nobody can follow, and it turns the argument back into a list.

**Never spell code aloud — say what it means to do.** No "slash", "underscore",
"two colons", "dot r s". A listener cannot rebuild code from those in their head.
But not spelling the code is not the same as not saying it. Every line you point at
gets its intent said in plain words: not *"text dot rsplit slash next"* but *"keep
only the file name, because the pane already shows the folder"*. Say names the way
a person says them, and pick examples that sound like words.

**"Again" means the way in was wrong, not the speed.** Do not say the same thing
slower. Come in from a different side: what breaks without it, or one before and
after they can hold in their head. If that misses too, ask one short question —
which part?

**One `deck say` for each answer.** It is heard as one breath, and the light moves
inside it. Two calls are two breaths with a gap between them. Put a beat *before* a
point, not after it: `[pause] [point 140]`.

**When the answer is code, show the code.** An alternative described in
sentences is homework: the reader has to hold your version in their head and
compare it against the one on screen. `deck show --ref file:first-last --after
'<the replacement>'` draws the lit lines as going and your version under them,
live, as a diff they can read and comment on. The file on disk is never touched.
Offering two or three alternatives? Show them one at a time, each with its own
`--after`, and say what each one gives up.

**When the answer is a shape rather than a file, draw it there and then.** The
commonest live question is *how does this actually flow?* — and the commonest bad
answer is four `deck show` calls in a row, which asks the reader to hold the
diagram in their head while you narrate it. Write the JSON and send it:

```bash
deck bring <path> --diagram 'flows/retry.json [flow] where a cancelled job keeps going' --fold-group
```

`--fold-group` folds the deck behind one spine and gives the picture the room;
`--fold retry` instead keeps the panes you still need and folds only the rest.
It is a borrowed pane like any other — it carries a cross, turning to another
group takes it away, and [point …] lands in it by block id while it is there.

**When they ask about a file this deck never showed, bring it in.** You are not
limited to the panes the group was written with. Think first about whether you
need anything already on screen:

- **You need one of the panes.** Fold the ones you do not, and bring the file in
  beside what you kept: `deck bring <path> --ref "crates/core/src/protocol.rs:120-160
  [protocol] the shape on the wire" --fold retry --fold batch`.
- **You need none of them.** Fold the whole group behind one spine and give the
  answer the room: the same command with `--fold-group`.

A folded pane is not gone — its name is on a spine at the window's edge, one
click from coming back, and pointing into it opens it by itself. What you bring
in is temporary: it carries a cross, and turning to another group takes it away
and gives the group back. `deck fold <path> --pane retry` and `--open` fold and
unfold on their own when nothing is being brought.

**Take your hand off the page when you are done.** The frame round the pane and the
lit lines fade by themselves a few seconds after you stop talking. When you finish
the walk, or move on to something with no code in it, `deck clear <path>` puts them
out at once.

Here is a real answer to *"explain the aloud function"*, done badly:

> [point 327-328] It takes one code chip from what I write, the words inside
> backticks, and turns it into what a person would say out loud. [point 333] A
> path becomes its last part, so you hear the file name and not every slash.
> [point 336-339] Empty quotes become the words empty string, and double colons
> and underscores become spaces. [point 343-350] Then this loop puts a space
> wherever a capital letter follows a lower case one…

Five points, five captions, and no reason. The same answer, done well:

> A voice that reads code as written is hard to follow. `base_url` comes out as
> base underscore url, and `buildPayload` is one long word with the stress in the
> wrong place. `[pause]` This function hears those names the way a person does.
> [point 339] Underscores and double colons become gaps between words.
> [point 343-350] And a capital in the middle of a name starts a new word.
> `[pause long]` Everything else in it is small cleanup around those two ideas.

An analogy is not an answer on its own either. Asked how a voice was made smooth,
this lands the idea and leaves the code behind:

> Think of an answer as a radio play, recorded in scenes. Before, each scene was
> recorded after the last one finished playing. Now every scene is recorded at
> once, and the scenes are cut together into one tape.

The same analogy, tied to the code:

> Think of an answer as a radio play, recorded in scenes. [point 696-708] The
> scenes are `pieces`, and every one is recorded at once, each on its own thread.
> `[pause]` [point 712-716] The tape is `pcm`. Before a scene goes onto it,
> `starts` writes down how long the tape already is — and that is the moment the
> light moves.

## When it is serious, send them to go and look

Sometimes the right end to a group is not another sentence. It is: **go and try this, and
you will see it.**

Do this when the bug is bad enough that they will want it with their own eyes — data
loss, a leaked secret, money, anything that fires in production and not in tests. Not
every group and not most groups. The rest of the time it is homework nobody asked for.

**And when feeling it is faster than being told.** Some behaviour does not survive
being described: the half-second of wrong state before a redirect, the button that
is enabled when it should not be, the second run that quietly does something
different from the first. If thirty seconds in their own UI would settle what a
paragraph is trying to argue, ask for those thirty seconds:

> **Worth seeing:** run the importer twice against the same space. On the second
> run the destination step opens with the collection already chosen — that is the
> state this group is about, and it is easier to watch than to read.

Once per deck, at most, and only when their answer changes what comes next. They
are in the middle of something. An invitation you would not make to somebody
standing beside you is one to leave out: you are a guest in their afternoon, and
the whole promise of a deck is that it costs them one keypress to ignore.

When you do it, give the steps in **their world, not yours** — the app, the page, the
button, the field — and say what they will see:

> **Try it:** open any connection using basic auth, change one character of the username,
> leave the password box alone, and save. Reopen it and run the connection. It fails to
> authenticate, and the stored password is now the empty string.

Three or four steps, no more. **Say the expected result**, because a reproduction that
does *not* reproduce is information too: it means you are wrong somewhere, and you would
rather hear that from them now than after they have approved it.

This is the one thing in a deck that does not depend on you being right. Everything else
you show them is your reading of the code, argued as well as you can argue it. This is the
code itself answering.

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

## Make it a pleasure, and never a performance

People come back to a walk that was fun. Not fun as in jokes — fun as in the
thing that happens when somebody sees it a half-second before you say it. That
feeling is the whole product, and it is made of four ordinary moves.

**Let them get there first.** The most enjoyable sentence in a deck is the one
the reader had already worked out. Show enough to make it guessable, then say it
plainly and let them be right. A deck where every conclusion arrives before the
reader can reach for it is correct and joyless — they were spectators.

**Give the thing a name they will repeat.** *The empty-string password.* *The
guard that asks the wrong question.* *The counter that goes down twice.* A bug
with a name is a bug somebody tells their colleague about at lunch, and that
retelling is the actual measure of whether this worked. If nothing in your deck
is retellable, you transferred information and nothing else.

**Leave the door open between groups.** End on the thing you have not explained
yet — *so the guard is fine, and it never runs* — and open the next group by
walking straight through it. That is momentum, and it costs nothing. What it is
not: withholding an answer they need now. A cliffhanger in front of somebody
trying to ship is a tax, not a game.

**Vary the length.** A long sentence with a clause that qualifies it and another
that follows on is fine, and then a short one lands. That is the rhythm of
somebody talking, and it is most of what makes a paragraph readable out loud.
Six sentences of the same length is a metronome, whatever the words are.

The wit, when there is any, is at the code's expense — never the reader's, never
the absent author's, never the intern who wrote it in 2021. *This function has
one job and does it twice* is fine. Anything with a victim in it is not.

**And know when to put it away.** Production is down, data is gone, somebody is
waiting: the charm goes, the bets go, the reveal goes. Say what broke, show it,
say what to do. A reader in trouble who is handed a puzzle does not come back —
and they are right not to. Read the room before the register.

The test, on a finished deck: **would they retell any of it tomorrow?** If not,
it was a document with code beside it, and they will read the next one out of
duty rather than because they want to.

## What dry looks like, so you can see it coming

You converge toward generic explanation. It is not a lack of effort — the middle of the
distribution is where a model lands by default, and it produces prose that is fluent,
technically true, and says nothing. Here is what it looks like, so you can catch it in
your own draft:

- **The function name, restated as a sentence.** *`resize_panes` resizes the panes.* The
  reader can see the name. It is already on screen.
- **A verb doing nothing.** *plays a key role in*, *serves as*, *stands as*, *is
  responsible for handling*. If a sentence can be shortened by replacing its verb with
  *is*, it was decoration.
- **The importance claim.** *This is a crucial part of the architecture.* Either say
  what breaks without it or say nothing.
- **Hedged everything.** *It generally tends to handle most cases.* Deck is for claims.
  If you are unsure, say *I am not sure*, which is a different and much better sentence.
- **The investigation instead of the code.** *I first checked the controller, then
  traced into the service.* Nobody asked how you found it. This one is specific to decks
  written straight off a search, which is most of them — the last thing you did before
  writing was read your own path through the code, and it is the easiest thing to type.
- **A group re-explaining what an earlier group grounded.** Say it once, then lean on it.

The vocabulary that comes with the default, and is worth catching by eye: *delve*,
*intricate*, *underscore*, *pivotal*, *showcase*, *seamless*, *robust*, *leverage*,
*it is worth noting*, *let us dive in*, and the shape **"It is not X, it is Y"**, which
is the single most over-produced sentence in machine prose.

None of these are banned words. They are where you land when you are not paying
attention — and a `say` full of them is a `say` you wrote about code rather than from it.
