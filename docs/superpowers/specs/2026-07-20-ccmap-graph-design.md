# `ccmap graph` — Design

## Status

Approved by user on 2026-07-20. Ready for implementation planning.

## Problem

`ccmap` today only reports as text (`ccmap analyse`, `ccmap latest`, `ccmap show`). The per-session
context map (`SessionAnalysis.context_map: Vec<ContextSourceSummary>`) already carries everything
needed for a visual view — source kind, label, token weight, occurrence count — but it's only ever
rendered as ranked bars in the terminal. There's also no way to see whether context usage is trending
up or down across a project's session history; `ccmap history` only lists sessions.

The implementation plan (`03-implementation-plan.md`, Step 13) already anticipates this next phase
("Mermaid renderer", "HTML report") but never scoped it. This spec scopes it.

## Goals

1. A visual bubble diagram of one session's context sources — bubble size by token weight, links
   showing the order sources were first touched.
2. A trend view across a project's session history — token totals and warning counts per session,
   oldest to newest — so a developer can see if context usage is creeping up over time.
3. Both are static, self-contained, local-first artifacts. No server, no CDN dependency, no
   auto-launching a browser.

## Non-goals

- No live/interactive server or auto-refresh.
- No cross-project aggregation (ADR-002 still applies — one project's data stays in that project).
- No session-to-session diffing command (`ccmap diff`) — separate future idea.
- No claim about causation beyond what `TriggerReason` already records (ADR-008 still applies).

## Command

```
ccmap graph                # graphs the latest session, same resolution as `ccmap latest`
ccmap graph <session-file> # graphs a specific session file, same resolution as `ccmap analyse <file>`
```

Output: writes `.claude/context-map/reports/<session-id>-graph.html` and prints the path. Opening the
file is left to the user (`open <path>` on macOS, etc.) — the command does not launch a browser itself.

The single HTML page contains two sections stacked vertically:

1. **Session bubble diagram** — for the resolved session.
2. **Project trend chart** — computed from every session file discovered for the current project
   (same discovery logic `ccmap history` already uses).

## Section 1 — Bubble diagram

**Node = one `ContextSourceSummary`.** These are already computed by
`analyse::summary::build_context_map` and stored on `SessionAnalysis.context_map` — no new
aggregation logic. One bubble per distinct `(source_kind, source_label)` pair (a specific file path,
a specific MCP server, a subagent name, etc.).

- **Size:** bubble *area* proportional to `approx_tokens` (not radius — area scaling matches
  perceived magnitude; radius scaling exaggerates differences).
- **Color:** by `source_kind`, reusing the same category colors the terminal report already assigns
  per `KindFilter` group, so the HTML view reads as the same tool, not a different product.
- **Label:** `source_label` (path / URL / MCP server / subagent name) plus its token count, shown on
  or near the bubble.
- **Layout:** force-directed (nodes repel each other, edges pull connected nodes together), computed
  client-side in plain JS/SVG or Canvas embedded in the HTML file. No charting library dependency —
  keeps the artifact self-contained and avoids a new Rust or JS dependency for something this
  contained.

**Edges = first-touch order.** Walk `SessionAnalysis.events` in their existing chronological order
and record the first event index at which each `(source_kind, source_label)` pair appears. Sort
sources by that first-touch index, then draw one edge between each consecutive pair in that order.
Each source is a single node (repeats are already folded into the node's `occurrences`/`approx_tokens`
via the existing aggregation) — this avoids a cluttered graph on sessions where the same file is read
many times, while still showing the shape of "what got touched, in what order."

Sources with no `source_label` (e.g. `Session`, `Unknown` — see ADR-009) are excluded from the graph
entirely, same as they're already excluded from `build_context_map` today.

## Section 2 — Project trend chart

For every session file discovered for the current project (oldest to newest), run the existing
analysis pipeline (`analyse_file`) and plot two series:

- **Primary:** `approx_context_tokens` per session (line or bar).
- **Secondary:** warning count per session, broken out by `WarningSeverity` (Low/Medium/High), shown
  as stacked markers or a secondary bar series in each severity's color.

**Ordering key:** session JSONL payloads carry no timestamp field (confirmed — hook events only carry
`hook_event_name`, `session_id`, `cwd`, and per-event fields). Order sessions by file modified-time
(`fs::metadata().modified()`); if metadata is unavailable on a given file, fall back to the existing
session-list ordering `ccmap history` already uses, and log nothing more precise than "session order"
in that case — do not fabricate a timestamp.

## New module

```
src/render/
  mod.rs        — orchestrates: load session analysis + project history, call graph_data/trend_data,
                  write the HTML file, return its path
  graph_data.rs — pure transform: SessionAnalysis -> { nodes, edges } for the bubble diagram
  trend_data.rs — pure transform: Vec<SessionAnalysis> (ordered) -> per-session trend points
  html.rs       — serializes { nodes, edges } + trend points into the final self-contained HTML string
                  (inline CSS/JS, no external requests)
```

Matches the existing convention (ADR-007) of keeping data-shaping separate from rendering/output, and
keeps `graph_data.rs`/`trend_data.rs` unit-testable as pure functions without any HTML/string concerns.

## CLI wiring

Add a `Graph` variant to the command enum in `cli.rs` alongside `Analyse`/`Latest`/`History`, taking
an optional session file path (same optionality pattern as `Analyse` vs. `Latest`). Reuses:

- `project::find_project` / `storage::Storage::for_project` for path resolution,
- `config::load_config` for project config,
- the same session-resolution helper `latest`/`analyse` already share (single-session case),
- `storage`'s existing session-discovery logic for the trend case (all sessions).

No changes needed to `model.rs`, `normalise.rs`, `classify.rs`, or `summary.rs` — this feature is
purely a new consumer of `SessionAnalysis`, not a change to how it's built.

## Testing

- `graph_data.rs`: unit tests on a fixture `SessionAnalysis` — assert node count/sizes match
  `context_map`, assert edge order matches first-touch order including a repeated-read case (edges
  should not duplicate).
- `trend_data.rs`: unit tests on a small `Vec<SessionAnalysis>` — assert per-session token/warning
  points are in the given order; assert graceful handling of a single-session project (no trend, or a
  single point).
- `html.rs`: a smoke test that the generated string is valid enough to open (contains expected node
  labels, no unescaped session content breaking HTML — source labels/paths must be HTML-escaped since
  they come from user file paths).

## Risks / open questions carried into implementation

- **HTML escaping:** `source_label` values are file paths / URLs / MCP server names sourced from the
  repo and tool responses — must be escaped when interpolated into HTML/JS to avoid injecting markup
  if a path or label contains special characters. This is a correctness requirement, not optional.
- **Force-directed layout without a library:** hand-rolling a small physics simulation in vanilla JS
  is the simplest local-first-compliant option, but is more implementation effort than pulling in a
  charting dependency. Accepted trade-off per the no-CDN-dependency, self-contained-artifact
  constraint.
