use crate::model::{
    ContextSourceKind, ContextSourceSummary, ReportOptions, SessionAnalysis, WarningSeverity,
};
use owo_colors::{OwoColorize, Style};

const DEFAULT_MAP_ROWS: usize = 15;
const BAR_CELLS: usize = 14;
const MIN_LABEL_WIDTH: usize = 24;
const TOKEN_COLUMN_WIDTH: usize = 8;
const MAX_CONTENT_WIDTH: usize = 120;

// Width of everything in a map row except the flexible label column:
//   "  " bar "  " "NNN%" "  " tag(7) "  " <label> "  " tokens(8)
const MAP_ROW_FIXED_WIDTH: usize = 2 + BAR_CELLS + 2 + 4 + 2 + 7 + 2 + 2 + TOKEN_COLUMN_WIDTH;

pub fn print_analysis(analysis: &SessionAnalysis, options: ReportOptions) {
    let label_width = label_column_width(options.terminal_width);
    let content_width = MAP_ROW_FIXED_WIDTH + label_width;

    print_header(analysis, options, content_width);
    print_context_map(analysis, options, label_width, content_width);
    print_warnings(analysis, options, content_width);
}

fn print_header(analysis: &SessionAnalysis, options: ReportOptions, content_width: usize) {
    let paint = Painter::new(options);

    println!(
        " {}  {}",
        paint.dim("Session"),
        paint.bold(&analysis.session_id)
    );
    print_rule(options, content_width);
    println!(
        " Files read {}    Edited {}    Written {}    Bash {}    Subagents {}",
        analysis.files_read,
        analysis.files_edited,
        analysis.files_written,
        analysis.bash_commands,
        analysis.subagents,
    );
    println!(
        " Instructions {}    Searches {}    Path lists {}",
        analysis.instruction_files_loaded, analysis.file_searches, analysis.file_path_lists,
    );
    println!(
        " {}  {}",
        paint.dim("Context"),
        paint.bold(&format!("~{} tokens", format_count(analysis.approx_context_tokens)))
    );
    print_rule(options, content_width);
}

fn print_context_map(
    analysis: &SessionAnalysis,
    options: ReportOptions,
    label_width: usize,
    content_width: usize,
) {
    if analysis.context_map.is_empty() {
        return;
    }

    let paint = Painter::new(options);
    let total = total_map_tokens(&analysis.context_map).max(1);

    let visible = if options.all {
        analysis.context_map.len()
    } else {
        DEFAULT_MAP_ROWS.min(analysis.context_map.len())
    };

    let heading = if options.all || visible >= analysis.context_map.len() {
        format!("{} sources", analysis.context_map.len())
    } else {
        format!("top {} of {} sources", visible, analysis.context_map.len())
    };
    println!(" {}  {}", paint.bold("Context map"), paint.dim(&heading));
    println!();

    for source in analysis.context_map.iter().take(visible) {
        print_map_row(source, total, label_width, &paint, options.all);
    }

    if visible < analysis.context_map.len() {
        let remaining = analysis.context_map.len() - visible;
        let tail_tokens: usize = analysis.context_map[visible..]
            .iter()
            .map(|source| source.approx_tokens)
            .sum();
        let rollup = format!("+ {remaining} more sources");
        // Right-align the tail total under the row token column.
        let token_cell = format!("~{}", format_count(tail_tokens));
        let gap = content_width
            .saturating_sub(2 + rollup.chars().count() + token_cell.chars().count());
        println!(
            "  {}{}{}",
            paint.dim(&rollup),
            " ".repeat(gap),
            paint.dim(&token_cell),
        );
    }

    print_rule(options, content_width);
}

fn print_map_row(
    source: &ContextSourceSummary,
    total: usize,
    label_width: usize,
    paint: &Painter,
    all: bool,
) {
    let share = source.approx_tokens as f64 / total as f64;
    let bar = token_bar(share, BAR_CELLS);
    let percent = format!("{:>3}%", (share * 100.0).round() as usize);
    let tag = format!("{:<7}", kind_tag(&source.source_kind));
    let label = fit_label(&source.source_kind, &source.source_label, label_width, all);
    let tokens = format_count(source.approx_tokens);

    println!(
        "  {}  {}  {}  {:<label_width$}  {:>8}",
        paint.bar(&bar, share),
        paint.dim(&percent),
        paint.kind(&tag, &source.source_kind),
        label,
        paint.token(&tokens),
    );
}

fn print_warnings(analysis: &SessionAnalysis, options: ReportOptions, content_width: usize) {
    let paint = Painter::new(options);

    if analysis.warnings.is_empty() {
        println!(" {}  none", paint.dim("Warnings"));
        return;
    }

    let counts = warning_counts(&analysis.warnings);
    println!(" {}  {}", paint.bold("Warnings"), paint.dim(&counts));
    println!();

    let title_budget = content_width.saturating_sub(2 + 2 + 7 + 2).max(MIN_LABEL_WIDTH);
    for warning in &analysis.warnings {
        let dot = paint.severity_dot(&warning.severity);
        let severity = format!("{:<7}", severity_label(&warning.severity));
        let title = shorten_warning_title(&warning.title, title_budget, options.all);
        println!("  {} {}  {}", dot, paint.dim(&severity), title);
    }
}

fn print_rule(options: ReportOptions, content_width: usize) {
    let paint = Painter::new(options);
    println!("{}", paint.dim(&"─".repeat(content_width)));
}

// Warning titles embed an absolute path after a "…: " prefix. Shorten the path
// portion so warnings stay one line, while keeping the descriptive prefix.
fn shorten_warning_title(title: &str, budget: usize, all: bool) -> String {
    if all || title.chars().count() <= budget {
        return title.to_string();
    }

    match title.split_once(": ") {
        Some((prefix, path)) if path.contains('/') => {
            let prefix_len = prefix.chars().count() + 2;
            let path_budget = budget.saturating_sub(prefix_len).max(MIN_LABEL_WIDTH);
            format!("{prefix}: {}", shorten_path(path, path_budget))
        }
        _ => truncate_chars(title, budget),
    }
}

fn total_map_tokens(context_map: &[ContextSourceSummary]) -> usize {
    context_map.iter().map(|source| source.approx_tokens).sum()
}

fn warning_counts(warnings: &[crate::model::ContextWarning]) -> String {
    let mut high = 0;
    let mut medium = 0;
    let mut low = 0;
    for warning in warnings {
        match warning.severity {
            WarningSeverity::High => high += 1,
            WarningSeverity::Medium => medium += 1,
            WarningSeverity::Low => low += 1,
        }
    }
    let mut parts = Vec::new();
    if high > 0 {
        parts.push(format!("{high} high"));
    }
    if medium > 0 {
        parts.push(format!("{medium} medium"));
    }
    if low > 0 {
        parts.push(format!("{low} low"));
    }
    parts.join(" · ")
}

fn label_column_width(terminal_width: usize) -> usize {
    let capped = terminal_width.min(MAX_CONTENT_WIDTH);
    capped
        .saturating_sub(MAP_ROW_FIXED_WIDTH)
        .max(MIN_LABEL_WIDTH)
}

// ── Pure text helpers (no ANSI; styling is applied separately at print time) ──

fn kind_tag(kind: &ContextSourceKind) -> &'static str {
    match kind {
        ContextSourceKind::Session => "session",
        ContextSourceKind::UserPrompt => "prompt",
        ContextSourceKind::Instruction => "instr",
        ContextSourceKind::FileRead => "file",
        ContextSourceKind::FileSearch => "search",
        ContextSourceKind::FilePathList => "paths",
        ContextSourceKind::ShellOutput => "shell",
        ContextSourceKind::FileEdit => "edit",
        ContextSourceKind::FileWrite => "write",
        ContextSourceKind::Subagent => "sub",
        ContextSourceKind::Web => "web",
        ContextSourceKind::McpTool { .. } => "mcp",
        ContextSourceKind::Unknown => "?",
    }
}

fn fit_label(kind: &ContextSourceKind, raw: &str, budget: usize, all: bool) -> String {
    let normalised = match kind {
        ContextSourceKind::FileRead
        | ContextSourceKind::FileEdit
        | ContextSourceKind::FileWrite
        | ContextSourceKind::Instruction => {
            if all {
                raw.to_string()
            } else {
                return shorten_path(raw, budget);
            }
        }
        ContextSourceKind::ShellOutput => summarise_command(raw),
        _ => collapse_whitespace(raw),
    };

    if all {
        return collapse_whitespace(&normalised);
    }

    truncate_chars(&collapse_whitespace(&normalised), budget)
}

fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate_chars(text: &str, budget: usize) -> String {
    if text.chars().count() <= budget {
        return text.to_string();
    }
    if budget == 0 {
        return String::new();
    }
    let kept: String = text.chars().take(budget - 1).collect();
    format!("{kept}…")
}

// Collapse a leading home/absolute prefix to "…/" and keep the meaningful tail,
// dropping whole leading path segments (never cutting mid-segment) until the
// result fits the budget. Falls back to a plain char-truncate only if a single
// trailing segment is itself longer than the budget.
fn shorten_path(path: &str, budget: usize) -> String {
    let collapsed = collapse_whitespace(path);
    if collapsed.chars().count() <= budget {
        return collapsed;
    }

    let segments: Vec<&str> = collapsed.split('/').filter(|s| !s.is_empty()).collect();
    if segments.is_empty() {
        return truncate_chars(&collapsed, budget);
    }

    for start in 0..segments.len() {
        let tail = segments[start..].join("/");
        let candidate = format!("…/{tail}");
        if candidate.chars().count() <= budget {
            return candidate;
        }
    }

    // Even the last segment alone overflows: keep its readable end.
    let last = segments.last().copied().unwrap_or("");
    truncate_chars(&format!("…/{last}"), budget)
}

// Summarise a shell command down to what actually ran: skip leading navigation
// noise (cd / env-var assignments / export), then take the first meaningful
// program and its leading arguments, with any heredoc rendered as
// «N-line heredoc» rather than dumped inline.
fn summarise_command(command: &str) -> String {
    let stage = first_meaningful_stage(command);
    let heredoc_lines = heredoc_line_count_from(command, stage);
    let stage = strip_env_assignments(stage);
    let head = stage.split(" | ").next().unwrap_or(stage);
    let head = collapse_whitespace(head);

    match heredoc_lines {
        Some(lines) => {
            let head = head
                .split("<<")
                .next()
                .map(str::trim_end)
                .unwrap_or(&head);
            format!("{head} «{lines}-line heredoc»")
        }
        None => head,
    }
}

// Split the command into stages on newlines, `&&`, and `;`, then return the
// first stage that isn't pure navigation (a bare `cd` or `export`). This skips
// the common `cd /repo` prologue on its own line and surfaces the command that
// actually ran. Falls back to the last stage if every stage is navigation.
fn first_meaningful_stage(command: &str) -> &str {
    let stages: Vec<&str> = command
        .lines()
        .flat_map(|line| line.split(" && "))
        .flat_map(|part| part.split("; "))
        .map(str::trim)
        .filter(|stage| !stage.is_empty())
        .collect();

    stages
        .iter()
        .find(|stage| !is_navigation_stage(stage))
        .or_else(|| stages.last())
        .copied()
        .unwrap_or(command)
}

fn is_navigation_stage(stage: &str) -> bool {
    // Pure navigation only when the WHOLE stage is a cd/export — a leading
    // `VAR=value` prefix on a real command is stripped separately, not skipped.
    let stripped = strip_env_assignments(stage);
    let first_token = stripped.split_whitespace().next().unwrap_or("");
    first_token == "cd" || first_token == "export" || first_token.is_empty()
}

// Drop leading `NAME=value` environment-assignment prefixes, returning the
// command that actually runs (`SCRATCH=/tmp grep foo` → `grep foo`).
fn strip_env_assignments(stage: &str) -> &str {
    let mut rest = stage.trim_start();
    loop {
        let token = rest.split_whitespace().next().unwrap_or("");
        let is_assignment = token
            .split_once('=')
            .is_some_and(|(name, _)| !name.is_empty() && is_env_name(name));
        if !is_assignment {
            return rest;
        }
        rest = rest[token.len()..].trim_start();
    }
}

fn is_env_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !name.chars().next().unwrap().is_ascii_digit()
}

// Count the heredoc body lines that follow the chosen stage. Only reports a
// heredoc when the stage that actually ran opens one (`… << 'EOF'`), so a
// plain multi-line script isn't mislabelled as a heredoc.
fn heredoc_line_count_from(command: &str, stage: &str) -> Option<usize> {
    if !stage.contains("<<") {
        return None;
    }
    let lines: Vec<&str> = command.lines().collect();
    let stage_index = lines
        .iter()
        .position(|line| line.contains(stage.trim()))
        .unwrap_or(0);
    let body_lines = lines.len().saturating_sub(stage_index + 1);
    Some(body_lines.max(1))
}

fn token_bar(share: f64, cells: usize) -> String {
    let clamped = share.clamp(0.0, 1.0);
    let filled = (clamped * cells as f64).round() as usize;
    let filled = filled.min(cells);
    let empty = cells - filled;
    format!("{}{}", "█".repeat(filled), "░".repeat(empty))
}

fn format_count(value: usize) -> String {
    let digits = value.to_string();
    let bytes = digits.as_bytes();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    let first = bytes.len() % 3;
    for (index, byte) in bytes.iter().enumerate() {
        if index != 0 && index % 3 == first {
            out.push(',');
        }
        out.push(*byte as char);
    }
    out
}

fn severity_label(severity: &WarningSeverity) -> &'static str {
    match severity {
        WarningSeverity::Low => "low",
        WarningSeverity::Medium => "medium",
        WarningSeverity::High => "high",
    }
}

// ── Styling: every method is a no-op passthrough when colour is disabled ──

struct Painter {
    use_color: bool,
}

impl Painter {
    fn new(options: ReportOptions) -> Self {
        Self {
            use_color: options.use_color,
        }
    }

    fn apply(&self, text: &str, style: Style) -> String {
        if self.use_color {
            text.style(style).to_string()
        } else {
            text.to_string()
        }
    }

    fn dim(&self, text: &str) -> String {
        self.apply(text, Style::new().dimmed())
    }

    fn bold(&self, text: &str) -> String {
        self.apply(text, Style::new().bold())
    }

    fn kind(&self, text: &str, kind: &ContextSourceKind) -> String {
        let style = match kind {
            ContextSourceKind::FileRead => Style::new().blue(),
            ContextSourceKind::ShellOutput => Style::new().magenta(),
            ContextSourceKind::McpTool { .. } => Style::new().cyan(),
            ContextSourceKind::Subagent => Style::new().yellow(),
            ContextSourceKind::Instruction => Style::new().green(),
            ContextSourceKind::Web => Style::new().cyan(),
            ContextSourceKind::FileEdit | ContextSourceKind::FileWrite => Style::new().dimmed(),
            _ => Style::new().dimmed(),
        };
        self.apply(text, style)
    }

    fn bar(&self, text: &str, share: f64) -> String {
        let style = if share >= 0.20 {
            Style::new().red()
        } else if share >= 0.08 {
            Style::new().yellow()
        } else {
            Style::new().dimmed()
        };
        self.apply(text, style)
    }

    fn token(&self, text: &str) -> String {
        self.apply(text, Style::new().bold())
    }

    fn severity_dot(&self, severity: &WarningSeverity) -> String {
        let style = match severity {
            WarningSeverity::High => Style::new().red(),
            WarningSeverity::Medium => Style::new().yellow(),
            WarningSeverity::Low => Style::new().dimmed(),
        };
        self.apply("●", style)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_tags_are_short_and_stable() {
        assert_eq!(kind_tag(&ContextSourceKind::FileRead), "file");
        assert_eq!(kind_tag(&ContextSourceKind::ShellOutput), "shell");
        assert_eq!(
            kind_tag(&ContextSourceKind::McpTool {
                server: "linear".to_string()
            }),
            "mcp"
        );
    }

    #[test]
    fn format_count_inserts_thousands_separators() {
        assert_eq!(format_count(0), "0");
        assert_eq!(format_count(42), "42");
        assert_eq!(format_count(1_000), "1,000");
        assert_eq!(format_count(29_670), "29,670");
        assert_eq!(format_count(1_234_567), "1,234,567");
    }

    #[test]
    fn token_bar_fills_proportionally_at_fixed_width() {
        assert_eq!(token_bar(0.0, 10), "░░░░░░░░░░");
        assert_eq!(token_bar(1.0, 10), "██████████");
        assert_eq!(token_bar(0.5, 10), "█████░░░░░");
        assert_eq!(token_bar(2.0, 10).chars().count(), 10);
        assert_eq!(token_bar(-1.0, 10), "░░░░░░░░░░");
    }

    #[test]
    fn shorten_path_collapses_leading_segments_on_boundaries() {
        let path = "/Users/james/repos/decisioning/engine-runtime/adapters/mso/src/mso/routes.test.ts";
        let short = shorten_path(path, 40);
        assert!(short.chars().count() <= 40, "got {short:?}");
        assert!(short.starts_with("…/"));
        assert!(short.ends_with("routes.test.ts"));
        assert!(!short.contains("Users"));
    }

    #[test]
    fn shorten_path_leaves_a_short_path_untouched() {
        assert_eq!(shorten_path("src/main.rs", 40), "src/main.rs");
    }

    #[test]
    fn shorten_path_is_utf8_safe_for_an_overlong_final_segment() {
        let path = format!("/a/{}", "é".repeat(100));
        let short = shorten_path(&path, 20);
        assert!(short.chars().count() <= 20);
    }

    #[test]
    fn summarise_command_renders_a_heredoc_compactly() {
        let command = "python3 << 'EOF'\nimport json\nprint(json)\nEOF";
        let summary = summarise_command(command);
        assert!(summary.starts_with("python3"));
        assert!(summary.contains("heredoc"));
        assert!(!summary.contains('\n'));
    }

    #[test]
    fn summarise_command_keeps_the_first_pipeline_stage() {
        let command = "grep -rn source_label src | head -40";
        assert_eq!(summarise_command(command), "grep -rn source_label src");
    }

    #[test]
    fn summarise_command_skips_a_leading_cd_stage() {
        let command = "cd /Users/james/repos/decisioning && cargo test --quiet";
        assert_eq!(summarise_command(command), "cargo test --quiet");
    }

    #[test]
    fn summarise_command_skips_a_cd_on_its_own_line() {
        // The real-world shape: cd on line 1, the actual command on line 2.
        let command = "cd /Users/james/repos/personal/claude-context-map\ncargo test --quiet | grep result";
        assert_eq!(summarise_command(command), "cargo test --quiet");
    }

    #[test]
    fn summarise_command_skips_cd_then_summarises_a_heredoc() {
        let command = "cd /repo\npython3 << 'EOF'\nimport json\nprint(1)\nEOF";
        let summary = summarise_command(command);
        assert!(summary.starts_with("python3"), "got {summary:?}");
        assert!(summary.contains("heredoc"));
    }

    #[test]
    fn summarise_command_skips_a_leading_env_assignment() {
        let command = "SCRATCH=/tmp/x grep -rn foo src";
        assert_eq!(summarise_command(command), "grep -rn foo src");
    }

    #[test]
    fn summarise_command_keeps_last_stage_when_all_navigation() {
        let command = "cd /a && cd /b";
        assert_eq!(summarise_command(command), "cd /b");
    }

    #[test]
    fn summarise_command_handles_semicolon_separated_stages() {
        let command = "cd /repo; pnpm test";
        assert_eq!(summarise_command(command), "pnpm test");
    }

    #[test]
    fn summarise_command_collapses_a_multiline_command_to_one_line() {
        let command = "cargo   test\n  --quiet";
        assert!(!summarise_command(command).contains('\n'));
    }

    #[test]
    fn fit_label_all_mode_returns_full_untruncated_text() {
        let path = "/Users/james/repos/decisioning/very/long/path/to/a/file.ts";
        let fitted = fit_label(&ContextSourceKind::FileRead, path, 20, true);
        assert_eq!(fitted, path);
    }

    #[test]
    fn fit_label_default_mode_shortens_a_path_within_budget() {
        let path = "/Users/james/repos/decisioning/very/long/path/to/a/file.ts";
        let fitted = fit_label(&ContextSourceKind::FileRead, path, 25, false);
        assert!(fitted.chars().count() <= 25);
        assert!(fitted.ends_with("file.ts"));
    }

    #[test]
    fn fit_label_passes_mcp_labels_through_when_short() {
        let label = "linear: get_issue(DEC-259)";
        let fitted = fit_label(
            &ContextSourceKind::McpTool {
                server: "linear".to_string(),
            },
            label,
            40,
            false,
        );
        assert_eq!(fitted, label);
    }

    #[test]
    fn painter_emits_no_ansi_when_color_disabled() {
        let paint = Painter::new(ReportOptions {
            all: false,
            use_color: false,
            terminal_width: 80,
        });
        let styled = paint.bold("hello");
        assert_eq!(styled, "hello");
        assert!(!styled.contains('\u{1b}'));
        assert!(!paint.kind("file", &ContextSourceKind::FileRead).contains('\u{1b}'));
        assert!(!paint.bar("███", 0.9).contains('\u{1b}'));
    }

    #[test]
    fn painter_emits_ansi_when_color_enabled() {
        let paint = Painter::new(ReportOptions {
            all: false,
            use_color: true,
            terminal_width: 80,
        });
        assert!(paint.bold("hello").contains('\u{1b}'));
    }

    #[test]
    fn label_column_width_respects_the_minimum() {
        assert_eq!(label_column_width(0), MIN_LABEL_WIDTH);
        assert!(label_column_width(200) > MIN_LABEL_WIDTH);
    }

    #[test]
    fn label_column_width_caps_at_max_content_width() {
        let widest = label_column_width(10_000);
        assert_eq!(widest, MAX_CONTENT_WIDTH - MAP_ROW_FIXED_WIDTH);
    }

    #[test]
    fn shorten_warning_title_shortens_the_embedded_path() {
        let title = "Large context source detected: /Users/james/repos/decisioning/engine-runtime/adapters/mso/src/mso/ingress.test.ts";
        let short = shorten_warning_title(title, 70, false);
        assert!(short.chars().count() <= 70, "got {} chars: {short:?}", short.chars().count());
        assert!(short.starts_with("Large context source detected: "));
        assert!(short.ends_with("ingress.test.ts"));
    }

    #[test]
    fn shorten_warning_title_all_mode_is_untouched() {
        let title = "Large context source detected: /very/long/path/to/file.ts";
        assert_eq!(shorten_warning_title(title, 20, true), title);
    }
}
