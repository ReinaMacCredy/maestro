# Session Brain - Learnings

## Commands

- `python skills/orchestrator/scripts/preflight.py detect '<inbox_json>'` - Detect active sessions from Agent Mail inbox
- `python skills/orchestrator/scripts/session_identity.py generate <agent>` - Generate session ID with timestamp
- `python skills/orchestrator/scripts/session_identity.py parse <session_id>` - Parse session ID into components
- `python skills/orchestrator/scripts/session_cleanup.py find-stale '<sessions_json>' --threshold 10` - Find stale sessions

## Gotchas

- Session ID format: `{BaseAgent}-{timestamp}` (internal), `{BaseAgent} (session HH:MM)` (display)
- rsplit("-", 1) handles hyphenated agent names like Blue-Lake correctly
- Preflight triggers on `/conductor-implement` and `/conductor-orchestrate`, skips for `ds` and query commands
- Agent Mail timeout is 3 seconds - proceed with warning if slow
- Stale threshold is 10 minutes since last heartbeat
- Scripts use stdlib only (no external dependencies) - claudekit-skills pattern
- Test imports need sys.path.insert for the scripts directory when running pytest

## Patterns

- **Session Brain Pattern:** Phase 0 (Preflight) runs before existing orchestrator phases for multi-session coordination
- **Advisory File Reservations:** Warn on file conflicts but don't block - user decides
- **Heartbeat Protocol:** 5-minute intervals, 10-minute stale threshold, auto-cleanup via message age
- **Hybrid Identity:** Internal format for uniqueness, display format for humans
- **Takeover Prompt:** [T]ake over / [W]ait / [I]gnore options for stale sessions
- **Conflict Types:** Track conflicts, file reservation overlaps, bead claim conflicts
