<div align="center">

# ccmap — Claude Context Map

**See where your AI coding session's context actually comes from.**

A local-first Rust CLI that observes, analyses, and visualises the context Claude Code pulls into a session — file reads, shell output, MCP calls, subagents, and more — so you can catch context pollution before it burns your budget.

[![CI](https://img.shields.io/badge/build-cargo%20test-2ea44f?style=flat-square)](#development)
[![Rust](https://img.shields.io/badge/rust-2024%20edition-orange?style=flat-square&logo=rust)](Cargo.toml)
[![Version](https://img.shields.io/badge/version-0.1.0-blue?style=flat-square)](Cargo.toml)
[![License](https://img.shields.io/badge/license-unlicensed-lightgrey?style=flat-square)](#license)
[![Local first](https://img.shields.io/badge/local--first-no%20cloud%2C%20no%20telemetry-6f42c1?style=flat-square)](#why)

</div>

---

## Why

Claude Code reads files, greps, runs shell commands, loads instructions, calls MCP tools, and delegates to subagents on your behalf — but you only ever see the final diff or answer. You don't see *how it got there*.

`ccmap` makes that path observable:

- Did it read the files you expected, or wander into `node_modules`?
- Did a build log or test run eat half the context budget?
- Did a lockfile get read directly instead of just listed?
- Is one file being re-read over and over across the session?

It does this by hooking into Claude Code's own lifecycle events, capturing them locally, and turning them into a report — nothing leaves your machine.

> **Note:** ccmap reports what Claude Code *appeared to receive* — observable context surfaces like tool calls and file reads. It is not a window into model reasoning.

## Features

| Command | What it does |
|---|---|
| `ccmap init` | Sets up local storage and wires the required Claude Code hooks into `.claude/settings.local.json` — idempotent, safe to re-run |
| `ccmap capture` | Hook entrypoint: reads one Claude Code lifecycle event from stdin and appends it to the session log |
| `ccmap analyse <path>` | Full report for a given session file — context map, token estimates, warnings |
| `ccmap latest` | Same report, auto-picked from the most recently active session |
| `ccmap show <n>` | Full detail on one context source from the latest session |
| `ccmap digest` | A short "here's where your last session's context went" summary, auto-injected at the start of your next session |
| `ccmap history` | List all captured sessions, newest first |
| `ccmap graph [path]` | Renders an interactive HTML context graph for a session |
| `ccmap doctor` | Diagnose project setup — root, config, storage |

## Quickstart

```bash
# Build
cargo build --release

# Inside the repo you want to observe:
ccmap init

# ...use Claude Code as normal — hooks capture automatically...

# See what happened
ccmap latest
```

### Sample output

```text
 Session  smoke-test-session
────────────────────────────────────────────────────────────────────────────────
 Files read 1    Edited 1    Written 0    Bash 1    Subagents 0
 Instructions 0    Searches 0    Path lists 0
 Context  ~148 tokens
────────────────────────────────────────────────────────────────────────────────
 Context map  4 of 4 sources

    1. ████████████░░   87%  shell    ls -la                                 129
    2. █░░░░░░░░░░░░░    9%  mcp      linear: list_issues                     14
    3. ░░░░░░░░░░░░░░    3%  file     …/README.md                              5
    4. ░░░░░░░░░░░░░░    0%  edit     …/main.rs                                0
────────────────────────────────────────────────────────────────────────────────
 Warnings  none
```

`analyse` and `latest` both accept:

```bash
ccmap latest --all              # show full paths, no truncation
ccmap latest --kind shell       # filter to one source kind (file, shell, mcp, edit, write, sub, web, search, paths, prompt, session)
ccmap latest --top 5            # cap the number of rows shown
ccmap latest --detail           # expand each entry (e.g. full shell command)
```

### Session-start digest

Once initialised, ccmap injects a short digest of your *previous* session into Claude's context at the start of the next one:

```text
ccmap — previous session (01e1536f): ~103,313 tokens across 121 sources.
Top consumers: sds_text.txt 31% (read 23×), ingress.test.ts 6%, spec.md 5%.
Warnings: 2 medium, 4 low.
```

It only fires when there's real signal (a warning, or one source dominating the session) — quiet sessions stay quiet. Configurable under `[digest]` in `config.toml`, and can be switched off with `enabled = false`.

## Warning rules

Every warning is config-driven — thresholds live in `config.toml`, not hardcoded in Rust.

| Rule | Severity | Trigger |
|---|---|---|
| Large context source | Medium | Any event above `large_context_token_threshold` tokens |
| Lockfile read directly | High | A `Read` (not just a path listing) of a configured lockfile |
| Generated/dependency path observed | Medium | Path contains a configured generated-path segment (`/dist/`, `/node_modules/`, …) |
| Repeated file read | Low | Same file read more than `repeated_read_threshold` times |
| Large shell output | Medium | Shell output above `large_shell_output_token_threshold` tokens |

Warnings are phrased as review signals ("may be low-signal unless relevant"), never as verdicts — ccmap doesn't judge whether Claude did the right thing, only surfaces what it's worth checking.

## Configuration

`ccmap init` writes a default `.claude/context-map/config.toml`:

```toml
mode = "safe"

[warning_rules]
large_context_token_threshold = 4000
repeated_read_threshold = 3
large_shell_output_token_threshold = 4000
large_mcp_response_token_threshold = 4000
lockfile_names = ["package-lock.json", "pnpm-lock.yaml", "yarn.lock", "Cargo.lock", "go.sum"]
generated_path_segments = ["/dist/", "/build/", "/coverage/", "/target/", "/node_modules/", "/.next/"]

[links]
# linear = "https://linear.app/your-team"

[digest]
enabled = true
dominant_share_threshold = 0.25
min_events = 5
```

## Design principles

- **Local-first.** No cloud backend, no hosted dashboard, no auth, no telemetry. Everything lives under `.claude/context-map/` in your repo.
- **Config over code.** Warning thresholds and rule data live in `config.toml`, never hardcoded.
- **CLI before UI.** The analysis pipeline is the product; a web UI is deliberately out of scope until the CLI is solid.
- **Observable, not omniscient.** ccmap reports tool calls and file access it can see — it makes no claim about what the model was "thinking."

## Development

```bash
cargo build
cargo test
```

Commits in this repo follow [Conventional Commits](docs/ai-context/07-conventional-commits.md). Architectural decisions are logged in [ADRs](docs/ai-context/05-adrs.md) — check those before changing direction.

## License

No license file has been added yet — all rights reserved by default until one is chosen.
