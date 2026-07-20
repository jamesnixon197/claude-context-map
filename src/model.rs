#[derive(Debug, Clone)]
pub struct ReportOptions {
    pub all: bool,
    pub use_color: bool,
    pub use_links: bool,
    pub terminal_width: usize,
    pub linear_base: Option<String>,
    pub kinds: Vec<KindFilter>,
    pub top: Option<usize>,
    pub detail: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KindFilter {
    File,
    Shell,
    Mcp,
    Instr,
    Edit,
    Write,
    Sub,
    Web,
    Search,
    Paths,
    Prompt,
    Session,
}

impl KindFilter {
    pub fn from_str_opt(value: &str) -> Option<Self> {
        Some(match value {
            "file" => KindFilter::File,
            "shell" => KindFilter::Shell,
            "mcp" => KindFilter::Mcp,
            "instr" => KindFilter::Instr,
            "edit" => KindFilter::Edit,
            "write" => KindFilter::Write,
            "sub" => KindFilter::Sub,
            "web" => KindFilter::Web,
            "search" => KindFilter::Search,
            "paths" => KindFilter::Paths,
            "prompt" => KindFilter::Prompt,
            "session" => KindFilter::Session,
            _ => return None,
        })
    }

    pub fn matches(&self, kind: &ContextSourceKind) -> bool {
        matches!(
            (self, kind),
            (KindFilter::File, ContextSourceKind::FileRead)
                | (KindFilter::Shell, ContextSourceKind::ShellOutput)
                | (KindFilter::Mcp, ContextSourceKind::McpTool { .. })
                | (KindFilter::Instr, ContextSourceKind::Instruction)
                | (KindFilter::Edit, ContextSourceKind::FileEdit)
                | (KindFilter::Write, ContextSourceKind::FileWrite)
                | (KindFilter::Sub, ContextSourceKind::Subagent)
                | (KindFilter::Web, ContextSourceKind::Web)
                | (KindFilter::Search, ContextSourceKind::FileSearch)
                | (KindFilter::Paths, ContextSourceKind::FilePathList)
                | (KindFilter::Prompt, ContextSourceKind::UserPrompt)
                | (KindFilter::Session, ContextSourceKind::Session)
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ContextSourceKind {
    Session,
    UserPrompt,
    Instruction,
    FileRead,
    FileSearch,
    FilePathList,
    ShellOutput,
    FileEdit,
    FileWrite,
    Subagent,
    Web,
    McpTool { server: String },
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TriggerReason {
    SessionStart,
    UserPrompt,
    DirectToolCall,
    SubagentActivity { subagent: String },
    SubagentLifecycle,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContextConfidence {
    High,
    Medium,
    Low,
    Opaque,
    None,
}

#[derive(Debug, Clone)]
pub struct ContextEvent {
    pub session_id: String,
    pub event_name: String,
    pub source_kind: ContextSourceKind,
    pub path: Option<String>,
    pub command: Option<String>,
    pub tool_name: Option<String>,
    pub approx_chars: usize,
    pub approx_tokens: usize,
    pub confidence: ContextConfidence,
    pub source_label: Option<String>,
    pub trigger_reason: TriggerReason,
}

#[derive(Debug, Clone)]
pub enum WarningSeverity {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone)]
pub struct ContextWarning {
    pub severity: WarningSeverity,
    pub title: String,
    pub detail: String,
}

#[derive(Debug, Clone)]
pub struct ContextSourceSummary {
    pub source_kind: ContextSourceKind,
    pub source_label: String,
    pub occurrences: usize,
    pub approx_tokens: usize,
    pub trigger_reasons: Vec<TriggerReason>,
}

#[derive(Debug, Clone)]
pub struct SessionAnalysis {
    pub session_id: String,
    pub instruction_files_loaded: usize,
    pub files_read: usize,
    pub file_searches: usize,
    pub file_path_lists: usize,
    pub bash_commands: usize,
    pub files_edited: usize,
    pub files_written: usize,
    pub subagents: usize,
    pub approx_context_tokens: usize,
    pub context_map: Vec<ContextSourceSummary>,
    pub events: Vec<ContextEvent>,
    pub warnings: Vec<ContextWarning>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_filter_parses_known_tokens() {
        assert_eq!(KindFilter::from_str_opt("file"), Some(KindFilter::File));
        assert_eq!(KindFilter::from_str_opt("shell"), Some(KindFilter::Shell));
        assert_eq!(KindFilter::from_str_opt("mcp"), Some(KindFilter::Mcp));
        assert_eq!(KindFilter::from_str_opt("nope"), None);
    }

    #[test]
    fn kind_filter_matches_source_kind() {
        assert!(KindFilter::File.matches(&ContextSourceKind::FileRead));
        assert!(!KindFilter::File.matches(&ContextSourceKind::ShellOutput));
        assert!(KindFilter::Mcp.matches(&ContextSourceKind::McpTool {
            server: "linear".into()
        }));
    }
}
