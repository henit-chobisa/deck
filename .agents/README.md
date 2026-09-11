# .agents

Skills for working **on** deck, not deck's own skill — that one lives at
`crates/app/skill/SKILL.md` and ships inside the binary for agents to use deck
*with*.

Each of these is a workflow that has been got wrong at least once, written down
with the reason attached:

- **`change-the-window`** — `crates/app` is GPUI, and four mistakes account for
  nearly every bug it has had. Three abort the process rather than failing.
- **`change-the-protocol`** — `PROTOCOL.md` is frozen at v1. Additions only, and
  a field has to land in five places to be real.
- **`change-the-skill`** — editing the file that ships in the binary. It has been
  rewritten twice and come back worse both times.
- **`cut-a-release`** — six things in four places, in an order learned by getting
  it wrong.

The convention is `~/.agents/skills`, which several tools read directly and the
rest reach by symlink. A skill here is scoped to this repository.

[AGENTS.md](../AGENTS.md) is the general guide; these are the specific ones.
