# Cross-session run-event awareness

## Current state

record path: 'maestro hook record' (and the per-agent record.sh hook) normalizes one event and appends to .maestro/runs/<session_dir>/events.jsonl. src/domain/run/record.rs:25, append.rs:18.

Per-session silo: each session_id gets its own run dir; cross-session view requires reading + merging N dirs by ts. Accepted events: SessionStart, UserPromptSubmit, PreToolUse, PermissionRequest, PostToolUse, Stop, skill_activation (embedded/hooks/events.yaml).

Already shared + concurrency-safe: appends use openat/O_APPEND + symlink hardening + newline repair, so multiple sessions writing at once is already safe (append.rs). The data is ALREADY a shared multi-writer log on disk.

Existing read paths: 'maestro query friction' already reads across ALL session logs but only emits counts (sessions/events/prompts/corrections/event_kinds), no timeline (query.rs:427). On Stop, run_evidence.yaml is written per session (tools_used, duration, interventions, commits). 'maestro watch snapshot' shows TASKS not run events. 'maestro resume'/'status' serve handoff.

Coordination already exists elsewhere: card claims ('maestro claim <id>') declare intent + ownership durably. Run events are telemetry, not a coordination channel.

Constraints: maestro stays passive (no daemon/watcher/push; pull-only). SessionStart prunes runs/ to 20 dirs (append.rs:26) -> bounds how much retrospective history survives.

Identity: agent runtime exports CLAUDE_SESSION_ID/MAESTRO_SESSION etc; claim_session()/cli_run_id() (cli/mod.rs:1253,1280) read them. Card claims already bind session->card via assignee 'agent/session'. Run events bucket by that session, OR fall back to cli-<date> when no session env is set.

Stop semantics (D4 gate): embedded/hooks/events.yaml installs SessionStart/UserPromptSubmit/PreToolUse/PermissionRequest/PostToolUse/Stop -- NO SessionEnd. In Claude Code, Stop fires at the END OF EACH TURN, not session close. So a live session waiting for human input has a RECENT Stop as its last event. Empirically the real run logs here hold 0 Stop events (44 skill_activation, 1 PostToolUse) because record is driven manually. Conclusion: 'no Stop = live' is inverted -- it would hide live-but-waiting sessions. Liveness must rest on RECENCY of last event, not on Stop. There is no recorded clean-close signal at all (would need adding SessionEnd to the contract).

SESSION-IDENTITY COLLAPSE (empirically confirmed 2026-06-13): every recorded event in .maestro/runs/ carries session_id of the form cli-<date> (cli-2026-06-12 alone holds 32 events from multiple sessions merged). Root cause: cli_run_id() (src/interfaces/cli/mod.rs:1253) reads MAESTRO_SESSION_ID/MAESTRO_RUN_ID/CODEX_SESSION_ID/CLAUDE_SESSION_ID/CLAUDECODE_SESSION_ID -- ALL empty in a real Claude Code session. The actually-populated per-session var is CLAUDE_CODE_SESSION_ID=df536043-... (verified via env), which is in NEITHER cli_run_id() nor claim_session() (mod.rs:1280) lookup lists. So CLI-path events fall back to cli-<date> and all same-day sessions share one bucket. The hook stdin path (record.rs:68) DOES read payload session_id, but hooks are currently routed to agent-notify.sh not record.sh, so that path never fires here. Net: 'one row per live session' (D5) cannot work until session identity reads the populated runtime var.

## Problem

## Verification notes

Multi-session repro gotcha: a plain terminal sets no session env, so every manual 'hook record' collapses into the single cli-<date> bucket (observed: activation -> runs/cli-2026-06-13). The e2e check must set CLAUDE_SESSION_ID/MAESTRO_SESSION per invocation (or drive real agent sessions) to exercise >1 session.

## Preconditions

RECORDING PIPELINE (advisor-flagged 2026-06-13): Even with D9 identity fixed, D4 liveness labels ([working]/[waiting]/[idle]) need the PreToolUse/PostToolUse firehose. That firehose only exists when the agent's hook events are routed to record.sh. Currently this repo routes SessionStart/PostToolUse/SessionEnd to .claude/hooks/agent-notify.sh (the separate collaborator tool), NOT record.sh, so the only events recorded are manual skill_activation calls. Restoring the firehose requires 'maestro install --agent claude' to write the record.sh hook entries into .claude/settings.local.json (see src/domain/install/hooks.rs). Until then 'maestro active' has only skill_activation + card_touch events and liveness is coarse (age-based, no [working]/[waiting] distinction). This is a runtime dependency, not a code change in this contract. Related: the doctor blind-spot (check_install verifies mirror files exist but never parses settings for hook entries) is being fixed under a separate task.
