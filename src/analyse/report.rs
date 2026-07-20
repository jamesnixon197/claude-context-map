use crate::model::{SessionAnalysis, WarningSeverity};

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

fn severity_label(severity: &WarningSeverity) -> &'static str {
    match severity {
        WarningSeverity::Low => "low",
        WarningSeverity::Medium => "medium",
        WarningSeverity::High => "high",
    }
}
