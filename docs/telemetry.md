# Usage counts

deck can share anonymous usage counts, so we know what to make next: how many
people use it, on which versions, and which parts they reach for.

**Nothing is counted or sent until you say yes.** You're asked once, either in
`deck setup` or in a strip above a deck window's footer, and either answer is
final until you change it. Setup only asks at a terminal: run without one, by
the installer or by an agent, it records no answer, and the window asks.

## What is sent

One event, at most once an hour, while a deck window is open (a window looks
every five minutes whether one is due):

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
| `ask_now` | `1` | Questions sent with *Ask now*. |
| `questions_offered` | `30` | Suggested questions written into groups. |
| `questions_asked`, `questions_edited` | `3`, `1` | Suggested questions clicked, or ⌘-clicked (Ctrl-clicked on Windows and Linux) into the comment box. |
| `walks` | `1` | Walks started. |
| `upgrades` | `1` | Upgrades to a newer deck. |
| `agents` | `{"claude-code": 4}` | Which agent created the decks: `claude-code`, `codex`, `cursor`, `amp`, `gemini` or `other`. Worked out from the *names* of environment variables, never their values. |

A count that is zero is left out, and so is `agents` when no deck was created.
Some counts overlap on purpose: a suggested question that is clicked counts as
`questions_asked`, and also as an `ask_now` and a `comment`, because that is
what it becomes.

Alongside these, every event carries the fields PostHog needs to file it, and
nothing that identifies you:

| Field | Value | Why |
|---|---|---|
| `event` | `deck usage` | The event's name. |
| `api_key` | `phc_…` | deck's PostHog project key. It can add events, never read them. |
| `$process_person_profile` | `false` | No person is created for the id. |
| `$geoip_disable` | `true` | No location is looked up. |
| `$ip` | `null` | No address is attached. |
| `$lib` | `deck` | Which program sent it. |

The request also carries a `User-Agent: deck/<version>` header.

**Never sent:** code, file paths, repository or project names, deck titles,
narration, comments, questions, or anything you type.

The counts go to [PostHog](https://posthog.com), hosted in the EU. Each event is
sent without a person profile and with location lookup turned off, and the
project discards IP addresses.

## Seeing it for yourself

```sh
deck telemetry show      # exactly what the next send would carry, and when — or that nothing would be sent, and why
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
  deck makes no request of its own. A `config.toml` that will not read counts
  as this too, since it may be the one saying so.

`deck telemetry on` turns it back on.
