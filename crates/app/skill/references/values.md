# Values on the line

Part of §3: `--values` and `deck values`. Read it before you put a value on a line.

§1–§2 and §6–§8 are in [SKILL.md](../SKILL.md), §3 in [groups.md](groups.md), §4 in [pictures.md](pictures.md), §5 in [pages.md](pages.md). "Plan the context" and the other sections on how to tell it are in [writing.md](writing.md).
## Values on the line, when the claim is a value

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

**Leave it out when** the claim is about structure (who calls whom, why a module
exists), when the value is plain from the code (`const limit = 10` needs no
`10` beside it), or when nobody's claim turns on it. A value that could be
removed without losing anything is noise: remove it.

**Only values you have.** From a test run, a log, a payload you read, a
debugger, or arithmetic anyone can check from the code. A value on a line reads
as observed, so one you are inferring goes in the prose, said as an inference —
*I expect orderId is undefined here* — and not on the line. **Never secrets:**
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
  values in a group; more is two groups.
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
