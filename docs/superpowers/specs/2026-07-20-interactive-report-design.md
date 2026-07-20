# Interactive-ish `ccmap` report — design

**Date:** 2026-07-20
**Builds on:** `2026-07-20-pretty-report-design.md` (the styled report already shipped).
**Scope (Phase 1 — this doc):** make the *static* report navigable and self-explanatory using terminal hyperlinks, better shell summarisation, a drill-in subcommand, and filtering. **Phase 2 (a full `ratatui` TUI) is explicitly out of scope here** — noted at the end as a follow-up project.

## Goals

- Linear ticket rows and file rows are **clickable** (OSC 8 hyperlinks) — ⌘-click opens the ticket / file.
- Collapsed shell rows tell you **what actually ran** (skip `echo` headers, show verb + targets), with a way to see the **full script** on demand.
- **Filter** the map by source kind, and expand beyond the top 15 on demand.
- Everything degrades cleanly: no hyperlinks when the terminal doesn't support them / output is piped / `NO_COLOR`.
- Stays a print-and-exit, pipe-friendly Unix tool.

## Non-goals

- No full-screen TUI, no event loop, no alternate screen (Phase 2).
- No change to capture / normalisation / token estimation.

## 1. Terminal hyperlinks (OSC 8)

Emit `OSC 8 ; ; <url> ST <text> OSC 8 ; ; ST` for link rows. Gated exactly like colour: only when stdout is a TTY and not `NO_COLOR`. A `Painter::link(text, url)` method; when links are disabled it returns `text` unchanged.

- **Linear rows** (`ContextSourceKind::McpTool { server: "linear" }` whose label parses out a ticket id like `DEC-259`): link to `{linear_base}/issue/DEC-259`.
- **File rows** (`FileRead` / `FileEdit` / `FileWrite` / `Instruction` with an absolute path): link to `file://<abs-path>`.
- Warning rows that reference a file path: same `file://` treatment.

Ticket-id extraction: match `[A-Z]+-\d+` inside the MCP label. Only Linear server labels get the Linear base; unknown servers stay plain.

## 2. Link configuration

Extend `CcmapConfig` with an optional links section:

```toml
[links]
linear = "https://linear.app/finovatech"
```

Resolution order for the Linear base (first non-empty wins):
1. `LINEAR_URL` environment variable.
2. `config.toml` `[links] linear`.
3. None → Linear rows render as plain text (no link), everything else unaffected.

New config type `LinkConfig { linear: Option<String> }`, `#[serde(default)]`, added to `CcmapConfig`. Trailing slash on the base is normalised. This threads into `ReportOptions` as a resolved `linear_base: Option<String>` so `report.rs` stays free of env/config lookups (kept in `main.rs`, like `use_color`).

## 3. Shell summarisation (compose all four)

Applied in order when building a shell row's display label:

a. **Skip `echo` section headers.** Treat a stage whose command is `echo "=== … ==="` (or `printf` of a banner) as a *label*, not the work. Walk to the next meaningful stage. If the header is the ONLY stage, keep it (it *is* the command).

b. **Verb + targets extraction.** For the chosen stage, produce `<verb> · <targets>`:
   - verb = the primary program (`grep`, `cargo test`, `python3`, `sed`, `find`).
   - targets = the file-ish arguments (paths, globs, `*.ts`), de-duplicated by basename, capped at ~3 with `+N` overflow.
   - Falls back to the plain summarised command when no targets are detectable.
   - Example: `grep -rn "mortgages/v1" engine-runtime --include=*.ts` → `grep · engine-runtime *.ts`.
   - Heredocs still render `python3 «14-line heredoc»`.

c. **`ccmap show <n>` drill-in.** New subcommand printing the full, untruncated record for the n-th source (by token rank). Prints: rank, kind, full label / full command text, occurrences, tokens, trigger reasons, and (for shell) the entire script verbatim.

d. **`--detail` flag** on `latest` / `analyse`: under each *shell* row in the visible set, print the full script dimly indented. Default off (scan view stays one line per source).

## 4. Filtering & sizing flags

On `latest` and `analyse`:
- `--kind <k>` (repeatable): show only sources of the given kind(s). Accepts `file`, `shell`, `mcp`, `instr`, `edit`, `write`, `sub`, `web`, `search`, `paths`, `prompt`, `session`. e.g. `ccmap latest --kind mcp --kind file`.
- `--top <N>`: show N rows instead of the default 15.
- `--all` (existing): show every row, full labels.
- Filters and `--top` compose: filter first, then take top N of the filtered set.

**Percentages/bars stay relative to the whole session's total tokens** (decided): filtering never re-bases the %, so a shell source's 2% is always 2% of everything. The rollup line sums the hidden remainder *of the filtered view*.

## 5. Row index (rank)

Prefix each map row with its 1-based token rank in the **full** sorted list: `" 3."`. The rank is stable regardless of `--top`/`--kind` (it's the source's global position), so the number you see is the number you pass to `ccmap show`. Right-aligned to 3 cols. Rank is computed once over the full sorted `context_map`, then carried on the row so filtering doesn't renumber.

## 6. Layout polish

- **Row spacing**: a blank line cadence (every row, or grouped) so per-row bars read individually instead of forming one solid spine. (Exact cadence: one blank line between rows in the default view; suppressed in `--all` to keep long lists dense. Open to tuning in review.)
- Rollup line gains the affordance: `+ 79 more sources   ~23,634   ·  --all`.
- One extra space of gutter between `%`, kind, and label columns.

## Target output (default `ccmap latest`)

```text
 Context map  top 15 of 94 sources

   1.  ████████░░░░░░  32%  file    …/scratchpad/sds_text.txt              29,670

   2.  ██░░░░░░░░░░░░   6%  file    …/mso/src/mso/ingress.test.ts           5,761

   8.  ░░░░░░░░░░░░░░   2%  mcp     linear: get_issue(DEC-259)              2,220   ← ⌘-click
   9.  ░░░░░░░░░░░░░░   2%  shell   grep · scenario.ts ingress.ts           1,704
  …
  + 79 more sources                                      ~23,634   ·  --all
```

`ccmap show 9`:
```text
 #9  shell  ·  1,704 tokens  ·  1 occurrence  ·  direct tool call

 echo "=== diff of scenario.ts / ingress.ts ==="
 git diff --stat engine-runtime/adapters/mso/src/mso/scenario.ts \
   engine-runtime/adapters/mso/src/mso/ingress.ts
```

## Components (all `report.rs` unless noted)

- `main.rs`: resolve `linear_base` (env → config → none); parse `--kind/--top/--detail`; new `Show { n }` command; thread into `ReportOptions`.
- `cli.rs`: flags + `Show` subcommand.
- `config/defaults.rs`: `LinkConfig`.
- `model.rs`: extend `ReportOptions` (`linear_base`, `kinds: Vec<…>`, `top`, `detail`); add `rank` to the row summary (or compute at print time).
- `report.rs`:
  - `Painter::link`, OSC 8 emitter, gating.
  - `extract_ticket_id`, `linear_url`.
  - shell: `strip_echo_header`, `verb_and_targets`.
  - `ccmap show` renderer.
  - filter + top-N + rank + spacing in the map printer.

## Testing (pure helpers, no ANSI/OSC in assertions)

- `extract_ticket_id`: `DEC-259` from a linear label; none from a non-ticket label.
- `linear_url`: env over config; trailing-slash normalise; None when unset.
- `strip_echo_header`: `echo "=== x ==="; grep …` → `grep …`; header-only stays.
- `verb_and_targets`: grep/cargo/python cases; target de-dup + `+N` overflow; no-target fallback.
- filtering: `--kind` selects; `--top` caps after filter; % stays session-relative.
- rank stability: rank unchanged under filter/top.
- link gating: `use_links=false` emits no `\x1b]8`.

## Backwards / edge cases

- Terminal without OSC 8 (or piped): plain text, still aligned. (We gate on TTY; we don't sniff specific terminals — OSC 8 is ignored harmlessly by terminals that don't grok it, but gating on TTY is the safe floor.)
- No `linear` base configured: Linear rows plain, no error.
- `ccmap show <n>` out of range: friendly error with the valid range.
- `--kind` with no matches: prints `Context map  0 of 94 sources (filtered)` and no rows.

## Phase 2 (follow-up, NOT this doc)

A `ccmap explore` TUI (`ratatui` + `crossterm`): arrow-key nav, Enter to expand a row into a detail pane (full script, token breakdown, trigger reasons), `o` to open ticket/file, `/` to filter, sort toggles. Scoped separately because it adds an event loop, real deps, and a non-pipe-friendly mode; the Phase-1 primitives (ticket-id/link resolution, shell summarisation, filtering) are all reused by it.
```
