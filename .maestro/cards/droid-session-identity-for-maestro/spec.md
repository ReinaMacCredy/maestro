# Droid session identity for Maestro

## Current state

Maestro session identity for CLI-originated run events currently flows through cli_run_id(), which delegates to session_token() in src/foundation/core/session.rs:12-39. That resolver reads MAESTRO_SESSION_ID, MAESTRO_RUN_ID, CODEX_THREAD_ID, CLAUDE_SESSION_ID, CLAUDECODE_SESSION_ID, and CLAUDE_CODE_SESSION_ID, then falls back to cli-<date>.

Droid does not expose a normal shell env var equivalent in this session. Factory hook documentation says each hook input includes session_id and transcript_path, so Droid attribution is available at hook boundaries via JSON stdin rather than process env.

## Problem

Maestro has explicit runtime env support for Claude and Codex session ids, but Droid's stable session id is not available to ordinary shell commands. Without a Droid-specific contract, Droid-authored Maestro events collapse into the cli-<date> fallback or depend on users guessing an env var that does not exist. The card should make Droid attribution explicit through hook JSON session_id, and update skill guidance or hook wiring so Droid users can reliably bind run events to their real session.

## Precedent

Precedent cards: cross-session-run-event-awareness locked dec-session-identity-for-run-events-must-6368 for populated runtime vars; cross-agent-links-codex-session-identity-active-link-status-and-linked-card-messaging locked dec-codex-session-identity-codex-thread-id-98f9 for Codex's CODEX_THREAD_ID; task-session-identity-reads-claude-code-5ce5 added CLAUDE_CODE_SESSION_ID.

## Intended flow

Droid hook stdin
  session_id: <droid-session>
        |
        v
maestro hook record --session <droid-session>
        |
        v
.maestro/runs/<encoded-session>/events.jsonl
