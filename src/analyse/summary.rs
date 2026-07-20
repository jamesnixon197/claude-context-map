use crate::analyse::warnings::build_warnings;
use crate::config::CcmapConfig;
use crate::model::{ContextEvent, ContextSourceKind, SessionAnalysis};

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
