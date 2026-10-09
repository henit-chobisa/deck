# Usage counts

deck can share anonymous usage counts, so we know what to make next: how many
people use it, on which versions, and which parts they reach for.

**Nothing is counted or sent until you say yes.** You're asked once, either in
`deck setup` or in a strip above a deck window's footer, and either answer is
final until you change it.

## What is sent

One event, at most once an hour, while a deck window is open:

| Field | Example | What it is |
|---|---|---|
| `distinct_id` | `3f9c…e21a` | A random id made on your machine the first time counts are sent. It is not derived from your machine, name, email or anything else. |
| `version` | `0.1.5` | The deck you run. |
| `os`, `arch` | `macos`, `aarch64` | Your operating system and processor type. |
| `decks_created` | `4` | `deck new`, since the last send. |
| `decks_opened` | `3` | Deck windows opened. |
| `groups_written` | `17` | `deck group`. |
| `groups_viewed` | `21` | Groups brought on screen. |
| `comments` | `5` | Comments left on decks. |
| `reviews` | `2` | Reviews submitted. |
| `ask_now` | `1` | Times *Ask now* sent a question. |
| `questions_offered` | `30` | Suggested questions written into groups. |
| `questions_asked`, `questions_edited` | `3`, `1` | Suggested questions clicked, or ⌘-clicked into the comment box. |
| `walks` | `1` | Walks started. |
| `upgrades` | `1` | Upgrades to a newer deck. |
| `agents` | `{"claude-code": 4}` | Which agent created the decks: `claude-code`, `codex`, `cursor`, `amp`, `gemini` or `other`. Worked out from the *names* of environment variables, never their values. |

A count that is zero is left out.

**Never sent:** code, file paths, repository or project names, deck titles,
narration, comments, questions, or anything you type.

The counts go to [PostHog](https://posthog.com), hosted in the EU. Each event is
sent without a person profile and with location lookup turned off, and the
project discards IP addresses.

## Seeing it for yourself

```sh
deck telemetry show      # exactly what the next send would carry
deck telemetry status    # whether counts are shared, and if not, why not
```

The counts waiting to be sent are in `~/.deck/usage.json`.

## Turning it off

Any one of these is enough, and all of them win over a yes:

- `deck telemetry off`. Anything counted and not yet sent is forgotten, and so is the id.
- `DO_NOT_TRACK=1` in your environment.
- `DECK_TELEMETRY=0` in your environment.
- A `CI` environment.
- `[updates] automatic = false` in `~/.deck/config.toml`, which already means
  deck makes no request of its own.

`deck telemetry on` turns it back on.
