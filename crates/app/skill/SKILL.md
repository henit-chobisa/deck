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

## The situations this comes up in

Named, because "explain code" is easy to not recognise yourself doing. Each of
these is a deck, and the shape that works for it is not the same.

**A PR or a branch review.** One deck **per concern**, never one deck per PR. A
branch that adds a connection, changes a schema and touches the UI is three
decks walked in order, not fifteen groups in one. Say which chunk this is and
what is still coming — *this is the schema half; the wiring is next* — so they
know whether to comment now or wait. Ship mode: point crisply, reason in a line.

**A root cause.** Open at the surprise: the line that does the wrong thing, not
three groups about where the file lives. Then the evidence, then the
consequence. If you have a timeline of what happened when, that is a diagram.

**"How does X work" / a flow.** Start with the diagram — the whole path in one
picture, with a flow per route through it — then a group per step that matters,
each pointing at real lines. The picture says *where*, the refs say *what*.

**Onboarding somebody to an area.** Context before mechanism. What is this
group of files responsible for, what problem does it solve, and only then how.
Teaching mode: make them predict before you reveal.

**A plan, before you write it.** The change you are about to make, as the code
that exists now plus what you would do to it. Cheaper to argue with than a
diff, and the review loop is exactly the right shape for *yes, but not that
bit*.

**Two approaches, one decision.** A group each, the same question answered both
ways, and a last group that is the trade-off. End on the decision — that is the
comment you actually want.

**What a refactor moved.** The old shape and the new one side by side, one pair
per group. A rename across forty files is one group showing two of them, not
forty refs.

**Findings after an investigation.** What you looked at, what you found, and —
said plainly — what you could not verify. The unverified part is the most
valuable group in the deck, because it is the one they can answer.

**Something you are unsure about.** *This is the ugly bit.* *I am not sure this
is right.* A deck that admits its weak spot is trusted everywhere else, and it
is the group that gets commented on.

**An error or a stack trace.** A trace *is* a list of file:line locations, so
the count has already fired before you have written a word. Do not paste it back
to them — they have it. Take the two or three frames that matter and show the
code at each, in order, ending at the one that is actually wrong.

**A failing test.** Three things belong on screen together and almost never are:
the assertion that failed, the code under test, and the change that broke it.
That is one group with three refs, and it is the clearest deck there is.

**"Why did you do it that way?"** A decision already made and now being
questioned. Show the constraint that forced it, then the obvious alternative and
the line that rules it out. If nothing rules it out, say so — you have just
learned you were wrong, and that is worth more than the defence.

**A bug you cannot reproduce.** Show the path you traced and the exact point
your understanding runs out. End on the question that would settle it: *which of
these three is your case?* Far better than asking for more information cold,
because they can see what you already know.

**Something you found that nobody asked about.** A real problem noticed in
passing. Keep it small — one claim, two refs — and end on *worth fixing now, or
later?* An unasked-for deck earns its interruption by being short and by
deferring to them on what happens next.

**Performance.** The hot path, the measurement, and the fix, in that order. The
measurement must be real and the group must say where it came from. A
performance deck with no number in it is an opinion.

**A dependency or API upgrade.** What changed upstream, beside every place here
that touches it. One group per call site that has to change, not one group
listing them.

**Reading somebody else's code.** A library's internals, a teammate's branch,
an unfamiliar service. Context before mechanism, and no criticism — the point is
to understand it, not to judge it. If something is genuinely wrong, that is a
separate deck.

**"It does not actually do that."** Correcting a belief is the hardest thing to
do in prose, because you are arguing with something they can already picture.
Point at the absence: the branch that is never taken, the handler nobody
registers, the field written and never read. One ref, well chosen, ends the
argument.

**Picking up where you left off.** A session resuming, or a handover. One group
per thing in flight, each pointing at the half-finished code rather than
describing it. What is done, what is not, what you were about to do next.

**They asked the same thing twice.** If you explained it in prose and the
question came back, the prose failed. Do not explain it again more slowly —
build the deck. The second asking is the signal.

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

9. **One file per page when panes relate.** Don't show one file in multiple panes. If a
   page uses multiple panes, show one file per pane where a change in one impacts the
   other — e.g. you removed a function, now one callsite per pane per file. If two
   ranges of one file are close together they are **one range** — widen it and use one
   pane. Deck refuses ranges in one file less than 30 lines apart, because the second
   pane opens below the first and repeats most of it.

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

## When they answer back

`deck wait` returns before the review is finished whenever the reader says
something. Instead of a review you get `{"asked": …}` — what they said, and what
was on screen when they said it. Answer with `deck say`, then call `deck wait`
again. Same verb, and the loop continues until they submit.

The conversation is always there: it does not need a mode turned on, and neither
do you. Assume somebody may talk to you at any point after `deck open`.

**Most remarks will not wake you, and that is the default.** A comment is
*deferred* unless the reader says otherwise: it goes into the review and you see
it when they submit. `deck wait` returns `{"asked": …}` only when they chose to
interrupt you or to wait for a gap — which means they want an answer *now*, in
the walk. Treat one as exactly that, and do not go looking for the others: a
quiet waiter is a reader reading, not a reader with nothing to say.

**Answer fast. Being right slowly is worse than being useful now.** Somebody is
sitting in front of a panel with nothing in it but a bar that is filling up. A
reply in three seconds that needs a follow-up beats a perfect one in forty,
because they are still looking at the thing they asked about.

So do not re-investigate. You wrote this deck; the answer is almost always
already in what you read to write it. If the question genuinely needs new work,
say so in one line first — *"looking"* — and then go and look. Silence is the one
thing that reads as broken.

**Say what you are doing while you do it.** `deck doing <path> --text "reading
the retry loop"` puts those words in the panel, and the next one replaces them.
Send one before each step of anything that takes more than a moment — the file
you are opening, the thing you are counting, the subagent you are waiting on.

This is not politeness. The panel cannot tell a working agent from a dead one,
so after two minutes of silence it stops guessing and says `no answer yet` in
front of somebody who is still waiting on you. Every note restarts that clock.
The reader gets *reading the retry loop*, then *counting the callers*, and a
wait with a shape to it is a wait people sit through.

Notes are not turns: they are not read aloud, they are not in the review, and
they do not answer the question. Do not use one to say the thing — that is
`deck say`.

**Hand the slow part to a subagent if you must.** If answering means real
digging, spawn one with the context you already have and let it work while you
keep the conversation alive. Do not make them wait on your whole investigation.

**Two or three sentences.** They are listening, not reading — they cannot skim
and they cannot glance back. Say the thing, stop, and let them ask the next one.
The back-and-forth is the feature; a lecture in reply to a question is the thing
it replaces.

And move their eyes while you answer. `deck show` puts the spotlight on what you
are talking about, which is the half of an answer that prose cannot carry, and
`[point …]` inside what you say walks your finger down it as you speak.

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

**Every line of that is required, and the last one most of all.** A turn that
ends after `deck seal` looks finished from your side and is a dead end from
theirs: they submit a review nobody is listening for. §7 is not a formality.

**Every verb, so you know what you have.** Six of these build the deck and six
are for while somebody is in front of it. Reach for the second six: a walk where
the agent never moves anything is a document with a bar under it.

```
deck new      the header, and the bar
deck open     the window, in the background, straight away
deck group    one group, one call
deck seal     no more coming
deck wait     in the background, then end the turn
deck next     what the reader just did, when you want it without blocking

deck show     move their eyes: --ref, --pane, --after for a proposed change
deck say      say something, spoken if they have a voice, with [point …] in it
deck doing    what you are doing, while you do it
deck bring    a file this group never showed, or --diagram for a picture you write now,
              or --page for markup that moves
deck fold     put a pane on its spine, or --open to bring it back
deck clear    take your hand off the page
```

Check `deck <verb> --help` rather than inventing a flag. This file describes the
deck the reader has installed, and it is older than their copy by however long
it has been since they upgraded.

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

**Quote the `say` with single quotes.** Narration is full of `` `code` `` chips,
and backticks inside double quotes are a command substitution: the shell runs
what is between them and puts the output in your narration. It is silent when it
works — the group is written, with holes where the words were — so nothing tells
you it happened. If the prose has an apostrophe in it, use a quoted heredoc
rather than escaping your way out.

```bash
deck group <path> \
  --say 'Okay, drag the line between two panes. Left one grows, right one gives way,
nothing else on the page twitches. Feels like the most ordinary thing in the world.

Here is what I like about it: **a pane has no width.** Nowhere. Not in pixels, not in
any struct, not hiding in a layout pass. [point 239-240] That is [state] — two lists
of plain `f32`, `shares` and `widths`, and that is your lot.

So before you scroll — if the drag is not setting a size, what is it doing? Have a
guess. The answer is in [drag] and I think it is nicer than you expect.' \
  --ref 'crates/app/src/view.rs:239-240 [state] this is all the state there is' \
  --ref 'crates/app/src/view.rs:1324-1342 [drag] and this is the entire drag'
```

That is the length and the register to aim for. Read it against the four things above:
it opens on their hands rather than on a function, it names the surprise, it has an
opinion (*here is what I like about it*), it bets before it reveals, and the two panes
only mean something together — the state, and the thing that changes the state. The
next group pays the bet off and says what the drag actually moves.

Notice what the prose does with the panes, because it is the part that gets left out:
it **names both of them** — [state] and [drag] — and it **points** at the lines while
it talks about them. A group whose `say` mentions neither is a chat message with code
sitting next to it. `deck group` says so on the way past when it sees one.

**Points are not a voice feature.** Nobody has to be listening. A point is what ties a
sentence to the lines it is about, so pressing that sentence in the rail lights them,
and the light follows the reader. Write points in every group you write, whether or not
anybody has a voice set up.

Here is a second one with every job labelled, because the register is easy to
admire and hard to copy:

```bash
deck group <path> \
  --say 'You cancelled the job and the log kept going for another ninety seconds.

In [retry], the guard is the first thing in the loop. [point 106-110] It reads
`cancelled` before it sleeps, which is why cancelling between two attempts works
exactly the way you expect. [pause]

So why the ninety seconds? [point 118-121] Because the attempt already in flight
is never asked. `run` is awaited here, and nothing in this function can reach
inside it. [test] proves the part that works and says nothing about this one —
worth knowing before we trust it.

The attempt you are waiting on is the attempt nobody is cancelling.' \
  --ref 'src/worker.ts:100-125 [retry] the loop, and the guard at the top of it' \
  --ref 'test/worker.test.ts:18-26 [test] what we actually check'
```

Read it by the job each part is doing:

- **First line: their world, not the code.** What they saw, in the words they
  would have used for it. No file is named yet.
- **The pane arrives as a noun.** *In [retry]* — named, not placed, and one
  sentence saying what it is before it is leaned on.
- **The point lands on what the sentence names.** `cancelled`, by name, on the
  lines that read it. Not "this bit here".
- **A beat where they need to catch up**, once, before the turn.
- **The bet, then the payoff.** A question they can answer from what is lit, and
  the answer directly under it.
- **The shaky part is named out loud.** [test] covers the working half; saying so
  is what makes the rest of the deck worth believing.
- **A landing sentence with no new nouns in it.** The thing they already have,
  put down.

A `say` of one sentence is almost always a group that has not been written yet.

**One Bash call per group. Never batch them.** Putting two groups in the same call, or in
the same parallel block, means neither lands until you have generated both — which throws
away the entire reason this is a directory. Write the first, then think about the second.

They are on group 1 while you write group 2. If they walk past the last group you have
written, deck parks them there and opens the next one by itself the moment it lands, so
you never need to tell them to press anything.

- `--say` is Markdown, inline only: `**bold**`, `*emphasis*` (painted in the accent, so it
  means *look at this word*), `` `code` ``, and a blank line between paragraphs.
  **Never a fenced code block.** The band cannot draw one — it comes out as a
  wrapped paragraph with the backticks still in it, and deck refuses the group.
  Code that exists goes in a `--ref`; code that does not exist yet goes in
  `--after`. Either way the reader sees it as code, highlighted, and can
  comment on it.
- `--ref` is `file:first-last`, a space, then a short note. A bare number is one line.
  `*starred*` words in the note are accented. Give it once per pane.
- A note may start with a **name** in brackets — `[retry]`, `[batch-2]` — one lowercase
  word. Write the same `[retry]` in the `say` and it becomes a chip the reader can
  point at to light that pane. `deck show --pane retry` takes the name too.

**Name panes; never place them.** Not "the left pane", "the one on the right", "the
second pane". Where a pane sits is decided by the window's width after you wrote the
sentence, so a place you name is often wrong by the time it is read. A name is right
at every width, and the reader can put the pointer on it to see which pane it means.
Name every pane you mention in the prose.
- `--diagram <file.json>` adds a picture. See §4.
- `--page <file.html>` adds a page: markup you write, for the thing that only
  makes sense moving. See §5.

### A code pane is a highlight or a diff, and you need both

Every pane of code you show is one of two things. Reaching for the wrong one is
the commonest way a group lands flat, and the reader feels it as *you told me
instead of showing me*.

**A highlight** is `--ref` on its own. The lines as they are on disk, lit. It
says **look at this**. Use it for code that exists and is staying: the caller
that has to keep working, the field the bug is about, the function you are
teaching.

**A diff** is `--ref` with `--after` or `--before` behind it. The pane reads as
a change rather than a highlight. It says **this became that**, and which flag
you want depends on one thing only: whether the edit is on disk yet.

- **`--after`** — you have *not* made the change. The range reads as going and
  the `--after` text as arriving. The file is never touched, so the reader can
  say no while saying no is still free.
- **`--before`** — you have *already* made it. The range is the new code, lit as
  what arrived, and the `--before` text is what it replaced, drawn above it with
  no line numbers, because the file does not contain those lines any more.
- **`--before ""`** — you *added* it, and it replaced nothing. The range is
  marked as having arrived, with nothing drawn over it. This is the one people
  forget, and it is the commonest case of all.

Never both on one ref. A range is what is going or what arrived, and deck
refuses a group that claims it is both.

```bash
deck group <path> \
  --say "The counter is decremented on the **error path** too." \
  --ref "src/batch.ts:140-148 decremented *twice* when the write fails" \
  --after "    } catch (err) {
      logger.error(err);
      continue;
    }"
```

`--after` goes straight after the `--ref` it changes, because that is what it
attaches to. A group can mix the two: one pane a change, the next the caller
that has to keep working either way.

**The test is whether you are pointing or proposing.** If the sentence you are
about to write has *should*, *would*, *instead of*, *replace* or *the fix is* in
it, you mean a change — and showing an untouched highlight while you say it
makes the reader assemble the diff in their own head, which is the work this
tool exists to take off them.

**A diff is a proposal, so draw it before you make the edit.** This is the shape
a plan wants and the reason to reach for it early: the code that exists plus
what you would do to it is cheaper to argue with than a change already on disk,
and the answer comes back while changing your mind still costs nothing.

**Asked to change something and then show it? Never show it as a highlight.**
This is the one that gets got wrong every time: the edit goes in, the deck comes
up with the new lines lit, and that is a highlight of the result — the answer to
a question nobody asked. They wanted to see the change. Both flags exist so that
you always have one.

If you have not edited yet, prefer `--after`: write the group against the code
as it stands, seal it, and make the edit afterwards. You lose nothing by that
order and they get to argue before it lands, which is the whole point of handing
it over.

If the edit is already written — and it usually is, because *make this change*
came before *show me* — reach for `--before`. You still have the old text; you
replaced it a moment ago, and it exists nowhere else now. That is exactly why
the flag is there.

```bash
deck group <path> \
  --say 'It allocated on every pass. [point 140-148] [batch] reuses one buffer now.' \
  --ref "src/batch.ts:140-148 [batch] what it became" \
  --before "  for (const row of rows) {
    const buf = Buffer.alloc(row.size);"
```

Do not reach for the other one to fake it. Putting the old code in `--after`
draws a revert — *this should become what it used to be* — and reads as one.

**New code is still a change, and this is the one that gets missed.** When you
have added something — a new endpoint, a new route, a whole function that was
not there — there is nothing it replaced, and the reflex is to reach for a plain
highlight. Do not. A highlight of lines you just wrote looks exactly like a
highlight of lines that have been there for two years, so the reader has to take
*I added these* on your word and then work out which ones you mean:

```bash
deck group <path> \
  --say 'Three new routes. [point 35-48] [urls] hangs them off the path the
existing one already lives under.' \
  --ref "apps/api/urls/asset.py:35-48 [urls] the three new ones" \
  --before ""
```

Empty is the whole trick: *what did this replace? nothing.* The range comes up
marked as arrived, and the lines around it stay ordinary, so which ones are
yours is a thing the reader can see rather than a thing you asserted.

**And keep the range to what is actually new.** A range that opens a few lines
early to give context quietly claims those lines too. Context is what the rest
of the pane is for — it is all on screen anyway.

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
- **Cite what you have read.** `deck group` opens every file you point at and
  refuses the group if the file is not there or the range runs past the end of
  it. That is not a formality — a ref you guessed at and a ref you read look
  identical in prose, and they do not look identical in line numbers. If deck
  turns a ref down, go and read the file rather than adjusting the number.
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

### Read your own `say` back before you send it

Six things, every group. They take a second each and they are the difference
between a walk and a wall of text with code beside it:

1. **Does every pane get named?** Both ways: every pane you talk about carries a
   `[name]`, and every name you gave a ref turns up in the prose.
2. **Does it point?** At least once, on the lines the sentence is about — and
   written bare, with no backticks round it.
3. **Does each sentence still read with the points taken out?** That is what
   reaches the eye and the ear. *[point 106] that returns early* is not a
   sentence.
4. **Does it name something in the code?** A function, a field, a variable — not
   "this line", not "here", not "the whole create path".
5. **Is there one sentence that adds nothing?** Somewhere to put their weight
   down, before the next fact.
6. **Would you say this out loud to somebody at their desk?** If it reads like a
   commit message, it is a commit message.

And once for the deck, not the group:
**does the opening say where somebody was standing?**
A first group that begins on a variable has started in the middle.

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
  --say 'You press Connect and nothing happens — no error, no spinner, nothing.

[point q] In [flow], the request stops at the installed check before it ever
leaves the browser. [pause] [point 44-46] And [guard] is that check: it returns
early when the app is missing, without saying so to anybody.' \
  --diagram 'connect.json [flow] where the click stops' \
  --ref 'web/silo/connect.tsx:40-52 [guard] the check that returns early'
```

**A picture is a pane, so it takes a name and a point like any other.** The
`--diagram` argument has the same shape as a `--ref`: the file, then an optional
`[name]`, then a note. The prose says [flow] and the reader can press it. And
[point q] lights the block whose `id` is `q` — the id you wrote in the JSON is
the name a sentence can carry, which is a reason to give nodes ids that mean
something rather than `n1`, `n2`, `n3`.

While a point is on one block, the rest of the picture steps back — the same
thing a spotlight does to the lines around it. So a diagram is walked the way
code is: one block at a time, in the order the sentences go.

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

  Flows and points share the picture and do not fight over it. A point is your
  finger while you talk; a flow is the reader pressing play, and theirs wins for
  as long as it runs. Write both: the flows let them retrace it on their own
  after you have stopped talking, which is when most of the understanding
  happens.

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

**Every node needs an edge.** A box with nothing joined to it is the reader
stopping mid-sentence to work out what it is doing there. If something belongs in
the story but not in the flow, say it in the prose and leave it out of the
picture. And a picture brought again under a name already borrowed replaces the
one that was there, so *draw it better* is a revision rather than a second
drawing beside the first.

Code refs are always laid out before diagrams whatever order you typed them. If the
picture has to be read first, it needs a group of its own.

## 5. When the answer only makes sense moving

Some things are neither a file nor a picture. Two rows merging on the same key, a queue
filling up until the producer is told to stop, a node coming out of the middle of a chain
— the argument **is** the movement, and a diagram of it draws the start and the end with
the interesting part missing.

That is what a page is for: HTML you write, rendered in a pane of the deck, wearing the
deck's own colours and hearing the same points the code does.

```bash
deck group <path> \
  --say 'Two rows arrive with the same key. [point 24-26 hop] The later one lands
on top of the earlier — [hop] is the line that picks the winner, and [rows] shows
which field actually survives it.' \
  --page 'merge.html [rows] two rows meeting' \
  --ref 'src/merge.ts:24-26 [hop] the line that decides'
```

`--page` takes the same shape `--ref` and `--diagram` do: the file, then an optional
`[name]`, then a note. It is a pane like any other — it takes a name the prose can say,
it folds, and a point can land in it.

**Reach for a picture first.** A diagram costs you eight lines of JSON, deck draws every
pixel of it, and it folds and animates for free. A page is markup you own end to end and
a native view composited above everything deck paints, which is why it hides itself while
the room is moving. If boxes and arrows can say the thing, say it with boxes and arrows.
A page is for what a still picture genuinely cannot hold.

### Forty words, and that is the whole rule

`deck group` counts the words a page shows and refuses it over forty. Not a style note —
the command fails and you write it again.

The prose carries the argument. Words on the page are a second narration competing with
the band, read at a different speed by the same reader, and that is the one bad decision
this pane is always one step away from. **Labels, numbers, a field name.** Not sentences,
not a heading and three bullets, not an explanation of what they are looking at. If the
page needs a paragraph to make sense, the paragraph belongs in the `say`.

### It starts at rest, and moves when you point

This is the rule that is most often got wrong, and getting it wrong ruins the pane.

A page must come up still. No autoplay, no timer, no `setTimeout` chain that starts on
load. The reader is listening to a sentence; if the animation is running on its own clock
it is already three steps past the words, and they are watching one thing while hearing
another.

What moves it is a point:

```js
window.deck = window.deck || { at: null }   // deck installs this before your script runs

addEventListener('deck:point', (e) => {
  render(e.detail)        // e.detail is the id you pointed at, or null
})

addEventListener('deck:remark', (e) => {
  // e.detail is the text of the comment the reader just pressed in the rail
})
```

`window.deck.at` holds the point the reader is on right now, and `deck:point` fires every
time it changes — including once more when the document finishes loading, so a page
brought in halfway through a walk comes up on the step the narration is actually on rather
than its first frame.

So write the page as a **function of the point**, not as a sequence that plays. Given
`hop`, draw the state at `hop`. Given `null`, go back to rest. A page written that way can
be walked forwards, walked backwards, and re-entered in the middle, which is what the
reader will actually do to it.

`deck:remark` is the other half: the reader pressing a comment in the rail, saying *this
part*, about something that moves. Nothing else they could have written it against can
hear them.

### Say what you answer to

A point finds its pane by name, and a page answers to two things: the `id` of any element
in it, and whatever it declares here.

```html
<meta name="deck-points" content="rest merge hop settle">
```

Declare them whenever the interesting names are arrangements rather than things on screen
— states, steps, moments. Most good pages are like this: `hop` is not an element, it is
what the page looks like at a moment.

Without that, `[point hop]` names nothing, and `deck group` says so while you are still
running and can fix it. The failure it was added for is worth knowing, because it is
silent from the reader's side: the code lit up beside a page that never moved.

### Wear the deck's colours

Deck hands every page its palette as CSS variables, before your markup:

```
--deck-bg  --deck-fg  --deck-accent  --deck-on-accent  --deck-muted
--deck-edge  --deck-wash  --deck-add  --deck-del  --deck-comment
```

Use them and nothing else. **Never write a literal colour.** The reader chose their
editor's theme and the rest of the window is honouring it; one page with `#1e1e1e` hard
coded in it is the pane that looks like it came from somewhere else — which is exactly how
it feels to read.

The background is transparent and the font is already set to the window's mono. Build on
that rather than reasserting it.

### Nothing loads from anywhere

There is no network behind a page. No CDN, no Google Fonts, no `fetch`, no image URL.
Every script and style is inline in the file you write.

This is deliberate and not a limitation to work around: a surface for reading your own
code should not be making requests while you read it. Write SVG and CSS transitions by
hand. Forty words of content does not need a framework.

### Fill the room you were given

A page gets a whole pane, and a pane is whatever shape the window's width left
it — often tall and narrow, sometimes wide and short. The drawing has to answer
that shape, and this is the rule that gets missed, because a page written against
a browser looks fine in a browser and lost in a pane.

```html
<style>
  body { margin: 0; display: grid; place-items: center; height: 100vh }
  svg  { width: 100%; height: auto }
</style>
<svg viewBox="0 0 240 120">
```

`100vh` and `place-items: center` put it in the middle of whatever it is given;
a `viewBox` with no fixed width lets it grow to the room instead of sitting at
whatever size you happened to type. **A drawing a quarter of the way up a tall
pane with empty space under it reads as something that failed to load**, and no
amount of correctness in the part that did draw makes up for it.

### Everything you name gets an edge

If the prose says four actors, the picture joins four actors. A box sitting to
one side with no line into it is the reader stopping to work out what it is doing
there — and they stop *while you are still talking*, so they lose the next
sentence too.

That applies hardest to the thing you mention last. It is the one most likely to
have been added to the drawing after the layout was settled, and the one most
likely to be floating.

### Do it again, in the same place

Asked for a clearer one? **Bring it again under the same `[name]`.**

```bash
deck bring <path> --page 'flow.html [flow] the whole path, one screen'   # and again, revised
```

A page or a picture brought under a name that is already borrowed replaces what
was there, keeping its place in the room and the fold it already had. Two
drawings of one idea side by side is the reader deciding which one you meant,
which is worse than the first drawing was.

### What a good page looks like

```html
<meta name="deck-points" content="rest arrive hop settle">
<style>
  body { margin: 0; display: grid; place-items: center; height: 100vh }
  .row { transition: transform .45s ease, opacity .45s ease }
  .row.gone { opacity: .25 }
  .win { fill: var(--deck-accent) }
</style>
<svg viewBox="0 0 240 120" width="100%">
  <g id="old" class="row"><rect width="200" height="34" fill="var(--deck-wash)"/>
    <text x="8" y="22" fill="var(--deck-fg)">qty 3</text></g>
  <g id="new" class="row"><rect width="200" height="34" class="win"/>
    <text x="8" y="22" fill="var(--deck-on-accent)">qty 7</text></g>
</svg>
<script>
  const at = { rest: 0, arrive: 1, hop: 2, settle: 3 }
  addEventListener('deck:point', (e) => draw(at[e.detail] ?? 0))
  function draw(step) { /* position the two rows for that step */ }
</script>
```

Two shapes, four states, one transition, and every colour borrowed. That is the size of
thing that works. A page with a legend, a title and six controls is a small web app, and
the reader did not open deck to use a small web app.

### Bringing one mid-walk

When they ask about something that only makes sense moving, write it there and then:

```bash
deck bring <path> --page 'live/evict.html [evict] what happens to the third entry' --fold-group
```

Borrowed like any other pane — it carries a cross, turning to another group takes it away,
and `[point …]` lands in it while it is there. If a group already has a page and the
borrowed one answers to the same name, the borrowed one wins for as long as it is up.

## 6. Seal it

```bash
deck seal <path>
```

Until this lands, deck tells them groups are still coming and warns them if they try to
submit. Forget it and they get warned about groups that were never going to arrive.

**Sealing is not delivering.** All it says is that no more groups are coming. It
does not listen for the reader, and a turn that ends here has handed somebody a
deck with nobody on the other end of it.

## 7. Arm the waiter, then stop

```bash
deck wait <path>          # run this as a background command
```

**This step is not optional, and it is the one that gets skipped.** The sequence
is `deck new` → `deck open` → groups → `deck seal` → **`deck wait`**. A deck
without a waiter is the whole tool failing in the way that is hardest to see:
they walk it, they write three comments, they press submit — and nothing ever
comes back. From where they are sitting your side simply stopped caring halfway
through. They will not use it again, and they will be right not to.

Run it with `run_in_background: true`. Then **end your turn** and say nothing further —
they are reading, not waiting on you.

**Never announce the deck instead of waiting for it.** "The deck is ready" with
no waiter armed is the failure above with a sentence painted over it. If you have
said anything about the deck at all, the waiter is already running.

**One waiter, and only one.** If one is already armed for this deck, keep it
rather than starting a second. Two consumers race for the same review and one of
them gets nothing.

**Re-arm after every live answer.** Answering a question through `deck say` ends
that waiter — it exited to hand you the question. The reader is still in the
deck, still reading, still going to submit. Arm it again before your turn ends,
every time, for as long as they are in there.

**Do not run `deck wait` in the foreground.** A review takes minutes, a foreground command
has a timeout, and a reader cannot be hurried.

If your environment genuinely cannot hold a background process, say that plainly
to the reader — that you cannot be woken, and they should tell you when they have
submitted. That is a worse tool, honestly described. It is not the same as
leaving and hoping.

### The waiter can end without a review, and you must be able to tell

A backgrounded waiter also ends when they close the deck without answering, when they
interrupt the turn, or when the session is killed. The exit code is how you tell them
apart:

| exit | means |
| ---- | ----- |
| 0 | something came back: read it before you believe it |
| 4 | they closed the deck without answering |
| 3 | no review yet — only from `--timeout`, which you should not need |
| 1 | something was wrong; the message on stderr says what |

**Zero is not the same as approval, and this is the one that will catch you.**
The waiter also comes back at zero when somebody asks you something mid-walk,
because a question they cannot get an answer to is a reader stuck. So look at
what it printed:

- `{"asked": …}` → **a question, not a review and not consent.** Answer it with
  `deck say`, move their eyes with `deck show`, and then **arm the waiter again**
  — you are back where you were, and nothing has been submitted.
- A review payload — the object with `comments` in it — → they submitted. Proceed.
- **Anything else is NOT a review.** Nothing was submitted. They may not have seen the
  deck at all.

When there is no review: say plainly that the deck went unreviewed, and ask whether to
re-arm or to walk it in the terminal instead. Re-arm at most once without being asked.

**Never convert an absent review into agreement.** Do not write "review came back with no
comments", do not record verdicts, do not treat silence as sign-off, and do not move to
the next stage. An interrupted waiter means they were doing something else, not that they
agreed with you.

## 8. When you are woken

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
- When they have asked for prose outright — *just tell me*, *in one line*. Honour
  it. Answer the way they asked, and offer the deck in a sentence if the answer
  still deserves one.
- As a substitute for actually making the change. A deck is for the argument, not the diff.
- To Socratically quiz them on a routine change they just want to approve (ship mode). Read
  the mode; do not force teaching where they want speed.
- For a one-line fix, or for something they watched you do.
