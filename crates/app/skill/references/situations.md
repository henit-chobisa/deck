# Situations

The kinds of question that are a deck, and the shape that works for each. Read it, whole, before `deck new`.

§1–§2 and §6–§8 are in [SKILL.md](../SKILL.md), §3 in [groups.md](groups.md), §4 in [pictures.md](pictures.md), §5 in [pages.md](pages.md). "Plan the context" and the other sections on how to tell it are in [writing.md](writing.md).

## Contents

- The situations this comes up in
- This is the default, not the exception

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
