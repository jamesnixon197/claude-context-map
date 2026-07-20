# Session-end digest becomes the default — design

## Status

Approved by user on 2026-07-20. Ready for implementation planning.

## Problem

`ccmap digest --for-injection` and its `SessionStart` injection format already exist and are
tested (`docs/superpowers/specs/2026-07-20-session-digest-injection-design.md`,
`docs/superpowers/plans/2026-07-20-session-digest.md`). They answer the real product gap in this
tool: `ccmap`'s value ("where did context go last session") is retrospective, but it's a pull
command — nobody remembers to run it. The design already solved this by injecting a terse digest
into the next session's context via a `SessionStart` hook, so Claude itself can raise it unprompted.

But `ccmap init`'s `write_local_settings` (`src/config/init.rs`) was never updated to actually wire
that hook. It writes a `SessionStart` array containing only `ccmap capture`. The digest hook was
manually added to one project's `settings.local.json` (per Task 5 of the original plan, explicitly
scoped to "the decisioning repo, not this crate") for testing, but never became part of what every
`ccmap init` generates going forward. That's the concrete gap this design closes.

Separately, `write_local_settings` unconditionally overwrites the entire `settings.local.json` file
on every `init` run. If a developer has customized that file (added other hooks, other tool
permissions), re-running `ccmap init` — which will now be the mechanism for rolling out the digest
hook to existing projects — would silently destroy their customization. This must be fixed as part
of making the digest hook a safe default to roll out.

## Goals

1. Every fresh `ccmap init` wires a `SessionStart` hook running `ccmap digest --for-injection`,
   alongside the existing `ccmap capture` hook.
2. Re-running `ccmap init` on an already-initialized project safely adds any hooks the current
   `ccmap` version expects but the project's `settings.local.json` is missing — without touching or
   removing anything else in that file (other hooks, other settings, prior manual edits).
3. A developer who finds the digest noisy can turn it off with one config line, no hook-JSON editing.

## Non-goals

- No change to the digest's own analysis logic (`has_signal`, `digest_body`,
  `wrap_for_injection`, `previous_substantive_session`) — all already correct and tested.
- No general-purpose JSON-merge library or schema for `settings.local.json` — only the specific,
  narrow merge behavior described below.
- No UI/prompt asking the developer whether they want the hook; per-project opt-out is a config
  value, not an interactive step during `init`.

## Design

### 1. Wire the digest hook

Add a second entry to the `SessionStart` hooks array `write_local_settings` generates:

```json
"SessionStart": [
  { "matcher": "", "hooks": [ { "type": "command", "command": "ccmap capture" } ] },
  { "matcher": "", "hooks": [ { "type": "command", "command": "ccmap digest --for-injection" } ] }
]
```

Both entries have `matcher: ""` (fire on every SessionStart) and both already run today for
`ccmap capture` alone — Claude Code runs every matching hook entry in an event's array, so adding a
second entry does not affect the first.

### 2. `[digest] enabled` config flag

Extend `DigestConfig` (`src/config/defaults.rs`) with a third field:

```rust
pub struct DigestConfig {
    pub enabled: bool,                    // NEW, default true
    pub dominant_share_threshold: f64,    // existing, default 0.25
    pub min_events: usize,                // existing, default 5
}
```

The `Command::Digest` arm in `main.rs` checks `config.digest.enabled` first, before resolving a
session or running any analysis. If `false`, it returns immediately with no output — the same
"silent" contract already used for the no-signal case, so a disabled project behaves identically
(from the hook's perspective) to a project where the previous session was simply clean.

A developer disables it with:

```toml
[digest]
enabled = false
```

### 3. Merge instead of overwrite in `write_local_settings`

Current behavior: `write_local_settings` always builds the full hardcoded `settings` JSON object
and writes it, unconditionally replacing whatever was at `storage.settings_file` before.

New behavior:

- If `storage.settings_file` does not exist: write the full hardcoded settings object, same as
  today (now including the digest hook from Part 1). No behavior change for a brand-new project.
- If it exists: read and parse it as JSON.
  - For each hook event ccmap manages (`SessionStart`, `InstructionsLoaded`, `PostToolUse`,
    `PostToolBatch`, `SubagentStart`, `SubagentStop`, `Stop`):
    - If the event key is missing from the existing file's `hooks` object, add it with ccmap's full
      entry array for that event.
    - If the event key exists, walk ccmap's entries for that event. For each one, check whether any
      existing entry under that event already has a hook with the identical `command` string
      (e.g. `"ccmap capture"`, `"ccmap digest --for-injection"`). If a match exists, skip adding it
      (idempotent — re-running `init` any number of times converges, never duplicates). If no match
      exists, append ccmap's entry to the existing array.
  - Every other key in the parsed file — any hook event ccmap doesn't manage, any hook entry within
    a managed event that isn't one of ccmap's own commands, any other top-level setting — is left
    byte-for-byte untouched in the in-memory structure and is present in the final write.
  - Write the merged structure back to `storage.settings_file`.

This makes `ccmap init` safe and idempotent to re-run on any project, at any time, which is the
intended way existing (pre-digest-hook) projects pick up the new default: run `ccmap init` again.

## Testing implications

- `write_local_settings` (or a newly extracted pure merge helper) needs tests for:
  - Fresh file (no existing settings.local.json): output is identical in shape to today's, plus the
    new digest hook entry.
  - Existing file with a hook event ccmap doesn't manage (e.g. a hypothetical `PreToolUse` a
    developer added themselves): that event and its contents are preserved unchanged after merge.
  - Existing file already containing ccmap's exact hook commands (simulating a second `init` run):
    merge produces no duplicate entries — array lengths unchanged.
  - Existing file missing only the new digest hook (simulating an old project generated before this
    change): merge adds exactly the digest hook entry, leaves the existing `ccmap capture` entry
    and any other content alone.
- `DigestConfig`: default `enabled` is `true`; TOML with `enabled = false` parses correctly.
- `Command::Digest` arm: `enabled = false` produces empty stdout regardless of the previous
  session's actual signal (verifiable by constructing a config with `enabled: false` and a session
  file that would otherwise definitely signal, e.g. one with warnings).

## Risks / open questions carried into implementation

- **JSON merge fidelity:** hand-rolling this merge over `serde_json::Value` needs care to avoid
  reordering keys in a way that produces noisy diffs for developers who track `settings.local.json`
  in git, or silently coercing types (e.g. an event value that's an object instead of an array, if a
  developer hand-edited it unusually) — the implementation should handle a malformed/unexpected
  existing shape by falling back safely (e.g. treating an unexpected non-array event value as "leave
  alone, don't touch, log nothing more than what's needed") rather than panicking or corrupting the
  file.
- **Command-string matching is exact-string, not semantic:** if a developer manually wrote
  `"ccmap  digest --for-injection"` (extra space) or a different flag order, it would not be
  recognized as "already ours" and ccmap would add a second, functionally-duplicate hook entry. This
  is an accepted limitation of the simpler matching approach chosen over a managed-entry marker.
