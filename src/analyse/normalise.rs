use crate::analyse::classify::{classify_source_kind, match_confidence_for_kind};
use crate::analyse::estimate::{
    estimate_approximate_characters_used, estimate_approximate_tokens_used,
};
use crate::model::{ContextEvent, ContextSourceKind};
use serde_json::Value;

pub fn normalise_event(raw_value: &Value) -> ContextEvent {
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
