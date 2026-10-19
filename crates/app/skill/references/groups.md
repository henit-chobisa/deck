# Writing a group

§3 of the deck skill: every `deck group` flag, highlights and diffs, values on a line, the checks to run on each group, and the pills under it. Read it, whole, before `deck new`.

§1–§2 and §6–§8 are in [SKILL.md](../SKILL.md), §3 in [groups.md](groups.md), §4 in [pictures.md](pictures.md), §5 in [pages.md](pages.md). "Plan the context" and the other sections on how to tell it are in [writing.md](writing.md).

## Contents

- 3. Write one group per tool call
  - A code pane is a highlight or a diff, and you need both
  - Values on the line, when the claim is a value
  - Check before you write the first group
  - Read your own `say` back before you send it
- Leave them a way in: the moves they will make next
  - Read the reader first
  - Fill the slots
  - Writing them
  - When one is pressed
  - After every answer, look at the row again

## 3. Write one group per tool call

**Quote the `say` with single quotes.** Narration is full of `` `code` `` chips,
and backticks inside double quotes are a command substitution: the shell runs
what is between them and puts the output in your narration. It is silent when it
works — the group is written, with holes where the words were — so nothing tells
you it happened. If the prose has an apostrophe in it, use a quoted heredoc
rather than escaping your way out.

**Give every group a `--title`.** It heads the window while that group is on
screen, beside the group's number, with the deck's title in a small line above
it. The deck's title is the question; a group's title is the step of the
answer on screen now — *The drag moves a share, not a width* — in sentence case,
one line, at most 80 characters. Never the deck's title again, and never a label
(*Drag handling*): a reader who skims only the titles should still follow the
story. A group without one is headed by the deck's title, which is the confusion
this exists to end.

**Say how sure you are, by what you did.** Every group gets `--confidence`, and
a `--why`: required below high, expected on high. Confidence is not a probability and not a feeling: it is how good your
basis is. Models rate themselves high whatever the truth, so the level comes from
the steps below, never from how the claim feels to you.

*1. Start from the strongest kind of evidence you actually used:*

- **high**: you watched it behave. You ran the code, reproduced the bug, ran a
  test that covers this exact case, or read a trace or log from the system the
  claim is about.
- **medium**: you read every line of code the claim depends on, every function
  and branch, without running it.
- **low**: you inferred it, from names, docs, part of the code, a similar pattern
  elsewhere, or memory.

*2. Drop one level for each of these that applies:*

- **Indirect**: the evidence is from somewhere other than what the claim is about.
  The sandbox, not production; an older commit; a similar input, not this one.
- **Incomplete**: a link in the path you did not see. A config value, an
  environment variable, dynamic dispatch, generated code, another service, a
  function you did not open.
- **Conflicting**: something you saw disagrees, a test, a comment, a log, and you
  have not explained why.
- **Never or always**: the claim says something cannot happen, or always does, and
  rests on a few runs or a partial read. Examples cannot show absence.

Count each gap once. A value you did not see is the incomplete drop, not a reason
to start lower; code you did not read is what makes the start low, not a drop on
top of it.

*3. Floors and the one way up.* Below low, do not make the claim: say what you
could not determine instead. Doing the same kind of thing again never raises the
level. A second, independent kind of evidence that agrees cancels one drop, but
only a drop it does not share: a test and a read, both from the sandbox, are still
indirect. Between two levels, pick the lower.

*4. `--why` is the basis, one line the reader can check.* On high, say what showed
it ("ran checkout with a card that declines once"). Below high, name the drop or
what is missing ("indirect: reproduced in the sandbox; production retries may
differ"). The window shows the level as bars after the group's title and the basis
on hover. A reader who finds your highs earned trusts them, and one who catches a
single unearned high distrusts every badge you give.

```bash
deck group <path> \
  --title 'A pane has no width' \
  --confidence medium \
  --why 'read both functions; did not drag it' \
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

That is the length and the register to aim for. Read it against the four things in "Who is talking" (writing.md):
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
  --title 'Cancel never reaches the attempt in flight' \
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
  Write the blank line as a real one. A `\n` typed as two characters is read as
  a break too, outside backticks — so a `\n` you mean literally goes in backticks.
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
- `--page <file.html>` adds a page: markup you write, for the thing they should
  watch happen — never for boxes and arrows, which are a picture. See §5.

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
  --title 'The error path counts twice' \
  --say "The counter is decremented on the **error path** too." \
  --ref "src/batch.ts:140-148 decremented *twice* when the write fails" \
  --after "    } catch (err) {
      logger.error(err);
      continue;
    }"
```

**Point at one half at a time.** A pane showing a change holds two rows for
every line — the one going and the one arriving — and both answer to the same
number, because the arriving side is numbered as it will be once the edit lands.
So a bare point means the code **as it is**, and a `+` means the code **as it
would be**:

```bash
deck group <path> \
  --title 'Two checks become one pass' \
  --say 'That check ran twice. [point 140-142] Both calls hit the same key.
[pause] [point +140-143] The new one folds them into a single pass.' \
  --ref "src/batch.ts:140-142 [batch] the check that runs twice" \
  --after "  if (!ok) {
    pending -= 1;
    retry(key);
  }"
```

Two sentences, two lights. *This was the issue*, then *this is what it becomes* —
which is how somebody reads a change, and it cannot be done if one point lights
both halves at once.

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
as it stands, seal it, and make the edit once their review is in. You lose nothing by that
order and they get to argue before it lands, which is the whole point of handing
it over.

If the edit was already written before the deck opened — and it usually is,
because *make this change* came before *show me* — reach for `--before`. You still have the old text; you
replaced it a moment ago, and it exists nowhere else now. That is exactly why
the flag is there.

```bash
deck group <path> \
  --title 'One buffer, reused' \
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
  --title 'Three routes under the existing path' \
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

### Values on the line, when the claim is a value

A line can carry what it held when the code ran: `orderId` is `undefined`, the
total is `81`, the session had the id one key over. A single value is written
right after its name. An object becomes a preview under its line that opens to
its keys. The reader stops taking your word for the value and reads it off the
line that produced it.

**Use it when the claim is a value.**

- The bug is a wrong value: `orderId` is `undefined`, the total is `81` and not
  `90`, `rowCount` is `0`. The value *is* the evidence; a sentence only reports it.
- The wrong field: the data was there, one key over. Show the object with both
  keys — the one that was read and the one that held it.
- A value that changes as the code runs: a loop, a retry, a discount applied
  twice. Write the line's history, `100 → 90  90 → 81`.
- A payload that drives a branch: a webhook, a response, a config. Show the
  field the branch turns on.
- They ask *what was X*, or *show me*, about code already on screen. Answer with
  `deck show --values`, not a sentence.
- They ask you to explain it *with values*, *with an example*, or to *walk it
  through*. That is a request for values on the lines, every time — the answer
  is the values landing where they live, one point at a time, not a paragraph
  that mentions them.
- **Your own narration names a value.** If a sentence says the key is `42`, the
  name comes back as `Acme`, the row gets the id `opt-9`, that value belongs on
  the line that holds it. A value in prose is read once and then has to be
  carried in the reader's head to the line it belongs to; a value on its line is
  read exactly where it matters. If you are about to write a value into a
  sentence, put it on the line and point at it as well: the sentence may still
  say it, the line is where it lives. A value the code already spells out —
  `const limit = 10` — needs nothing beside it; this is for values the code
  does not show.

**Leave it out when** the claim is about structure (who calls whom, why a module
exists), when the value is plain from the code (`const limit = 10` needs no
`10` beside it), or when nobody's claim turns on it. A value that could be
removed without losing anything is noise: remove it.

**Only values you have, or an example you said is one.** From a test run, a
log, a payload you read, a debugger, or arithmetic anyone can check from the
code. A value on a line reads as observed, so a guess about what a real run
held goes in the prose, said as a guess — *I expect orderId is undefined here*
— and not on the line.

**An example is the exception, and it is the commonest case in a plan.** For a
design, a plan, or code that has not run, nothing was observed — so pick one
concrete case and walk it through on the lines. Say it is an example first, in
the prose: *say the Customer property is `p-7` and the CRM returns `42`,
`Acme`*. Then put those values on every line they pass through, the same key
the whole way, pointing at each as you go — `42` on the line that receives it,
`42 → Acme` on the lookup, `opt-9` on the row it becomes. Keep the example true
to the code: the arithmetic and the shape must be what this code would really
do with that input. Never mix an example's values and observed ones in one
group.

An example has to be visible as one on the line too, because the line stays on
screen after the sentence that announced it has scrolled away: the note of
every ref carrying example values starts `example:`, and the group's `--why`
says *worked example, not a run*. Choose the example's inputs before `deck new`
and reuse exactly those values in every group — `p-7` in group two is `p-7` in
group four. **Never secrets:**
no keys, tokens, passwords, card numbers or personal emails. Redact them,
`"sk_live_…"`, `"m***@…"`.

**How much.**

- Only on lines the `say` points at with `[point …]`. The value is the evidence
  for that sentence. When a sentence points at a line holding one of a pane's
  values, that pane keeps all its values away until such a sentence is first
  reached; then they all come out together, once, and stay. A pane none of your
  points land on a value line of shows its values from the start. Values you
  write live come out at once, and bring out the rest of that pane's values with
  them. The reader can hide or show a pane's values with the eye in its header.
- At most two values on a line, usually one. At most three or four lines with
  values in a group; more is two groups — a walk that passes eight lines is two
  or three groups in a row, or, live, two or three shows. A loop shows its first
  two steps and where it ends, `100 → 90  90 → 81  … → 53 (5 times)`. An arrow
  on one line is that line's history; a value that becomes another across lines
  is a value on each line, not an arrow.
- `"lit": true` on the one value the claim turns on, never more than one per
  sentence.

**Objects.** Give one `keys` only when the claim needs two or more keys of it —
the field read and the field that held it. The preview in `is` starts with the
key that matters: `Session { client_reference_id: "ord_8F2kQ71", … }`. Name only
the keys the prose talks about, plus the one that makes the contrast (the empty
`metadata` beside the full `client_reference_id`), up to about six, in the
value's own order. Every run you leave out is `{ "more": N }`. Each named key may
carry a `why` of a few words — *set at checkout*, *nothing was put here* — never
a sentence.

**With the prose.** Name the value in the sentence that points at it: *line 6
reads `metadata`, which arrived empty, so `orderId` is undefined.* The words and
the line say the same thing. Write a key's name exactly as it is in the payload:
a sentence that names a key opens its preview when the reader picks it, and
closes it again when they let the sentence go, as long as the pane's values are
out.

**With a diff.** On a change already made (`--before`), values go on the lines
that arrived and the lines that still run; lines that went carry none. On a
proposed change (`--after`), the values on the lines going are the code as it is
today — the bug. **Then give the fix its values too**, with `"after": true`,
numbered as the replacement will be: the reader should see `total 90, then 81`
on the line going and `total 90` on the line arriving, both halves changing, not
the bug's numbers beside a silent fix. They are drawn in italic, as predicted.
Only values you can work out from the code — the same arithmetic, the same
inputs. If the fix's values depend on something you cannot see, leave them out
and say so.

`--values` follows the `--ref` it belongs to, as `--after` does. `at` is the
name exactly as it is written on that line.

```bash
deck group "$DECK" \
  --title 'orderId is undefined here' \
  --say 'In [webhook], line 6 reads the order id from metadata, which arrived empty, so [point 6] orderId is undefined.' \
  --ref 'apps/api/src/stripe/webhooks.ts:5-7 [webhook] the handler Stripe calls' \
  --values '[{"line":5,"at":"session","is":"Session { client_reference_id: \"ord_8F2kQ71\", … }","keys":[{"key":"client_reference_id","is":"\"ord_8F2kQ71\"","why":"set at checkout","lit":true},{"key":"metadata","is":"{}","why":"nothing was put here"},{"more":22}]},{"line":6,"at":"orderId","is":"undefined","lit":true}]'
```

Live, the same list goes on `deck show --ref file:first-last --values '…'`,
onto code already on screen. An answer that spans panes is one movement, so
write it with `deck values`, which lands every pane in the same frame — two
shows are paced a second apart:

```
deck values "$DECK" --on webhook --values '[…]' --on markpaid --values '[…]'
```

### Check before you write the first group

You write groups one at a time, but you plan the whole story before `deck new` — that is
where `--total` comes from. So do the count now, on the plan: **if every group has exactly
one ref, you have planned a list, not a presentation.** Go back and find what belongs
beside what. A deck of single-ref groups makes `next page` and `next group` do the same
thing and means they never see two pieces of evidence at once, which is the entire point.

And one more plan check, for teaching decks: **does the story make them reason, or just
receive?** If the deck reveals everything top-down with nothing for them to predict, you
have written a lecture. Find the spot where you can point at the code and ask them first.

**Then ask which panes each group needs.** Not one answer — most groups want more than
one, and the mistake is stopping at the first, which is the one you reach for without
noticing:

- **A file**, when the evidence is code that exists. Most groups.
- **A picture**, when answering means naming three or more places and the order they run
  in. See §4.
- **A page**, when the answer is something they should watch happen — a race on a
  timeline, a queue filling, a structure changing, a chart bending. When you would have
  to draw it **twice**, because the thing being explained is the bit in between. See §5.

**A picture or a page almost always wants a file beside it.** The picture says *where* in
the flow you are, the refs say *what* the code does there; the page shows the movement,
the ref shows the line that causes it. §4 and §5 both pair them, and a group that answers
with a shape and no code has left the reader to find the code themselves.

That last test is the one that makes the difference, because a page is the pane everybody
forgets — and, when it is remembered, the one written as boxes, which is a picture's job.
Two requests racing for one row, a queue filling until the producer is told to stop, a
node coming out of the middle of a chain, a pointer walking a list: draw any of those once
and you have drawn the start, or the end, and not the thing. Those are pages, and a deck that
answers one with a static picture has handed the reader the two frames either side of the
only frame they wanted.

**Which of these, and when, is decided by "Plan the context: when to draw what".** Run it
on the plan, and again on each group.

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

## Leave them a way in: the moves they will make next

A group says one thing. The reader either understands it and decides, or they
don't, and nod anyway. Most of them will not type what is on their mind.
Almost all of them will press it, if it is there.

So give every group three to five pills, with `--ask`. They sit under the
window. Pressing one asks you, straight away, and you answer in the deck. The
bar is that **two of them are what the reader was about to type.**

```sh
deck group <path> --title "…" --say "…" --ref "…" \
  --ask "What's an outbox, basics first?" \
  --ask "Draw it: enqueue → worker → email" \
  --ask "Does the refund worker also retry forever?" \
  --ask "Are you sure the PDF is slow? Show me where."
```

People rarely type a fresh, clever question about one detail. They make a
handful of moves, again and again: *say that simpler*, *draw it*, *does the
other one do this too*, *are you sure*, *do we need this at all*. The group in
front of them tells you which one is coming. So do not hunt for a fresh,
clever question. Fill the slots below with the moves they make.

### Read the reader first

You have been in the conversation, so use it. Look at everything they said and
everything they typed into earlier decks: their habits, their words, what they
doubt, what they already know. That is the best evidence there is of what they
will type next. Where a slot below can be filled the way they have typed
before, fill it that way. Never ask about something they said, built or
decided.

### Fill the slots

**1. Say it simpler, about the exact thing. Always.** Name the one term, field
or mechanism in this group they have not used themselves: *What's txn_ref,
basics first?* Only when nothing stands out, use *Say that again, simpler*.
Not following is the most common reason anyone types.

**2. Make it concrete. Pick what the group's shape calls for.**
- An idea or an analogy with no pointed lines → *Walk me through the code itself*.
- A fix or a plan described in words with no diff → *Show me the diff for <fix>*.
- Three or more parts, or a before and after → *Draw it: <A> → <B> → <C>*.
- A subsystem named but not shown → *Show me <function> where it <does X>*.

**3. The question this group raises most.** Take the first that applies:
- **Reuse.** Something new is added (a helper, endpoint, model, step) → *Is
  there an existing <kind> we can reuse?* With a base class on screen → *Could
  this live in <base>?*
- **The other one.** One importer, level, mode or route is shown doing
  something. Name its counterpart the group never mentions → *Does <the other
  one> also <behaviour>?*
- **Edge.** An optional key → *What if <key> is null?* A failure → *Would a
  retry fix it?* A new hop or default → *What does <it> cost or break?* One
  hole patched → *Will more like this turn up?*
- **Why this.** A literal number, a fixed list, a storage choice → *Why <0.3 /
  these four / rows, not JSON>?*

**4. Push back. Take the strongest:**
- **Doubt the claim.** The group's main assertion, especially a cause, a
  *never* or an *always*, or anything you did not point at → *Are you sure
  <claim>? Show me where.*
- **Why not the obvious alternative.** The design a senior engineer would
  reach for first → *Why not <a join table / a soft delete / one queue>?*
- **A simpler way.** Code you built → *Is there a simpler way to <do X>?*
- **Do we need it.** The narration says *rare*, *unlikely* or *not shipped* →
  *Do we need this at all?*
- **How far it reaches.** The change sits in shared code → *Does this hit
  every <route>, not just <this one>?*

**5. Only when the group lists options:** *What are the tradeoffs between <A>
and <B>?* Otherwise, a second question from slot 3.

When the group ends on a bottom line (*no UI change*, *only X moves*), a
confirm pill takes slot 5, unless the group lists options; then the
tradeoffs pill keeps it. *So nothing changes on <X>?*

**Three is enough when the group is small**: one ref and a short say. Keep slot 1, slot 4, and the
better of 2 and 3.

### Writing them

- **Their names, their voice.** Fill every <placeholder> with this group's own
  file, field, function, number or other one. Write it in first person, as they
  would type it. Keep the plain frames, like *Say that again, simpler*, the same
  every time; that sameness is what makes them findable.
- **Short.** Under about 60 characters; `deck group` refuses past 90.
- **Not already answered.** A pill the narration above it already answers is
  dropped and replaced by the next one down. For slot 1, that means *Say
  that again, simpler*. In a group that asks them to
  predict, leave out the pill that gives the answer away: no *Show me the
  diff* before the reveal.
- **Never the decision.** No *Go with A*, *Lock it*, *Approve*, and no *Which
  would you pick?* Deciding is the reader's, and it is the reason this deck
  exists. When a group asks for a decision, the pills help them decide well
  (the doubt, the alternative, the tradeoffs), and they type the decision.
- **Never a quiz.** No question aimed back at them, and nothing about a toy
  example instead of their system.

### When one is pressed

It arrives like any question asked mid-deck, and you answer it the same way:
by showing. Bring the file, point at the line, draw the timeline. A pressed
question answered with three paragraphs teaches the reader that pressing is
not worth it.

Answer the pill they pressed, at the depth they pressed it. *Say it simpler*
gets the plain mechanism, not the edge cases. *Are you sure* gets the evidence
and your honest confidence in it.

### After every answer, look at the row again

Their question is the best evidence you will get of what they will ask next.
So once you have answered, and before you go back to `deck wait`, look at the
pills under that group with their question in mind. Renew the row when the
question changed what is worth asking:

- **the pill you just answered** goes;
- **any pill the answer made stale** goes: answered along the way, or no
  longer the right doubt;
- **what the answer opened** comes in: the term you just used that they have
  not met, *does the other one do this too*, the doubt your answer raised,
  following the same method as the first row.

```sh
deck say <path> --text "…your answer…"
deck asks <path> --group g3 \
  --ask "Draw it: enqueue → worker → email" \
  --ask "Why not render the PDF in the worker?" \
  --ask "Does the invoice email render one too?"
deck wait <path>
```

Give `deck asks` the whole row you want: the pills that stay and the new ones.
Those that stay keep their places and the new ones join after them, so give
them in any order. The window waits for your answer to finish before the row
renews, and the reader watches the old questions sink away and the new ones
come in, which tells them the row is following them. It works on a sealed
deck, and only after you have answered.

When the row still fits, after a small question or one it already covered,
leave it alone. A row that changes after every answer stops meaning anything.
But a row still offering the question they just asked, word for word, is the
one sure sign it was not read again.
