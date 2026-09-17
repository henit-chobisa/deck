# Live review: text-first implementation record

Status: accepted design checkpoint for issue [#11](https://github.com/henit-chobisa/deck/issues/11).

This note records the decisions that must stay stable while the implementation is split into testable changes. The product amendment is controlling: Deck first proves that a reader can resolve uncertainty through an anchored text conversation. Speech is optional delivery, not the product and not a prerequisite for the first useful loop.

## Baseline

The implementation starts from commit `192181c` (`Let a deck be listened to`). Before this work, the checkout had uncommitted changes in `README.md`, `crates/app/src/setup.rs`, and `crates/app/src/speech.rs`. They improve Premium/Enhanced voice ranking and update the macOS settings path. They belong to the reader and must be preserved.

The baseline passes:

- `cargo fmt --all -- --check`
- `RUSTFLAGS="-D warnings" cargo clippy --workspace --all-targets`
- `cargo test --workspace` (226 tests)

The present implementation has no live command channel. Group polling ends when the authored deck is sealed, comments resolve their pane when saved, and `deck wait` only polls the final review file. These are constraints to replace, not capabilities to build on.

## Accepted compatibility additions

Version 1 remains frozen. The live review feature uses only additive changes:

1. `Kind::Noted` records acknowledgement without changing the meaning of `nit`.
2. `Review.transcript` is optional and omitted for a static review.

Existing fields keep their types and meanings. Unknown kinds still become `question`, so an old client can read a `noted` comment but loses its positive meaning. That limitation is documented rather than called lossless compatibility. Transcript event decoding must preserve actionable comments when a newer event type is unknown.

If either addition is rejected later, ordinary `deck wait` cannot truthfully return the complete conversation. A private sidecar is not an equivalent implementation of issue #11.

## Product contract

Scan remains the default. A live agent does not open, raise, or focus a window and speech never starts automatically. Discuss is an escalation at one uncertain point: the reader creates an immutable anchored question, the original agent receives it, and its answer appears beside the evidence.

The following are distinct facts and remain distinct in state and on the wire:

- text was displayed;
- audio completed;
- the reader acknowledged one point;
- a question received an answer;
- a must-fix remains actionable;
- the review was submitted.

None implies approval.

## Module seams

`deck-core::live` is the pure state module. Its small interface accepts an action plus explicit time or external results and returns effects or a typed refusal. It owns lifecycle, following, audio generations, stage identity, and immutable captured anchors. It does not read files, know about GPUI, or execute speech.

The file mailbox belongs in `deck-cli::live`. It canonicalizes the deck path, owns the advisory lock and generation directory, atomically publishes bounded typed messages, and implements cursor reads. The app-side owner belongs in `deck-app::live`; it survives hide/reopen and routes accepted effects to the current view. GPUI render methods never perform mailbox I/O.

Speech is a separate seam. The caller starts one utterance, cancels it, and observes typed status carrying the utterance identity. System, command, and later Google are adapters behind that interface. The UI does not know whether playback is a child process or streamed PCM.

## Local transport

The first transport is a private file-backed mailbox under `deck_core::home::deck()/live`. A session key is derived from the canonical deck-directory path, not only `Header.id`. A session-owned OS advisory lock is authority; PID and heartbeat data are diagnostic only. Each open creates a random generation and keeps requests, replies, ordered events, state, and the transcript journal inside that generation.

Committed files use a temporary sibling plus rename. Request ids are idempotency keys: the same id and payload replays its prior result, while the same id with different content is an error. Expired unapplied requests are withdrawn. Events are append-only and use generation-scoped sequences. Explicit cursors are the reliable interface; the convenience cursor is at-least-once and advances only after JSON reaches stdout.

A sealed authored deck does not end this owner. Hide retains it and stops audio. Close and submit publish terminal outcomes long enough for late waiters to observe them.

## Source snapshots and stage movement

The first `show` implementation only targets code panes already present in the selected authored group. The pane keeps two ranges:

- the authored range, which fixes proposed-replacement insertion geometry;
- the live spotlight, which may move and is revealed independently.

Before applying source coordinates, the owner compares the current file with the retained snapshot. A mismatch returns `source-changed`; it does not apply current-disk lines to old displayed text. Continuous source refresh is deferred.

A one-second minimum dwell applies between different accepted targets. Repeats are no-ops. Early movement is refused with a retry duration and is never queued. Reader scrolling, selection, composition, group navigation, hiding, and inactive-window state pause following. Follow resumes control but does not replay refused movement.

The anchor captured on keydown or composer open contains the group, optional ref and file, source range, exact quote, snapshot identity, and optional stage/transcript-entry identity. Saving never resolves a pane index again.

## Command outcomes

New live commands reserve these process outcomes:

| exit | meaning |
| --- | --- |
| `0` | acknowledged result or event, including a normally interrupted utterance |
| `1` | operational failure |
| `3` | timeout |
| `4` | closed or lost owner without a review |
| `5` | recoverable refusal: not ready, paused, busy, paced, or source changed |

Stdout is JSON only. Diagnostics go to stderr. `show` succeeds only after application, and `say` records original Markdown even when `--silent`, muted, unavailable, interrupted, or failed. The first text-loop milestone requires `say --silent`; a bare live speech request reports unsupported until the cancellable worker is connected.

## Provider research and deferral

Google is deliberately after the text-loop experiment. The implementation gate remains an authenticated spike against primary sources:

- Google documents Chirp 3: HD pricing and the free character allowance on its [Text-to-Speech pricing page](https://cloud.google.com/text-to-speech/pricing).
- `StreamingSynthesize` is a bidirectional RPC whose first request is configuration-only, followed by input requests, in the [streaming guide](https://cloud.google.com/text-to-speech/docs/create-audio-text-streaming) and [official protobuf](https://github.com/googleapis/googleapis/blob/master/google/cloud/texttospeech/v1/cloud_tts.proto).
- Authentication starts with Application Default Credentials as documented in [Google Cloud authentication](https://cloud.google.com/text-to-speech/docs/authentication).
- Provider limits must be checked against the current [quota documentation](https://cloud.google.com/text-to-speech/quotas), without treating bytes and characters as interchangeable.

No cloud adapter is called implemented until a reader-provided ADC run establishes first-chunk playback, actual PCM format, cancellation latency, rate behavior, and playback before full paragraph completion. The command backend and text-only loop can ship while this is blocked.

## First evaluation

Use real straightforward, multi-file, and uncertain reviews. Compare scan-only, anchored text discussion, and optional speech separately. Record time to a decision, time resolving questions, meaningful issues, targeting/control failures, effort, task complexity, and prior familiarity. Do not add telemetry or upload artifacts. Proceed to native Google work only if readers voluntarily repeat the live discussion and do so without anchor/control failures or an evident loss of review quality.
