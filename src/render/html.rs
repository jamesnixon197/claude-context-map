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
    let legend_html = render_legend();
    let node_colors_json = node_colors_json();

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
<section id="legend-section">
{legend_html}
</section>
<section id="trend-section">
  <h2>Context usage over time</h2>
  <div class="legend">
    <span class="legend-item"><span class="legend-swatch" style="background:#3b82f6"></span>Tokens used per session</span>
    <span class="legend-item"><span class="legend-swatch" style="background:#ef4444"></span>High-severity warning</span>
    <span class="legend-item"><span class="legend-swatch" style="background:#eab308"></span>Medium-severity warning</span>
    <span class="legend-item"><span class="legend-swatch" style="background:#6b7280"></span>Low-severity warning</span>
  </div>
  <svg id="trend-svg" width="900" height="280"></svg>
  <div id="trend-tooltip" class="cy-tooltip" hidden></div>
</section>
<script id="graph-data" type="application/json">{graph_json}</script>
<script id="trend-data" type="application/json">{trend_json}</script>
<script id="node-colors-data" type="application/json">{node_colors_json}</script>
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

// Single source of truth for the legend AND the NODE_COLORS JSON blob below —
// every entry here must have a matching kind_name() output (enforced by the
// every_kind_name_has_a_color_entry test) so the legend can never drift out
// of sync with what a bubble is actually colored.
const LEGEND_ENTRIES: &[(&str, &str, &str)] = &[
    ("file", "File read", "#3b82f6"),
    ("shell", "Shell output", "#d946ef"),
    ("mcp", "MCP tool", "#06b6d4"),
    ("web", "Web fetch", "#06b6d4"),
    ("sub", "Subagent", "#eab308"),
    ("instr", "Instruction", "#22c55e"),
    ("edit", "File edit", "#6b7280"),
    ("write", "File write", "#6b7280"),
    ("session", "Session", "#6b7280"),
    ("prompt", "User prompt", "#6b7280"),
    ("search", "File search", "#6b7280"),
    ("paths", "Path list", "#6b7280"),
    ("unknown", "Unknown", "#6b7280"),
];

fn render_legend() -> String {
    // Grey (#6b7280) covers several kinds — collapse consecutive/duplicate
    // colors into one legend swatch labeled with all their names, rather
    // than showing seven identical grey dots in a row.
    let mut by_color: Vec<(&str, Vec<&str>)> = Vec::new();
    for (_, label, color) in LEGEND_ENTRIES {
        match by_color.iter_mut().find(|(c, _)| c == color) {
            Some((_, labels)) => labels.push(label),
            None => by_color.push((color, vec![label])),
        }
    }

    let items: String = by_color
        .iter()
        .map(|(color, labels)| {
            format!(
                r#"<span class="legend-item"><span class="legend-swatch" style="background:{color}"></span>{}</span>"#,
                html_escape(&labels.join(" / "))
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(r#"<div class="legend">{items}</div>"#)
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

.legend { display: flex; flex-wrap: wrap; gap: 6px 18px; font-size: 12px; color: #8b949e; }
.legend-item { display: inline-flex; align-items: center; gap: 6px; white-space: nowrap; }
.legend-swatch { display: inline-block; width: 10px; height: 10px; border-radius: 50%; }

h2 { font-size: 0.95rem; font-weight: 600; margin: 0 0 0.75rem 0; color: #8b949e; }
#trend-svg { max-width: 100%; }
.trend-axis-line { stroke: #30363d; stroke-width: 1; }
.trend-gridline { stroke: #21262d; stroke-width: 1; }
.trend-axis-label { fill: #8b949e; font-size: 10px; }
.trend-line { fill: none; stroke: #3b82f6; stroke-width: 2; }
.trend-point { fill: #3b82f6; stroke: #0d1117; stroke-width: 1.5; }
.trend-high { fill: #ef4444; }
.trend-medium { fill: #eab308; }
.trend-low { fill: #6b7280; }
"#;

// Node color per ContextSourceKind, matching the terminal report's Painter::kind palette
// (src/analyse/report.rs) so the HTML view reads as the same tool. Derived from
// LEGEND_ENTRIES so the legend and the bubble colors can never drift apart.
fn node_colors_json() -> String {
    let pairs: Vec<String> = LEGEND_ENTRIES
        .iter()
        .map(|(kind, _, color)| format!("\"{kind}\":\"{color}\""))
        .collect();
    format!("{{{}}}", pairs.join(","))
}

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
    wheelSensitivity: 1.5,
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
  const trendTooltip = document.getElementById('trend-tooltip');
  const ns2 = 'http:' + '//' + 'www.w3.org/2000/svg';

  function svgEl(tag, attrs) {
    const el = document.createElementNS(ns2, tag);
    for (const k in attrs) el.setAttribute(k, attrs[k]);
    return el;
  }

  function shortSessionId(id) {
    return id.length > 8 ? id.slice(0, 8) + '…' : id;
  }

  function showTrendTooltip(evt, lines) {
    const div = document.createElement('div');
    lines.forEach((line) => {
      const p = document.createElement('div');
      p.textContent = line;
      div.appendChild(p);
    });
    trendTooltip.replaceChildren(div);
    trendTooltip.style.left = (evt.clientX + 14) + 'px';
    trendTooltip.style.top = (evt.clientY + 14) + 'px';
    trendTooltip.hidden = false;
  }

  function hideTrendTooltip() {
    trendTooltip.hidden = true;
  }

  if (trend.length > 0) {
    const tw = 900, th = 280;
    const padL = 56, padR = 20, padT = 16, padB = 40;
    const plotW = tw - padL - padR;
    const plotH = th - padT - padB;

    const maxTrendTokens = Math.max(...trend.map((t) => t.approx_context_tokens), 1);
    const stepX = trend.length > 1 ? plotW / (trend.length - 1) : 0;
    const xFor = (i) => padL + (trend.length > 1 ? i * stepX : plotW / 2);
    const yFor = (tokens) => padT + plotH - (tokens / maxTrendTokens) * plotH;

    // Y-axis gridlines + token-count labels, at 4 even steps.
    const ySteps = 4;
    for (let s = 0; s <= ySteps; s++) {
      const value = Math.round((maxTrendTokens / ySteps) * s);
      const y = padT + plotH - (s / ySteps) * plotH;
      trendSvg.appendChild(svgEl('line', {
        x1: padL, x2: tw - padR, y1: y, y2: y, class: 'trend-gridline',
      }));
      const label = svgEl('text', {
        x: padL - 8, y: y + 3, class: 'trend-axis-label', 'text-anchor': 'end',
      });
      label.textContent = value.toLocaleString();
      trendSvg.appendChild(label);
    }

    // Axis lines.
    trendSvg.appendChild(svgEl('line', { x1: padL, x2: padL, y1: padT, y2: padT + plotH, class: 'trend-axis-line' }));
    trendSvg.appendChild(svgEl('line', { x1: padL, x2: tw - padR, y1: padT + plotH, y2: padT + plotH, class: 'trend-axis-line' }));

    // X-axis session labels (skip some if too many to fit without overlap).
    const maxLabels = 12;
    const labelStride = Math.max(1, Math.ceil(trend.length / maxLabels));
    trend.forEach((t, i) => {
      if (i % labelStride !== 0 && i !== trend.length - 1) return;
      const label = svgEl('text', {
        x: xFor(i), y: padT + plotH + 16, class: 'trend-axis-label', 'text-anchor': 'middle',
      });
      label.textContent = shortSessionId(t.session_id);
      trendSvg.appendChild(label);
    });

    // Token line.
    const points = trend.map((t, i) => xFor(i) + ',' + yFor(t.approx_context_tokens)).join(' ');
    trendSvg.appendChild(svgEl('polyline', { points: points, class: 'trend-line' }));

    // Token points, with hover tooltip showing exact session + token count.
    trend.forEach((t, i) => {
      const x = xFor(i), y = yFor(t.approx_context_tokens);
      const point = svgEl('circle', { cx: x, cy: y, r: 3.5, class: 'trend-point' });
      point.addEventListener('mousemove', (evt) => showTrendTooltip(evt, [
        t.session_id,
        t.approx_context_tokens.toLocaleString() + ' tokens',
      ]));
      point.addEventListener('mouseleave', hideTrendTooltip);
      trendSvg.appendChild(point);
    });

    // Warning severity dots, stacked below the x-axis per session.
    trend.forEach((t, i) => {
      const x = xFor(i);
      const barBase = padT + plotH + 26;
      const sizes = [
        ['trend-high', t.high_warnings, t.high_warnings + ' high-severity warning(s)'],
        ['trend-medium', t.medium_warnings, t.medium_warnings + ' medium-severity warning(s)'],
        ['trend-low', t.low_warnings, t.low_warnings + ' low-severity warning(s)'],
      ];
      let offset = 0;
      for (const [cls, count, desc] of sizes) {
        if (count === 0) continue;
        const dot = svgEl('circle', { cx: x, cy: barBase + offset * 7, r: 2.5, class: cls });
        dot.addEventListener('mousemove', (evt) => showTrendTooltip(evt, [t.session_id, desc]));
        dot.addEventListener('mouseleave', hideTrendTooltip);
        trendSvg.appendChild(dot);
        offset++;
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

        let colors: serde_json::Value = serde_json::from_str(&node_colors_json()).unwrap();
        for kind in &kinds {
            let key = kind_name(kind);
            assert!(
                colors.get(key).is_some(),
                "expected a color entry for kind `{key}` ({kind:?}) in node_colors_json(), found none"
            );
        }
    }

    #[test]
    fn user_prompt_kind_specifically_has_a_color_entry() {
        let colors: serde_json::Value = serde_json::from_str(&node_colors_json()).unwrap();
        assert!(colors.get("prompt").is_some());
        assert!(colors.get("session").is_some());
    }

    #[test]
    fn legend_covers_every_kind_name_with_a_visible_label() {
        let legend = render_legend();
        for (_, label, _) in LEGEND_ENTRIES {
            assert!(
                legend.contains(label),
                "expected legend to mention `{label}`, found none in: {legend}"
            );
        }
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
