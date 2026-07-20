# Session-End Digest Becomes The Default Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `ccmap init` wire a `SessionStart` hook that runs `ccmap digest --for-injection` by default on every project, make `ccmap init` safe to re-run on an already-initialized project (merging in any hooks it's missing rather than overwriting the whole settings file), and add a `[digest] enabled` config flag so a developer can turn the digest off with one config line.

**Architecture:** A new pure function `merge_settings(existing: Option<&str>, hooks_to_ensure: &HooksSpec) -> Result<Value>` in `src/config/init.rs` takes the parsed existing `settings.local.json` (if any) and ccmap's desired hook set, and returns the merged JSON value — idempotent by exact `command` string match, leaves everything ccmap doesn't manage untouched. `write_local_settings` becomes a thin wrapper: read existing file (if present) → call the merge function → write result. Separately, `DigestConfig` gains an `enabled: bool` field (default `true`), and the `Command::Digest` arm in `main.rs` checks it before doing any analysis.

**Tech Stack:** Rust (existing crate, no new dependencies — `serde_json::Value` is already a dependency via `serde_json`).

## Global Constraints

- No new crates (spec Non-goals).
- The digest's own analysis logic (`has_signal`, `digest_body`, `wrap_for_injection`,
  `previous_substantive_session`) is unchanged — this plan only touches config/init/CLI wiring
  (spec Non-goals).
- `ccmap init` must be safe and idempotent to re-run any number of times: never duplicate a hook
  entry whose `command` string already exists under its event, never touch any hook event or
  top-level key ccmap doesn't manage (spec Design §3).
- ccmap manages exactly these hook events: `SessionStart`, `InstructionsLoaded`, `PostToolUse`,
  `PostToolBatch`, `SubagentStart`, `SubagentStop`, `Stop` (spec Design §3, matches the current
  hardcoded set in `src/config/init.rs`).
- `[digest] enabled = false` must produce identical (silent, empty-stdout, exit 0) behavior to the
  existing "previous session was clean" case, checked before resolving a session or running any
  analysis (spec Design §2).
- A malformed/unexpected existing `settings.local.json` shape (e.g. a hook event value that isn't
  an array) must not panic or corrupt the file — leave that event's existing value alone rather than
  guessing (spec Risks).
- Use Conventional Commits for every commit (project CLAUDE.md).
- Keep modules small and focused; one clear responsibility per file (project CLAUDE.md, ADR-007).

---

## File Structure

```
src/config/defaults.rs   — add `enabled: bool` to DigestConfig (Task 1)
src/main.rs              — add the enabled-check to Command::Digest's arm (Task 1)
src/config/init.rs        — add merge_settings() pure function + HooksSpec; rewrite
                            write_local_settings() to use it; add the digest hook to
                            the hardcoded hook set (Tasks 2–3)
docs/ai-context/01-product-spec.md — document the new default + config flag (Task 4)
```

No new files — `merge_settings` and its helper types live in the existing `src/config/init.rs`,
which already owns all settings-file writing logic.

---

## Task 1: `[digest] enabled` config flag + early-exit in the CLI

**Files:**
- Modify: `src/config/defaults.rs`
- Modify: `src/main.rs:150-184` (the `Command::Digest` arm)

**Interfaces:**
- Produces: `DigestConfig.enabled: bool` (default `true`), consumed by `main.rs`'s `Command::Digest`
  arm. No other task depends on this one.

**Behavior:** `DigestConfig` gains a third field, `enabled: bool`, defaulting to `true`. The
`Command::Digest` arm checks `config.digest.enabled` immediately after loading config — before
resolving a session path or running any analysis — and returns `Ok(())` with no output if `false`,
identical to the existing "no signal" and "no previous session" early-return paths already in that
function.

- [ ] **Step 1: Write the failing tests**

Add to the existing `#[cfg(test)] mod tests` block in `src/config/defaults.rs`:

```rust
    #[test]
    fn config_defaults_digest_enabled_to_true() {
        let config = CcmapConfig::default();
        assert!(config.digest.enabled);
    }

    #[test]
    fn config_parses_digest_enabled_override() {
        let toml = "mode = \"safe\"\n[digest]\nenabled = false\n";
        let config: CcmapConfig = toml::from_str(toml).unwrap();
        assert!(!config.digest.enabled);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test config::defaults::tests::config_defaults_digest_enabled_to_true`
Expected: FAIL to compile — no field `enabled` on `DigestConfig`.

- [ ] **Step 3: Add the field**

In `src/config/defaults.rs`, change:

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

to:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DigestConfig {
    pub enabled: bool,
    pub dominant_share_threshold: f64,
    pub min_events: usize,
}

impl Default for DigestConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            dominant_share_threshold: 0.25,
            min_events: 5,
        }
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test config::defaults::tests`
Expected: PASS (all config tests, including the two new ones and the pre-existing
`config_defaults_digest_thresholds` / `config_parses_digest_overrides`).

- [ ] **Step 5: Write the failing CLI test**

Rust integration tests for `main.rs`'s `match` arms don't exist in this codebase today (the arm's
logic is inline in `main`, which isn't unit-testable directly) — so instead, add the guard directly
and verify it manually in this step's Step 7, then cover the underlying config behavior with a unit
test on `DigestConfig` alone (already done in Steps 1–4). Skip to Step 6.

- [ ] **Step 6: Add the early-exit check**

In `src/main.rs`, inside the `Command::Digest { session, for_injection } => { ... }` arm, change:

```rust
        Command::Digest {
            session,
            for_injection,
        } => {
            let project = project::find_project()?;
            let storage = storage::Storage::for_project(&project);
            let config = config::load_config(&storage)?;

            let target = match session {
```

to:

```rust
        Command::Digest {
            session,
            for_injection,
        } => {
            let project = project::find_project()?;
            let storage = storage::Storage::for_project(&project);
            let config = config::load_config(&storage)?;

            if !config.digest.enabled {
                return Ok(());
            }

            let target = match session {
```

(Everything after `let target = match session {` is unchanged — the rest of the arm's body stays
exactly as it is today.)

- [ ] **Step 7: Manual verification**

Run:
```bash
cargo build
mkdir -p /tmp/ccmap-digest-test && cd /tmp/ccmap-digest-test
/path/to/ccmap init
echo 'mode = "safe"
[digest]
enabled = false' > .claude/context-map/config.toml
echo '{"hook_event_name":"SessionStart","session_id":"s1","cwd":"/tmp/ccmap-digest-test"}' | /path/to/ccmap capture
echo '{"hook_event_name":"PostToolUse","session_id":"s1","tool_name":"Read","tool_input":{"file_path":"/tmp/big.txt"},"tool_response":{"content":"'"$(head -c 20000 /dev/urandom | base64)"'"}}' | /path/to/ccmap capture
/path/to/ccmap digest --for-injection
```
Expected: no output at all (empty stdout), even though the session file has a large enough source
to normally trigger a signal — confirming `enabled = false` short-circuits before analysis runs.

(Replace `/path/to/ccmap` with the actual built binary path, e.g.
`$(pwd)/../claude-context-map/target/debug/ccmap`, or `cargo run --manifest-path
/path/to/claude-context-map/Cargo.toml --`.)

- [ ] **Step 8: Commit**

```bash
cd /path/to/claude-context-map
cargo fmt
cargo check
git add src/config/defaults.rs src/main.rs
git commit -m "feat(config): add [digest] enabled flag, default true"
```

---

## Task 2: `merge_settings` — pure JSON merge function

**Files:**
- Modify: `src/config/init.rs`

**Interfaces:**
- Consumes: nothing from other tasks.
- Produces (used by Task 3):
  ```rust
  pub(crate) fn merge_settings(
      existing: Option<&str>,
      hooks_to_ensure: &serde_json::Value,
  ) -> anyhow::Result<serde_json::Value>
  ```
  `existing` is the raw file content if `settings.local.json` already existed (`None` if it didn't).
  `hooks_to_ensure` is a JSON object shaped like `{"SessionStart": [...], "PostToolUse": [...], ...}`
  — the same shape as today's hardcoded `settings["hooks"]` value. Returns the final merged
  top-level JSON value to write (i.e. `{"hooks": {...}}`, matching today's file shape — this repo's
  `settings.local.json` currently contains only a `hooks` key, per `write_local_settings`'s current
  output).

**Behavior:**
- If `existing` is `None`: return `json!({"hooks": hooks_to_ensure.clone()})` — i.e. today's
  behavior for a brand-new file, unchanged.
- If `existing` is `Some(content)`:
  - Parse `content` as JSON. If parsing fails, treat it the same as `None` (there is no valid
    existing structure to merge against, so we fall back to producing the same output as a fresh
    file) — this keeps `init` from ever hard-failing on a corrupted settings file; it will simply
    regenerate a clean one.
  - Ensure the parsed value has an object at its top level with a `"hooks"` key that is itself an
    object; if `"hooks"` is missing or not an object, treat it as `{}` for merge purposes (build a
    fresh `hooks` object) while preserving every *other* top-level key untouched.
  - For each `(event_name, ccmap_entries)` pair in `hooks_to_ensure` (where `ccmap_entries` is a JSON
    array of hook-entry objects, e.g. `[{"matcher": "", "hooks": [{"type": "command", "command":
    "ccmap capture"}]}]`):
    - If the existing `hooks` object has no key `event_name`: insert `event_name: ccmap_entries`
      wholesale (this is the "old project, brand-new event ccmap didn't manage before" case).
    - If it exists but is not a JSON array: leave it completely alone (the malformed-shape fallback
      from the spec's Risks section) — do not attempt to merge into it, do not overwrite it.
    - If it exists and is an array: for each entry object in `ccmap_entries`, extract its inner
      command string(s) (each entry has shape `{"matcher": ..., "hooks": [{"type": "command",
      "command": "..."}, ...]}` — walk `entry["hooks"]` and collect every `"command"` string found).
      For each such command string, check whether ANY existing entry under this event already
      contains a hook with that exact `command` string anywhere in its own nested `"hooks"` array.
      If yes for all of that ccmap entry's commands, skip adding it (already present — idempotent).
      If any command from that ccmap entry is not found anywhere in the existing array, append the
      whole ccmap entry object to the existing array (simpler and more robust than trying to merge
      at the sub-entry level, and matches how the current hardcoded entries are always a single
      `{"matcher": "", "hooks": [{"type": "command", "command": "..."}]}` per event).
  - Every key in the parsed existing JSON that isn't `"hooks"`, and every key inside `"hooks"` that
    isn't one of the event names in `hooks_to_ensure`, is carried through into the result completely
    unchanged (by starting from the parsed existing value and mutating it in place, not building a
    fresh object).
  - Return the mutated existing value.

- [ ] **Step 1: Write the failing tests**

Add a `#[cfg(test)] mod tests` block at the bottom of `src/config/init.rs` (the file currently has
none):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ccmap_hooks() -> serde_json::Value {
        json!({
            "SessionStart": [
                { "matcher": "", "hooks": [ { "type": "command", "command": "ccmap capture" } ] },
                { "matcher": "", "hooks": [ { "type": "command", "command": "ccmap digest --for-injection" } ] }
            ],
            "Stop": [
                { "matcher": "", "hooks": [ { "type": "command", "command": "ccmap capture" } ] }
            ]
        })
    }

    #[test]
    fn merge_settings_with_no_existing_file_writes_ccmap_hooks_fresh() {
        let result = merge_settings(None, &ccmap_hooks()).unwrap();
        assert_eq!(result, json!({ "hooks": ccmap_hooks() }));
    }

    #[test]
    fn merge_settings_preserves_a_hook_event_ccmap_does_not_manage() {
        let existing = json!({
            "hooks": {
                "PreToolUse": [
                    { "matcher": "", "hooks": [ { "type": "command", "command": "my-custom-check" } ] }
                ]
            }
        })
        .to_string();

        let result = merge_settings(Some(&existing), &ccmap_hooks()).unwrap();

        assert_eq!(
            result["hooks"]["PreToolUse"],
            json!([{ "matcher": "", "hooks": [ { "type": "command", "command": "my-custom-check" } ] }])
        );
        assert_eq!(result["hooks"]["SessionStart"], ccmap_hooks()["SessionStart"]);
    }

    #[test]
    fn merge_settings_is_idempotent_when_ccmap_hooks_already_present() {
        let existing = json!({ "hooks": ccmap_hooks() }).to_string();

        let result = merge_settings(Some(&existing), &ccmap_hooks()).unwrap();

        assert_eq!(
            result["hooks"]["SessionStart"].as_array().unwrap().len(),
            2,
            "re-running the merge must not duplicate entries"
        );
        assert_eq!(
            result["hooks"]["Stop"].as_array().unwrap().len(),
            1,
            "re-running the merge must not duplicate entries"
        );
    }

    #[test]
    fn merge_settings_adds_only_the_missing_hook_to_an_old_project() {
        // Simulates an old project generated before the digest hook existed:
        // SessionStart has only the "ccmap capture" entry.
        let existing = json!({
            "hooks": {
                "SessionStart": [
                    { "matcher": "", "hooks": [ { "type": "command", "command": "ccmap capture" } ] }
                ],
                "Stop": [
                    { "matcher": "", "hooks": [ { "type": "command", "command": "ccmap capture" } ] }
                ]
            }
        })
        .to_string();

        let result = merge_settings(Some(&existing), &ccmap_hooks()).unwrap();

        let session_start = result["hooks"]["SessionStart"].as_array().unwrap();
        assert_eq!(session_start.len(), 2, "should add the missing digest hook");
        let commands: Vec<&str> = session_start
            .iter()
            .flat_map(|entry| entry["hooks"].as_array().unwrap())
            .map(|h| h["command"].as_str().unwrap())
            .collect();
        assert!(commands.contains(&"ccmap capture"));
        assert!(commands.contains(&"ccmap digest --for-injection"));

        assert_eq!(
            result["hooks"]["Stop"].as_array().unwrap().len(),
            1,
            "Stop already had ccmap's only entry, should not duplicate"
        );
    }

    #[test]
    fn merge_settings_leaves_a_non_array_hook_event_alone() {
        let existing = json!({
            "hooks": {
                "SessionStart": "not-an-array-somehow"
            }
        })
        .to_string();

        let result = merge_settings(Some(&existing), &ccmap_hooks()).unwrap();

        assert_eq!(result["hooks"]["SessionStart"], json!("not-an-array-somehow"));
    }

    #[test]
    fn merge_settings_falls_back_to_fresh_output_on_unparseable_existing_content() {
        let result = merge_settings(Some("{ this is not valid json"), &ccmap_hooks()).unwrap();
        assert_eq!(result, json!({ "hooks": ccmap_hooks() }));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test config::init::tests`
Expected: FAIL to compile — `merge_settings` not found.

- [ ] **Step 3: Write the implementation**

Add to `src/config/init.rs` (above the `#[cfg(test)]` block, after the existing functions):

```rust
pub(crate) fn merge_settings(
    existing: Option<&str>,
    hooks_to_ensure: &serde_json::Value,
) -> Result<serde_json::Value> {
    let mut root = match existing.and_then(|content| serde_json::from_str::<serde_json::Value>(content).ok()) {
        Some(value) if value.is_object() => value,
        _ => serde_json::json!({}),
    };

    if !root
        .get("hooks")
        .map(|hooks| hooks.is_object())
        .unwrap_or(false)
    {
        root["hooks"] = serde_json::json!({});
    }

    let hooks_map = root["hooks"].as_object_mut().expect("just ensured object");
    let ensure_map = hooks_to_ensure
        .as_object()
        .expect("hooks_to_ensure must be a JSON object");

    for (event_name, ccmap_entries) in ensure_map {
        let ccmap_entries = ccmap_entries
            .as_array()
            .expect("hooks_to_ensure entries must be arrays");

        match hooks_map.get_mut(event_name) {
            None => {
                hooks_map.insert(event_name.clone(), serde_json::Value::Array(ccmap_entries.clone()));
            }
            Some(existing_value) => {
                let Some(existing_array) = existing_value.as_array_mut() else {
                    // Malformed/unexpected shape — leave it alone rather than guessing.
                    continue;
                };

                for ccmap_entry in ccmap_entries {
                    let ccmap_commands = commands_in_entry(ccmap_entry);
                    let already_present = ccmap_commands
                        .iter()
                        .all(|cmd| existing_array.iter().any(|e| commands_in_entry(e).contains(cmd)));

                    if !already_present {
                        existing_array.push(ccmap_entry.clone());
                    }
                }
            }
        }
    }

    Ok(root)
}

fn commands_in_entry(entry: &serde_json::Value) -> Vec<String> {
    entry
        .get("hooks")
        .and_then(|h| h.as_array())
        .map(|hooks| {
            hooks
                .iter()
                .filter_map(|h| h.get("command").and_then(|c| c.as_str()))
                .map(|s| s.to_string())
                .collect()
        })
        .unwrap_or_default()
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test config::init::tests`
Expected: PASS (6 tests).

- [ ] **Step 5: Commit**

```bash
cargo fmt
cargo check
git add src/config/init.rs
git commit -m "feat(init): add idempotent settings-hooks merge helper"
```

---

## Task 3: Wire the digest hook + rewrite `write_local_settings` to merge

**Files:**
- Modify: `src/config/init.rs`

**Interfaces:**
- Consumes: `merge_settings` from Task 2 (same file).
- Produces: nothing new for later tasks — this is the last code task.

**Behavior:**
- Add a second entry to the hardcoded `SessionStart` array: `{"matcher": "", "hooks": [{"type":
  "command", "command": "ccmap digest --for-injection"}]}`, alongside the existing `ccmap capture`
  entry.
- Change `write_local_settings` to: read `storage.settings_file` if it exists (via
  `fs::read_to_string`, tolerating a read error the same way `merge_settings` tolerates unparseable
  content — treat as "no existing content"), call `merge_settings(existing_content, &hooks_value)`,
  then write the result.

- [ ] **Step 1: Write the failing test**

Add to the same `#[cfg(test)] mod tests` block in `src/config/init.rs` (added in Task 2):

```rust
    #[test]
    fn write_local_settings_includes_the_digest_hook_on_a_fresh_write() {
        let dir = std::env::temp_dir().join(format!(
            "ccmap-init-test-{}-{}",
            std::process::id(),
            "fresh"
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let storage = crate::storage::Storage {
            base_dir: dir.clone(),
            sessions_dir: dir.join("sessions"),
            reports_dir: dir.join("reports"),
            project_file: dir.join("project.json"),
            config_file: dir.join("config.toml"),
            settings_file: dir.join("settings.local.json"),
        };

        write_local_settings(&storage).unwrap();

        let content = std::fs::read_to_string(&storage.settings_file).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();

        let session_start = parsed["hooks"]["SessionStart"].as_array().unwrap();
        let commands: Vec<&str> = session_start
            .iter()
            .flat_map(|entry| entry["hooks"].as_array().unwrap())
            .map(|h| h["command"].as_str().unwrap())
            .collect();
        assert!(commands.contains(&"ccmap capture"));
        assert!(commands.contains(&"ccmap digest --for-injection"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn write_local_settings_is_safe_to_run_twice_without_duplicating_hooks() {
        let dir = std::env::temp_dir().join(format!(
            "ccmap-init-test-{}-{}",
            std::process::id(),
            "twice"
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let storage = crate::storage::Storage {
            base_dir: dir.clone(),
            sessions_dir: dir.join("sessions"),
            reports_dir: dir.join("reports"),
            project_file: dir.join("project.json"),
            config_file: dir.join("config.toml"),
            settings_file: dir.join("settings.local.json"),
        };

        write_local_settings(&storage).unwrap();
        write_local_settings(&storage).unwrap();

        let content = std::fs::read_to_string(&storage.settings_file).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        let session_start = parsed["hooks"]["SessionStart"].as_array().unwrap();
        assert_eq!(
            session_start.len(),
            2,
            "running init twice must not duplicate the SessionStart hooks"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test config::init::tests::write_local_settings`
Expected: FAIL — the current hardcoded `SessionStart` array has only one entry (`ccmap capture`),
so the digest-hook assertion fails; `write_local_settings` also doesn't yet call `merge_settings`.

- [ ] **Step 3: Write the implementation**

In `src/config/init.rs`, locate the current `write_local_settings` function (it builds a `settings`
`json!({...})` value with a hardcoded `SessionStart` array containing one entry, then unconditionally
writes it). Replace the whole function with:

```rust
fn write_local_settings(storage: &Storage) -> Result<()> {
    let hooks = json!({
        "SessionStart": [
            {
                "matcher": "",
                "hooks": [
                    {
                        "type": "command",
                        "command": "ccmap capture"
                    }
                ]
            },
            {
                "matcher": "",
                "hooks": [
                    {
                        "type": "command",
                        "command": "ccmap digest --for-injection"
                    }
                ]
            }
        ],
        "InstructionsLoaded": [
            {
                "matcher": "",
                "hooks": [
                    {
                        "type": "command",
                        "command": "ccmap capture"
                    }
                ]
            }
        ],
        "PostToolUse": [
            {
                "matcher": "",
                "hooks": [
                    {
                        "type": "command",
                        "command": "ccmap capture"
                    }
                ]
            }
        ],
        "PostToolBatch": [
            {
                "matcher": "",
                "hooks": [
                    {
                        "type": "command",
                        "command": "ccmap capture"
                    }
                ]
            }
        ],
        "SubagentStart": [
            {
                "matcher": "",
                "hooks": [
                    {
                        "type": "command",
                        "command": "ccmap capture"
                    }
                ]
            }
        ],
        "SubagentStop": [
            {
                "matcher": "",
                "hooks": [
                    {
                        "type": "command",
                        "command": "ccmap capture"
                    }
                ]
            }
        ],
        "Stop": [
            {
                "matcher": "",
                "hooks": [
                    {
                        "type": "command",
                        "command": "ccmap capture"
                    }
                ]
            }
        ]
    });

    let existing_content = fs::read_to_string(&storage.settings_file).ok();
    let merged = merge_settings(existing_content.as_deref(), &hooks)?;

    if let Some(parent) = storage.settings_file.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(&storage.settings_file, serde_json::to_string_pretty(&merged)?)?;

    Ok(())
}
```

(This is the same hook set as today plus the one new `SessionStart` entry, restructured to be built
as a local `hooks` value passed through `merge_settings` instead of being the entire file's content
directly.)

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test config::init`
Expected: PASS (all `config::init::tests`, including the two new ones and all six from Task 2).

- [ ] **Step 5: Run the full suite**

Run: `cargo test`
Expected: PASS — full suite green, no regressions in any other module.

- [ ] **Step 6: Manual end-to-end verification**

```bash
cargo build
rm -rf /tmp/ccmap-e2e-test && mkdir /tmp/ccmap-e2e-test && cd /tmp/ccmap-e2e-test
git init -q
/path/to/target/debug/ccmap init
cat .claude/settings.local.json
```
Expected: `SessionStart` array contains two entries — `ccmap capture` and `ccmap digest
--for-injection`.

```bash
# Simulate a developer's own customization, then re-run init.
python3 -c "
import json
with open('.claude/settings.local.json') as f:
    data = json.load(f)
data['hooks']['PreToolUse'] = [{'matcher': '', 'hooks': [{'type': 'command', 'command': 'my-custom-hook'}]}]
with open('.claude/settings.local.json', 'w') as f:
    json.dump(data, f, indent=2)
"
/path/to/target/debug/ccmap init
cat .claude/settings.local.json
```
Expected: `PreToolUse` with `my-custom-hook` is still present, unchanged; `SessionStart` still has
exactly two entries (not four) — confirms idempotency and preservation of unmanaged hooks together.

- [ ] **Step 7: Commit**

```bash
cargo fmt
cargo check
git add src/config/init.rs
git commit -m "feat(init): wire digest hook by default and merge settings idempotently"
```

---

## Task 4: Documentation

**Files:**
- Modify: `docs/ai-context/01-product-spec.md`

**Interfaces:** None — text only.

- [ ] **Step 1: Add the digest default + config flag to the product spec**

Read `docs/ai-context/01-product-spec.md` first to find the section documenting `ccmap init`'s
generated hooks (or the digest command, if already documented from the earlier digest feature) and
add, near that existing content:

```text
## Session-end digest (on by default)

`ccmap init` wires a `SessionStart` hook that runs `ccmap digest --for-injection` automatically,
alongside the existing `ccmap capture` hook. At the start of each new session, if the previous
substantive session had a warning or a dominant context source (>= 25% of total tokens by default),
Claude receives a terse digest of where that session's context went — and may raise it unprompted.
Silent when the previous session was clean.

Re-running `ccmap init` on an existing project is always safe: it merges in any hooks the current
ccmap version expects that are missing, without touching or duplicating anything else in
`.claude/settings.local.json`.

To disable the digest without removing the hook, set in `.claude/context-map/config.toml`:

```toml
[digest]
enabled = false
```
```

- [ ] **Step 2: Commit**

```bash
git add docs/ai-context/01-product-spec.md
git commit -m "docs: document session-end digest default and enabled flag"
```

---

## Self-Review Notes

**Spec coverage:**
- Goal 1 (wire the digest hook by default) — Task 3. ✓
- Goal 2 (safe/idempotent re-init, merge not overwrite) — Tasks 2–3, directly tested (fresh write,
  preserve-foreign-event, idempotent-no-duplicate, add-missing-hook-to-old-project, non-array
  fallback, unparseable-content fallback). ✓
- Goal 3 (`[digest] enabled` config flag) — Task 1. ✓
- Non-goal (digest's own analysis logic unchanged) — confirmed; Tasks 1–4 touch only
  `defaults.rs`, `main.rs`'s CLI arm (add one early-return, no other line changed), `init.rs`, and
  docs. `src/analyse/digest.rs` and `src/storage.rs`'s `previous_substantive_session` are untouched. ✓
- Risk: malformed existing JSON (non-array hook event) — Task 2's
  `merge_settings_leaves_a_non_array_hook_event_alone` test. ✓
- Risk: unparseable existing file content — Task 2's
  `merge_settings_falls_back_to_fresh_output_on_unparseable_existing_content` test. ✓
- Risk: exact-string command matching (accepted limitation, not a defect to fix) — reflected in
  `merge_settings`'s design (`commands_in_entry` does literal string comparison); no task attempts
  to "fix" this since the spec explicitly accepts it as a limitation of the chosen approach over a
  managed-entry marker.

**Placeholder scan:** No TBD/TODO. Task 1 Step 5 explicitly states why no separate CLI-level test is
added (no existing pattern for testing `main.rs` match arms directly in this codebase) and redirects
to the config-level test already covering the behavior plus a manual verification step — this is a
real, deliberate scoping decision, not a placeholder.

**Type consistency:** `merge_settings(existing: Option<&str>, hooks_to_ensure: &serde_json::Value) ->
Result<serde_json::Value>` defined in Task 2 is called identically in Task 3's rewritten
`write_local_settings` (`merge_settings(existing_content.as_deref(), &hooks)?`). `DigestConfig.enabled:
bool` defined in Task 1 is referenced as `config.digest.enabled` in Task 1's own `main.rs` change —
no other task touches this field. Test helper `Storage { ... }` struct-literal fields in Task 3's
tests match `src/storage.rs`'s actual `Storage` struct fields exactly (`base_dir`, `sessions_dir`,
`reports_dir`, `project_file`, `config_file`, `settings_file`).

**Verified during self-review:** confirmed `src/config/init.rs:1-7` directly against the repo — `use
crate::storage::Storage`, `use anyhow::Result`, `use serde_json::json`, and `use std::fs` are all
already imported at the top of the file, so every code snippet in Tasks 2 and 3 compiles as written
with no additional `use` statements needed.
