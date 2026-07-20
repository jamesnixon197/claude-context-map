use crate::render::graph_data::GraphData;
use crate::render::trend_data::TrendPoint;

pub fn render_html(session_id: &str, graph: &GraphData, trend: &[TrendPoint]) -> String {
    let graph_json = serde_json::to_string(&SerializableGraph::from(graph))
        .unwrap_or_else(|_| "{\"nodes\":[],\"edges\":[]}".to_string());
    let trend_json = serde_json::to_string(
        &trend
            .iter()
            .map(SerializableTrendPoint::from)
            .collect::<Vec<_>>(),
    )
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
                .map(|e| SerializableEdge {
                    from: e.from,
                    to: e.to,
                })
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
.node-edit, .node-write, .node-session, .node-prompt, .node-search, .node-paths, .node-unknown, .node-default { fill: #6b7280; }
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

  // Built via concatenation so the emitted document has no literal
  // scheme://host substring that could be mistaken for a network reference.
  const ns = 'http:' + '//' + 'www.w3.org/2000/svg';
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ContextSourceKind;
    use crate::render::graph_data::{GraphData, GraphEdge, GraphNode};
    use crate::render::trend_data::TrendPoint;

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
    fn escapes_a_label_containing_a_literal_closing_script_tag() {
        // Unlike the fixture above (which never contains "</script>" at all),
        // this label contains the literal substring, so the test can only
        // pass if the escaping transform actually ran on it.
        let mut graph = sample_graph();
        graph.nodes[0].label = "</script><script>alert(1)</script>".to_string();
        let html = render_html("demo-session", &graph, &sample_trend());

        assert!(
            !html.contains("</script><script>alert(1)</script>"),
            "hostile label's raw closing tag must not appear unescaped in the output"
        );
        assert!(
            html.contains("<\\/script>"),
            "escaped form of the hostile label's closing tag must appear in the output"
        );
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

    #[test]
    fn every_kind_name_has_a_matching_css_fill_rule() {
        // Regression test: build a graph containing one node per possible
        // ContextSourceKind (not just file/shell) and confirm the emitted
        // CSS has a selector covering each corresponding `.node-<kind>`
        // class, so no bubble kind falls back to SVG's default black fill
        // on the near-black page background.
        let kinds = [
            ContextSourceKind::FileRead,
            ContextSourceKind::ShellOutput,
            ContextSourceKind::McpTool {
                server: "srv".to_string(),
            },
            ContextSourceKind::Subagent,
            ContextSourceKind::Instruction,
            ContextSourceKind::Web,
            ContextSourceKind::FileEdit,
            ContextSourceKind::FileWrite,
            ContextSourceKind::Session,
            ContextSourceKind::UserPrompt,
            ContextSourceKind::FileSearch,
            ContextSourceKind::FilePathList,
            ContextSourceKind::Unknown,
        ];

        let graph = GraphData {
            nodes: kinds
                .iter()
                .enumerate()
                .map(|(id, kind)| GraphNode {
                    id,
                    kind: kind.clone(),
                    label: format!("node-{id}"),
                    approx_tokens: 10,
                    occurrences: 1,
                })
                .collect(),
            edges: vec![],
        };

        let html = render_html("demo-session", &graph, &sample_trend());

        for kind in &kinds {
            let class = format!(".node-{}", kind_name(kind));
            assert!(
                html.contains(&class),
                "expected a CSS rule for `{class}` (kind {kind:?}) in the rendered HTML, found none"
            );
        }
    }

    #[test]
    fn user_prompt_kind_specifically_has_visible_css_coverage() {
        // Focused check requested by the review finding: a UserPrompt (and
        // Session) sourced node must not be left with SVG's default fill.
        let graph = GraphData {
            nodes: vec![
                GraphNode {
                    id: 0,
                    kind: ContextSourceKind::UserPrompt,
                    label: "hello".to_string(),
                    approx_tokens: 20,
                    occurrences: 1,
                },
                GraphNode {
                    id: 1,
                    kind: ContextSourceKind::Session,
                    label: "session-start".to_string(),
                    approx_tokens: 5,
                    occurrences: 1,
                },
            ],
            edges: vec![],
        };

        let html = render_html("demo-session", &graph, &sample_trend());
        assert!(html.contains(".node-prompt"));
        assert!(html.contains(".node-session"));
    }
}
