# Session Digest + Injection Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax.

**Goal:** A `ccmap digest` command that emits a terse "where & why" summary of the previous substantive session, wired to a `SessionStart` hook so Claude sees it (via injected context) and can raise it unprompted. Silent when the previous session is clean.

**Architecture:** Reuse the existing `analyse_file` pipeline. New pure helpers in `src/analyse/digest.rs` decide signal and format the body/injection wrapper. New `Storage::previous_substantive_session` selects the session. `main.rs` gates on signal and prints the body, the injection wrapper, or nothing.

**Tech Stack:** Rust (edition 2024). No new dependencies.

## Global Constraints

- No new crates.
- The digest is injected into Claude's context — it must be TINY. Cap top consumers at 3; use basenames, not full paths; body ≤ ~6 lines.
- Silent when clean: when the previous session has no signal, `ccmap digest` prints NOTHING (empty stdout, exit 0) so the hook injects nothing.
- Signal = (≥1 warning) OR (top source share ≥ `dominant_share_threshold`, default 0.25).
- Current session identified by the `CLAUDE_CODE_SESSION_ID` env var (verified name). "Previous substantive" = most-recently-modified `.jsonl` that is NOT the current session and has ≥ `min_events` lines (default 5).
- Digest output is plain text, no ANSI (it is injected/read, never styled).
- Match existing style: small free fns, `#[cfg(test)] mod tests`, verb-led test names.

## File Structure

- `src/config/defaults.rs` — add `DigestConfig { dominant_share_threshold: f64, min_events: usize }` to `CcmapConfig`.
- `src/analyse/digest.rs` (new) — `has_signal`, `digest_body`, `wrap_for_injection` (+ tests).
- `src/analyse/mod.rs` — declare `mod digest;` and re-export the three fns.
- `src/storage.rs` — `previous_substantive_session(current_id: Option<&str>, min_events: usize)`.
- `src/cli.rs` — `Digest { session: Option<PathBuf>, for_injection: bool }`.
- `src/main.rs` — `Digest` arm.

---

## Task 1: DigestConfig

**Files:** Modify `src/config/defaults.rs`, `src/config/mod.rs`

**Interfaces:** Produces `CcmapConfig.digest: DigestConfig { dominant_share_threshold: f64, min_events: usize }`, defaults `0.25` / `5`.

- [ ] **Step 1: Write the failing test**

Add to the existing `#[cfg(test)] mod tests` in `src/config/defaults.rs`:

```rust
    #[test]
    fn config_defaults_digest_thresholds() {
        let config = CcmapConfig::default();
        assert_eq!(config.digest.dominant_share_threshold, 0.25);
        assert_eq!(config.digest.min_events, 5);
    }

    #[test]
    fn config_parses_digest_overrides() {
        let toml = "mode = \"safe\"\n[digest]\ndominant_share_threshold = 0.4\nmin_events = 10\n";
        let config: CcmapConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.digest.dominant_share_threshold, 0.4);
        assert_eq!(config.digest.min_events, 10);
    }
```

- [ ] **Step 2: Run — expect fail**

Run: `cargo test -p claude-context-map config::defaults::tests::config_defaults_digest_thresholds`
Expected: FAIL to compile — no field `digest`.

- [ ] **Step 3: Implement**

Add field to `CcmapConfig` struct: `pub digest: DigestConfig,`. Add to its `Default`: `digest: DigestConfig::default(),`. Add:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DigestConfig {
    pub dominant_share_threshold: f64,
    pub min_events: usize,
}

impl Default for DigestConfig {
    fn default() -> Self {
        Self {
            dominant_share_threshold: 0.25,
            min_events: 5,
        }
    }
}
```

Export in `src/config/mod.rs`: add `DigestConfig` to the `pub use defaults::{...}` list.

- [ ] **Step 4: Run — expect pass**

Run: `cargo test -p claude-context-map config::defaults::tests`
Expected: PASS (all config tests).

- [ ] **Step 5: Commit**

```bash
git add src/config/defaults.rs src/config/mod.rs
git commit -m "feat(config): add [digest] thresholds"
```

---

## Task 2: `previous_substantive_session` in storage

**Files:** Modify `src/storage.rs`

**Interfaces:** Produces `Storage::previous_substantive_session(&self, current_id: Option<&str>, min_events: usize) -> Result<Option<PathBuf>>`.

- [ ] **Step 1: Write the failing test**

Add a `#[cfg(test)] mod tests` at the bottom of `src/storage.rs`. Build a temp Storage over a temp dir; write session files with known mtimes and line counts. Since setting mtime is awkward, test the pure selection logic by extracting a helper `select_previous(files_lines_mtime, current_id, min_events)`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn select_previous_skips_current_and_trivial_picks_newest() {
        // (path, session_id, line_count, mtime_rank) — higher mtime_rank = newer
        let entries = vec![
            ("cur.jsonl".to_string(), "CUR".to_string(), 50usize, 3u64),
            ("big.jsonl".to_string(), "BIG".to_string(), 40usize, 2u64),
            ("tiny.jsonl".to_string(), "TINY".to_string(), 2usize, 1u64),
        ];
        let pick = select_previous(&entries, Some("CUR"), 5);
        assert_eq!(pick.as_deref(), Some("big.jsonl"));
    }

    #[test]
    fn select_previous_returns_none_when_only_current_or_trivial() {
        let entries = vec![
            ("cur.jsonl".to_string(), "CUR".to_string(), 50usize, 2u64),
            ("tiny.jsonl".to_string(), "TINY".to_string(), 1usize, 1u64),
        ];
        assert_eq!(select_previous(&entries, Some("CUR"), 5), None);
    }

    #[test]
    fn select_previous_without_current_id_still_skips_trivial() {
        let entries = vec![
            ("a.jsonl".to_string(), "A".to_string(), 9usize, 2u64),
            ("b.jsonl".to_string(), "B".to_string(), 3usize, 3u64),
        ];
        // newest (b) is trivial → falls back to a
        assert_eq!(select_previous(&entries, None, 5).as_deref(), Some("a.jsonl"));
    }
}
```

- [ ] **Step 2: Run — expect fail**

Run: `cargo test -p claude-context-map storage::tests::select_previous_skips_current_and_trivial_picks_newest`
Expected: FAIL to compile — `select_previous` not found.

- [ ] **Step 3: Implement**

Add the pure selector and the public method:

```rust
fn select_previous(
    entries: &[(String, String, usize, u64)],
    current_id: Option<&str>,
    min_events: usize,
) -> Option<String> {
    let mut candidates: Vec<&(String, String, usize, u64)> = entries
        .iter()
        .filter(|(_, id, lines, _)| {
            Some(id.as_str()) != current_id && *lines >= min_events
        })
        .collect();
    candidates.sort_by_key(|(_, _, _, mtime)| *mtime);
    candidates.last().map(|(path, _, _, _)| path.clone())
}

fn count_lines(path: &std::path::Path) -> usize {
    fs::read_to_string(path)
        .map(|c| c.lines().filter(|l| !l.trim().is_empty()).count())
        .unwrap_or(0)
}

fn session_id_of(path: &std::path::Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string()
}
```

Add the method inside `impl Storage`:

```rust
    pub fn previous_substantive_session(
        &self,
        current_id: Option<&str>,
        min_events: usize,
    ) -> Result<Option<PathBuf>> {
        let files = self.session_files()?;
        let mut entries: Vec<(String, String, usize, u64)> = Vec::new();
        for path in &files {
            let mtime = fs::metadata(path)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0);
            let path_str = path.to_string_lossy().to_string();
            entries.push((path_str, session_id_of(path), count_lines(path), mtime));
        }
        Ok(select_previous(&entries, current_id, min_events).map(PathBuf::from))
    }
```

- [ ] **Step 4: Run — expect pass**

Run: `cargo test -p claude-context-map storage::tests`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add src/storage.rs
git commit -m "feat(storage): previous_substantive_session selection"
```

---

## Task 3: digest module — signal, body, injection wrapper

**Files:** Create `src/analyse/digest.rs`; modify `src/analyse/mod.rs`

**Interfaces:**
- `pub fn has_signal(analysis: &SessionAnalysis, dominant_share_threshold: f64) -> bool`
- `pub fn digest_body(analysis: &SessionAnalysis) -> String`
- `pub fn wrap_for_injection(body: &str) -> String` (empty body → empty string)

- [ ] **Step 1: Write the failing tests**

Create `src/analyse/digest.rs` with the tests first:

```rust
use crate::model::{ContextSourceSummary, SessionAnalysis, WarningSeverity};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ContextSourceKind, ContextWarning};

    fn analysis(tokens: usize, sources: Vec<(usize, &str)>, warnings: usize) -> SessionAnalysis {
        SessionAnalysis {
            session_id: "01e1536f-aaaa".to_string(),
            instruction_files_loaded: 0,
            files_read: 0,
            file_searches: 0,
            file_path_lists: 0,
            bash_commands: 0,
            files_edited: 0,
            files_written: 0,
            subagents: 0,
            approx_context_tokens: tokens,
            context_map: sources
                .into_iter()
                .map(|(t, label)| ContextSourceSummary {
                    source_kind: ContextSourceKind::FileRead,
                    source_label: label.to_string(),
                    occurrences: 1,
                    approx_tokens: t,
                    trigger_reasons: Vec::new(),
                })
                .collect(),
            events: Vec::new(),
            warnings: (0..warnings)
                .map(|_| ContextWarning {
                    severity: WarningSeverity::Medium,
                    title: "Large context source detected: /x/y.txt".to_string(),
                    detail: String::new(),
                })
                .collect(),
        }
    }

    #[test]
    fn has_signal_true_when_a_warning_exists() {
        let a = analysis(1000, vec![(100, "a")], 1);
        assert!(has_signal(&a, 0.25));
    }

    #[test]
    fn has_signal_true_when_top_source_dominates() {
        let a = analysis(1000, vec![(400, "big"), (100, "small")], 0);
        assert!(has_signal(&a, 0.25)); // 400/500 = 0.8
    }

    #[test]
    fn has_signal_false_when_clean_and_no_dominant() {
        let a = analysis(1000, vec![(100, "a"), (100, "b"), (100, "c")], 0);
        assert!(!has_signal(&a, 0.25)); // each ~0.33 of map? -> recompute
    }

    #[test]
    fn digest_body_lists_top_consumers_as_basenames_with_share() {
        let a = analysis(
            10_000,
            vec![
                (3200, "/Users/j/repo/scratchpad/sds_text.txt"),
                (600, "/Users/j/repo/src/ingress.test.ts"),
            ],
            2,
        );
        let body = a_body(&a);
        assert!(body.contains("sds_text.txt"));
        assert!(!body.contains("/Users/"));
        assert!(body.contains('%'));
        assert!(body.contains("2"));
    }

    #[test]
    fn wrap_for_injection_empty_body_is_empty() {
        assert_eq!(wrap_for_injection(""), "");
    }

    #[test]
    fn wrap_for_injection_wraps_non_empty() {
        let out = wrap_for_injection("hello");
        assert!(out.contains("<ccmap-previous-session-digest>"));
        assert!(out.contains("hello"));
    }

    fn a_body(a: &SessionAnalysis) -> String {
        digest_body(a)
    }
}
```

NOTE on `has_signal_false_when_clean_and_no_dominant`: the map total is 300, each source is 100 = 0.33 > 0.25 → this WOULD signal. Fix the fixture so no source dominates: use `vec![(100,"a"),(100,"b"),(100,"c"),(100,"d"),(100,"e")]` (each 0.20 < 0.25). Update the test body accordingly before running.

- [ ] **Step 2: Run — expect fail**

Run: `cargo test -p claude-context-map analyse::digest`
Expected: FAIL to compile — module not declared / fns missing.

- [ ] **Step 3: Implement**

Declare the module in `src/analyse/mod.rs`: add `mod digest;` and extend the re-export: `pub use digest::{digest_body, has_signal, wrap_for_injection};`.

Implement in `src/analyse/digest.rs` (above the `#[cfg(test)]`):

```rust
pub fn has_signal(analysis: &SessionAnalysis, dominant_share_threshold: f64) -> bool {
    if !analysis.warnings.is_empty() {
        return true;
    }
    let total: usize = analysis.context_map.iter().map(|s| s.approx_tokens).sum();
    if total == 0 {
        return false;
    }
    analysis
        .context_map
        .iter()
        .map(|s| s.approx_tokens)
        .max()
        .map(|top| top as f64 / total as f64 >= dominant_share_threshold)
        .unwrap_or(false)
}

fn basename(label: &str) -> String {
    label.rsplit('/').next().unwrap_or(label).to_string()
}

fn count_severity(analysis: &SessionAnalysis) -> (usize, usize, usize) {
    let mut high = 0;
    let mut medium = 0;
    let mut low = 0;
    for w in &analysis.warnings {
        match w.severity {
            WarningSeverity::High => high += 1,
            WarningSeverity::Medium => medium += 1,
            WarningSeverity::Low => low += 1,
        }
    }
    (high, medium, low)
}

pub fn digest_body(analysis: &SessionAnalysis) -> String {
    let total: usize = analysis.context_map.iter().map(|s| s.approx_tokens).sum();
    let total = total.max(1);

    let short_id = analysis.session_id.split('-').next().unwrap_or(&analysis.session_id);

    let top: Vec<String> = analysis
        .context_map
        .iter()
        .take(3)
        .map(|s| {
            let pct = (s.approx_tokens as f64 / total as f64 * 100.0).round() as usize;
            let occ = if s.occurrences > 1 {
                format!(" (read {}×)", s.occurrences)
            } else {
                String::new()
            };
            format!("{} {}%{}", basename(&s.source_label), pct, occ)
        })
        .collect();

    let (high, medium, low) = count_severity(analysis);
    let mut warn_parts = Vec::new();
    if high > 0 {
        warn_parts.push(format!("{high} high"));
    }
    if medium > 0 {
        warn_parts.push(format!("{medium} medium"));
    }
    if low > 0 {
        warn_parts.push(format!("{low} low"));
    }
    let warn_line = if warn_parts.is_empty() {
        "Warnings: none.".to_string()
    } else {
        format!("Warnings: {}.", warn_parts.join(", "))
    };

    format!(
        "ccmap — previous session ({}): ~{} tokens across {} sources.\nTop consumers: {}.\n{}",
        short_id,
        analysis.approx_context_tokens,
        analysis.context_map.len(),
        top.join(", "),
        warn_line,
    )
}

pub fn wrap_for_injection(body: &str) -> String {
    if body.is_empty() {
        return String::new();
    }
    format!(
        "<ccmap-previous-session-digest>\nContext usage from the user's previous session in this project:\n{body}\n\nIf relevant to how this session starts, you may briefly mention where the user's context went last time and offer to work in a way that avoids it. Don't force it if the user is already focused on a task.\n</ccmap-previous-session-digest>"
    )
}
```

- [ ] **Step 4: Run — expect pass**

Run: `cargo test -p claude-context-map analyse::digest`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/analyse/digest.rs src/analyse/mod.rs
git commit -m "feat(digest): signal gate, terse body, injection wrapper"
```

---

## Task 4: `ccmap digest` command wiring

**Files:** Modify `src/cli.rs`, `src/main.rs`

**Interfaces:** `ccmap digest [--session <path>] [--for-injection]`.

- [ ] **Step 1: Add the CLI variant**

In `src/cli.rs` `Command` enum, add:

```rust
    Digest {
        #[arg(long, value_name = "PATH")]
        session: Option<PathBuf>,
        #[arg(long)]
        for_injection: bool,
    },
```

- [ ] **Step 2: Add the main.rs arm**

In `src/main.rs`, add before `Command::History`:

```rust
        Command::Digest { session, for_injection } => {
            let project = project::find_project()?;
            let storage = storage::Storage::for_project(&project);
            let config = config::load_config(&storage)?;

            let target = match session {
                Some(path) => Some(path),
                None => {
                    let current = std::env::var("CLAUDE_CODE_SESSION_ID").ok();
                    storage.previous_substantive_session(
                        current.as_deref(),
                        config.digest.min_events,
                    )?
                }
            };

            let Some(path) = target else {
                return Ok(());
            };

            let analysis = analyse::analyse_file(&path, &config)?;
            if !analyse::has_signal(&analysis, config.digest.dominant_share_threshold) {
                return Ok(());
            }

            let body = analyse::digest_body(&analysis);
            if for_injection {
                print!("{}", analyse::wrap_for_injection(&body));
            } else {
                println!("{body}");
            }
        }
```

- [ ] **Step 3: Build + full test**

Run: `cargo build -p claude-context-map && cargo test -p claude-context-map`
Expected: builds clean; all tests PASS.

- [ ] **Step 4: Commit**

```bash
git add src/cli.rs src/main.rs
git commit -m "feat(cli): ccmap digest command (previous session, --for-injection)"
```

---

## Task 5: Install, wire the hook, verify

**Files:** `.claude/settings.local.json` in the DECISIONING repo (not this crate) — user-facing wiring.

- [ ] **Step 1: Install**

Run: `cargo install --path . --quiet`
Expected: Installed/Replacing.

- [ ] **Step 2: Manual — plain digest of the biggest session**

Run: `ccmap digest --session .claude/context-map/sessions/<big-session>.jsonl` (from the decisioning repo)
Expected: 3 lines — token total, top-3 basenames with %, warning counts. No ANSI.

- [ ] **Step 3: Manual — injection wrapper**

Run: `ccmap digest --session <big-session> --for-injection`
Expected: same body wrapped in `<ccmap-previous-session-digest>…</…>` with the light-suggestion sentence.

- [ ] **Step 4: Manual — silent when clean**

Run: `ccmap digest --session <a-tiny-session>` (a 1–2 event file)
Expected: empty output, exit 0.

- [ ] **Step 5: Wire the SessionStart hook (decisioning repo)**

Add to `.claude/settings.local.json` `hooks`:

```json
"SessionStart": [
  { "matcher": "",
    "hooks": [ { "type": "command", "command": "ccmap digest --for-injection" } ] }
]
```

(Preserve any existing SessionStart entries — append, don't replace.)

- [ ] **Step 6: Update docs**

Add `ccmap digest` and the `[digest]` config + SessionStart hook to `docs/ai-context/01-product-spec.md`. Commit:

```bash
git add docs/ai-context/01-product-spec.md
git commit -m "docs: document ccmap digest and SessionStart injection"
```

---

## Self-Review Notes

- **Spec coverage:** DigestConfig (T1), previous-session selection excluding current + trivial (T2), signal gate + terse body + wrapper (T3), command + env-var current-session detection (T4), install/hook/verify (T5).
- **Verified fact:** current session env var is `CLAUDE_CODE_SESSION_ID` (checked live), not `CLAUDE_SESSION_ID`.
- **Silent-when-clean** enforced in the main arm (early return before printing) AND in `wrap_for_injection` (empty body → empty).
- **Injection budget:** body capped at 3 consumers + basenames; the whole thing is a handful of lines.
- **Test fixture caveat (T3):** the "clean, no dominant" case needs 5 equal sources (0.20 each) to stay under 0.25 — noted inline.
