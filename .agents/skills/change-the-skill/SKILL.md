---
name: change-the-skill
description: >-
  Edit crates/app/skill/SKILL.md, the file that tells an agent when and how to
  use deck. Use whenever the skill is being improved, rewritten, or a rule is
  being added to it. It is compiled into the binary, it is pinned by tests that
  look strange until you know why, and rewriting it has quietly destroyed the
  parts that made it work — twice.
---

# Changing the skill

`crates/app/skill/SKILL.md` is what makes an agent reach for deck at all. The
commands are the easy half; this file is the product.

## Where it lives, and the copy that is read now

```
crates/app/skill/SKILL.md      the source. include_str!'d into the binary.
~/.agents/skills/deck/SKILL.md what agents on this machine read right now.
```

An edit to the source does nothing until either a release ships or the file is
copied:

```sh
cp crates/app/skill/SKILL.md ~/.agents/skills/deck/SKILL.md
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
cp crates/app/skill/SKILL.md ~/.agents/skills/deck/SKILL.md
```

Then the real test, which is not a test: use it. Ask the person you are working
with whether decks got better or worse. Both rewrites passed every assertion in
the file and were worse anyway.
