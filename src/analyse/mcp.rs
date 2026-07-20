use serde_json::Value;

const IDENTIFIER_KEYS: &[&str] = &[
    "issue_id",
    "issue_key",
    "ticket_id",
    "board_id",
    "file_id",
    "document_id",
    "page_id",
    "task_id",
    "task_gid",
    "query",
    "url",
    "id",
    "name",
];

pub fn extract_mcp_label(tool_name: &str, server: &str, raw_value: &Value) -> String {
    let operation = tool_name.rsplit("__").next().unwrap_or(tool_name);
    let identifier = extract_mcp_identifier(raw_value);

    match identifier {
        Some(identifier) => format!("{server}: {operation}({identifier})"),
        None => format!("{server}: {operation}"),
    }
}

fn extract_mcp_identifier(raw_value: &Value) -> Option<String> {
    let tool_input = raw_value.get("tool_input")?.as_object()?;

    IDENTIFIER_KEYS
        .iter()
        .find_map(|key| tool_input.get(*key).and_then(|value| value.as_str()))
        .map(|value| value.to_string())
}
