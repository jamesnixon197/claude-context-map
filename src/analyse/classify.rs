use crate::model::{ContextConfidence, ContextSourceKind};

pub fn classify_source_kind(event_name: &str, tool_name: Option<&str>) -> ContextSourceKind {
    match event_name {
        "SessionStart" => ContextSourceKind::Session,
        "UserPromptSubmitted" => ContextSourceKind::UserPrompt,
        "InstructionsLoaded" => ContextSourceKind::Instruction,
        "SubagentStart" | "SubagentStop" => ContextSourceKind::Subagent,

        "PostToolUse" | "PostToolBatch" => {
            if let Some(server) = tool_name.and_then(extract_mcp_server) {
                return ContextSourceKind::McpTool { server };
            }

            match tool_name {
                Some("Read") => ContextSourceKind::FileRead,
                Some("Grep") => ContextSourceKind::FileSearch,
                Some("Glob") => ContextSourceKind::FilePathList,
                Some("Bash") => ContextSourceKind::ShellOutput,
                Some("Edit") => ContextSourceKind::FileEdit,
                Some("MultiEdit") => ContextSourceKind::FileEdit,
                Some("Write") => ContextSourceKind::FileWrite,
                Some("WebFetch") => ContextSourceKind::Web,
                _ => ContextSourceKind::Unknown,
            }
        }

        _ => ContextSourceKind::Unknown,
    }
}

pub fn extract_mcp_server(tool_name: &str) -> Option<String> {
    let remainder = tool_name.strip_prefix("mcp__")?;
    let server = remainder.split("__").next()?;

    if server.is_empty() {
        return None;
    }

    Some(server.to_string())
}

pub fn match_confidence_for_kind(kind: &ContextSourceKind) -> ContextConfidence {
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
        ContextSourceKind::McpTool { .. } => ContextConfidence::Medium,
        ContextSourceKind::Unknown => ContextConfidence::Opaque,
    }
}
