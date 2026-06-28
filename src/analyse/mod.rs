use crate::model::{
    ContextConfidence, ContextEvent, ContextSourceKind, ContextWarning, SessionAnalysis,
    WarningSeverity,
};
use anyhow::{Result, anyhow};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

pub fn analyse_file(path: &Path) -> Result<SessionAnalysis> {
    let content = fs::read_to_string(path)?;

    let mut raw_events = Vec::new();

    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }

        let value: Value = serde_json::from_str(line)?;
        raw_events.push(value);
    }

    if raw_events.is_empty() {
        return Err(anyhow!("Session file contains no events"));
    }

    let context_events = raw_events.iter().map(get_context_event).collect::<Vec<_>>();

    Ok(build_event_analysis(context_events))
}

fn get_context_event(raw_value: &Value) -> ContextEvent {
    let session_id = get_string_field("session_id", raw_value)
        .unwrap_or_else(|| "unknown-session-id".to_string());
    let event_name = get_string_field("hook_event_name", raw_value)
        .unwrap_or_else(|| "Unknown Event Name".to_string());
    let tool_name = get_string_field("tool_name", raw_value);

    let event_source_kind = classify_source_kind(&event_name, tool_name.as_deref());
    let confidence = match_confidence_for_kind(&event_source_kind);

    let path = extract_path(raw_value, &event_source_kind);
    let command = extract_command(raw_value);
    let approximate_characters_used =
        estimate_approximate_characters_used(raw_value, &event_source_kind);
    let approximate_tokens_used = estimate_approximate_tokens_used(approximate_characters_used);

    ContextEvent {
        session_id,
        event_name,
        source_kind: event_source_kind,
        path,
        command,
        tool_name,
        approx_chars: approximate_characters_used,
        approx_tokens: approximate_tokens_used,
        confidence,
    }
}

fn classify_source_kind(event_name: &str, tool_name: Option<&str>) -> ContextSourceKind {
    match event_name {
        "SessionStart" => ContextSourceKind::Session,
        "UserPromptSubmitted" => ContextSourceKind::UserPrompt,
        "InstructionsLoaded" => ContextSourceKind::Instruction,
        "SubagentStart" | "SubagentStop" => ContextSourceKind::Subagent,

        "PostToolUse" | "PostToolBatch" => match tool_name {
            Some("Read") => ContextSourceKind::FileRead,
            Some("Grep") => ContextSourceKind::FileSearch,
            Some("Glob") => ContextSourceKind::FilePathList,
            Some("Bash") => ContextSourceKind::ShellOutput,
            Some("Edit") => ContextSourceKind::FileEdit,
            Some("MultiEdit") => ContextSourceKind::FileEdit,
            Some("Write") => ContextSourceKind::FileWrite,
            Some("WebFetch") => ContextSourceKind::Web,
            _ => ContextSourceKind::Unknown,
        },

        _ => ContextSourceKind::Unknown,
    }
}

fn match_confidence_for_kind(kind: &ContextSourceKind) -> ContextConfidence {
    match kind {
        ContextSourceKind::Instruction => ContextConfidence::High,
        ContextSourceKind::FileRead => ContextConfidence::High,
        ContextSourceKind::FileSearch => ContextConfidence::Medium,
        ContextSourceKind::FilePathList => ContextConfidence::Low,
        ContextSourceKind::ShellOutput => ContextConfidence::Opaque,
        ContextSourceKind::Subagent => ContextConfidence::Opaque,
        ContextSourceKind::FileEdit => ContextConfidence::None,
        ContextSourceKind::FileWrite => ContextConfidence::None,
        ContextSourceKind::Session => ContextConfidence::None,
        ContextSourceKind::UserPrompt => ContextConfidence::High,
        ContextSourceKind::Web => ContextConfidence::Medium,
        ContextSourceKind::Unknown => ContextConfidence::Opaque,
    }
}

fn get_string_field(field: &str, raw_value: &Value) -> Option<String> {
    raw_value
        .get(field)
        .and_then(|value| value.as_str())
        .map(|value| value.to_string())
}

fn extract_path(raw_value: &Value, kind: &ContextSourceKind) -> Option<String> {
    match kind {
        ContextSourceKind::Instruction => raw_value
            .get("file_path")
            .and_then(|value| value.as_str())
            .map(|value| value.to_string()),
        ContextSourceKind::FileRead
        | ContextSourceKind::FileEdit
        | ContextSourceKind::FileWrite => raw_value
            .pointer("/tool_input/file_path")
            .and_then(|value| value.as_str())
            .map(|value| value.to_string()),
        _ => None,
    }
}

fn extract_command(raw_value: &Value) -> Option<String> {
    raw_value
        .pointer("/tool_input/command")
        .and_then(|value| value.as_str())
        .map(|value| value.to_string())
}

fn estimate_approximate_characters_used(raw_value: &Value, kind: &ContextSourceKind) -> usize {
    match kind {
        ContextSourceKind::FileRead
        | ContextSourceKind::FileSearch
        | ContextSourceKind::ShellOutput
        | ContextSourceKind::Web => raw_value
            .get("tool_response")
            .map(|value| value.to_string().chars().count())
            .unwrap_or(0),

        ContextSourceKind::Instruction => raw_value
            .get("content")
            .and_then(|value| value.as_str())
            .map(|value| value.to_string().chars().count())
            .unwrap_or(0),

        _ => 0,
    }
}

fn estimate_approximate_tokens_used(approximate_characters: usize) -> usize {
    if approximate_characters == 0 {
        return 0;
    }

    (approximate_characters as f64 / 4.0).ceil() as usize
}

fn build_event_analysis(events: Vec<ContextEvent>) -> SessionAnalysis {
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

    let warnings = build_warnings(&events);

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

fn build_warnings(events: &[ContextEvent]) -> Vec<ContextWarning> {
    let mut warnings = Vec::new();

    warnings.extend(warn_large_context_sources(events));
    warnings.extend(warn_lockfile_reads(events));

    warnings
}

fn warn_large_context_sources(events: &[ContextEvent]) -> Vec<ContextWarning> {
    events
        .iter()
        .filter(|event| event.approx_tokens > 4_000)
        .map(|event| ContextWarning {
            severity: WarningSeverity::Medium,
            title: format!("Large context source detected: {}", describe_event(event)),
            detail: format!(
                "The context source '{}' has approximately {} tokens, which may be too large for effective processing.",
                describe_event(event),
                event.approx_tokens
            ),
        })
        .collect()
}

fn warn_lockfile_reads(events: &[ContextEvent]) -> Vec<ContextWarning> {
    events
        .iter()
        .filter(|event| {
            if let Some(path) = &event.path {
                path.ends_with("package-lock.json") || path.ends_with("yarn.lock")
            } else {
                false
            }
        })
        .map(|event| ContextWarning {
            severity: WarningSeverity::Low,
            title: format!("Lockfile read detected: {}", describe_event(event)),
            detail: format!(
                "The context source '{}' is a lockfile, which may not provide meaningful context for analysis.",
                describe_event(event)
            ),
        })
        .collect()
}

fn describe_event(event: &ContextEvent) -> String {
    if let Some(path) = &event.path {
        return path.clone();
    }

    if let Some(command) = &event.command {
        return format!("Command: {}", command);
    }

    event
        .tool_name
        .clone()
        .unwrap_or_else(|| format!("{:?}", event.source_kind))
}
