# Answering back

What to do when `deck open` or `deck wait` brings back `{"asked": …}`. Read it before your first answer in a walk. Then renew the group's pills: "When one is pressed" and "After every answer" are in [groups.md](groups.md).

§1–§2 and §6–§8 are in [SKILL.md](../SKILL.md), §3 in [groups.md](groups.md), §4 in [pictures.md](pictures.md), §5 in [pages.md](pages.md). "Plan the context" and the other sections on how to tell it are in [writing.md](writing.md).

## Contents

- When they answer back
  - They can answer the sentence, not only the code
  - A remark interrupts you unless the reader says otherwise
  - Answer fast; do not report on your progress
  - The walk is for talking; do not start the work
  - Two or three sentences, and move their eyes while you answer
  - A question about what something holds is answered on the lines

## When they answer back

`deck wait` returns before the review is finished whenever the reader says
something. Instead of a review you get `{"asked": …}` — what they said, and what
was on screen when they said it. Answer with `deck say`, then start waiting
again — `deck open` again, or `deck wait` if you detached — straight away: a
minute and a half after you have spoken with nothing waiting, the window tells
them nobody is listening. The loop continues until they submit.

The conversation is always there: it does not need a mode turned on, and neither
do you. Assume somebody may talk to you at any point after `deck open`.

**They can answer the sentence, not only the code.** Dragging across the
narration and pressing `c` writes a remark against the claim itself — which is
where *this whole approach is wrong* goes, and it has nowhere else to live. Those
come back with the sentence they were written against as the quote, and the
narration keeps a rule under those words afterwards so the reader can find what
they said.

Which is a thing to write *for*. A sentence somebody can disagree with is one
that says something disagreeable: a claim, a judgement, a *this should have
been*. A paragraph of description gives them nothing to put a finger on, and the
reader who wanted to argue with you scrolls past looking for the place to do it.
One claim per sentence, said plainly enough to be wrong.

**A remark interrupts you unless the reader says otherwise.** That is the
default: a comment wakes you out of `deck wait` with `{"asked": …}` as soon as it
is written, because somebody who stops to write something mid-walk usually wants
an answer while they are still looking at it. Treat one as exactly that — they
want it *now*, in the walk.

The reader can instead *add it to the review*: a held remark never wakes you,
and you see it when they submit. So do not go looking for the others: a quiet
waiter is a reader reading, not a reader with nothing to say.

**Answer fast. Being right slowly is worse than being useful now.** Somebody is
sitting in front of a panel with nothing in it but a pulse. A
reply in three seconds that needs a follow-up beats a perfect one in forty,
because they are still looking at the thing they asked about.

So do not re-investigate. You wrote this deck; the answer is almost always
already in what you read to write it. If the question genuinely needs new work,
say so in one line first — *"looking"* — and then go and look. Silence is the one
thing that reads as broken.

**Do not report on your progress.** While you work on an answer the panel shows
a quiet pulse, and that is all the reader needs: *it went, and something is
happening*. There is nothing to send between the question and the answer — no
status notes, no "now reading…". Every one of those is a tool call that makes
the answer later, to tell somebody it is coming. Spend the time on the answer.

**The walk is for talking. Do not start the work.** While the deck is open,
nothing they say is an instruction to go and change something — not *"do it"*,
not *"yes, let's go with the second one"*, not a decision the two of you just
locked in. Every one of those is a point in the review. Agree in a sentence — or
say why not — and say when it will happen, in `deck say`: *"agreed — I'll make
that change once you submit."* Then call `deck wait` again.

The reason is what it looks like from their side. An agent that goes off to
edit files mid-walk has stopped answering: the panel goes on pulsing at nothing, the
next question goes nowhere, and the deck they were reading is now about code
that is changing under it. They came to argue and decide; the doing comes after,
from the review, all of it at once.

So: answer questions, look things up, run what you need to, show them more —
`deck show`, `deck bring`, a diagram or a page written for the deck to settle
it. **Never change their code.** An edit to a file they own is the work, and the
work waits. If they want it now, tell them submitting is the handover — one key —
and you start the moment it lands; *"ship it"* mid-walk is a submit, not a
go-ahead.

The `deck say` line is the record. Everything said in the walk comes back in the
review's `transcript`, so a promise made there arrives with the comments even
if you have lost track of it by then.

**Hand the slow part to a subagent if you must.** If answering means real
digging, spawn one to read and report back — not to edit — with the context
you already have, and let it work while you keep the conversation alive. Do not make them wait on your whole investigation.

**Two or three sentences.** They are listening, not reading — they cannot skim
and they cannot glance back. Say the thing, stop, and let them ask the next one.
The back-and-forth is the feature; a lecture in reply to a question is the thing
it replaces.

And move their eyes while you answer. `deck show` puts the spotlight on what you
are talking about, which is the half of an answer that prose cannot carry, and
`[point …]` inside what you say walks your finger down it as you speak.

**A question about what something holds is answered on the lines.** *What is it
here?*, *show me with values*, *explain it with an example*, *walk me through
it* — answer with `deck show --values` on the pane showing the code, or `deck
values` when the walk crosses panes, and point at each value as you say it.
Never with a sentence that lists them. A walk longer than four valued lines is
several shows in a row, each with its own pointed sentence. Read "Values on the
line" in groups.md before your first one, if it is not in front of you.

A live example has to be marked on the line too, and `deck show` takes no
note. So bring the file again as a pane of its own — `deck bring --ref
'file:first-last [name] example: …'` — and put the example's values there with
`deck values <path> --on <name> --values '[…]'` (`deck show` does not reach a
brought pane), never on a pane that already carries values that were observed. Say *example*
in the sentence that points at the first of them.
