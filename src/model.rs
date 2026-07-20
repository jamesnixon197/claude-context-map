#[derive(Debug, Clone, Copy)]
pub struct ReportOptions {
    pub all: bool,
    pub use_color: bool,
    pub terminal_width: usize,
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
