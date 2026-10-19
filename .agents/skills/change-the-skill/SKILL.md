---
name: change-the-skill
description: >-
  Edit crates/app/skill/SKILL.md and its references/, the files that tell an
  agent when and how to use deck. Use whenever the skill is being improved,
  rewritten, or a rule is being added to it. It is compiled into the binary,
  it is pinned by tests that look strange until you know why, and rewriting it
  has quietly destroyed the parts that made it work — twice.
---

# Changing the skill

`crates/app/skill/SKILL.md` is what makes an agent reach for deck at all. The
commands are the easy half; this file is the product.

## Where it lives, and the copy that is read now

```
crates/app/skill/SKILL.md      the loop, and which reference to read when.
crates/app/skill/references/   the rest: writing, groups, pictures, pages, …
~/.agents/skills/deck/         what agents on this machine read right now.
```

All of it is `include_str!`'d into the binary, and written to disk together.

## SKILL.md stays short; the detail goes in a reference

`SKILL.md` is read whole every time the skill is used and stays in context
for the session. After a compaction only its first 5,000 tokens come back. At
2,551 lines everything past the voice sections — the loop, the waiter, the
review — was what a long session lost. A test holds it under 500 lines and
20,000 bytes.

So a new rule goes in the reference for the job it belongs to, and
`SKILL.md` keeps only the loop and the table that says which reference to read
when. A new reference needs a row in that table, an entry in `REFERENCES` in
`crates/app/src/skill.rs`, and a contents list if it runs past 100 lines; the
tests check all three.

Moving text between files is a move, not a rewrite. When it was split into
references every line went across word for word, and only the cross-references
that now pointed into another file were changed.

An edit to the source does nothing until either a release ships or the file is
copied:

```sh
cp -R crates/app/skill/SKILL.md crates/app/skill/references ~/.agents/skills/deck/
```

Do that when testing a change — otherwise you are watching the old one.

**Warn the reader not to run `deck setup` before upgrading.** It writes the
binary's embedded copy over the file, which will be the *older* text until a
release goes out.

## The tests that look strange

`crates/app/src/skill.rs` asserts that specific sentences are present:

```rust
assert!(front.contains("TWO OR MORE"));
assert!(SKILL.contains("Predict before reveal"));
```

Pinning prose looks wrong until you have watched a rewrite drop the rule that
made the thing work. **Each of those assertions is a rule that was lost once.**
If one fails, the question is whether you meant to remove that rule — not how to
make the test go away.

One of them parses the example diagram out of the file as a real `Diagram`, so
an example that would not draw cannot ship.

## What rewriting it keeps costing

The file has been rewritten twice and both times it came back shorter,
better-shaped, and worse. Numbered rules became prose, an argument became an
assertion, and the reader said the explanations had got worse — which is the only
measure that counts.

- **Do not tidy it.** Length is not the problem it looks like.
- **An argument is not the same as a rule.** "A group with one ref should be a
  decision, not a default" works because the paragraph under it *argues* for it —
  the pairs to look for, why the relationship is the thing being taught. Compress
  that to the claim and it stops changing anybody's behaviour.
- **Examples are what get copied.** A model follows the worked example over the
  prose every time, so a rule with no example is a rule that gets read and not
  applied.

## The description decides everything

The frontmatter `description` is read while a model is choosing whether this is
a deck moment. The body is read *after*. A rule that only lives in the body
arrives too late to be the thing that decides.

That has been got wrong: a countable trigger was put in the description, then a
later restore brought back an older description and the count survived only in
the body. It stopped firing.

Whatever decides *whether* to build a deck belongs in the description.

## Checking a change

```sh
cargo test -p deck-app skill
cp -R crates/app/skill/SKILL.md crates/app/skill/references ~/.agents/skills/deck/
```

Then the real test, which is not a test: use it. Ask the person you are working
with whether decks got better or worse. Both rewrites passed every assertion in
the file and were worse anyway.
