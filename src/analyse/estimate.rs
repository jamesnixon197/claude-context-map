use crate::model::ContextSourceKind;
use serde_json::Value;

pub fn estimate_approximate_characters_used(raw_value: &Value, kind: &ContextSourceKind) -> usize {
    match kind {
        ContextSourceKind::FileRead
        | ContextSourceKind::FileSearch
        | ContextSourceKind::ShellOutput
        | ContextSourceKind::Web
        | ContextSourceKind::McpTool { .. } => raw_value
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

pub fn estimate_approximate_tokens_used(approximate_characters: usize) -> usize {
    if approximate_characters == 0 {
        return 0;
    }

    (approximate_characters as f64 / 4.0).ceil() as usize
}
