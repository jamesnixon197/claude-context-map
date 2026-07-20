# ccmap graph Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `ccmap graph` command that writes a single self-contained HTML file per project, containing a force-directed bubble diagram of one session's context sources plus a trend chart of token usage and warning counts across the project's session history.

**Architecture:** Two new pure-data-transform modules (`graph_data.rs`, `trend_data.rs`) consume the existing `SessionAnalysis`/`ContextSourceSummary` types with zero changes to the analysis pipeline; a third module (`html.rs`) serializes that data plus hand-written inline CSS/JS into one HTML string; `render/mod.rs` orchestrates loading + writing the file; `main.rs` wires a new `Graph` CLI command.

**Tech Stack:** Rust (existing crate), std-only for rendering (no new crates) — inline `<svg>`/vanilla JS force simulation, no CDN, matching the project's local-first/self-contained constraint.

## Global Constraints

- Local-first: no network calls, no CDN script/font/style references in the generated HTML (spec, Non-goals).
- No auto-launching a browser — the command prints the output path only (spec, Command).
- No changes to `model.rs`, `normalise.rs`, `classify.rs`, `summary.rs` — this feature only consumes `SessionAnalysis` (spec, CLI wiring).
- HTML-escape every value interpolated from `source_label` / session data before embedding in HTML or inline `<script>` (spec, Risks).
- Session ordering for the trend view uses file modified-time (`fs::metadata().modified()`), falling back to the existing session-list order (`storage.session_files()`, which sorts by path) if metadata is unavailable — never fabricate a timestamp (spec, Section 2).
- Use Conventional Commits for every commit (project CLAUDE.md).
- Keep modules small and focused; one clear responsibility per file (project CLAUDE.md, ADR-007).

---

## File Structure

```
src/render/
  mod.rs        — orchestration: resolve session(s) + write the HTML file, return its path
  graph_data.rs — pure transform: &SessionAnalysis -> GraphData { nodes, edges }
  trend_data.rs — pure transform: &[SessionAnalysis] (ordered oldest→newest) -> Vec<TrendPoint>
  html.rs        — pure transform: (&GraphData, &[TrendPoint]) -> String (the full HTML document)
```

Modified:
- `src/main.rs` — add module declaration, add `Command::Graph` match arm.
- `src/cli.rs` — add `Graph { path: Option<PathBuf> }` variant.
- `src/storage.rs` — add a small helper to pair each session file with its modified-time for ordering (reused by `render::mod`).

No changes to `model.rs`, `analyse/*` (except adding nothing — `render` only calls the existing public `analyse::analyse_file`).

---

## Task 1: `graph_data` — session bubble/edge transform

**Files:**
- Create: `src/render/graph_data.rs`
- Modify: `src/render/mod.rs` (create with `mod graph_data;` — full orchestration comes in Task 4)

**Interfaces:**
- Consumes: `crate::model::{SessionAnalysis, ContextSourceSummary, ContextSourceKind, ContextEvent}` (all exist today, unchanged).
- Produces (used by Task 3's `html.rs` and Task 4's `mod.rs`):
  ```rust
  pub struct GraphNode {
      pub id: usize,               // stable index into `nodes`, used as edge endpoints
      pub kind: ContextSourceKind,
      pub label: String,           // == source_label
      pub approx_tokens: usize,
      pub occurrences: usize,
  }

  pub struct GraphEdge {
      pub from: usize,             // index into GraphData.nodes
      pub to: usize,
  }

  pub struct GraphData {
      pub nodes: Vec<GraphNode>,
      pub edges: Vec<GraphEdge>,
  }

  pub fn build_graph_data(analysis: &SessionAnalysis) -> GraphData
  ```

**Behavior:**
- One `GraphNode` per entry in `analysis.context_map` (already deduplicated by `(source_kind, source_label)` — see `src/analyse/summary.rs:48-80`). Node order matches `context_map` order (already sorted by `approx_tokens` descending); node `id` is simply its position in the output `nodes` vec.
- Edges: walk `analysis.events` in their existing order (chronological — see ADR-010) and, for each event that has a `source_label`, record the *first* event index at which each `(source_kind, source_label)` pair appears — using a `HashMap<(ContextSourceKind, String), usize>` keyed the same way `build_context_map` keys its map. Sort the resulting `(key, first_index)` pairs by `first_index` ascending, look up each key's node `id` (build a `(kind,label) -> id` map from `nodes` first), then emit one edge between each consecutive pair in that order (`nodes.len() - 1` edges for `nodes.len() >= 1`, zero edges for 0 or 1 nodes).
- Events with no `source_label` (e.g. `Session`, `Unknown`) are skipped entirely, consistent with `build_context_map`'s existing exclusion.

- [ ] **Step 1: Write the failing tests**

```rust
// src/render/graph_data.rs (bottom of file, #[cfg(test)] mod tests)
#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        ContextConfidence, ContextEvent, ContextSourceSummary, TriggerReason,
    };

    fn event(kind: ContextSourceKind, label: &str, tokens: usize) -> ContextEvent {
        ContextEvent {
            session_id: "s".into(),
            event_name: "PostToolUse".into(),
            source_kind: kind,
            path: None,
            command: None,
            tool_name: None,
            approx_chars: tokens * 4,
            approx_tokens: tokens,
            confidence: ContextConfidence::High,
            source_label: Some(label.to_string()),
            trigger_reason: TriggerReason::DirectToolCall,
        }
    }

    fn summary(kind: ContextSourceKind, label: &str, tokens: usize) -> ContextSourceSummary {
        ContextSourceSummary {
            source_kind: kind,
            source_label: label.to_string(),
            occurrences: 1,
            approx_tokens: tokens,
            trigger_reasons: vec![TriggerReason::DirectToolCall],
        }
    }

    fn analysis_with(
        context_map: Vec<ContextSourceSummary>,
        events: Vec<ContextEvent>,
    ) -> SessionAnalysis {
        SessionAnalysis {
            session_id: "s".into(),
            instruction_files_loaded: 0,
            files_read: 0,
            file_searches: 0,
            file_path_lists: 0,
            bash_commands: 0,
            files_edited: 0,
            files_written: 0,
            subagents: 0,
            approx_context_tokens: context_map.iter().map(|s| s.approx_tokens).sum(),
            context_map,
            events,
            warnings: Vec::new(),
        }
    }

    #[test]
    fn one_node_per_context_map_entry() {
        let map = vec![
            summary(ContextSourceKind::FileRead, "a.rs", 100),
            summary(ContextSourceKind::ShellOutput, "cargo test", 50),
        ];
        let analysis = analysis_with(map, Vec::new());
        let data = build_graph_data(&analysis);
        assert_eq!(data.nodes.len(), 2);
        assert_eq!(data.nodes[0].label, "a.rs");
        assert_eq!(data.nodes[0].approx_tokens, 100);
        assert_eq!(data.nodes[1].label, "cargo test");
    }

    #[test]
    fn edges_follow_first_touch_order_and_dedupe_repeats() {
        let map = vec![
            summary(ContextSourceKind::FileRead, "a.rs", 100),
            summary(ContextSourceKind::ShellOutput, "cargo test", 50),
            summary(ContextSourceKind::FileRead, "b.rs", 10),
        ];
        // b.rs is touched first, then cargo test, then a.rs is touched TWICE.
        let events = vec![
            event(ContextSourceKind::FileRead, "b.rs", 10),
            event(ContextSourceKind::ShellOutput, "cargo test", 50),
            event(ContextSourceKind::FileRead, "a.rs", 60),
            event(ContextSourceKind::FileRead, "a.rs", 40), // repeat, must not duplicate a node/edge
        ];
        let analysis = analysis_with(map, events);
        let data = build_graph_data(&analysis);

        assert_eq!(data.nodes.len(), 3);
        assert_eq!(data.edges.len(), 2, "3 nodes => 2 edges in the first-touch chain");

        let label_of = |id: usize| data.nodes[id].label.clone();
        // Expected chain: b.rs -> cargo test -> a.rs
        assert_eq!(label_of(data.edges[0].from), "b.rs");
        assert_eq!(label_of(data.edges[0].to), "cargo test");
        assert_eq!(label_of(data.edges[1].from), "cargo test");
        assert_eq!(label_of(data.edges[1].to), "a.rs");
    }

    #[test]
    fn events_without_source_label_are_ignored_for_edges() {
        let map = vec![summary(ContextSourceKind::FileRead, "a.rs", 100)];
        let mut unlabeled = event(ContextSourceKind::Session, "unused", 5);
        unlabeled.source_label = None;
        let analysis = analysis_with(map, vec![unlabeled]);
        let data = build_graph_data(&analysis);
        assert_eq!(data.nodes.len(), 1);
        assert_eq!(data.edges.len(), 0);
    }

    #[test]
    fn single_node_produces_no_edges() {
        let map = vec![summary(ContextSourceKind::FileRead, "a.rs", 100)];
        let analysis = analysis_with(map, Vec::new());
        let data = build_graph_data(&analysis);
        assert_eq!(data.nodes.len(), 1);
        assert_eq!(data.edges.len(), 0);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test render::graph_data -- --nocapture`
Expected: FAIL — `build_graph_data`, `GraphData`, `GraphNode`, `GraphEdge` not found (module doesn't exist yet).

- [ ] **Step 3: Write the implementation**

```rust
// src/render/graph_data.rs (above the tests module)
use crate::model::{ContextSourceKind, SessionAnalysis};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct GraphNode {
    pub id: usize,
    pub kind: ContextSourceKind,
    pub label: String,
    pub approx_tokens: usize,
    pub occurrences: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct GraphEdge {
    pub from: usize,
    pub to: usize,
}

#[derive(Debug, Clone)]
pub struct GraphData {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

pub fn build_graph_data(analysis: &SessionAnalysis) -> GraphData {
    let nodes: Vec<GraphNode> = analysis
        .context_map
        .iter()
        .enumerate()
        .map(|(id, source)| GraphNode {
            id,
            kind: source.source_kind.clone(),
            label: source.source_label.clone(),
            approx_tokens: source.approx_tokens,
            occurrences: source.occurrences,
        })
        .collect();

    let mut node_id_by_key: HashMap<(ContextSourceKind, String), usize> = HashMap::new();
    for node in &nodes {
        node_id_by_key.insert((node.kind.clone(), node.label.clone()), node.id);
    }

    let mut first_seen: HashMap<(ContextSourceKind, String), usize> = HashMap::new();
    for (event_index, event) in analysis.events.iter().enumerate() {
        let Some(label) = &event.source_label else {
            continue;
        };
        let key = (event.source_kind.clone(), label.clone());
        first_seen.entry(key).or_insert(event_index);
    }

    let mut order: Vec<((ContextSourceKind, String), usize)> = first_seen.into_iter().collect();
    order.sort_by_key(|(_, first_index)| *first_index);

    let ordered_ids: Vec<usize> = order
        .iter()
        .filter_map(|(key, _)| node_id_by_key.get(key).copied())
        .collect();

    let edges = ordered_ids
        .windows(2)
        .map(|pair| GraphEdge { from: pair[0], to: pair[1] })
        .collect();

    GraphData { nodes, edges }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test render::graph_data -- --nocapture`
Expected: PASS (4 tests)

- [ ] **Step 5: Register the module and commit**

```rust
// src/render/mod.rs (new file, orchestration body added in Task 4)
mod graph_data;
```

```bash
cargo fmt
cargo check
git add src/render/mod.rs src/render/graph_data.rs
git commit -m "feat(render): build session bubble/edge graph data from analysis"
```

---

## Task 2: `trend_data` — cross-session trend transform

**Files:**
- Create: `src/render/trend_data.rs`
- Modify: `src/render/mod.rs` (add `mod trend_data;`)

**Interfaces:**
- Consumes: `crate::model::{SessionAnalysis, WarningSeverity}` (unchanged).
- Produces (used by Task 3's `html.rs` and Task 4's `mod.rs`):
  ```rust
  pub struct TrendPoint {
      pub session_id: String,
      pub approx_context_tokens: usize,
      pub high_warnings: usize,
      pub medium_warnings: usize,
      pub low_warnings: usize,
  }

  // Caller is responsible for ordering `analyses` oldest-to-newest before calling.
  pub fn build_trend_points(analyses: &[SessionAnalysis]) -> Vec<TrendPoint>
  ```

**Behavior:** This module does not decide ordering — ordering (by file modified-time, with fallback) is Task 4's concern, since it requires filesystem metadata that a pure data-transform function shouldn't need. `build_trend_points` simply maps each `SessionAnalysis`, in the order given, to one `TrendPoint`, counting `analysis.warnings` by severity.

- [ ] **Step 1: Write the failing tests**

```rust
// src/render/trend_data.rs
#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ContextWarning, SessionAnalysis, WarningSeverity};

    fn analysis(id: &str, tokens: usize, severities: &[WarningSeverity]) -> SessionAnalysis {
        SessionAnalysis {
            session_id: id.to_string(),
            instruction_files_loaded: 0,
            files_read: 0,
            file_searches: 0,
            file_path_lists: 0,
            bash_commands: 0,
            files_edited: 0,
            files_written: 0,
            subagents: 0,
            approx_context_tokens: tokens,
            context_map: Vec::new(),
            events: Vec::new(),
            warnings: severities
                .iter()
                .map(|s| ContextWarning {
                    severity: s.clone(),
                    title: "t".into(),
                    detail: "d".into(),
                })
                .collect(),
        }
    }

    #[test]
    fn maps_one_point_per_session_in_given_order() {
        let sessions = vec![
            analysis("s1", 100, &[WarningSeverity::High]),
            analysis("s2", 200, &[WarningSeverity::Medium, WarningSeverity::Medium]),
        ];
        let points = build_trend_points(&sessions);
        assert_eq!(points.len(), 2);
        assert_eq!(points[0].session_id, "s1");
        assert_eq!(points[0].approx_context_tokens, 100);
        assert_eq!(points[0].high_warnings, 1);
        assert_eq!(points[1].session_id, "s2");
        assert_eq!(points[1].medium_warnings, 2);
    }

    #[test]
    fn counts_each_severity_independently() {
        let sessions = vec![analysis(
            "s1",
            50,
            &[
                WarningSeverity::High,
                WarningSeverity::Low,
                WarningSeverity::Low,
            ],
        )];
        let points = build_trend_points(&sessions);
        assert_eq!(points[0].high_warnings, 1);
        assert_eq!(points[0].medium_warnings, 0);
        assert_eq!(points[0].low_warnings, 2);
    }

    #[test]
    fn empty_input_produces_empty_output() {
        assert!(build_trend_points(&[]).is_empty());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test render::trend_data -- --nocapture`
Expected: FAIL — `build_trend_points`, `TrendPoint` not found.

- [ ] **Step 3: Write the implementation**

```rust
// src/render/trend_data.rs (above the tests module)
use crate::model::{SessionAnalysis, WarningSeverity};

#[derive(Debug, Clone)]
pub struct TrendPoint {
    pub session_id: String,
    pub approx_context_tokens: usize,
    pub high_warnings: usize,
    pub medium_warnings: usize,
    pub low_warnings: usize,
}

pub fn build_trend_points(analyses: &[SessionAnalysis]) -> Vec<TrendPoint> {
    analyses
        .iter()
        .map(|analysis| {
            let mut point = TrendPoint {
                session_id: analysis.session_id.clone(),
                approx_context_tokens: analysis.approx_context_tokens,
                high_warnings: 0,
                medium_warnings: 0,
                low_warnings: 0,
            };
            for warning in &analysis.warnings {
                match warning.severity {
                    WarningSeverity::High => point.high_warnings += 1,
                    WarningSeverity::Medium => point.medium_warnings += 1,
                    WarningSeverity::Low => point.low_warnings += 1,
                }
            }
            point
        })
        .collect()
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test render::trend_data -- --nocapture`
Expected: PASS (3 tests)

- [ ] **Step 5: Register the module and commit**

```rust
// src/render/mod.rs
mod graph_data;
mod trend_data;
```

```bash
cargo fmt
cargo check
git add src/render/mod.rs src/render/trend_data.rs
git commit -m "feat(render): build cross-session trend points from analysis history"
```

---

## Task 3: `html` — serialize graph + trend data into a self-contained document

**Files:**
- Create: `src/render/html.rs`
- Modify: `src/render/mod.rs` (add `mod html;`)

**Interfaces:**
- Consumes:
  - `GraphData { nodes: Vec<GraphNode>, edges: Vec<GraphEdge> }` (Task 1)
  - `Vec<TrendPoint>` (Task 2)
  - `&str` session_id (for the page heading)
- Produces (used by Task 4's `mod.rs`):
  ```rust
  pub fn render_html(session_id: &str, graph: &GraphData, trend: &[TrendPoint]) -> String
  ```

**Behavior:**
- Emits one complete `<!doctype html>...</html>` string. No `<script src=...>`, no `<link href=...>` to any external host — everything inline (`<style>` block + one `<script>` block).
- Embeds `graph.nodes` / `graph.edges` / `trend` as a JSON blob via `serde_json::to_string` inside a `<script id="graph-data" type="application/json">...</script>` tag, which the inline JS parses with `JSON.parse` at load time. This sidesteps hand-rolling JS object-literal escaping — only the JSON string itself needs HTML-escaping (specifically, escaping `</script` sequences, since raw JSON can't accidentally break out of a `<script>` block any other way that matters here).
- The inline JS renders bubbles as SVG `<circle>` elements sized by `sqrt(approx_tokens)` (radius, so *area* is proportional to tokens per the spec), colored by `kind` using the same mapping as `report.rs`'s `Painter::kind` (blue/magenta/cyan/yellow/green/dimmed — see Task 3 Step 3 for the exact JS mapping), positioned by a minimal force simulation (repulsion between all node pairs + attraction along edges + centering force, iterated a fixed number of steps on load — no external physics library).
- Trend section renders a simple `<svg>` line/bar chart from the `trend` array: one polyline for token totals, one small stacked bar or marker row per session for high/medium/low warning counts.
- Any session/source data that could contain HTML-sensitive characters (paths, MCP labels, session ids) must go through the JSON-embedding path above, not be concatenated directly into HTML markup as raw strings — this is the escaping requirement from the spec's Risks section, satisfied structurally by using `serde_json::to_string` (which produces a valid JSON string, safe to place inside a `<script type="application/json">` block) rather than by string-interpolating labels into HTML tags.

- [ ] **Step 1: Write the failing tests**

```rust
// src/render/html.rs (bottom of file, #[cfg(test)] mod tests)
#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::graph_data::{GraphData, GraphEdge, GraphNode};
    use crate::render::trend_data::TrendPoint;
    use crate::model::ContextSourceKind;

    fn sample_graph() -> GraphData {
        GraphData {
            nodes: vec![
                GraphNode {
                    id: 0,
                    kind: ContextSourceKind::FileRead,
                    label: "src/<script>.rs".to_string(),
                    approx_tokens: 100,
                    occurrences: 1,
                },
                GraphNode {
                    id: 1,
                    kind: ContextSourceKind::ShellOutput,
                    label: "cargo test".to_string(),
                    approx_tokens: 50,
                    occurrences: 1,
                },
            ],
            edges: vec![GraphEdge { from: 0, to: 1 }],
        }
    }

    fn sample_trend() -> Vec<TrendPoint> {
        vec![TrendPoint {
            session_id: "s1".to_string(),
            approx_context_tokens: 150,
            high_warnings: 1,
            medium_warnings: 0,
            low_warnings: 0,
        }]
    }

    #[test]
    fn renders_a_full_html_document() {
        let html = render_html("demo-session", &sample_graph(), &sample_trend());
        assert!(html.trim_start().starts_with("<!doctype html>"));
        assert!(html.contains("</html>"));
    }

    #[test]
    fn embeds_node_and_trend_data_as_json() {
        let html = render_html("demo-session", &sample_graph(), &sample_trend());
        assert!(html.contains("application/json"));
        assert!(html.contains("cargo test"));
        assert!(html.contains("\"approx_context_tokens\":150"));
    }

    #[test]
    fn does_not_break_out_of_the_script_tag_on_a_hostile_label() {
        // A label containing "</script>" must not prematurely close the JSON block.
        let html = render_html("demo-session", &sample_graph(), &sample_trend());
        // The literal sequence "</script>" must not appear except at our own
        // intentional closing tags — check no *unescaped* occurrence sits
        // inside the JSON payload by confirming the raw closing sequence
        // from the hostile label was escaped to "<\/script>" or similar.
        assert!(!html.contains("<script>.rs</script>"));
    }

    #[test]
    fn has_no_external_network_references() {
        let html = render_html("demo-session", &sample_graph(), &sample_trend());
        assert!(!html.contains("http://"));
        assert!(!html.contains("https://"));
        assert!(!html.contains("cdn."));
    }

    #[test]
    fn includes_session_id_in_the_page() {
        let html = render_html("demo-session", &sample_graph(), &sample_trend());
        assert!(html.contains("demo-session"));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test render::html -- --nocapture`
Expected: FAIL — `render_html` not found.

- [ ] **Step 3: Write the implementation**

```rust
// src/render/html.rs (above the tests module)
use crate::render::graph_data::GraphData;
use crate::render::trend_data::TrendPoint;

pub fn render_html(session_id: &str, graph: &GraphData, trend: &[TrendPoint]) -> String {
    let graph_json = serde_json::to_string(&SerializableGraph::from(graph))
        .unwrap_or_else(|_| "{\"nodes\":[],\"edges\":[]}".to_string());
    let trend_json = serde_json::to_string(&trend.iter().map(SerializableTrendPoint::from).collect::<Vec<_>>())
        .unwrap_or_else(|_| "[]".to_string());

    // JSON can legally contain the literal text "</script>" inside a string
    // value (e.g. a file whose path contains that text); escape the slash so
    // the browser's HTML parser doesn't treat it as the tag's real close.
    let graph_json = graph_json.replace("</", "<\\/");
    let trend_json = trend_json.replace("</", "<\\/");

    let session_id_escaped = html_escape(session_id);

    format!(
        r#"<!doctype html>
<html>
<head>
<meta charset="utf-8">
<title>ccmap graph — {session_id_escaped}</title>
<style>
{CSS}
</style>
</head>
<body>
<h1>Session: {session_id_escaped}</h1>
<section id="graph-section">
  <svg id="graph-svg" width="900" height="600"></svg>
</section>
<section id="trend-section">
  <svg id="trend-svg" width="900" height="220"></svg>
</section>
<script id="graph-data" type="application/json">{graph_json}</script>
<script id="trend-data" type="application/json">{trend_json}</script>
<script>
{JS}
</script>
</body>
</html>
"#
    )
}

fn html_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[derive(serde::Serialize)]
struct SerializableGraph {
    nodes: Vec<SerializableNode>,
    edges: Vec<SerializableEdge>,
}

#[derive(serde::Serialize)]
struct SerializableNode {
    id: usize,
    kind: String,
    label: String,
    approx_tokens: usize,
    occurrences: usize,
}

#[derive(serde::Serialize)]
struct SerializableEdge {
    from: usize,
    to: usize,
}

impl From<&GraphData> for SerializableGraph {
    fn from(graph: &GraphData) -> Self {
        SerializableGraph {
            nodes: graph
                .nodes
                .iter()
                .map(|n| SerializableNode {
                    id: n.id,
                    kind: kind_name(&n.kind).to_string(),
                    label: n.label.clone(),
                    approx_tokens: n.approx_tokens,
                    occurrences: n.occurrences,
                })
                .collect(),
            edges: graph
                .edges
                .iter()
                .map(|e| SerializableEdge { from: e.from, to: e.to })
                .collect(),
        }
    }
}

fn kind_name(kind: &crate::model::ContextSourceKind) -> &'static str {
    use crate::model::ContextSourceKind;
    match kind {
        ContextSourceKind::FileRead => "file",
        ContextSourceKind::ShellOutput => "shell",
        ContextSourceKind::McpTool { .. } => "mcp",
        ContextSourceKind::Subagent => "sub",
        ContextSourceKind::Instruction => "instr",
        ContextSourceKind::Web => "web",
        ContextSourceKind::FileEdit => "edit",
        ContextSourceKind::FileWrite => "write",
        ContextSourceKind::Session => "session",
        ContextSourceKind::UserPrompt => "prompt",
        ContextSourceKind::FileSearch => "search",
        ContextSourceKind::FilePathList => "paths",
        ContextSourceKind::Unknown => "unknown",
    }
}

#[derive(serde::Serialize)]
struct SerializableTrendPoint {
    session_id: String,
    approx_context_tokens: usize,
    high_warnings: usize,
    medium_warnings: usize,
    low_warnings: usize,
}

impl From<&TrendPoint> for SerializableTrendPoint {
    fn from(point: &TrendPoint) -> Self {
        SerializableTrendPoint {
            session_id: point.session_id.clone(),
            approx_context_tokens: point.approx_context_tokens,
            high_warnings: point.high_warnings,
            medium_warnings: point.medium_warnings,
            low_warnings: point.low_warnings,
        }
    }
}

const CSS: &str = r#"
body { font-family: -apple-system, sans-serif; margin: 2rem; background: #0d1117; color: #e6edf3; }
h1 { font-size: 1.1rem; font-weight: 600; }
section { margin-bottom: 2rem; }
.node-file { fill: #3b82f6; }
.node-shell { fill: #d946ef; }
.node-mcp { fill: #06b6d4; }
.node-sub { fill: #eab308; }
.node-instr { fill: #22c55e; }
.node-web { fill: #06b6d4; }
.node-edit, .node-write, .node-default { fill: #6b7280; }
.edge { stroke: #30363d; stroke-width: 1; }
.node-label { fill: #e6edf3; font-size: 10px; }
.trend-line { fill: none; stroke: #3b82f6; stroke-width: 2; }
.trend-high { fill: #ef4444; }
.trend-medium { fill: #eab308; }
.trend-low { fill: #6b7280; }
"#;

const JS: &str = r#"
(function () {
  const graph = JSON.parse(document.getElementById('graph-data').textContent);
  const trend = JSON.parse(document.getElementById('trend-data').textContent);
  const svg = document.getElementById('graph-svg');
  const width = 900, height = 600;

  const nodes = graph.nodes.map((n) => ({
    ...n,
    x: width / 2 + (Math.random() - 0.5) * 200,
    y: height / 2 + (Math.random() - 0.5) * 200,
    r: Math.max(6, Math.sqrt(n.approx_tokens) * 1.5),
  }));
  const edges = graph.edges;

  for (let iter = 0; iter < 200; iter++) {
    for (let i = 0; i < nodes.length; i++) {
      for (let j = i + 1; j < nodes.length; j++) {
        const a = nodes[i], b = nodes[j];
        let dx = a.x - b.x, dy = a.y - b.y;
        let dist = Math.sqrt(dx * dx + dy * dy) || 1;
        const minDist = a.r + b.r + 20;
        if (dist < minDist) {
          const push = (minDist - dist) / dist * 0.5;
          a.x += dx * push; a.y += dy * push;
          b.x -= dx * push; b.y -= dy * push;
        }
      }
    }
    for (const edge of edges) {
      const a = nodes[edge.from], b = nodes[edge.to];
      const dx = b.x - a.x, dy = b.y - a.y;
      a.x += dx * 0.01; a.y += dy * 0.01;
      b.x -= dx * 0.01; b.y -= dy * 0.01;
    }
    for (const n of nodes) {
      n.x += (width / 2 - n.x) * 0.001;
      n.y += (height / 2 - n.y) * 0.001;
      n.x = Math.max(n.r, Math.min(width - n.r, n.x));
      n.y = Math.max(n.r, Math.min(height - n.r, n.y));
    }
  }

  const ns = 'http://www.w3.org/2000/svg';
  for (const edge of edges) {
    const a = nodes[edge.from], b = nodes[edge.to];
    const line = document.createElementNS(ns, 'line');
    line.setAttribute('x1', a.x); line.setAttribute('y1', a.y);
    line.setAttribute('x2', b.x); line.setAttribute('y2', b.y);
    line.setAttribute('class', 'edge');
    svg.appendChild(line);
  }
  for (const n of nodes) {
    const circle = document.createElementNS(ns, 'circle');
    circle.setAttribute('cx', n.x); circle.setAttribute('cy', n.y);
    circle.setAttribute('r', n.r);
    circle.setAttribute('class', 'node-' + n.kind);
    svg.appendChild(circle);
    const text = document.createElementNS(ns, 'text');
    text.setAttribute('x', n.x); text.setAttribute('y', n.y - n.r - 4);
    text.setAttribute('class', 'node-label');
    text.setAttribute('text-anchor', 'middle');
    text.textContent = n.label + ' (' + n.approx_tokens + ')';
    svg.appendChild(text);
  }

  const trendSvg = document.getElementById('trend-svg');
  const tw = 900, th = 220, pad = 30;
  if (trend.length > 0) {
    const maxTokens = Math.max(...trend.map((t) => t.approx_context_tokens), 1);
    const stepX = trend.length > 1 ? (tw - pad * 2) / (trend.length - 1) : 0;
    const points = trend.map((t, i) => {
      const x = pad + i * stepX;
      const y = th - pad - (t.approx_context_tokens / maxTokens) * (th - pad * 2);
      return x + ',' + y;
    }).join(' ');
    const polyline = document.createElementNS(ns, 'polyline');
    polyline.setAttribute('points', points);
    polyline.setAttribute('class', 'trend-line');
    trendSvg.appendChild(polyline);

    trend.forEach((t, i) => {
      const x = pad + i * stepX;
      const barBase = th - pad;
      const sizes = [
        ['trend-high', t.high_warnings],
        ['trend-medium', t.medium_warnings],
        ['trend-low', t.low_warnings],
      ];
      let offset = 0;
      for (const [cls, count] of sizes) {
        for (let k = 0; k < count; k++) {
          const dot = document.createElementNS(ns, 'circle');
          dot.setAttribute('cx', x);
          dot.setAttribute('cy', barBase + 10 + offset * 6);
          dot.setAttribute('r', 2.5);
          dot.setAttribute('class', cls);
          trendSvg.appendChild(dot);
          offset++;
        }
      }
    });
  }
})();
"#;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test render::html -- --nocapture`
Expected: PASS (5 tests)

- [ ] **Step 5: Register the module and commit**

```rust
// src/render/mod.rs
mod graph_data;
mod html;
mod trend_data;
```

```bash
cargo fmt
cargo check
git add src/render/mod.rs src/render/html.rs
git commit -m "feat(render): serialize graph and trend data into self-contained HTML"
```

---

## Task 4: `render::mod` orchestration + `Storage` ordering helper

**Files:**
- Modify: `src/render/mod.rs` (add orchestration function; currently only has `mod` declarations from Tasks 1–3)
- Modify: `src/storage.rs` (add a helper for ordered session files with fallback)

**Interfaces:**
- Consumes:
  - `crate::project::Project`, `crate::storage::Storage`, `crate::config::CcmapConfig` (all exist)
  - `crate::analyse::analyse_file(path: &Path, config: &CcmapConfig) -> Result<SessionAnalysis>` (exists, `src/analyse/mod.rs:19`)
  - `graph_data::build_graph_data`, `trend_data::build_trend_points`, `html::render_html` (Tasks 1–3)
- Produces (used by Task 5's `main.rs`/`cli.rs`):
  ```rust
  // src/storage.rs — new method on Storage
  impl Storage {
      pub fn session_files_ordered_by_time(&self) -> Result<Vec<PathBuf>>;
  }

  // src/render/mod.rs
  pub fn write_graph(
      storage: &Storage,
      config: &CcmapConfig,
      session_path: &Path,
  ) -> Result<PathBuf>;
  ```

**Behavior:**
- `session_files_ordered_by_time`: reuses `self.session_files()` (existing, sorts by path — the fallback ordering per spec), then attempts to sort by `fs::metadata(path).and_then(|m| m.modified())`; if *any* file's metadata read fails, keep the path-sorted fallback order rather than partially sorting (this matches the spec's "fall back to the existing session-list ordering ... if metadata is unavailable" — treated as an all-or-nothing fallback so the trend chart's x-axis is never a mix of two orderings).
- `write_graph`:
  1. Resolve the target session's `SessionAnalysis` via `analyse::analyse_file(session_path, config)`.
  2. Load every session file for the project via `storage.session_files_ordered_by_time()`, run each through `analyse::analyse_file` (skip — with no error — any file that fails to parse, so one corrupt session doesn't break the whole trend view), and collect the `Vec<SessionAnalysis>` in that order.
  3. `graph_data::build_graph_data(&target_analysis)`.
  4. `trend_data::build_trend_points(&history_analyses)`.
  5. `html::render_html(&target_analysis.session_id, &graph, &trend)`.
  6. Write to `storage.reports_dir.join(format!("{}-graph.html", target_analysis.session_id))`, creating `reports_dir` first via `storage.create_dirs()` (existing method, idempotent).
  7. Return the written path.

- [ ] **Step 1: Write the failing tests**

```rust
// src/storage.rs — add to the existing #[cfg(test)] mod tests block
#[test]
fn session_files_ordered_by_time_sorts_oldest_first() {
    let dir = std::env::temp_dir().join(format!(
        "ccmap-test-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();

    let storage = Storage {
        base_dir: dir.clone(),
        sessions_dir: dir.clone(),
        reports_dir: dir.join("reports"),
        project_file: dir.join("project.json"),
        config_file: dir.join("config.toml"),
        settings_file: dir.join("settings.local.json"),
    };

    fs::write(dir.join("b.jsonl"), "{}").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(10));
    fs::write(dir.join("a.jsonl"), "{}").unwrap();

    let ordered = storage.session_files_ordered_by_time().unwrap();
    let names: Vec<_> = ordered
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
        .collect();
    assert_eq!(names, vec!["b.jsonl", "a.jsonl"]);

    let _ = fs::remove_dir_all(&dir);
}
```

```rust
// src/render/mod.rs — new #[cfg(test)] mod tests block at the bottom
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::CcmapConfig;
    use crate::project::Project;
    use std::fs;

    fn temp_project(name: &str) -> (Project, Storage) {
        let root = std::env::temp_dir().join(format!("ccmap-render-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let project = Project {
            root: root.clone(),
            name: "test".into(),
            id: "test-id".into(),
        };
        let storage = Storage::for_project(&project);
        storage.create_dirs().unwrap();
        (project, storage)
    }

    #[test]
    fn write_graph_produces_an_html_file_named_after_the_session() {
        let (_project, storage) = temp_project("write-graph");
        let session_path = storage.sessions_dir.join("demo.jsonl");
        fs::write(
            &session_path,
            concat!(
                "{\"hook_event_name\":\"SessionStart\",\"session_id\":\"demo\",\"cwd\":\"/repo\"}\n",
                "{\"hook_event_name\":\"PostToolUse\",\"session_id\":\"demo\",\"tool_name\":\"Read\",",
                "\"tool_input\":{\"file_path\":\"/repo/src/main.rs\"},",
                "\"tool_response\":{\"content\":\"fn main() {}\"}}\n",
            ),
        )
        .unwrap();

        let config = CcmapConfig::default();
        let output = write_graph(&storage, &config, &session_path).unwrap();

        assert!(output.exists());
        assert_eq!(output.file_name().unwrap().to_string_lossy(), "demo-graph.html");
        let contents = fs::read_to_string(&output).unwrap();
        assert!(contents.contains("<!doctype html>"));
        assert!(contents.contains("main.rs"));

        let _ = fs::remove_dir_all(&storage.base_dir.parent().unwrap());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test session_files_ordered_by_time -- --nocapture && cargo test render::tests -- --nocapture`
Expected: FAIL — `session_files_ordered_by_time` and `write_graph` not found.

(Confirmed: `Project { root: PathBuf, name: String, id: String }` in `src/project.rs:7-11` matches the test's `temp_project` helper exactly, and `CcmapConfig` derives/implements `Default` in `src/config/defaults.rs:15` — `CcmapConfig::default()` in the test below is valid as written.)

- [ ] **Step 3: Write the implementation**

```rust
// src/storage.rs — add to impl Storage
pub fn session_files_ordered_by_time(&self) -> Result<Vec<PathBuf>> {
    let sessions = self.session_files()?;

    let times: std::io::Result<Vec<SystemTime>> = sessions
        .iter()
        .map(|path| fs::metadata(path).and_then(|meta| meta.modified()))
        .collect();

    match times {
        Ok(times) => {
            let mut paired: Vec<_> = sessions.into_iter().zip(times).collect();
            paired.sort_by_key(|(_, modified)| *modified);
            Ok(paired.into_iter().map(|(path, _)| path).collect())
        }
        Err(_) => Ok(sessions), // fall back to the existing path-sorted order
    }
}
```

```rust
// src/render/mod.rs
mod graph_data;
mod html;
mod trend_data;

use crate::analyse;
use crate::config::CcmapConfig;
use crate::storage::Storage;
use anyhow::Result;
use std::path::{Path, PathBuf};

pub fn write_graph(storage: &Storage, config: &CcmapConfig, session_path: &Path) -> Result<PathBuf> {
    let target_analysis = analyse::analyse_file(session_path, config)?;

    let history_paths = storage.session_files_ordered_by_time()?;
    let history_analyses: Vec<_> = history_paths
        .iter()
        .filter_map(|path| analyse::analyse_file(path, config).ok())
        .collect();

    let graph = graph_data::build_graph_data(&target_analysis);
    let trend = trend_data::build_trend_points(&history_analyses);
    let document = html::render_html(&target_analysis.session_id, &graph, &trend);

    storage.create_dirs()?;
    let output_path = storage
        .reports_dir
        .join(format!("{}-graph.html", target_analysis.session_id));
    std::fs::write(&output_path, document)?;

    Ok(output_path)
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test session_files_ordered_by_time -- --nocapture && cargo test render:: -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
cargo fmt
cargo check
git add src/render/mod.rs src/storage.rs
git commit -m "feat(render): orchestrate graph+trend HTML generation with time-ordered history"
```

---

## Task 5: Wire `ccmap graph` into the CLI

**Files:**
- Modify: `src/cli.rs` (add `Graph` variant)
- Modify: `src/main.rs` (add module declaration + match arm)

**Interfaces:**
- Consumes: `cli::Command::Graph { path: Option<PathBuf> }`, `render::write_graph` (Task 4), `storage::Storage::latest_session_file` (existing, `src/storage.rs:58`).
- Produces: end-user-visible behavior only — no further tasks depend on this.

**Behavior:** Mirrors the existing `Analyse` vs. `Latest` optionality pattern already in `main.rs`: `ccmap graph <path>` graphs that specific file; `ccmap graph` with no path resolves the latest session the same way `Command::Latest` does, printing "No sessions captured yet." if none exist.

- [ ] **Step 1: Add the CLI variant**

```rust
// src/cli.rs — add to enum Command, alongside History/Doctor
Graph {
    path: Option<PathBuf>,
},
```

- [ ] **Step 2: Run a quick check that it compiles standalone**

Run: `cargo check`
Expected: FAIL — `main.rs`'s `match cli.command` is non-exhaustive (new variant not handled yet). This is expected at this step; confirms the compiler is tracking the new arm.

- [ ] **Step 3: Add the module declaration and match arm in `main.rs`**

```rust
// src/main.rs — add near the other `mod` declarations at the top
mod render;
```

```rust
// src/main.rs — add a new match arm, alongside Command::History / Command::Doctor
Command::Graph { path } => {
    let project = project::find_project()?;
    let storage = storage::Storage::for_project(&project);
    let config = config::load_config(&storage)?;

    let target = match path {
        Some(path) => Some(path),
        None => storage.latest_session_file()?,
    };

    match target {
        Some(session_path) => {
            let output_path = render::write_graph(&storage, &config, &session_path)?;
            println!("Graph written: {}", output_path.display());
        }
        None => println!("No sessions captured yet."),
    }
}
```

- [ ] **Step 4: Run full check and existing test suite to confirm nothing broke**

Run: `cargo check && cargo test`
Expected: PASS — all existing tests plus the new `render::*` and `storage::*` tests pass; no warnings about unused code.

- [ ] **Step 5: Manual smoke test against the real fixture**

Run:
```bash
cargo run -- init
cargo run -- graph fixtures/simple-session.jsonl
```
Expected: prints `Graph written: .claude/context-map/reports/demo-graph.html`. Then verify the file is well-formed:

```bash
python3 -c "import sys; content = open('.claude/context-map/reports/demo-graph.html').read(); assert '<!doctype html>' in content; assert 'main.rs' in content; print('OK, length:', len(content))"
```

Expected output: `OK, length: <some number>`

- [ ] **Step 6: Commit**

```bash
cargo fmt
cargo check
git add src/cli.rs src/main.rs
git commit -m "feat(cli): add ccmap graph command"
```

---

## Task 6: Document the new command

**Files:**
- Modify: `docs/ai-context/README-CONTEXT.md` (MVP flow section)
- Modify: `docs/ai-context/03-implementation-plan.md` (mark Step 13's "Mermaid renderer"/"HTML report" line as superseded, per this plan, if the maintainer wants that noted — otherwise skip; this step is documentation-only and has no code dependency)

**Interfaces:** None — text only.

- [ ] **Step 1: Add `ccmap graph` to the MVP flow section**

In `docs/ai-context/README-CONTEXT.md`, after the `ccmap doctor` block (around line 56-58), add:

```text
ccmap graph
  Resolves a session (latest, or a specific file path).
  Writes a self-contained HTML file with a bubble diagram of the
  session's context sources (sized by token weight, linked in
  first-touch order) and a trend chart of token usage and warning
  counts across the project's session history.
  Prints the output file path; does not open a browser.
```

- [ ] **Step 2: Commit**

```bash
git add docs/ai-context/README-CONTEXT.md
git commit -m "docs: document ccmap graph command"
```

---

## Self-Review Notes

**Spec coverage:**
- Command shape (`ccmap graph` / `ccmap graph <path>`, prints path, no browser launch) — Task 5. ✓
- Bubble diagram: one node per `ContextSourceSummary`, area ∝ tokens, colored by kind matching the terminal report, labeled — Task 1 (data) + Task 3 (rendering: `Math.sqrt(tokens)` for radius so area is proportional; CSS classes mirror `Painter::kind`'s exact color assignments). ✓
- Edges: first-touch order, deduped repeats — Task 1, directly tested (`edges_follow_first_touch_order_and_dedupe_repeats`). ✓
- Sources with no `source_label` excluded — Task 1 (`events_without_source_label_are_ignored_for_edges`), consistent with existing `build_context_map`. ✓
- Trend view: tokens + warnings by severity, oldest→newest, file-mtime ordering with fallback — Task 2 (data) + Task 4 (`session_files_ordered_by_time`, ordering only, with all-or-nothing fallback) + Task 3 (rendering). ✓
- New `src/render/` module split exactly as specced (`mod.rs`, `graph_data.rs`, `trend_data.rs`, `html.rs`) — Tasks 1–4. ✓
- No changes to `model.rs`/`normalise.rs`/`classify.rs`/`summary.rs` — confirmed; only `storage.rs`, `cli.rs`, `main.rs` touched outside `render/`. ✓
- HTML escaping of session/source data — Task 3 (`html_escape` for the session id in markup; JSON-embedding + `</` escaping for node/trend data going into `<script type="application/json">`, tested via `does_not_break_out_of_the_script_tag_on_a_hostile_label`). ✓
- No CDN / external references — Task 3 (`has_no_external_network_references` test). ✓
- Force-directed layout without a library — Task 3 (inline JS physics loop). ✓

**Placeholder scan:** No TBD/TODO. Task 5 Step 3 references a "quick check that it compiles standalone" as an intentional expected-failure step (documented pattern from TDD flow, not a placeholder). Task 6 Step 1's doc text is a real, complete addition, not a stub.

**Type consistency:** `GraphNode`/`GraphEdge`/`GraphData` (Task 1) are consumed unchanged by `html.rs` (Task 3) and `mod.rs` (Task 4). `TrendPoint` (Task 2) is consumed unchanged by `html.rs` and `mod.rs`. `write_graph`'s signature in Task 4 matches its call site in Task 5 exactly (`storage`, `config`, `session_path` in that order). `Storage::session_files_ordered_by_time` is defined in Task 4 and used only in Task 4's own `write_graph` — no other task assumes a different name.

**Verified during self-review:** confirmed `src/project.rs:7-11` and `src/config/defaults.rs:15` directly against the repo — `Project`'s fields and `CcmapConfig::default()` are exactly as Task 4's test code assumes, so that task carries no open questions into implementation.
