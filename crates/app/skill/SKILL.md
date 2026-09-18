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

**Point as you talk.** `[point 106-110]` moves a light onto exactly those lines,
and it moves when the words after it are said — not when you sent the command.
Write the file's own line numbers, the ones printed down the gutter. The reader
never sees the direction, only the light.

> `[point 118-121]` The comparison is right here, and it is `!=`. `[pause]`
> `[point 140]` Then the result is thrown away on this line.

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

> So the way this works: here we have `joined`, and if you look — `[point
> 696-708]` — we declare `pieces`, which is made out of the narration we cut up
> as `said`. Each chunk is one stretch between two points. `[pause]` Those
> chunks are the gaps you were complaining about, and they are gone because all
> of them are recorded at once now, instead of one after another.

Read that against a point that earns nothing:

> `[point 696-708]` Now every piece of an answer is made at the same time.

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

> `[point 327-328]` It takes one code chip from what I write, the words inside
> backticks, and turns it into what a person would say out loud. `[point 333]` A
> path becomes its last part, so you hear the file name and not every slash.
> `[point 336-339]` Empty quotes become the words empty string, and double colons
> and underscores become spaces. `[point 343-350]` Then this loop puts a space
> wherever a capital letter follows a lower case one…

Five points, five captions, and no reason. The same answer, done well:

> A voice that reads code as written is hard to follow. `base_url` comes out as
> base underscore url, and `buildPayload` is one long word with the stress in the
> wrong place. `[pause]` This function hears those names the way a person does.
> `[point 339]` Underscores and double colons become gaps between words.
> `[point 343-350]` And a capital in the middle of a name starts a new word.
> `[pause long]` Everything else in it is small cleanup around those two ideas.

An analogy is not an answer on its own either. Asked how a voice was made smooth,
this lands the idea and leaves the code behind:

> Think of an answer as a radio play, recorded in scenes. Before, each scene was
> recorded after the last one finished playing. Now every scene is recorded at
> once, and the scenes are cut together into one tape.

The same analogy, tied to the code:

> Think of an answer as a radio play, recorded in scenes. `[point 696-708]` The
> scenes are `pieces`, and every one is recorded at once, each on its own thread.
> `[pause]` `[point 712-716]` The tape is `pcm`. Before a scene goes onto it,
> `starts` writes down how long the tape already is — and that is the moment the
> light moves.

## When it is serious, send them to go and look

Sometimes the right end to a group is not another sentence. It is: **go and try this, and
you will see it.**

Do this when the bug is bad enough that they will want it with their own eyes — data
loss, a leaked secret, money, anything that fires in production and not in tests. Not
every group and not most groups. The rest of the time it is homework nobody asked for.

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
  --say "Okay, drag the line between two panes. Left one grows, right one gives way,
nothing else on the page twitches. Feels like the most ordinary thing in the world.

Here is what I like about it: **a pane has no width.** Nowhere. Not in pixels, not in
any struct, not hiding in a layout pass. `[point 239-240]` That is [state] — two lists
of plain `f32`, `shares` and `widths`, and that is your lot.

So before you scroll — if the drag is not setting a size, what is it doing? Have a
guess. The answer is in [drag] and I think it is nicer than you expect." \
  --ref "crates/app/src/view.rs:239-240 [state] this is all the state there is" \
  --ref "crates/app/src/view.rs:1324-1342 [drag] and this is the entire drag"
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
- `--after` turns the ref in front of it into a **proposed change**: the range
  is drawn as going, and this is spliced in underneath it, so the pane reads as
  a diff rather than a highlight. The file on disk is never touched.
- `--diagram <file.json>` adds a picture. See §4.

**Use `--after` whenever you are proposing rather than pointing.** A highlight
says *look at this*. A change says *this should become that*, and if that is
what you mean, saying it in the `say` while the pane shows an untouched
highlight makes the reader hold the diff in their head — which is the work this
tool exists to take off them.

```bash
deck group <path> \
  --say "The counter is decremented on the **error path** too." \
  --ref "src/batch.ts:140-148 decremented *twice* when the write fails" \
  --after "    } catch (err) {
      logger.error(err);
      continue;
    }"
```

It goes straight after the `--ref` it changes, because that is what it attaches
to. A group can mix them: one pane a change, another pane the caller that has
to keep working.

This is also the right shape for **a plan**. The code that exists now, plus
what you would do to it, is cheaper to argue with than a diff — and the review
comes back before the edit is made rather than after.

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

Three things, every group. They take a second each and they are the difference
between a walk and a wall of text with code beside it:

1. **Does it name a pane?** Every pane you talk about, by its `[name]`.
2. **Does it point?** At least once, on the lines the sentence is about.
3. **Does it name something in the code?** A function, a field, a variable — not
   "this line", not "here", not "the whole create path".

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
- When they have asked for prose outright — *just tell me*, *in one line*. Honour
  it. Answer the way they asked, and offer the deck in a sentence if the answer
  still deserves one.
- As a substitute for actually making the change. A deck is for the argument, not the diff.
- To Socratically quiz them on a routine change they just want to approve (ship mode). Read
  the mode; do not force teaching where they want speed.
- For a one-line fix, or for something they watched you do.
