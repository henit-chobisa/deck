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

One exception: a question too big for seven groups — a big feature, *explain the
whole event stream*. Do not ask whether to build. Agree the chapters in a few lines,
and build chapter one when they say go. See "Agree the chapters first", in
[references/writing.md](references/writing.md).

**And do not send the prose version as well.** A summary carrying the whole
argument is why nobody opens the deck. Say one line — *a deck is on the bar* —
and let the deck do the work.

## The rest of this skill is in references/

This file is the loop. How to write what goes into it is in `references/`,
beside this file, one file per job. They are not extras: they are the rest of
the skill, kept apart so each is read when it is needed. Read each one whole.

| Read | When |
| ---- | ---- |
| [writing.md](references/writing.md) | before `deck new`: whether this is one deck or chapters, who is talking, how to present, what to draw and where, points and beats |
| [situations.md](references/situations.md) | before the first group: which kind of deck this is, and its shape |
| [groups.md](references/groups.md) | before the first group: §3, every `deck group` flag, highlights and diffs, values on a line, the checks, the pills |
| [pictures.md](references/pictures.md) | before a `--diagram`, §4: the first group of a deck about a whole system or a flow is one |
| [pages.md](references/pages.md) | before a `--page`: §5 |
| [answering.md](references/answering.md) | when `{"asked": …}` comes back, with "When one is pressed" and "After every answer" in groups.md |

Every deck needs the first three. Read writing.md before `deck new`, since it
decides how many decks this is; read the other two while the bar is up,
before the first group. After your context is compacted, read all three again
before your next group: a summary keeps this file and drops them.

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
deck open     the bar, in the background — and what wakes you at the end
deck group    one group, one call — --title for its heading, --confidence (and --why) for how sure you are, --ask for the pills
deck asks     after an answer: renew one group's pills (--group g3 --ask …)
deck seal     no more coming
deck wait     only with -d, when you detached the window yourself
deck next     what the reader just did, when you want it without blocking

deck show     move their eyes: --ref, --pane, --after for a proposed change,
              --values to write what a line held onto it
deck values   values onto several panes at once: --on <pane> --values <json>, repeated
deck say      say something, spoken if they have a voice, with [point …] in it
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
  It names the whole deck: on the bar, and in a small line over every group's
  heading in the window.

## 2. Put the bar up, before you have written anything

```bash
deck open <path>          # run this as a background command
```

**This is also how you hear back.** `deck open` holds until the reader answers,
then prints the review as JSON and exits — so the background command you start
here is the one that wakes you at the end. There is no second command to
remember, which is the whole reason it works this way now: arming a listener
used to be a separate step, and a separate step is a step to forget.

Background, always. In the foreground it blocks you for as long as somebody is
reading, which is minutes, and a foreground command has a timeout.

Run it **second**, right after `deck new`, before the research — and only again after you
answer a question mid-walk (§7), never otherwise for
that deck. A bar appears at the bottom of the screen showing the title and `writing group
1…`. Everything after this point is them waiting on *less and less*.

The reader can open the deck straight away. Until the first group lands the window says
the deck is being written, and each group appears in it as you write it — so opening this
early is safe, and the first group is worth getting to them quickly.

**You never open the deck itself.** There is no flag for it and there must be no attempt
at one. The bar is the whole invitation; when to read is theirs to choose, and it is
almost never the moment you happen to finish. Sealing a deck is not a cue to put a window
in front of somebody.

## 3. Write one group per tool call

In [references/groups.md](references/groups.md).

## 4. When the answer is a picture

In [references/pictures.md](references/pictures.md).

## 5. When the answer only makes sense moving

In [references/pages.md](references/pages.md).

## 6. Seal it

```bash
deck seal <path>
```

Until this lands, deck tells them groups are still coming and warns them if they try to
submit. Forget it and they get warned about groups that were never going to arrive.

**Sealing is not delivering.** All it says is that no more groups are coming. It
does not listen for the reader, and a turn that ends here has handed somebody a
deck with nobody on the other end of it.

## 7. Stop, and let the open command wake you

The sequence is `deck new` → `deck open` → groups → `deck seal` → **stop**.

There is nothing to arm. The `deck open` you started in step 2 is still running,
and it ends when the reader answers — printing the review, waking you with it.
**This step is not optional, and it is the one that gets skipped**: not the
waiting any more, but the stopping. End your turn and say nothing further. They
are reading, not waiting on you.

**Never announce the deck instead of leaving it alone.** "The deck is ready,
let me know what you think" is a turn spent asking somebody to do the thing they
were already doing, and it arrives as a notification while they read.

**One listener, and only one.** `deck open` is already it. Do not add a
`deck wait` beside it for the same deck — both would wake you with the same
review, and you would answer it twice.

### Every way they can reach you comes back the same way

`deck open` ends, and hands you whatever happened, for **all** of these:

- they submitted — you get the review, exit 0
- they asked something mid-walk — you get the question, exit 0
- they interrupted — same, straight away rather than at the next gap
- they closed it without answering — exit 4
- nothing, for as long as your timeout allows — exit 3

The reason it can do that is worth knowing, because it is the rule the whole
design turns on: **a process wakes you by ending.** Not by printing, not by a
file appearing. So whatever wakes you has to be a thing that can afford to die —
and a window cannot, because somebody asking a question still wants the deck in
front of them. So the window is a process of its own and the listener is this
one.

**Answered a question? Run `deck open` again.** It finds the window already
there, leaves it alone, and goes back to listening. That is the loop for a live
walk: open, get woken, answer with `deck say`, open again. Every time, for as
long as they are in there.
How to answer is in [references/answering.md](references/answering.md), and
how to renew the pills after it is "After every answer", in groups.md.

```bash
deck say <path> --text "…"   # answer them
deck open <path>             # and listen again, in the background
```

`deck wait` is for one case only: you passed `-d` and are doing the listening
yourself. Otherwise you never need it.

**Neither in the foreground, ever.** A review takes minutes, a foreground
command has a timeout, and a reader cannot be hurried.

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
  How: [references/answering.md](references/answering.md).
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
      "text": "why does this assume sorted input?" } ],
  // only after a live walk: what was shown and said, in order — yours included
  "transcript": [
    { "at_ms": 41200, "what": "said", "group": "g2", "when": "queue",
      "text": "agreed — I'll make the retry bounded once you submit." } ] }
```

`range` is where the comment sits *now* (deck tracked it through any edits to the file while they
were reading). `quote` is the text it was pinned to when they wrote it. If the two
disagree, trust `quote` — search for it. `source` says how far to trust `range`: `extmark`
and `diff` are exact, `fingerprint` is a good guess, `stale` means the line moved and could
not be found.

`kind` is on every comment and is always the same: the reader is not asked to grade what
they wrote, so weigh the words rather than the field. `when` is the one that carries
intent — `interrupt` meant *answer me now*, `defer` meant *this can wait for the review*.
A comment with **no `ref` and no `range`** is about the group's claim rather than any
line — often the most important one in the review.

Act on every comment, and on everything you agreed to during the walk — the
changes they asked for in the conversation and the decisions you locked in
together are part of this review even though they are not in `comments`. Read
`transcript` for what you said you would do: every promise there is a comment
too. Answer the questions, fix what they objected to. Their comments are
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
  The two modes are in writing.md.
- For a one-line fix, or for something they watched you do.
