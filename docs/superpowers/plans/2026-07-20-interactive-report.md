# Interactive-ish `ccmap` Report Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the static `ccmap` report navigable and self-explanatory — clickable Linear/file rows, smarter shell summaries, a `ccmap show <n>` drill-in, kind filtering, and per-row spacing/rank.

**Architecture:** All presentation stays print-and-exit in `report.rs`. Pure helpers (link URLs, shell summarisation, filtering, ranking) are unit-tested with no ANSI/OSC in assertions; styling and hyperlinks are applied only at print time via `Painter`, gated on a resolved `use_color`/`use_links` flag computed in `main.rs`. New link config lives in `config/defaults.rs`; env/config resolution stays in `main.rs`.

**Tech Stack:** Rust (edition 2024), `owo-colors` (already added), `clap` derive, `serde`/`toml`. No new dependencies.

## Global Constraints

- No new crates. OSC 8 hyperlinks are emitted by hand.
- Hyperlinks and colour are gated identically: only when stdout is a TTY and `NO_COLOR` is unset. When gated off, every `Painter` method returns its text unchanged (no `\x1b`).
- Pure helpers contain no ANSI/OSC; tests assert on plain text only.
- Percentages/bars always use the whole-session token total, never a filtered subtotal.
- Source rank is the 1-based position in the full token-sorted `context_map`, stable under `--kind`/`--top`.
- Match existing code style: small free functions, `#[cfg(test)] mod tests`, verb-led test names, no comments except where a non-obvious invariant needs one.
- `ReportOptions` is the single carrier of resolved presentation state; `report.rs` never reads env or config directly.

---

## File Structure

- `src/config/defaults.rs` — add `LinkConfig { linear: Option<String> }` to `CcmapConfig`.
- `src/model.rs` — extend `ReportOptions` with `use_links`, `linear_base: Option<String>`, `kinds: Vec<KindFilter>`, `top: Option<usize>`, `detail: bool`; add `KindFilter` enum.
- `src/cli.rs` — add `--all/--kind/--top/--detail` to `Latest`/`Analyse`; add `Show { n }`.
- `src/main.rs` — resolve link base (env→config→none) + `use_links`; parse new flags; wire `Show`.
- `src/analyse/report.rs` — link helpers, ticket-id extraction, shell echo-skip + verb·targets, filtering, rank, spacing, `print_source_detail` for `show`.
- `src/analyse/mod.rs` — re-export the new `print_source_detail`.

---

## Task 1: `LinkConfig` in config

**Files:**
- Modify: `src/config/defaults.rs`

**Interfaces:**
- Produces: `CcmapConfig.links: LinkConfig`; `LinkConfig { linear: Option<String> }` (both `#[serde(default)]`, `Serialize + Deserialize + Clone + Debug`).

- [ ] **Step 1: Write the failing test**

Add to a `#[cfg(test)] mod tests` at the bottom of `src/config/defaults.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_parses_links_linear_base() {
        let toml = "mode = \"safe\"\n[links]\nlinear = \"https://linear.app/acme\"\n";
        let config: CcmapConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.links.linear.as_deref(), Some("https://linear.app/acme"));
    }

    #[test]
    fn config_defaults_links_to_none() {
        let config = CcmapConfig::default();
        assert_eq!(config.links.linear, None);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p claude-context-map config::defaults::tests -- --nocapture`
Expected: FAIL to compile — no field `links` on `CcmapConfig`.

- [ ] **Step 3: Write minimal implementation**

In `src/config/defaults.rs`, add the field to `CcmapConfig` and the new struct + Default:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CcmapConfig {
    pub mode: CaptureMode,
    pub warning_rules: WarningRules,
    pub links: LinkConfig,
}
```

Add to the `impl Default for CcmapConfig` body: `links: LinkConfig::default(),`.

Add the new type:

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LinkConfig {
    pub linear: Option<String>,
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p claude-context-map config::defaults::tests`
Expected: PASS (2 tests).

- [ ] **Step 5: Export the type**

In `src/config/mod.rs`, extend the re-export line:

```rust
pub use defaults::{CaptureMode, CcmapConfig, LinkConfig, WarningRules, load_config};
```

- [ ] **Step 6: Commit**

```bash
git add src/config/defaults.rs src/config/mod.rs
git commit -m "feat(config): add [links] linear base to config"
```

---

## Task 2: `KindFilter` + extended `ReportOptions`

**Files:**
- Modify: `src/model.rs`

**Interfaces:**
- Consumes: none.
- Produces:
  - `enum KindFilter { File, Shell, Mcp, Instr, Edit, Write, Sub, Web, Search, Paths, Prompt, Session }` (`Debug, Clone, Copy, PartialEq, Eq`), with `KindFilter::matches(&self, kind: &ContextSourceKind) -> bool` and `KindFilter::from_str_opt(s: &str) -> Option<KindFilter>`.
  - `ReportOptions { all: bool, use_color: bool, use_links: bool, terminal_width: usize, linear_base: Option<String>, kinds: Vec<KindFilter>, top: Option<usize>, detail: bool }` (drop `Copy`; keep `Debug, Clone`).

- [ ] **Step 1: Write the failing test**

Add a test module at the bottom of `src/model.rs`:

```rust
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
        assert!(KindFilter::Mcp.matches(&ContextSourceKind::McpTool { server: "linear".into() }));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p claude-context-map model::tests`
Expected: FAIL to compile — `KindFilter` not found.

- [ ] **Step 3: Write minimal implementation**

In `src/model.rs`, replace the existing `ReportOptions` struct with:

```rust
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
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p claude-context-map model::tests`
Expected: PASS (2 tests). NOTE: `report.rs` and `main.rs` will not compile yet (they build `ReportOptions` with the old shape and it's no longer `Copy`). That is fixed in Tasks 3–8; do not attempt a full `cargo build` until Task 8.

- [ ] **Step 5: Commit**

```bash
git add src/model.rs
git commit -m "feat(model): add KindFilter and extend ReportOptions for links/filter/detail"
```

---

## Task 3: Link helpers (ticket id, linear url, OSC 8)

**Files:**
- Modify: `src/analyse/report.rs`

**Interfaces:**
- Consumes: `ReportOptions` (Task 2).
- Produces (all in `report.rs`):
  - `fn extract_ticket_id(label: &str) -> Option<String>` — first `[A-Z]{2,}-[0-9]+` match.
  - `fn linear_ticket_url(base: &str, ticket: &str) -> String` — `{base_no_trailing_slash}/issue/{ticket}`.
  - `Painter::link(&self, text: &str, url: &str) -> String` — OSC 8 wrap when `use_links`, else `text`.
  - `Painter` gains a `use_links: bool` field set from `options.use_links`.

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `src/analyse/report.rs`:

```rust
#[test]
fn extract_ticket_id_finds_a_linear_key() {
    assert_eq!(extract_ticket_id("linear: get_issue(DEC-259)").as_deref(), Some("DEC-259"));
    assert_eq!(extract_ticket_id("linear: list_issues").as_deref(), None);
    assert_eq!(extract_ticket_id("Google_Drive: read_file(abc)").as_deref(), None);
}

#[test]
fn linear_ticket_url_joins_without_double_slash() {
    assert_eq!(linear_ticket_url("https://linear.app/acme/", "DEC-1"), "https://linear.app/acme/issue/DEC-1");
    assert_eq!(linear_ticket_url("https://linear.app/acme", "DEC-1"), "https://linear.app/acme/issue/DEC-1");
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
```

Add this test helper inside the `tests` module (used by later tasks too):

```rust
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
```

Update the two existing painter tests to build via `opts(...)` instead of the old literal:
- `painter_emits_no_ansi_when_color_disabled`: `Painter::new(&opts(false, false))`.
- `painter_emits_ansi_when_color_enabled`: `Painter::new(&opts(true, false))`.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p claude-context-map analyse::report::tests::extract_ticket_id_finds_a_linear_key`
Expected: FAIL to compile — `extract_ticket_id` / `linear_ticket_url` / `Painter::link` not found; `Painter::new` signature mismatch.

- [ ] **Step 3: Write minimal implementation**

Change `Painter` to carry links and take `&ReportOptions`:

```rust
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
    // ... existing methods unchanged ...
}
```

Add the free helpers near the other pure helpers:

```rust
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
```

Every existing `Painter::new(options)` call site in `report.rs` must become `Painter::new(&options)` — update the calls in `print_header`, `print_context_map`, `print_map_row` (receives `&Painter`, unchanged), `print_warnings`, `print_rule`. (`print_analysis` passes `options` by value to those fns; since `ReportOptions` is no longer `Copy`, change those helper signatures to take `&ReportOptions` — see Task 8 which finalises signatures. For now, to keep this task compiling in isolation, pass `&options` and borrow.)

NOTE: full-crate build still fails until Task 8 fixes `main.rs`. Run only the targeted unit tests in this task.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p claude-context-map analyse::report::tests::extract_ticket_id_finds_a_linear_key analyse::report::tests::linear_ticket_url_joins_without_double_slash analyse::report::tests::painter_link_wraps_in_osc8_when_enabled analyse::report::tests::painter_link_is_plain_when_links_disabled`
Expected: PASS (4 tests).

- [ ] **Step 5: Commit**

```bash
git add src/analyse/report.rs
git commit -m "feat(report): OSC 8 link helpers + linear ticket url + ticket-id extraction"
```

---

## Task 4: Shell summary — skip echo headers, verb · targets

**Files:**
- Modify: `src/analyse/report.rs`

**Interfaces:**
- Consumes: existing `first_meaningful_stage`, `strip_env_assignments`, `collapse_whitespace`, `summarise_command`.
- Produces:
  - `fn is_echo_banner(stage: &str) -> bool` — true for `echo "=== … ==="`/`printf` banners.
  - Updated `first_meaningful_stage` to also skip echo-banner stages (keeps last stage if all are banners/nav).
  - `fn verb_and_targets(stage: &str) -> Option<String>` — `"grep · a.ts b.ts"`; `None` when no file-ish targets.
  - Updated `summarise_command` to prefer `verb_and_targets` output, falling back to the current summary.

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module:

```rust
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
    assert!(got.contains("+1") || got.matches(".ts").count() <= 3);
}

#[test]
fn verb_and_targets_returns_none_without_file_targets() {
    assert_eq!(verb_and_targets("cargo test --quiet"), None);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p claude-context-map analyse::report::tests::verb_and_targets_extracts_program_and_files`
Expected: FAIL to compile — `is_echo_banner` / `verb_and_targets` not found.

- [ ] **Step 3: Write minimal implementation**

Add helpers:

```rust
fn is_echo_banner(stage: &str) -> bool {
    let stripped = strip_env_assignments(stage);
    let mut tokens = stripped.split_whitespace();
    let first = tokens.next().unwrap_or("");
    if first != "echo" && first != "printf" {
        return false;
    }
    stripped.contains("===") || stripped.contains("---")
}

fn looks_like_target(token: &str) -> bool {
    let t = token.trim_matches(|c| c == '"' || c == '\'');
    if t.starts_with('-') || t.contains("://") {
        return false;
    }
    t.contains('/') || t.contains('.') || t.contains('*')
}

fn basename(token: &str) -> String {
    let t = token.trim_matches(|c| c == '"' || c == '\'');
    t.rsplit('/').next().unwrap_or(t).to_string()
}

fn verb_and_targets(stage: &str) -> Option<String> {
    let stripped = strip_env_assignments(stage);
    let mut tokens = stripped.split_whitespace();
    let verb = tokens.next()?;
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
```

Update `first_meaningful_stage`'s predicate to also skip banners:

```rust
    stages
        .iter()
        .find(|stage| !is_navigation_stage(stage) && !is_echo_banner(stage))
        .or_else(|| stages.last())
        .copied()
        .unwrap_or(command)
```

Update `summarise_command` to prefer verb·targets, keeping the heredoc branch. Replace the body after computing `head` (the non-heredoc `None` arm) so it tries `verb_and_targets(stage)` first:

```rust
fn summarise_command(command: &str) -> String {
    let stage = first_meaningful_stage(command);
    let heredoc_lines = heredoc_line_count_from(command, stage);
    let stage_body = strip_env_assignments(stage);
    let head = stage_body.split(" | ").next().unwrap_or(stage_body);
    let head = collapse_whitespace(head);

    match heredoc_lines {
        Some(lines) => {
            let head = head.split("<<").next().map(str::trim_end).unwrap_or(&head);
            format!("{head} «{lines}-line heredoc»")
        }
        None => verb_and_targets(&head).unwrap_or(head),
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p claude-context-map analyse::report::tests`
Expected: PASS. All pre-existing `summarise_command_*` tests must still pass. If `summarise_command_keeps_the_first_pipeline_stage` now returns `"grep · source_label"`-style output instead of `"grep -rn source_label src"`, update that assertion to the verb·targets form: `assert_eq!(summarise_command("grep -rn source_label src | head -40"), "grep · src");` — the pipeline stage is still honoured (only the first stage is summarised), the representation is just the richer verb·targets one. Confirm by reading the failure output before editing.

- [ ] **Step 5: Commit**

```bash
git add src/analyse/report.rs
git commit -m "feat(report): skip echo banners; summarise shell as verb · targets"
```

---

## Task 5: Rank + filtering + spacing in the map

**Files:**
- Modify: `src/analyse/report.rs`

**Interfaces:**
- Consumes: `ReportOptions.kinds`, `.top`, `.all`; `KindFilter::matches`.
- Produces:
  - `fn ranked_sources(map: &[ContextSourceSummary]) -> Vec<(usize, &ContextSourceSummary)>` — pairs each source with its 1-based rank.
  - `fn filter_sources<'a>(ranked: &'a [(usize, &'a ContextSourceSummary)], kinds: &[KindFilter]) -> Vec<(usize, &'a ContextSourceSummary)>` — keep rank; empty `kinds` = keep all.
  - `print_context_map` uses rank, filter, `top`, and prints a blank line between rows (suppressed under `--all`).

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module (build small fixtures):

```rust
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
    let filtered = filter_sources(&ranked, &[KindFilter::Shell]);
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].0, 2); // rank preserved (global position)
    assert_eq!(filtered[0].1.source_label, "b");
}

#[test]
fn filter_sources_empty_filter_keeps_all() {
    let map = vec![summary(ContextSourceKind::FileRead, 100, "a")];
    let ranked = ranked_sources(&map);
    assert_eq!(filter_sources(&ranked, &[]).len(), 1);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p claude-context-map analyse::report::tests::ranked_sources_numbers_from_one_in_order`
Expected: FAIL to compile — `ranked_sources` / `filter_sources` not found.

- [ ] **Step 3: Write minimal implementation**

Add helpers:

```rust
fn ranked_sources(map: &[ContextSourceSummary]) -> Vec<(usize, &ContextSourceSummary)> {
    map.iter().enumerate().map(|(i, s)| (i + 1, s)).collect()
}

fn filter_sources<'a>(
    ranked: &[(usize, &'a ContextSourceSummary)],
    kinds: &[KindFilter],
) -> Vec<(usize, &'a ContextSourceSummary)> {
    ranked
        .iter()
        .filter(|(_, s)| kinds.is_empty() || kinds.iter().any(|k| k.matches(&s.source_kind)))
        .copied()
        .collect()
}
```

Rewrite `print_context_map` to: rank → filter → choose `visible` (`top` overrides `DEFAULT_MAP_ROWS`; `all` shows everything) → print rows with a leading rank column and a blank line between them (skip blank lines when `options.all`) → rollup over the hidden filtered remainder → rule. Full replacement:

```rust
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
        format!("{} of {} sources{}", filtered.len(), analysis.context_map.len(), filtered_note)
    } else {
        format!("top {} of {} sources{}", visible, filtered.len(), filtered_note)
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
        println!("  {}{}{}", paint.dim(&rollup), " ".repeat(gap), paint.dim(&token_cell));
    }

    print_rule(options, content_width);
}
```

Rewrite `print_map_row` to take `rank`, `&ReportOptions`, add a rank column, and link file/linear labels. Full replacement:

```rust
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
        "  {:>3}. {}  {}  {}  {}  {:>8}",
        paint.dim(&rank.to_string()),
        paint.bar(&bar, share),
        paint.dim(&percent),
        paint.kind(&tag, &source.source_kind),
        pad_display(&linked, &label, label_width),
        paint.token(&tokens),
    );
}
```

Add two small helpers (linking + padding that ignores escape sequences):

```rust
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
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p claude-context-map analyse::report::tests`
Expected: PASS (the three new tests plus all prior report tests). Crate build still incomplete until Task 8.

- [ ] **Step 5: Commit**

```bash
git add src/analyse/report.rs
git commit -m "feat(report): rank column, kind filtering, --top, row spacing, clickable rows"
```

---

## Task 6: `--detail` — full script under shell rows

**Files:**
- Modify: `src/analyse/report.rs`

**Interfaces:**
- Consumes: `ReportOptions.detail`.
- Produces: `fn print_shell_detail(command: &str, paint: &Painter)` printing the full command indented + dimmed; called from `print_map_row` when `options.detail` and the source is `ShellOutput`.

- [ ] **Step 1: Write the failing test**

`print_shell_detail` writes to stdout, so test its formatting via a pure sibling `format_shell_detail(command: &str) -> String` and have `print_shell_detail` print it. Test the pure fn:

```rust
#[test]
fn format_shell_detail_indents_every_line() {
    let out = format_shell_detail("cd /repo\ncargo test");
    for line in out.lines() {
        assert!(line.starts_with("        "), "line not indented: {line:?}");
    }
    assert!(out.contains("cargo test"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p claude-context-map analyse::report::tests::format_shell_detail_indents_every_line`
Expected: FAIL to compile — `format_shell_detail` not found.

- [ ] **Step 3: Write minimal implementation**

```rust
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
```

In `print_map_row`, after the row `println!`, add:

```rust
    if options.detail && matches!(source.source_kind, ContextSourceKind::ShellOutput) {
        print_shell_detail(&source.source_label, paint);
    }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p claude-context-map analyse::report::tests::format_shell_detail_indents_every_line`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/analyse/report.rs
git commit -m "feat(report): --detail prints full shell script under shell rows"
```

---

## Task 7: `print_source_detail` for `ccmap show <n>`

**Files:**
- Modify: `src/analyse/report.rs`, `src/analyse/mod.rs`

**Interfaces:**
- Consumes: `SessionAnalysis`, `ReportOptions`.
- Produces: `pub fn print_source_detail(analysis: &SessionAnalysis, n: usize, options: &ReportOptions) -> anyhow::Result<()>` — prints full detail of the n-th (1-based, token-rank) source; `Err` with a friendly message when out of range. Re-exported from `analyse::mod`.

- [ ] **Step 1: Write the failing test**

Detail printing goes to stdout; test the range logic via a pure sibling `fn source_at_rank(map: &[ContextSourceSummary], n: usize) -> Result<&ContextSourceSummary, String>`:

```rust
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p claude-context-map analyse::report::tests::source_at_rank_resolves_one_based_index`
Expected: FAIL to compile — `source_at_rank` not found.

- [ ] **Step 3: Write minimal implementation**

```rust
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
```

In `src/analyse/mod.rs`, extend the re-export:

```rust
pub use report::{print_analysis, print_source_detail};
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p claude-context-map analyse::report::tests::source_at_rank_resolves_one_based_index`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/analyse/report.rs src/analyse/mod.rs
git commit -m "feat(report): print_source_detail + source_at_rank for ccmap show"
```

---

## Task 8: CLI flags, `Show` command, option resolution — wire everything

**Files:**
- Modify: `src/cli.rs`, `src/main.rs`, `src/analyse/report.rs` (finalise fn signatures to `&ReportOptions`)

**Interfaces:**
- Consumes: everything above.
- Produces: working `ccmap latest/analyse [--all --detail --top N --kind K...]` and `ccmap show <n>`.

- [ ] **Step 1: Finalise report.rs signatures**

Ensure these all take `options: &ReportOptions` (by reference, since it's no longer `Copy`): `print_analysis`, `print_header`, `print_context_map`, `print_warnings`, `print_rule`. `print_analysis` signature becomes:

```rust
pub fn print_analysis(analysis: &SessionAnalysis, options: &ReportOptions) {
    let label_width = label_column_width(options.terminal_width);
    let content_width = MAP_ROW_FIXED_WIDTH + label_width;
    print_header(analysis, options, content_width);
    print_context_map(analysis, options, label_width, content_width);
    print_warnings(analysis, options, content_width);
}
```

Update `print_header`/`print_warnings`/`print_rule` to `options: &ReportOptions` and their internal `Painter::new(options)` calls (already `&`). `shorten_warning_title` reads `options.all` — pass `options.all` as today.

- [ ] **Step 2: Update CLI**

In `src/cli.rs`, replace the `Command` enum:

```rust
#[derive(Subcommand)]
pub enum Command {
    Init,
    Capture,
    Analyse {
        path: PathBuf,
        #[arg(short, long)]
        all: bool,
        #[arg(long, value_name = "KIND")]
        kind: Vec<String>,
        #[arg(long, value_name = "N")]
        top: Option<usize>,
        #[arg(long)]
        detail: bool,
    },
    Latest {
        #[arg(short, long)]
        all: bool,
        #[arg(long, value_name = "KIND")]
        kind: Vec<String>,
        #[arg(long, value_name = "N")]
        top: Option<usize>,
        #[arg(long)]
        detail: bool,
    },
    Show {
        n: usize,
    },
    History,
    Doctor,
}
```

- [ ] **Step 3: Update main.rs option resolution + dispatch**

Replace `resolve_report_options` and the `Analyse`/`Latest` arms; add `Show`. New resolver:

```rust
use model::{KindFilter, ReportOptions};

struct ReportFlags {
    all: bool,
    kind: Vec<String>,
    top: Option<usize>,
    detail: bool,
}

fn resolve_report_options(flags: ReportFlags, config: &config::CcmapConfig) -> ReportOptions {
    let stdout_is_terminal = std::io::stdout().is_terminal();
    let color_disabled = std::env::var_os("NO_COLOR").is_some();
    let enabled = stdout_is_terminal && !color_disabled;

    let linear_base = std::env::var("LINEAR_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| config.links.linear.clone());

    let kinds = flags
        .kind
        .iter()
        .filter_map(|k| KindFilter::from_str_opt(k))
        .collect();

    ReportOptions {
        all: flags.all,
        use_color: enabled,
        use_links: enabled,
        terminal_width: terminal_width().unwrap_or(FALLBACK_TERMINAL_WIDTH),
        linear_base,
        kinds,
        top: flags.top,
        detail: flags.detail,
    }
}
```

Dispatch arms:

```rust
        Command::Analyse { path, all, kind, top, detail } => {
            let project = project::find_project()?;
            let storage = storage::Storage::for_project(&project);
            let config = config::load_config(&storage)?;
            let analysis = analyse::analyse_file(&path, &config)?;
            let options = resolve_report_options(ReportFlags { all, kind, top, detail }, &config);
            analyse::print_analysis(&analysis, &options);
        }
        Command::Latest { all, kind, top, detail } => {
            let project = project::find_project()?;
            let storage = storage::Storage::for_project(&project);
            let config = config::load_config(&storage)?;
            match storage.latest_session_file()? {
                Some(path) => {
                    let analysis = analyse::analyse_file(&path, &config)?;
                    let options = resolve_report_options(ReportFlags { all, kind, top, detail }, &config);
                    analyse::print_analysis(&analysis, &options);
                }
                None => println!("No sessions captured yet."),
            }
        }
        Command::Show { n } => {
            let project = project::find_project()?;
            let storage = storage::Storage::for_project(&project);
            let config = config::load_config(&storage)?;
            match storage.latest_session_file()? {
                Some(path) => {
                    let analysis = analyse::analyse_file(&path, &config)?;
                    let options = resolve_report_options(
                        ReportFlags { all: true, kind: Vec::new(), top: None, detail: false },
                        &config,
                    );
                    analyse::print_source_detail(&analysis, n, &options)?;
                }
                None => println!("No sessions captured yet."),
            }
        }
```

- [ ] **Step 4: Build + full test suite**

Run: `cargo build -p claude-context-map && cargo test -p claude-context-map`
Expected: builds clean; all tests PASS (target ~45+).

- [ ] **Step 5: Commit**

```bash
git add src/cli.rs src/main.rs src/analyse/report.rs
git commit -m "feat(cli): --kind/--top/--detail flags, ccmap show, LINEAR_URL resolution"
```

---

## Task 9: Install + manual verification on a real session

**Files:** none (verification only).

- [ ] **Step 1: Install**

Run: `cargo install --path . --quiet`
Expected: `Installed package claude-context-map` (or `Replacing`).

- [ ] **Step 2: Default view has ranks, spacing, clickable rows**

Run: `script -q /dev/null ccmap latest`
Expected: each row starts with `1.`, `2.`, …; blank line between rows; `linear:` rows and `file` rows are OSC 8 links (in Ghostty, ⌘-clickable).

- [ ] **Step 3: Filtering**

Run: `ccmap latest --kind mcp | sed -E 's/\x1b\[[0-9;]*m//g'`
Expected: only `mcp` rows; heading says `N of M sources (filtered)`; percentages still reflect whole-session share.

Run: `ccmap latest --kind shell --kind file --top 5 | cat`
Expected: at most 5 rows, only shell/file kinds.

- [ ] **Step 4: Detail + show**

Run: `ccmap latest --detail | sed -E 's/\x1b\[[0-9;]*m//g' | head -40`
Expected: full shell scripts printed indented under shell rows.

Run: `ccmap show 1`
Expected: full detail of the top source; `ccmap show 9999` prints a friendly out-of-range error and exits non-zero.

- [ ] **Step 5: Linear link honours env + config**

Run: `LINEAR_URL=https://linear.app/finovatech script -q /dev/null ccmap latest | grep -a "issue/DEC"`
Expected: OSC 8 sequences containing `https://linear.app/finovatech/issue/DEC-259`.

- [ ] **Step 6: Piped = no escapes (regression guard)**

Run: `ccmap latest | grep -c $'\x1b'`
Expected: `0`.

- [ ] **Step 7: Commit any doc updates**

Update `docs/ai-context/01-product-spec.md` "Core terminal output" to mention `--kind/--top/--detail/--all`, `ccmap show <n>`, `[links] linear`/`LINEAR_URL`, and the clickable rows. Commit:

```bash
git add docs/ai-context/01-product-spec.md
git commit -m "docs: document interactive report flags, ccmap show, and link config"
```

---

## Self-Review Notes

- **Spec coverage:** OSC 8 links (T3,T5), link config env+config (T1,T8), shell echo-skip + verb·targets (T4), `ccmap show` (T7,T8), `--detail` (T6), `--kind`/`--top` filtering session-relative % (T5), rank (T5), row spacing (T5), rollup `--all` affordance (T5). Phase-2 TUI intentionally excluded.
- **Alignment caveat (verify in T5/T9):** OSC 8 bytes are invisible but occupy string length; `pad_display` pads by the *visible* label length so columns stay aligned. Confirm visually in Task 9 Step 2.
- **Ordering caveat:** Tasks 2–7 leave the crate un-buildable in isolation (ReportOptions shape change); only targeted `cargo test <module>::tests` runs pass until Task 8 wires `main.rs`. This is called out in each task. A subagent executing one task must run the *targeted* test command shown, not a full build.
