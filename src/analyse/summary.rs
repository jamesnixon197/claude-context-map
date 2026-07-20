use crate::analyse::warnings::build_warnings;
use crate::config::CcmapConfig;
use crate::model::{ContextEvent, ContextSourceKind, ContextSourceSummary, SessionAnalysis};
use std::collections::HashMap;

pub fn build_analysis(events: Vec<ContextEvent>, config: &CcmapConfig) -> SessionAnalysis {
    let session_id = events
        .first()
        .map(|event| event.session_id.clone())
        .unwrap_or_else(|| "unknown-session-id".to_string());

    let instruction_files_loaded = count_events_of_kind(&events, ContextSourceKind::Instruction);
    let files_read = count_events_of_kind(&events, ContextSourceKind::FileRead);
    let file_searches = count_events_of_kind(&events, ContextSourceKind::FileSearch);
    let file_path_lists = count_events_of_kind(&events, ContextSourceKind::FilePathList);
    let bash_commands = count_events_of_kind(&events, ContextSourceKind::ShellOutput);
    let files_edited = count_events_of_kind(&events, ContextSourceKind::FileEdit);
    let files_written = count_events_of_kind(&events, ContextSourceKind::FileWrite);
    let subagents = count_events_of_kind(&events, ContextSourceKind::Subagent);

    let warnings = build_warnings(&events, config);
    let context_map = build_context_map(&events);

    SessionAnalysis {
        session_id,
        instruction_files_loaded,
        files_read,
        file_searches,
        file_path_lists,
        bash_commands,
        files_edited,
        files_written,
        subagents,
        approx_context_tokens: events.iter().map(|event| event.approx_tokens).sum(),
        context_map,
        events,
        warnings,
    }
}

fn count_events_of_kind(events: &[ContextEvent], kind: ContextSourceKind) -> usize {
    events
        .iter()
        .filter(|event| event.source_kind == kind)
        .count()
}

fn build_context_map(events: &[ContextEvent]) -> Vec<ContextSourceSummary> {
    let mut by_source: HashMap<(ContextSourceKind, String), ContextSourceSummary> = HashMap::new();

    for event in events {
        let Some(label) = &event.source_label else {
            continue;
        };

        let key = (event.source_kind.clone(), label.clone());

        let summary = by_source
            .entry(key)
            .or_insert_with(|| ContextSourceSummary {
                source_kind: event.source_kind.clone(),
                source_label: label.clone(),
                occurrences: 0,
                approx_tokens: 0,
                trigger_reasons: Vec::new(),
            });

        summary.occurrences += 1;
        summary.approx_tokens += event.approx_tokens;

        if !summary.trigger_reasons.contains(&event.trigger_reason) {
            summary.trigger_reasons.push(event.trigger_reason.clone());
        }
    }

    let mut context_map: Vec<_> = by_source.into_values().collect();
    context_map.sort_by(|a, b| b.approx_tokens.cmp(&a.approx_tokens));

    context_map
}
