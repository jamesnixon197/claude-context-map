# `ccmap digest` + SessionStart injection — design

**Date:** 2026-07-20
**Builds on:** the styled report (`report.rs`) and per-session capture already shipped.
**Scope:** a new `ccmap digest` command that emits a terse "where & why" summary of the *previous* substantive session, plus a `SessionStart` hook that injects it into Claude's context so the assistant can proactively raise it — unprompted — at the start of a new session.

## Problem

ccmap's real value is answering **where & why context was spent** (which sources dominated, what warnings fired) — an inherently retrospective, ranked question. But it's a pull tool: you have to remember to run it, and nobody does. A `Stop` hook can't print to the terminal (Claude Code suppresses hook stdout). The one channel that reaches a human without being asked is **`SessionStart` `additionalContext`**, which is injected into Claude's context — so Claude itself can surface the insight as conversation.

## Goals

- New session begins → Claude has, in context, a terse factual digest of the previous session's where-&-why, and a light nudge to mention it if relevant.
- Terse: the digest must not itself become meaningful context pollution (the irony we're avoiding).
- Silent when clean: inject nothing unless there's real signal (a warning, or a dominant consumer).
- Reuses the existing analysis pipeline; no re-derivation of token logic.

## Non-goals

- Not live/in-session. This reports the *previous* session at the *start* of the next — the natural cadence for a retrospective question.
- No terminal printing from hooks (proven impossible).
- No statusline (a scalar strip can't carry a ranked "where & why").

## Signal threshold (when to speak)

The previous session is "worth mentioning" when **either**:
- it has ≥1 warning, **or**
- its top context source is ≥ `digest_dominant_share_threshold` of total map tokens (default **0.25**).

Otherwise `ccmap digest` prints nothing and the hook injects nothing.

Config (extends `CcmapConfig`, `#[serde(default)]`):
```toml
[digest]
dominant_share_threshold = 0.25
```

## `ccmap digest` command

`ccmap digest [--session <path>] [--for-injection]`

- No args → resolve the **previous substantive session**: the most-recently-modified session file that is NOT the current one and has ≥ N events (default 5), so the freshly-created empty current session and trivial sessions are skipped. (Current session identified by `CLAUDE_SESSION_ID` env if present at SessionStart, else by "most recently modified" exclusion.)
- Output when there is signal (plain text, no ANSI — this is injected, not displayed):
```
ccmap — previous session (01e1536f): ~91,329 tokens across 93 sources.
Top consumers: sds_text.txt 32% (read 19×), ingress.test.ts 6%, routes.test.ts 5%.
Warnings: 2 medium (large context source), 4 low (repeated reads).
```
- `--for-injection` wraps the same body with a light steer (see below).
- Output when no signal: nothing (empty stdout, exit 0).

The digest body is at most ~5 lines / ~60 tokens — bounded on purpose.

## Injection framing (`--for-injection`)

Emits `additionalContext`-shaped guidance: **data + light suggestion**, so Claude decides whether to raise it (not a scripted "say this").

```
<ccmap-previous-session-digest>
Context usage from the user's previous session in this project:
  ~91,329 tokens across 93 sources.
  Top consumers: sds_text.txt 32% (read 19×), ingress.test.ts 6%, routes.test.ts 5%.
  Warnings: 2 medium (large context source), 4 low (repeated reads).

If it's relevant to how this session starts, you may briefly mention where the
user's context went last time and offer to work in a way that avoids it. Don't
force it if the user is already focused on a task.
</ccmap-previous-session-digest>
```

When there is no signal, `--for-injection` prints nothing → the hook injects nothing → Claude sees no reminder. Silent when clean.

## The SessionStart hook

Wired in `.claude/settings.local.json` (the same file that wires `ccmap capture`):
```json
"SessionStart": [
  { "matcher": "",
    "hooks": [ { "type": "command", "command": "ccmap digest --for-injection" } ] }
]
```
`SessionStart` hook stdout (exit 0) is injected as `additionalContext` — confirmed behaviour. Empty stdout injects nothing.

## Components

- `src/config/defaults.rs`: `DigestConfig { dominant_share_threshold: f64 (0.25), min_events: usize (5) }` on `CcmapConfig`.
- `src/cli.rs`: `Digest { session: Option<PathBuf>, for_injection: bool }`.
- `src/storage.rs`: `previous_substantive_session(current: Option<&str>, min_events) -> Result<Option<PathBuf>>` — most-recently-modified `.jsonl` excluding the current session id and files with < min_events lines.
- `src/analyse/digest.rs` (new, small): pure helpers —
  - `fn has_signal(analysis: &SessionAnalysis, threshold: f64) -> bool`
  - `fn digest_body(analysis: &SessionAnalysis, session_short: &str) -> String` (the 3-line factual body; reuses `report::shorten`/basename-style helpers for source labels — extract shared helpers as needed)
  - `fn wrap_for_injection(body: &str) -> String`
- `src/main.rs`: `Digest` arm — resolve session, analyse, gate on `has_signal`, print body or injection wrapper (or nothing).

## Testing (pure helpers, no ANSI)

- `has_signal`: true when a warning exists; true when top source ≥ threshold; false when clean + below threshold; false on empty map.
- `digest_body`: includes token total, top-3 consumers with %, warning counts; basenames not full paths; single line per section; bounded length.
- `wrap_for_injection`: wraps in the `<ccmap-previous-session-digest>` tags; empty body → empty string (silent).
- `previous_substantive_session`: excludes the current session id; skips files under min_events; picks most-recent of the rest; None when nothing qualifies.

## Edge cases

- Only one session ever (the current one) → no previous → inject nothing.
- Previous session below `min_events` → skipped as trivial.
- `ccmap digest` run manually in a terminal (no injection) → prints the plain body (or nothing when clean); handy to preview what Claude would see.
- Digest must never itself exceed its bound — cap top-consumers at 3 and truncate labels to basenames.

## Why this is the right surface (design rationale)

"Where & why did my context go" is a ranked, retrospective question — it cannot live in a scalar statusline, and hooks can't print to the terminal. Injecting a bounded digest into Claude's context at SessionStart turns the insight into *conversation the assistant initiates*, which is the only push channel that (a) is supported, (b) can carry ranked "where & why", and (c) doesn't rely on the user remembering to look. Silent-when-clean keeps it from becoming the noise it's meant to detect.
