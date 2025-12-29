# Specification: Orchestrator Skill

## Overview

Skill điều phối multi-agent parallel execution với autonomous workers, tích hợp vào maestro workflow. Workers tự claim/close beads qua Agent Mail coordination.

## Functional Requirements

### FR-1: Skill Structure

- **FR-1.1**: Create `skills/orchestrator/SKILL.md` with frontmatter (name, version, description)
- **FR-1.2**: Create `skills/orchestrator/references/` directory với workflow docs
- **FR-1.3**: Prerequisites: maestro-core, conductor, agent_mail MCP

### FR-2: Workflow Phases

- **FR-2.1**: Phase 1 - Read plan.md từ `conductor/tracks/<id>/`, extract Track Assignments
- **FR-2.2**: Phase 2 - Initialize Agent Mail (ensure_project, register_agent)
- **FR-2.3**: Phase 3 - Spawn workers via Task() tool với worker prompt template
- **FR-2.4**: Phase 4 - Monitor progress via fetch_inbox, search_messages
- **FR-2.5**: Phase 5 - Handle cross-track blockers via reply_message
- **FR-2.6**: Phase 6 - Verify completion, send summary, close epic

### FR-3: Worker Autonomy

- **FR-3.1**: Workers can register_agent()
- **FR-3.2**: Workers can bd update/close (self claim/close beads)
- **FR-3.3**: Workers can file_reservation_paths() (reserve files)
- **FR-3.4**: Workers can send_message() (report to orchestrator, save context)
- **FR-3.5**: Workers read track thread for context between beads

### FR-4: plan.md Extended Format

- **FR-4.1**: Add "Orchestration Config" section (epic_id, max_workers, mode)
- **FR-4.2**: Add "Track Assignments" table (Track, Agent, Beads, File Scope, Depends On)
- **FR-4.3**: Add "Cross-Track Dependencies" list
- **FR-4.4**: Maintain backward compatibility với existing plan.md format

### FR-5: maestro-core Integration

- **FR-5.1**: Add orchestrator to Skill Hierarchy (Level 3)
- **FR-5.2**: Add `/conductor-orchestrate` to Command Routing table
- **FR-5.3**: Add trigger disambiguation for "run parallel", "spawn workers"

### FR-6: Agent Mail Integration

- **FR-6.1**: Epic thread for progress reports, bead completions, blockers
- **FR-6.2**: Track threads for bead context, learnings (per worker)
- **FR-6.3**: Heartbeat messages every 5 min during work
- **FR-6.4**: Graceful fallback if Agent Mail unavailable

### FR-7: Move from coordination/

- **FR-7.1**: Move patterns/* to orchestrator/references/patterns/
- **FR-7.2**: Move examples/* to orchestrator/references/examples/
- **FR-7.3**: Keep execution-routing.md and subagent-prompt.md in conductor (shared)

## Non-Functional Requirements

### NFR-1: Performance
- Worker spawn: < 5 seconds per worker
- Monitor interval: 30 seconds between checks
- Heartbeat: Every 5 minutes

### NFR-2: Reliability
- Stale worker detection: 10 minutes without heartbeat
- Cross-track dep timeout: 30 minutes
- Graceful fallback to sequential if Agent Mail unavailable

### NFR-3: Scalability
- Default max workers: 3
- Configurable via --max-workers flag

## Acceptance Criteria

| # | Criterion | Verification |
|---|-----------|--------------|
| AC-1 | `/conductor-orchestrate` spawns parallel workers | Run command, verify Task() calls |
| AC-2 | Workers self claim/close beads | Check bd list after worker complete |
| AC-3 | Agent Mail messages for progress | Check epic thread messages |
| AC-4 | Cross-track deps handled | Worker 2 waits for Worker 1's bead |
| AC-5 | Graceful fallback | Disable MCP, verify sequential mode |
| AC-6 | plan.md Track Assignments works | Create plan with tracks, run orchestrate |
| AC-7 | maestro-core routing updated | Run /conductor-orchestrate, verify routing |

## Out of Scope

- Auto-generate track assignments (manual for v1)
- Real-time dashboard
- Cross-repository orchestration
- Worker skill (embed in prompt instead)

## Dependencies

- agent_mail MCP (required)
- beads CLI - bd, bv (required)
- conductor skill (required)
- maestro-core skill (required)

## Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| Worker crash mid-bead | Medium | Heartbeat detection, force release, re-spawn |
| File reservation conflict | Low | Orchestrator mediates via messages |
| Agent Mail unavailable | Medium | Graceful fallback to sequential |
| Cross-track dep timeout | Low | Configurable timeout, manual intervention option |
