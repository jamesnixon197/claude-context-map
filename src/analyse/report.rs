use crate::model::{
    ContextSourceKind, ContextSourceSummary, ReportOptions, SessionAnalysis, WarningSeverity,
};
use owo_colors::{OwoColorize, Style};

const DEFAULT_MAP_ROWS: usize = 15;
const BAR_CELLS: usize = 14;
const MIN_LABEL_WIDTH: usize = 24;
const TOKEN_COLUMN_WIDTH: usize = 8;
const MAX_CONTENT_WIDTH: usize = 120;

const RANK_COLUMN_WIDTH: usize = 3;

// Width of everything in a map row except the flexible label column:
//   "  " "NNN." " " bar "  " "NNN%" "  " tag(7) "  " <label> "  " tokens(8)
const MAP_ROW_FIXED_WIDTH: usize =
    2 + RANK_COLUMN_WIDTH + 2 + BAR_CELLS + 2 + 4 + 2 + 7 + 2 + 2 + TOKEN_COLUMN_WIDTH;

pub fn print_analysis(analysis: &SessionAnalysis, options: &ReportOptions) {
    let label_width = label_column_width(options.terminal_width);
    let content_width = MAP_ROW_FIXED_WIDTH + label_width;

    print_header(analysis, options, content_width);
    print_context_map(analysis, options, label_width, content_width);
    print_warnings(analysis, options, content_width);
}

fn print_header(analysis: &SessionAnalysis, options: &ReportOptions, content_width: usize) {
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
    options: &ReportOptions,
    label_width: usize,
    content_width: usize,
) {
    if analysis.context_map.is_empty() {
        return;
    }

    let paint = Painter::new(options);
    let total = total_map_tokens(&analysis.context_map).max(1);

    let ranked = ranked_sources(&analysis.context_map);
    let filtered = filter_sources(&ranked, &options.kinds);

    let default_rows = options.top.unwrap_or(DEFAULT_MAP_ROWS);
    let visible = if options.all {
        filtered.len()
    } else {
        default_rows.min(filtered.len())
    };

    let filtered_note = if options.kinds.is_empty() { "" } else { " (filtered)" };
    let heading = if options.all || visible >= filtered.len() {
        format!(
            "{} of {} sources{}",
            filtered.len(),
            analysis.context_map.len(),
            filtered_note
        )
    } else {
        format!(
            "top {} of {} sources{}",
            visible,
            filtered.len(),
            filtered_note
        )
    };
    println!(" {}  {}", paint.bold("Context map"), paint.dim(&heading));
    println!();

    for (rank, source) in filtered.iter().take(visible) {
        print_map_row(*rank, source, total, label_width, &paint, options);
        if !options.all {
            println!();
        }
    }

    if visible < filtered.len() {
        let remaining = filtered.len() - visible;
        let tail_tokens: usize = filtered[visible..].iter().map(|(_, s)| s.approx_tokens).sum();
        let rollup = format!("+ {remaining} more sources");
        let token_cell = format!("~{}  ·  --all", format_count(tail_tokens));
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

fn ranked_sources(map: &[ContextSourceSummary]) -> Vec<(usize, &ContextSourceSummary)> {
    map.iter().enumerate().map(|(i, s)| (i + 1, s)).collect()
}

fn filter_sources<'a>(
    ranked: &[(usize, &'a ContextSourceSummary)],
    kinds: &[crate::model::KindFilter],
) -> Vec<(usize, &'a ContextSourceSummary)> {
    ranked
        .iter()
        .filter(|(_, s)| kinds.is_empty() || kinds.iter().any(|k| k.matches(&s.source_kind)))
        .copied()
        .collect()
}

fn print_map_row(
    rank: usize,
    source: &ContextSourceSummary,
    total: usize,
    label_width: usize,
    paint: &Painter,
    options: &ReportOptions,
) {
    let share = source.approx_tokens as f64 / total as f64;
    let bar = token_bar(share, BAR_CELLS);
    let percent = format!("{:>3}%", (share * 100.0).round() as usize);
    let tag = format!("{:<7}", kind_tag(&source.source_kind));
    let label = fit_label(&source.source_kind, &source.source_label, label_width, options.all);
    let linked = link_for_source(paint, source, options, &label);
    let tokens = format_count(source.approx_tokens);

    println!(
        "  {}. {}  {}  {}  {}  {:>8}",
        paint.dim(&format!("{rank:>RANK_COLUMN_WIDTH$}")),
        paint.bar(&bar, share),
        paint.dim(&percent),
        paint.kind(&tag, &source.source_kind),
        pad_display(&linked, &label, label_width),
        paint.token(&tokens),
    );

    if options.detail && matches!(source.source_kind, ContextSourceKind::ShellOutput) {
        print_shell_detail(&source.source_label, paint);
    }
}

fn link_for_source(
    paint: &Painter,
    source: &ContextSourceSummary,
    options: &ReportOptions,
    label: &str,
) -> String {
    match &source.source_kind {
        ContextSourceKind::FileRead
        | ContextSourceKind::FileEdit
        | ContextSourceKind::FileWrite
        | ContextSourceKind::Instruction => {
            if source.source_label.starts_with('/') {
                paint.link(label, &format!("file://{}", source.source_label))
            } else {
                label.to_string()
            }
        }
        ContextSourceKind::McpTool { server } if server == "linear" => {
            match (&options.linear_base, extract_ticket_id(&source.source_label)) {
                (Some(base), Some(ticket)) => paint.link(label, &linear_ticket_url(base, &ticket)),
                _ => label.to_string(),
            }
        }
        _ => label.to_string(),
    }
}

// Left-pad to label_width using the VISIBLE label length, so OSC 8 escape
// bytes in `linked` don't throw off column alignment.
fn pad_display(linked: &str, visible: &str, width: usize) -> String {
    let pad = width.saturating_sub(visible.chars().count());
    format!("{linked}{}", " ".repeat(pad))
}

fn print_warnings(analysis: &SessionAnalysis, options: &ReportOptions, content_width: usize) {
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

fn print_rule(options: &ReportOptions, content_width: usize) {
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

pub(crate) fn fit_label(kind: &ContextSourceKind, raw: &str, budget: usize, all: bool) -> String {
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
    let stage_body = strip_env_assignments(stage);
    let head = stage_body.split(" | ").next().unwrap_or(stage_body);
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
        None => verb_and_targets(&head).unwrap_or(head),
    }
}

fn format_shell_detail(command: &str) -> String {
    command
        .lines()
        .map(|line| format!("        {}", line.trim_end()))
        .collect::<Vec<_>>()
        .join("\n")
}

fn print_shell_detail(command: &str, paint: &Painter) {
    println!("{}", paint.dim(&format_shell_detail(command)));
}

// True when a stage is just a section-header banner (`echo "=== … ==="`),
// which is a label rather than the work the command actually did.
fn is_echo_banner(stage: &str) -> bool {
    let stripped = strip_env_assignments(stage);
    let first = stripped.split_whitespace().next().unwrap_or("");
    if first != "echo" && first != "printf" {
        return false;
    }
    stripped.contains("===") || stripped.contains("---")
}

fn looks_like_target(token: &str) -> bool {
    // A quoted argument is almost always a search pattern/string, not a path.
    if token.starts_with('"') || token.starts_with('\'') {
        return false;
    }
    if token.starts_with('-') || token.contains("://") {
        return false;
    }
    token.contains('/') || token.contains('.') || token.contains('*')
}

fn basename(token: &str) -> String {
    let t = token.trim_matches(|c| c == '"' || c == '\'');
    t.rsplit('/').next().unwrap_or(t).to_string()
}

// Render a command as `<verb> · <targets>` — the program plus the file-ish
// arguments it touched (de-duplicated by basename, capped at three with a
// `+N` overflow). Returns None when no file targets are detectable so the
// caller can fall back to the plain summarised command.
fn verb_and_targets(stage: &str) -> Option<String> {
    let stripped = strip_env_assignments(stage);
    let mut tokens = stripped.split_whitespace();
    let verb = tokens.next()?;
    // Navigation commands aren't "verb · targets" — leave them verbatim.
    if verb == "cd" || verb == "export" {
        return None;
    }
    let mut targets: Vec<String> = Vec::new();
    for token in tokens {
        if looks_like_target(token) {
            let name = basename(token);
            if !name.is_empty() && !targets.contains(&name) {
                targets.push(name);
            }
        }
    }
    if targets.is_empty() {
        return None;
    }
    let shown = targets.len().min(3);
    let mut list = targets[..shown].join(" ");
    if targets.len() > shown {
        list.push_str(&format!(" +{}", targets.len() - shown));
    }
    Some(format!("{verb} · {list}"))
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
        .find(|stage| !is_navigation_stage(stage) && !is_echo_banner(stage))
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

pub(crate) fn format_count(value: usize) -> String {
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

// Find the first ticket key (two-plus uppercase letters, dash, digits) in a
// label, e.g. "DEC-259" in "linear: get_issue(DEC-259)".
fn extract_ticket_id(label: &str) -> Option<String> {
    let bytes = label.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let start = i;
        while i < bytes.len() && bytes[i].is_ascii_uppercase() {
            i += 1;
        }
        let letters = i - start;
        if letters >= 2 && i < bytes.len() && bytes[i] == b'-' {
            let dash = i;
            i += 1;
            let digits_start = i;
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            if i > digits_start {
                return Some(label[start..i].to_string());
            }
            i = dash + 1;
        } else if i == start {
            i += 1;
        }
    }
    None
}

fn linear_ticket_url(base: &str, ticket: &str) -> String {
    format!("{}/issue/{ticket}", base.trim_end_matches('/'))
}

fn source_at_rank(map: &[ContextSourceSummary], n: usize) -> Result<&ContextSourceSummary, String> {
    if n == 0 || n > map.len() {
        return Err(format!("source {n} is out of range (valid: 1..={})", map.len()));
    }
    Ok(&map[n - 1])
}

pub fn print_source_detail(
    analysis: &SessionAnalysis,
    n: usize,
    options: &ReportOptions,
) -> anyhow::Result<()> {
    let source = source_at_rank(&analysis.context_map, n).map_err(|e| anyhow::anyhow!(e))?;
    let paint = Painter::new(options);

    println!(
        " {}  {}  ·  {} tokens  ·  {} occurrence(s)",
        paint.bold(&format!("#{n}")),
        paint.kind(kind_tag(&source.source_kind), &source.source_kind),
        format_count(source.approx_tokens),
        source.occurrences,
    );
    println!();
    println!("{}", source.source_label);
    Ok(())
}

// ── Styling: every method is a no-op passthrough when colour is disabled ──

struct Painter {
    use_color: bool,
    use_links: bool,
}

impl Painter {
    fn new(options: &ReportOptions) -> Self {
        Self {
            use_color: options.use_color,
            use_links: options.use_links,
        }
    }

    fn link(&self, text: &str, url: &str) -> String {
        if self.use_links {
            format!("\u{1b}]8;;{url}\u{1b}\\{text}\u{1b}]8;;\u{1b}\\")
        } else {
            text.to_string()
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

    fn opts(use_color: bool, use_links: bool) -> ReportOptions {
        ReportOptions {
            all: false,
            use_color,
            use_links,
            terminal_width: 80,
            linear_base: None,
            kinds: Vec::new(),
            top: None,
            detail: false,
        }
    }

    #[test]
    fn extract_ticket_id_finds_a_linear_key() {
        assert_eq!(
            extract_ticket_id("linear: get_issue(DEC-259)").as_deref(),
            Some("DEC-259")
        );
        assert_eq!(extract_ticket_id("linear: list_issues").as_deref(), None);
        assert_eq!(
            extract_ticket_id("Google_Drive: read_file(abc)").as_deref(),
            None
        );
    }

    #[test]
    fn linear_ticket_url_joins_without_double_slash() {
        assert_eq!(
            linear_ticket_url("https://linear.app/acme/", "DEC-1"),
            "https://linear.app/acme/issue/DEC-1"
        );
        assert_eq!(
            linear_ticket_url("https://linear.app/acme", "DEC-1"),
            "https://linear.app/acme/issue/DEC-1"
        );
    }

    #[test]
    fn painter_link_is_plain_when_links_disabled() {
        let paint = Painter::new(&opts(false, false));
        assert_eq!(paint.link("DEC-1", "https://x/issue/DEC-1"), "DEC-1");
        assert!(!paint.link("DEC-1", "https://x").contains('\u{1b}'));
    }

    #[test]
    fn painter_link_wraps_in_osc8_when_enabled() {
        let paint = Painter::new(&opts(true, true));
        let out = paint.link("DEC-1", "https://x/issue/DEC-1");
        assert!(out.contains("\u{1b}]8;;https://x/issue/DEC-1\u{1b}\\"));
        assert!(out.contains("DEC-1"));
    }

    fn summary(kind: ContextSourceKind, tokens: usize, label: &str) -> ContextSourceSummary {
        ContextSourceSummary {
            source_kind: kind,
            source_label: label.to_string(),
            occurrences: 1,
            approx_tokens: tokens,
            trigger_reasons: Vec::new(),
        }
    }

    #[test]
    fn source_at_rank_resolves_one_based_index() {
        let map = vec![
            summary(ContextSourceKind::FileRead, 100, "a"),
            summary(ContextSourceKind::ShellOutput, 50, "b"),
        ];
        assert_eq!(source_at_rank(&map, 2).unwrap().source_label, "b");
        assert!(source_at_rank(&map, 0).is_err());
        assert!(source_at_rank(&map, 3).is_err());
    }

    #[test]
    fn format_shell_detail_indents_every_line() {
        let out = format_shell_detail("cd /repo\ncargo test");
        for line in out.lines() {
            assert!(line.starts_with("        "), "line not indented: {line:?}");
        }
        assert!(out.contains("cargo test"));
    }

    #[test]
    fn ranked_sources_numbers_from_one_in_order() {
        let map = vec![
            summary(ContextSourceKind::FileRead, 100, "a"),
            summary(ContextSourceKind::ShellOutput, 50, "b"),
        ];
        let ranked = ranked_sources(&map);
        assert_eq!(ranked[0].0, 1);
        assert_eq!(ranked[1].0, 2);
    }

    #[test]
    fn filter_sources_keeps_rank_and_selected_kinds() {
        let map = vec![
            summary(ContextSourceKind::FileRead, 100, "a"),
            summary(ContextSourceKind::ShellOutput, 50, "b"),
            summary(ContextSourceKind::FileRead, 10, "c"),
        ];
        let ranked = ranked_sources(&map);
        let filtered = filter_sources(&ranked, &[crate::model::KindFilter::Shell]);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].0, 2);
        assert_eq!(filtered[0].1.source_label, "b");
    }

    #[test]
    fn filter_sources_empty_filter_keeps_all() {
        let map = vec![summary(ContextSourceKind::FileRead, 100, "a")];
        let ranked = ranked_sources(&map);
        assert_eq!(filter_sources(&ranked, &[]).len(), 1);
    }

    #[test]
    fn is_echo_banner_detects_section_headers() {
        assert!(is_echo_banner("echo \"=== MSO adapter tree ===\""));
        assert!(is_echo_banner("echo '=== x ==='"));
        assert!(!is_echo_banner("echo hello"));
        assert!(!is_echo_banner("grep -rn foo src"));
    }

    #[test]
    fn summarise_command_skips_echo_banner_and_shows_real_work() {
        let command = "echo \"=== diff ===\"\ngit diff --stat scenario.ts ingress.ts";
        let summary = summarise_command(command);
        assert!(summary.starts_with("git"), "got {summary:?}");
        assert!(!summary.contains("==="));
    }

    #[test]
    fn summarise_command_keeps_echo_when_it_is_the_only_stage() {
        let command = "echo \"=== just a banner ===\"";
        assert!(summarise_command(command).contains("banner"));
    }

    #[test]
    fn verb_and_targets_extracts_program_and_files() {
        let got = verb_and_targets("grep -rn \"mortgages/v1\" engine-runtime/foo.ts bar.ts");
        assert_eq!(got.as_deref(), Some("grep · foo.ts bar.ts"));
    }

    #[test]
    fn verb_and_targets_dedupes_and_caps_targets() {
        let got = verb_and_targets("cat a.ts a.ts b.ts c.ts d.ts").unwrap();
        assert!(got.starts_with("cat · "));
        assert!(got.contains("+1"));
    }

    #[test]
    fn verb_and_targets_returns_none_without_file_targets() {
        assert_eq!(verb_and_targets("cargo test --quiet"), None);
    }

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
        let paint = Painter::new(&opts(false, false));
        let styled = paint.bold("hello");
        assert_eq!(styled, "hello");
        assert!(!styled.contains('\u{1b}'));
        assert!(!paint.kind("file", &ContextSourceKind::FileRead).contains('\u{1b}'));
        assert!(!paint.bar("███", 0.9).contains('\u{1b}'));
    }

    #[test]
    fn painter_emits_ansi_when_color_enabled() {
        let paint = Painter::new(&opts(true, false));
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
