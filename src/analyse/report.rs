use crate::model::{ContextSourceKind, SessionAnalysis, TriggerReason, WarningSeverity};

pub fn print_analysis(analysis: &SessionAnalysis) {
    println!("Session: {}", analysis.session_id);
    println!(
        "  Instructions loaded: {}",
        analysis.instruction_files_loaded
    );
    println!("  Files read:          {}", analysis.files_read);
    println!("  File searches:       {}", analysis.file_searches);
    println!("  File path lists:     {}", analysis.file_path_lists);
    println!("  Bash commands:       {}", analysis.bash_commands);
    println!("  Files edited:        {}", analysis.files_edited);
    println!("  Files written:       {}", analysis.files_written);
    println!("  Subagents:           {}", analysis.subagents);
    println!(
        "  Approx context tokens: {}",
        analysis.approx_context_tokens
    );

    print_context_map(analysis);

    if analysis.warnings.is_empty() {
        println!("  Warnings: none");
        return;
    }

    println!("  Warnings:");

    for warning in &analysis.warnings {
        println!(
            "    [{}] {}",
            severity_label(&warning.severity),
            warning.title
        );
        println!("      {}", warning.detail);
    }
}

fn print_context_map(analysis: &SessionAnalysis) {
    if analysis.context_map.is_empty() {
        return;
    }

    println!("  Context map:");

    for source in &analysis.context_map {
        println!(
            "    {} [{}] — {} occurrence(s), ~{} tokens ({})",
            source.source_label,
            source_kind_label(&source.source_kind),
            source.occurrences,
            source.approx_tokens,
            source
                .trigger_reasons
                .iter()
                .map(trigger_reason_label)
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
}

fn source_kind_label(kind: &ContextSourceKind) -> &'static str {
    match kind {
        ContextSourceKind::Session => "session",
        ContextSourceKind::UserPrompt => "user prompt",
        ContextSourceKind::Instruction => "instruction",
        ContextSourceKind::FileRead => "file read",
        ContextSourceKind::FileSearch => "file search",
        ContextSourceKind::FilePathList => "file path list",
        ContextSourceKind::ShellOutput => "shell output",
        ContextSourceKind::FileEdit => "file edit",
        ContextSourceKind::FileWrite => "file write",
        ContextSourceKind::Subagent => "subagent",
        ContextSourceKind::Web => "web",
        ContextSourceKind::McpTool { .. } => "mcp tool",
        ContextSourceKind::Unknown => "unknown",
    }
}

fn trigger_reason_label(reason: &TriggerReason) -> String {
    match reason {
        TriggerReason::SessionStart => "session start".to_string(),
        TriggerReason::UserPrompt => "user prompt".to_string(),
        TriggerReason::DirectToolCall => "direct tool call".to_string(),
        TriggerReason::SubagentActivity { subagent } => format!("subagent: {subagent}"),
        TriggerReason::SubagentLifecycle => "subagent lifecycle".to_string(),
        TriggerReason::Unknown => "unknown".to_string(),
    }
}

fn severity_label(severity: &WarningSeverity) -> &'static str {
    match severity {
        WarningSeverity::Low => "low",
        WarningSeverity::Medium => "medium",
        WarningSeverity::High => "high",
    }
}
