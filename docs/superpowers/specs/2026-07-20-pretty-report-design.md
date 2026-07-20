# Pretty `ccmap` report — design

**Date:** 2026-07-20
**Scope:** Redesign the terminal output of `ccmap latest` / `ccmap analyse` (the `print_analysis` renderer in `src/analyse/report.rs`). No changes to capture, normalisation, summarisation, or warning logic — this is presentation only.

## Problem

The current report is a flat wall of same-weight text. Long absolute paths and multi-line shell commands dominate each line, the `[kind] — N occurrence(s), ~N tokens` metadata is ragged and unaligned, there is no colour or hierarchy, and the value the user actually wants (which sources cost the most tokens) is buried on the right. An earlier hotfix blunt-truncated labels with `…`, which cut shell commands mid-string and lost meaning.

## Goals

- One screen, scannable at a glance; hotspots (high-token sources) pop.
- Token cost is the primary axis — it drives ordering, a per-row bar, and a share %.
- Labels are *intelligently* fit, never blunt-cut: paths smart-shortened, shell commands summarised to what actually ran.
- A `--all` escape hatch prints every row with full, untruncated labels.
- Degrade cleanly: no colour when piped / `NO_COLOR` set / not a TTY.

## Non-goals

- No change to what is captured or how tokens are estimated.
- No box-drawing table framework (kept lightweight, terminal-first per product spec §UX principles).

## Dependency

Add **`owo-colors`** (single lightweight crate, no transitive deps) for ANSI styling, and use `std::io::IsTerminal` (std, no dep) to auto-disable styling when stdout is not a terminal. Honour the `NO_COLOR` env convention.

## Target output

```text
 Session  4530eb1b-7bf6-49ac-89dc-c7774ad5de7c
──────────────────────────────────────────────────────────────────────
 Files read 6    Edited 7    Written 0    Bash 22    Subagents 2
 Instructions 2    Searches 0    Path lists 0
 Context  ~14,754 tokens
──────────────────────────────────────────────────────────────────────
 Context map                                         top 15 of 41 sources

  ████████░░░░░░  35%  file    …/scratchpad/sds_text.txt          29,670
  ██░░░░░░░░░░░░   6%  spec    …/mso-originations-integration.md   5,247
  █░░░░░░░░░░░░░   5%  test    …/mso/src/mso/routes.test.ts        4,437
  █░░░░░░░░░░░░░   5%  file    …/mso/src/mso/ingress.test.ts       3,820
  ░░░░░░░░░░░░░░   4%  shell   cargo install --path .              3,962
  …
  + 26 more sources                                              ~8,210
──────────────────────────────────────────────────────────────────────
 Warnings  2 medium · 3 low

  ● medium  Large context source  …/scratchpad/sds_text.txt (~13,611 tok)
  ● low     Repeated read ×19     …/scratchpad/sds_text.txt
```

Colour roles (auto-off when not a TTY):
- **Session id / total tokens** — bold/bright.
- **Kind tag** — one stable colour per kind (file=blue, shell=magenta, mcp=cyan, subagent=yellow, instruction=green, edit/write=dim, unknown=dim).
- **Token bar** — the filled portion coloured by the row's share (hot = red/yellow for big consumers, dim for small).
- **Warnings** — medium=yellow dot, low=dim dot, high=red dot.

## Components (all in `report.rs`, small pure helpers, each unit-tested)

1. `format_kind_tag(kind) -> &str` — short fixed-width label (`file`, `shell`, `mcp`, `sub`, `instr`, `edit`, `write`, `web`).
2. `shorten_path(path, budget) -> String` — collapse leading repo/home prefix to `…/`, keep the meaningful tail within `budget` chars on a path-separator boundary. Never cuts mid-segment when it can drop a whole leading segment instead.
3. `summarise_command(cmd) -> String` — for shell labels: take the first real program + args on the first logical line; if it contains a heredoc, render `prog «N-line heredoc»`; collapse internal whitespace. Produces a single meaningful line, not a prefix.
4. `fit_label(kind, raw, budget, all) -> String` — dispatch: paths → `shorten_path`, shell → `summarise_command`, else collapse-whitespace. When `all == true`, return the full collapsed label untruncated.
5. `token_bar(share_fraction, width) -> String` — `█`/`░` bar of fixed cell width.
6. `format_count(n) -> String` — thousands separators (`29,670`).
7. `print_analysis(analysis, opts)` — orchestrates: header band, rule lines, top-N map + rollup, warnings. `opts` carries `all` and a resolved `use_color` + terminal width.

## Length handling

- Default: **top 15** sources by tokens, then one rollup line `+ N more sources  ~T` summing the tail.
- `--all`: print every source, full untruncated labels, no rollup.
- Column layout sizes the label column to `terminal_width − (fixed columns)`, clamped to a sane min; falls back to 80 cols when width is unavailable.

## CLI

Add `--all` (alias `-a`) to the `latest` and `analyse` subcommands (clap). Threads a small `ReportOptions { all: bool }` into `print_analysis`. Default behaviour (no flag) is the pretty top-15 view.

## Testing

Pure helpers are unit-tested (no ANSI in assertions — test the plain-text transform, style is applied at print time):
- `shorten_path`: prefix collapse, boundary safety, UTF-8 safety, budget respected, already-short unchanged.
- `summarise_command`: heredoc → `«N-line heredoc»`, pipeline first-stage, multi-line collapse, short command unchanged.
- `fit_label`: `all=true` returns full text; dispatch per kind.
- `token_bar`: 0%, 100%, rounding, fixed width.
- `format_count`: separators.
- Colour gating: a `use_color=false` path emits zero ANSI escapes (assert no `\x1b`).

## Backwards / edge cases

- Piped output (`ccmap latest | less`) and `NO_COLOR` → plain text, still aligned.
- Empty context map → omit the map section (existing behaviour preserved).
- No warnings → `Warnings  none` (existing behaviour preserved).
- Very narrow terminals → min label budget, still one row per source.
