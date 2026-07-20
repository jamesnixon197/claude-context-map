# Claude Context Map — AI Working Context

## Purpose

This pack gives Codex / Claude Code enough context to work on `claude-context-map` without needing a long chat transcript.

## Project summary

Claude Context Map is a local-first Rust CLI for observing how Claude Code gathers and consumes repository context during AI-assisted development sessions.

It captures Claude Code hook events, stores them as session-level JSONL logs, normalises raw hook events into a stable internal model, and reports which files, instructions, searches, commands, and subagents contributed observable context.

The goal is not to inspect Claude's hidden reasoning. The goal is to visualise and analyse the external context surfaces made available to Claude Code.

## Core concept

The project is about **AI context observability**.

Traditional observability answers:

- What happened inside this system?
- Which requests, logs, traces, and metrics explain behaviour?

Claude Context Map applies that idea to AI coding sessions:

- Which files did Claude Code read?
- Which instructions were loaded?
- Which searches returned context?
- Which shell outputs may have dominated context?
- Which generated files or lockfiles created possible context pollution?
- Which sessions were noisy, clean, or opaque?

## MVP flow

```text
ccmap init
  Creates repo-local storage and Claude Code hook config.

Claude Code session happens normally
  Claude reads files, runs commands, searches, edits, etc.

ccmap capture
  Called automatically by Claude Code hooks.
  Reads hook JSON from stdin.
  Appends event to .claude/context-map/sessions/<session-id>.jsonl.

ccmap latest
  Finds the newest session log.
  Loads project config.
  Analyses the session.
  Prints summary and warning signals.

ccmap history
  Lists previous sessions for the current project.

ccmap doctor
  Checks config, storage, and hook setup.

ccmap graph
  Resolves a session (latest, or a specific file path).
  Writes a self-contained HTML file with a bubble diagram of the
  session's context sources (sized by token weight, linked in
  first-touch order) and a trend chart of token usage and warning
  counts across the project's session history.
  Prints the output file path; does not open a browser.
```

## Implementation philosophy

Build in small verified slices:

1. CLI skeleton
2. Project/root detection
3. Storage layout
4. Init command
5. Capture command
6. JSONL session parsing
7. Normalisation
8. Analysis summary
9. Config-driven warnings
10. Latest/history/doctor commands
11. Mermaid/HTML rendering later

Do not jump to a web UI before the CLI analysis pipeline works.

## Constraints

- Local-first.
- Safe mode by default.
- Do not upload code or transcripts anywhere.
- Avoid storing raw source contents in the eventual public-quality default mode.
- Avoid one giant `analyse/mod.rs`; keep modules focused.
- Use Conventional Commits.
- Prefer clear, small commits over large ambiguous ones.

## Recommended wording

Use:

> Claude Context Map visualises observable context supplied to Claude Code through files, instructions, searches, tool responses, shell output, and subagent activity.

Avoid:

> This shows what Claude was thinking.

Better:

> This shows what Claude Code appeared to receive through observable context surfaces.
