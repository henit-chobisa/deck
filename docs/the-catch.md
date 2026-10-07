# The catch

`deck setup` installs a skill telling your agents when to build a deck. A skill
is a suggestion, and an agent that has just finished an investigation has
enormous momentum toward typing out what it found. Suggestions lose to that
often enough that you can install deck and then go a week without seeing one.

So setup also offers to turn on the catch. It reads each finished reply, and
when one names two or more `file:line` locations it sends the reply back and
says to build the deck instead — while the agent is still mid-turn and still
holding everything it just learned.

```
That reply names 3 code locations (view.rs:1109, protocol.rs:253,
protocol.rs:155). Locations in prose are the thing deck exists to
replace — the reader has to go and open each one and hold the
argument together themselves.
```

Two locations, not one: a single pointer is a sentence, and opening a window for
it would be ceremony. Two is an argument, and assembling it is the work deck
does for you.

It stays quiet whenever it might be wrong — a deck already built this turn, a
reply about deck itself, the same location cited twice, a version number or a
time of day that only looks like a citation. A hook that fires when it should
not is worse than one that never fires, because you turn it off.

## And after a compaction

A skill is loaded once, and when a long session is compacted only the start of
it is kept. Measured on a real session: with the skill fresh, 98% of the groups
an agent wrote pointed at their lines; after one compaction 66% did; after two,
48%, with no drawings and no files brought in. The rules had not changed. The
agent had lost them.

So the same hook answers one more moment. When a session picks up after a
compaction, and that session has used deck, it tells the agent to load the deck
skill again before the next group or answer. It says nothing on a fresh start,
and nothing in a session that never touched deck.

## Turning it on and off

Claude Code only, for now; it is the one agent with these hooks. Say no at the
prompt and everything else still works. Turn it on later with `deck setup`.

Both are entries in `~/.claude/settings.json` that run `deck hook`: one under
`Stop`, the catch, and one under `SessionStart` with the matcher `compact`, the
reload. To turn deck's hooks off, take out both. They are one choice: with the
catch left in, `deck setup` puts the reload back.

---

[← README](../README.md) · [The window](the-window.md) · [The walk](the-walk.md) · [Diagrams and pages](diagrams-and-pages.md) · [Theming](theming.md) · [How it works](how-it-works.md)
