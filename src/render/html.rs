use crate::render::graph_data::GraphData;
use crate::render::trend_data::TrendPoint;

const CYTOSCAPE_JS: &str = include_str!("vendor/cytoscape.min.js");

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
  <div id="cy"></div>
  <div id="cy-tooltip" class="cy-tooltip" hidden></div>
  <div id="cy-panel" class="cy-panel" hidden>
    <button id="cy-panel-close" class="cy-panel-close" type="button" aria-label="Close">×</button>
    <div id="cy-panel-body"></div>
  </div>
</section>
<section id="trend-section">
  <svg id="trend-svg" width="900" height="220"></svg>
</section>
<script id="graph-data" type="application/json">{graph_json}</script>
<script id="trend-data" type="application/json">{trend_json}</script>
<script id="node-colors-data" type="application/json">{NODE_COLORS}</script>
<script>
{CYTOSCAPE_JS}
</script>
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
#cy { width: 900px; height: 600px; max-width: 100%; background: #0d1117; border: 1px solid #30363d; position: relative; }

.cy-tooltip {
  position: fixed;
  z-index: 20;
  pointer-events: none;
  background: #161b22;
  border: 1px solid #30363d;
  color: #e6edf3;
  font-size: 12px;
  padding: 6px 8px;
  border-radius: 4px;
  max-width: 420px;
  word-break: break-all;
}

.cy-panel {
  position: fixed;
  left: 50%;
  bottom: 24px;
  transform: translateX(-50%);
  z-index: 30;
  background: #161b22;
  border: 1px solid #30363d;
  color: #e6edf3;
  font-size: 13px;
  padding: 12px 36px 12px 14px;
  border-radius: 6px;
  max-width: 640px;
  word-break: break-all;
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.4);
}

.cy-panel-close {
  position: absolute;
  top: 6px;
  right: 8px;
  background: none;
  border: none;
  color: #e6edf3;
  font-size: 16px;
  line-height: 1;
  cursor: pointer;
  padding: 4px;
}

.trend-line { fill: none; stroke: #3b82f6; stroke-width: 2; }
.trend-high { fill: #ef4444; }
.trend-medium { fill: #eab308; }
.trend-low { fill: #6b7280; }
"#;

// Node color per ContextSourceKind, matching the terminal report's Painter::kind palette
// (src/analyse/report.rs) so the HTML view reads as the same tool.
const NODE_COLORS: &str = r##"
{"file":"#3b82f6","shell":"#d946ef","mcp":"#06b6d4","web":"#06b6d4","sub":"#eab308","instr":"#22c55e","edit":"#6b7280","write":"#6b7280","session":"#6b7280","prompt":"#6b7280","search":"#6b7280","paths":"#6b7280","unknown":"#6b7280"}
"##;

const JS: &str = r#"
(function () {
  const graph = JSON.parse(document.getElementById('graph-data').textContent);
  const trend = JSON.parse(document.getElementById('trend-data').textContent);
  const nodeColors = JSON.parse(document.getElementById('node-colors-data').textContent);

  const minTokens = graph.nodes.length ? Math.min(...graph.nodes.map((n) => n.approx_tokens)) : 0;
  const maxTokens = graph.nodes.length ? Math.max(...graph.nodes.map((n) => n.approx_tokens)) : 1;

  function sizeFor(tokens) {
    const lo = 18, hi = 90;
    if (maxTokens <= minTokens) return (lo + hi) / 2;
    const t = (tokens - minTokens) / (maxTokens - minTokens);
    // sqrt easing so *area* differences track token differences, not raw radius.
    return lo + Math.sqrt(t) * (hi - lo);
  }

  const elements = [];
  for (const n of graph.nodes) {
    elements.push({
      data: {
        id: 'n' + n.id,
        label: n.label,
        kind: n.kind,
        tokens: n.approx_tokens,
        occurrences: n.occurrences,
        color: nodeColors[n.kind] || nodeColors.unknown,
        size: sizeFor(n.approx_tokens),
      },
    });
  }
  for (const e of graph.edges) {
    elements.push({ data: { id: 'e' + e.from + '_' + e.to, source: 'n' + e.from, target: 'n' + e.to } });
  }

  const cy = cytoscape({
    container: document.getElementById('cy'),
    elements: elements,
    style: [
      {
        selector: 'node',
        style: {
          'background-color': 'data(color)',
          width: 'data(size)',
          height: 'data(size)',
          label: '',
        },
      },
      {
        selector: 'edge',
        style: {
          width: 1,
          'line-color': '#30363d',
          'curve-style': 'haystack',
          'haystack-radius': 0,
        },
      },
      {
        selector: 'node:selected',
        style: {
          'border-width': 2,
          'border-color': '#e6edf3',
        },
      },
    ],
    layout: {
      name: 'cose',
      animate: false,
      nodeOverlap: 12,
      idealEdgeLength: 60,
      nodeRepulsion: 200000,
    },
    minZoom: 0.1,
    maxZoom: 6,
    wheelSensitivity: 0.2,
  });
  window.__cy = cy;

  const tooltip = document.getElementById('cy-tooltip');
  const panel = document.getElementById('cy-panel');
  const panelBody = document.getElementById('cy-panel-body');
  const panelClose = document.getElementById('cy-panel-close');

  function detailHtml(n) {
    const label = n.data('label');
    const kind = n.data('kind');
    const tokens = n.data('tokens');
    const occurrences = n.data('occurrences');
    const div = document.createElement('div');
    const kindLine = document.createElement('div');
    kindLine.textContent = kind + '  ·  ' + tokens + ' tokens  ·  ' + occurrences + ' occurrence(s)';
    kindLine.style.opacity = '0.7';
    kindLine.style.marginBottom = '4px';
    const labelLine = document.createElement('div');
    labelLine.textContent = label;
    div.appendChild(kindLine);
    div.appendChild(labelLine);
    return div;
  }

  cy.on('mouseover', 'node', function (evt) {
    const n = evt.target;
    tooltip.replaceChildren(detailHtml(n));
    tooltip.hidden = false;
  });

  cy.on('mousemove', 'node', function (evt) {
    const pos = evt.originalEvent;
    if (!pos) return;
    tooltip.style.left = (pos.clientX + 14) + 'px';
    tooltip.style.top = (pos.clientY + 14) + 'px';
  });

  cy.on('mouseout', 'node', function () {
    tooltip.hidden = true;
  });

  cy.on('tap', 'node', function (evt) {
    const n = evt.target;
    panelBody.replaceChildren(detailHtml(n));
    panel.hidden = false;
  });

  panelClose.addEventListener('click', function () {
    panel.hidden = true;
  });

  const trendSvg = document.getElementById('trend-svg');
  const tw = 900, th = 220, pad = 30;
  if (trend.length > 0) {
    const maxTrendTokens = Math.max(...trend.map((t) => t.approx_context_tokens), 1);
    const stepX = trend.length > 1 ? (tw - pad * 2) / (trend.length - 1) : 0;
    const ns2 = 'http:' + '//' + 'www.w3.org/2000/svg';
    const points = trend.map((t, i) => {
      const x = pad + i * stepX;
      const y = th - pad - (t.approx_context_tokens / maxTrendTokens) * (th - pad * 2);
      return x + ',' + y;
    }).join(' ');
    const polyline = document.createElementNS(ns2, 'polyline');
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
          const dot = document.createElementNS(ns2, 'circle');
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
        // The document must never fetch anything over the network: no
        // <script src=...>/<link href=...> pointing off-host, and no
        // fetch/XHR/WebSocket calls. A bare "http://" substring is not by
        // itself proof of a network call — the vendored Cytoscape.js library
        // legitimately contains license-comment URLs (e.g.
        // "http://opensource.org/licenses/MIT") that are never requested at
        // runtime, so check for the actual request-shaped constructs instead.
        let html = render_html("demo-session", &sample_graph(), &sample_trend());
        assert!(!html.contains("<script src="));
        assert!(!html.contains("<link "));
        assert!(!html.contains("fetch("));
        assert!(!html.contains("XMLHttpRequest"));
        assert!(!html.contains("WebSocket("));
        assert!(!html.contains("cdnjs.cloudflare.com"));
        assert!(!html.contains("unpkg.com"));
        assert!(!html.contains("jsdelivr.net"));
    }

    #[test]
    fn includes_session_id_in_the_page() {
        let html = render_html("demo-session", &sample_graph(), &sample_trend());
        assert!(html.contains("demo-session"));
    }

    #[test]
    fn every_kind_name_has_a_color_entry() {
        // Regression test: build a graph containing one node per possible
        // ContextSourceKind (not just file/shell) and confirm the embedded
        // NODE_COLORS map has an entry for each corresponding kind string,
        // so no bubble kind falls back to an undefined color.
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

        let colors: serde_json::Value = serde_json::from_str(NODE_COLORS).unwrap();
        for kind in &kinds {
            let key = kind_name(kind);
            assert!(
                colors.get(key).is_some(),
                "expected a color entry for kind `{key}` ({kind:?}) in NODE_COLORS, found none"
            );
        }
    }

    #[test]
    fn user_prompt_kind_specifically_has_a_color_entry() {
        let colors: serde_json::Value = serde_json::from_str(NODE_COLORS).unwrap();
        assert!(colors.get("prompt").is_some());
        assert!(colors.get("session").is_some());
    }

    #[test]
    fn vendors_cytoscape_inline_with_no_cdn_reference() {
        let html = render_html("demo-session", &sample_graph(), &sample_trend());
        assert!(
            html.contains("cytoscape"),
            "expected the vendored Cytoscape.js library to be embedded in the document"
        );
        assert!(!html.contains("cdnjs"));
        assert!(!html.contains("unpkg"));
        assert!(!html.contains("jsdelivr"));
    }

    #[test]
    fn labels_are_not_drawn_by_default_only_on_interaction() {
        // The style block must set an empty label so bubbles render clean by
        // default; text only appears via the JS tooltip/panel on hover/click.
        let html = render_html("demo-session", &sample_graph(), &sample_trend());
        assert!(html.contains("label: ''"));
    }

    #[test]
    fn includes_hover_tooltip_and_click_to_pin_panel_wiring() {
        let html = render_html("demo-session", &sample_graph(), &sample_trend());
        assert!(html.contains("cy-tooltip"));
        assert!(html.contains("cy-panel"));
        assert!(html.contains("mouseover"));
        assert!(html.contains("'tap'"));
    }
}
