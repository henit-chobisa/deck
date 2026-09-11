---
name: cut-a-release
description: >-
  Ship a new version of deck. Use whenever a release, a version bump or a tag is
  asked for — "cut a release", "log a release", "ship it", "let's release". Six
  things have to move together and they are in four different places; missing
  one leaves somebody installing a version that reports the wrong number or a
  tap pointing at a tag that does not exist.
---

# Cutting a release

Six things move together. They have been got wrong in every order, so this is
the order.

## Before anything

```sh
cargo fmt --all
RUSTFLAGS="-D warnings" cargo clippy --workspace --all-targets
cargo test --workspace
```

A release is the worst time to find out clippy is unhappy, because the tag is
already pushed by the time you notice.

## 1. What is the next number?

**Check the tags, do not assume.**

```sh
git tag | sort -V | tail -3
```

`Cargo.toml` is not the authority here — it has been behind a tag before. If
`v0.0.8` exists, the next one is `v0.0.9`, whatever the manifest says.

## 2. Bump the manifest

```sh
sed -i '' 's/^version = "0.0.8"$/version = "0.0.9"/' Cargo.toml
cargo check -q          # updates Cargo.lock, which must be committed too
```

The four crates take `version.workspace = true`, so this is the only edit. If
you skip `cargo check`, `Cargo.lock` still says the old version and the commit
is half a bump.

## 3. Commit and tag

```sh
git add -A
git commit -m "Cut 0.0.9

One line on what it is."

git tag -a v0.0.9 -m "deck v0.0.9

What changed, in a few lines."

git push origin main --follow-tags
```

`--follow-tags` pushes both. Pushing the branch and forgetting the tag is the
most common way to end up with a release nobody can install.

## 4. Point the tap at it

**Edit the tap Homebrew actually reads**, not a clone somewhere else:

```sh
TAP=/opt/homebrew/Library/Taps/henit-chobisa/homebrew-deck
sed -i '' 's/tag: "v0.0.8"/tag: "v0.0.9"/' "$TAP/Formula/deck.rb"
git -C "$TAP" commit -am "Point at v0.0.9"
git -C "$TAP" push origin main
```

This has gone wrong once and it was confusing: the edit was made in a clone
under `/tmp`, pushed to GitHub, and `brew upgrade` kept saying the current
version was already installed — because Homebrew reads its own checkout and
nothing had told it to fetch. Editing the one under `Library/Taps` avoids the
whole question.

Confirm before telling anybody it is ready:

```sh
brew info deck | head -2      # should say 0.0.8 -> stable 0.0.9
```

## 5. Write the release

```sh
gh release create v0.0.9 \
  --repo henit-chobisa/deck \
  --title "v0.0.9 — a short name for what changed" \
  --notes-file /tmp/notes.md \
  --prerelease
```

Notes are for somebody deciding whether to upgrade. Lead with what they get,
then what was broken. Say a limitation once, where it is relevant, and not
again — a release note that repeats the same caveat every time reads as a
project apologising for itself rather than one shipping.

## 6. Say what the reader has to do

Tell them the command, and whether they need it:

```sh
brew upgrade deck
```

If the release is only the skill, say so: `crates/app/skill/SKILL.md` is read
from disk by their agents, so a copy to `~/.agents/skills/deck/SKILL.md` reaches
them immediately and the upgrade only re-syncs the copy inside the binary.

**Warn them not to run `deck setup` before upgrading** in that case — it writes
the binary's older embedded skill back over the newer file.

## The checklist

- [ ] fmt, clippy with `-D warnings`, tests
- [ ] next number taken from `git tag`, not from the manifest
- [ ] `Cargo.toml` and `Cargo.lock` both bumped
- [ ] tag pushed (`--follow-tags`)
- [ ] tap under `Library/Taps` edited and pushed
- [ ] `brew info deck` shows the new version
- [ ] release notes written
- [ ] the reader told what to run
