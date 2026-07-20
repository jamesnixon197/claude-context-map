use crate::analyse::classify::{classify_source_kind, match_confidence_for_kind};
use crate::analyse::estimate::{
    estimate_approximate_characters_used, estimate_approximate_tokens_used,
};
use crate::analyse::mcp::extract_mcp_label;
use crate::model::{ContextEvent, ContextSourceKind, TriggerReason};
use serde_json::Value;

pub fn normalise_events(raw_events: &[Value]) -> Vec<ContextEvent> {
    let mut subagent_stack: Vec<String> = Vec::new();

    raw_events
        .iter()
        .map(|raw_value| normalise_event(raw_value, &mut subagent_stack))
        .collect()
}

fn normalise_event(raw_value: &Value, subagent_stack: &mut Vec<String>) -> ContextEvent {
    let session_id = get_string_field("session_id", raw_value)
        .unwrap_or_else(|| "unknown-session-id".to_string());
    let event_name = get_string_field("hook_event_name", raw_value)
        .unwrap_or_else(|| "Unknown Event Name".to_string());
    let tool_name = get_string_field("tool_name", raw_value);

    let event_source_kind = classify_source_kind(&event_name, tool_name.as_deref());
    let confidence = match_confidence_for_kind(&event_source_kind);

    let path = extract_path(raw_value, &event_source_kind);
    let command = extract_command(raw_value);
    let source_label = extract_source_label(
        raw_value,
        &event_source_kind,
        &path,
        &command,
        tool_name.as_deref(),
    );
    let trigger_reason = resolve_trigger_reason(&event_name, &event_source_kind, subagent_stack);

    update_subagent_stack(&event_name, raw_value, subagent_stack);

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
        source_label,
        trigger_reason,
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

fn extract_url(raw_value: &Value) -> Option<String> {
    raw_value
        .pointer("/tool_input/url")
        .and_then(|value| value.as_str())
        .map(|value| value.to_string())
}

fn extract_subagent_name(raw_value: &Value) -> Option<String> {
    raw_value
        .get("subagent_type")
        .or_else(|| raw_value.get("agent_name"))
        .and_then(|value| value.as_str())
        .map(|value| value.to_string())
}

fn extract_source_label(
    raw_value: &Value,
    kind: &ContextSourceKind,
    path: &Option<String>,
    command: &Option<String>,
    tool_name: Option<&str>,
) -> Option<String> {
    match kind {
        ContextSourceKind::McpTool { server } => {
            let tool_name = tool_name.unwrap_or(server);
            Some(extract_mcp_label(tool_name, server, raw_value))
        }
        ContextSourceKind::Web => extract_url(raw_value),
        ContextSourceKind::Subagent => extract_subagent_name(raw_value),
        ContextSourceKind::FileRead
        | ContextSourceKind::FileEdit
        | ContextSourceKind::FileWrite
        | ContextSourceKind::Instruction => path.clone(),
        ContextSourceKind::ShellOutput => command.clone(),
        _ => None,
    }
}

fn resolve_trigger_reason(
    event_name: &str,
    kind: &ContextSourceKind,
    subagent_stack: &[String],
) -> TriggerReason {
    match event_name {
        "SessionStart" => TriggerReason::SessionStart,
        "UserPromptSubmitted" => TriggerReason::UserPrompt,
        "SubagentStart" | "SubagentStop" => TriggerReason::SubagentLifecycle,
        _ => match (kind, subagent_stack.last()) {
            (ContextSourceKind::Unknown, _) => TriggerReason::Unknown,
            (_, Some(subagent)) => TriggerReason::SubagentActivity {
                subagent: subagent.clone(),
            },
            (_, None) => TriggerReason::DirectToolCall,
        },
    }
}

fn update_subagent_stack(event_name: &str, raw_value: &Value, subagent_stack: &mut Vec<String>) {
    match event_name {
        "SubagentStart" => {
            let subagent =
                extract_subagent_name(raw_value).unwrap_or_else(|| "unknown".to_string());
            subagent_stack.push(subagent);
        }
        "SubagentStop" => {
            subagent_stack.pop();
        }
        _ => {}
    }
}
