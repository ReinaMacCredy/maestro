# Specification: Orchestrator v2 - Gas Town Philosophy

**Track ID:** orchestrator-v2-gastown
**Version:** 1.0.0
**Status:** Ready for Implementation

## Overview

Integrate Gas Town orchestration philosophy into Maestro's orchestrator skill, enabling self-propelling workers, typed message coordination, ephemeral orchestration beads (wisps), and integrated crash recovery.

## Functional Requirements

### FR1: Message Protocol

**FR1.1** System SHALL support 11 typed message formats with YAML frontmatter:
- ASSIGN, WAKE, PING, PONG, PROGRESS, BLOCKED, COMPLETED, FAILED, STEAL, RELEASE, ESCALATE

**FR1.2** All messages SHALL include:
- `type`: Message type identifier
- `version`: Protocol version (currently 1)
- `schema_version`: Schema version for forward compatibility

**FR1.3** Message parser SHALL:
- Extract YAML frontmatter from message body
- Return `type: UNKNOWN` for messages without frontmatter
- Handle malformed YAML gracefully (log and skip)

**FR1.4** Subject line SHALL be human-readable decoration (`[TYPE] description`), not primary parser input

### FR2: Beads Enhancements

**FR2.1** Beads SHALL support `assignee` field:
- Nullable string for agent name
- `assigned_at` timestamp auto-set on assignment
- Query via `bd list --assignee=<name>`
- Self-reference via `bd list --assignee=self`

**FR2.2** Beads SHALL support stale detection:
- Query via `bd list --stale=<duration>` (e.g., `30m`, `1h`)
- Stale = `now - updated_at > duration`

**FR2.3** Beads SHALL support conditional updates:
- `bd update --expect-status=<status>` for atomic claiming
- Fail if current status doesn't match expected

**FR2.4** Beads SHALL support ephemeral beads (wisps):
- Create via `bd create --wisp`
- `ephemeral: true` field in schema
- Excluded from `bd list` by default (use `--wisps` or `--all`)
- Not committed to git during `bd sync`
- Delete via `bd burn <id>` or `bd burn --all-wisps`
- Compress via `bd squash <id> --into=<target>`

**FR2.5** Beads SHALL support blocked-by queries:
- `bd list --blocked-by=<id>` to find tasks depending on given ID

### FR3: Worker Protocol v2

**FR3.1** Workers SHALL self-propel on session start:
1. Call `macro_start_session()` first
2. Check inbox for ASSIGN message
3. If no ASSIGN: query `bd list --assignee=self --status=open`
4. Execute found tasks
5. Send COMPLETED message before exit

**FR3.2** Workers SHALL send heartbeat every 5 minutes:
- `bd update <id> --heartbeat`
- Updates `last_heartbeat` timestamp

**FR3.3** Workers SHALL respond to PING within timeout:
- Reply with PONG containing status, current_task, progress

**FR3.4** Workers SHALL report blocking immediately:
- Send BLOCKED message with blocker type, ref, reason
- Wait for WAKE signal or work on other tasks

### FR4: Orchestrator Enhancements

**FR4.1** Orchestrator SHALL assign work in Beads before dispatch:
- `bd update <id> --assignee=<worker>` for each task
- Send typed ASSIGN message via Agent Mail

**FR4.2** Orchestrator SHALL run witness patrol during monitor loop:
- Check for stale tasks (in_progress > 30min)
- PING stale workers, reassign if no PONG
- Check for unblocked tasks when dependencies complete
- Rebalance load if imbalance > 2 tasks

**FR4.3** Orchestrator SHALL use wisps for patrol operations:
- Create wisp at patrol start
- Log results to wisp notes
- Burn wisp at patrol end

**FR4.4** Orchestrator SHALL support work stealing:
- Send STEAL message to idle workers
- Update Beads assignee accordingly

### FR5: Recovery Command

**FR5.1** `/conductor-patrol` command SHALL:
- Scan Agent Mail for active sessions (last 30min)
- Find stale beads (in_progress without recent heartbeat)
- Find orphaned file reservations
- Rebuild implement_state.json from beads + mail
- Offer takeover/cleanup options

## Non-Functional Requirements

### NFR1: Performance
- Worker starts executing within 30s of session start
- Patrol loop runs every 60s (backoff to 10min when idle)
- Message parsing < 10ms

### NFR2: Reliability
- Crash recovery within 35min (30min stale + 5min PING timeout)
- Zero data loss on worker crash (Beads + Mail durability)

### NFR3: Compatibility
- Backward compatible with existing orchestrator
- Protocol versioning for future evolution
- Feature flags for gradual rollout

### NFR4: Maintainability
- Formal message catalog with all types documented
- Single source of truth for protocol constants
- Centralized parser module

## Technical Constraints

- No persistent daemon (Amp sessions are ephemeral)
- Agent Mail required for parallel execution (HALT without it)
- Beads CLI required (`bd` must be available)

## Dependencies

| Dependency | Type | Notes |
|------------|------|-------|
| Agent Mail MCP | Required | Coordination layer |
| Beads CLI | Required | State layer (needs enhancement) |
| Orchestrator skill | Modified | Add v2 protocol |
| Conductor skill | Modified | Add `/conductor-patrol` |

## Out of Scope

- Gas Town hooks (Amp has no persistent sessions)
- Deacon daemon (no background process)
- Multi-rig coordination (Mayor role)
- tmux session management
