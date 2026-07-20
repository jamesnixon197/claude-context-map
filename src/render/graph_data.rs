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
        .map(|pair| GraphEdge {
            from: pair[0],
            to: pair[1],
        })
        .collect();

    GraphData { nodes, edges }
}

#[cfg(test)]
mod tests {
    use crate::model::{
        ContextConfidence, ContextEvent, ContextSourceKind, ContextSourceSummary, TriggerReason,
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
    ) -> crate::model::SessionAnalysis {
        use crate::model::SessionAnalysis;
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
        let data = super::build_graph_data(&analysis);
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
        let data = super::build_graph_data(&analysis);

        assert_eq!(data.nodes.len(), 3);
        assert_eq!(
            data.edges.len(),
            2,
            "3 nodes => 2 edges in the first-touch chain"
        );

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
        let data = super::build_graph_data(&analysis);
        assert_eq!(data.nodes.len(), 1);
        assert_eq!(data.edges.len(), 0);
    }

    #[test]
    fn single_node_produces_no_edges() {
        let map = vec![summary(ContextSourceKind::FileRead, "a.rs", 100)];
        let analysis = analysis_with(map, Vec::new());
        let data = super::build_graph_data(&analysis);
        assert_eq!(data.nodes.len(), 1);
        assert_eq!(data.edges.len(), 0);
    }
}
