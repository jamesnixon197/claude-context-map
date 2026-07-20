# Product Specification

## Product name

Claude Context Map

## CLI binary

```bash
ccmap
```

## Primary user flows

### Flow 1 — Initialise a repository

```bash
ccmap init
```

Expected behaviour:

- Detect current project root.
- Create `.claude/context-map/`.
- Create `.claude/context-map/sessions/`.
- Create `.claude/context-map/reports/`.
- Write `.claude/context-map/project.json`.
- Write `.claude/context-map/config.toml`.
- Write/update `.claude/settings.local.json` with hook commands.

### Flow 2 — Capture hook events

```bash
ccmap capture
```

Expected behaviour:

- Read Claude Code hook JSON from stdin.
- Add `ccmap_captured_at`.
- Extract `session_id`.
- Append event to `.claude/context-map/sessions/<session-id>.jsonl`.

This command is normally called by Claude Code hooks, not directly by the user.

### Flow 3 — Analyse specific session

```bash
ccmap analyse fixtures/simple-session.jsonl
```

Expected behaviour:

- Load project config.
- Read JSONL events.
- Normalise raw events into internal context events.
- Build session summary.
- Apply warning rules.
- Print human-readable output.

### Flow 4 — Analyse latest session

```bash
ccmap latest
```

Expected behaviour:

- Detect project root.
- Load `.claude/context-map/config.toml`.
- Find newest session JSONL file.
- Analyse it.
- Print summary and warnings.

### Flow 5 — List project session history

```bash
ccmap history
```

Expected behaviour:

- List captured sessions for the current project.
- Show session ID, estimated context, and warning count.
- Keep output concise.

### Flow 6 — Diagnose setup

```bash
ccmap doctor
```

Expected behaviour:

- Print project root.
- Print project ID.
- Check `.claude/context-map/`.
- Check `sessions/`.
- Check `reports/`.
- Check `project.json`.
- Check `config.toml`.
- Check `.claude/settings.local.json`.

## Core terminal output

```text
Session: demo

Observed:
  Instruction files: 1
  Files read:        3
  File searches:     2
  Path lists:        1
  Bash commands:     2
  Files edited:      1
  Files written:     0
  Subagent events:   0

Estimated context:
  ~4,200 tokens

Potential context pollution:
  Medium: Large shell output
      Command "cargo test" produced approximately 5,800 tokens.
```

The `latest` and `analyse` reports render a ranked context map (token bar,
share %, kind tag, source label, tokens). Reporting flags:

- `--all` / `-a` — show every source with full, untruncated labels.
- `--top N` — show the top N sources instead of the default 15.
- `--kind K` (repeatable) — filter to source kinds (`file`, `shell`, `mcp`,
  `instr`, `edit`, `write`, `sub`, `web`, `search`, `paths`, `prompt`,
  `session`). Percentages stay relative to the whole session.
- `--detail` — print the full shell script under each shell row.

`ccmap show <n>` prints the full, untruncated record for the n-th source by
token rank in the latest session (the rank shown in each map row).

In a terminal, file rows link to the file (`file://`) and Linear MCP rows link
to the ticket. The Linear base URL resolves from the `LINEAR_URL` environment
variable, else `[links] linear` in config. Links and colour are emitted only to
a TTY (and never when `NO_COLOR` is set); piped output stays plain text.

## Configuration

Default config path:

```text
.claude/context-map/config.toml
```

Example:

```toml
mode = "safe"

[warning_rules]
large_context_token_threshold = 4000
repeated_read_threshold = 3
large_shell_output_token_threshold = 4000
lockfile_names = [
  "package-lock.json",
  "pnpm-lock.yaml",
  "yarn.lock",
  "Cargo.lock",
  "go.sum"
]
generated_path_segments = [
  "/dist/",
  "/build/",
  "/coverage/",
  "/target/",
  "/node_modules/",
  "/.next/"
]

[links]
linear = "https://linear.app/your-workspace"
```

## Warning types

### Large context source

Flags any context event whose estimated tokens exceed the configured threshold.

### Lockfile read directly

Flags high-confidence file reads of configured lockfile names.

### Generated or dependency file observed

Flags paths containing configured generated/dependency path segments.

### Repeated file read

Flags files read more than configured threshold.

### Large shell output

Flags shell output above configured token threshold.

## UX principles

- Prefer short commands.
- Prefer actionable warnings.
- Avoid judgemental wording.
- Make setup diagnosable.
- Make local/private behaviour obvious.
- Keep MVP terminal-first.
