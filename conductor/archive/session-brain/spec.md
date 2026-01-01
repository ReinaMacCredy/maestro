# Orchestrator Session Brain - Specification

## Overview

Add **Phase 0 (Preflight)** to the Orchestrator workflow that enables multi-session coordination on the same project. This feature auto-registers session identity with Agent Mail, detects active sessions, warns on conflicts, and prompts for stale session takeover.

## Problem Statement

Multiple Amp sessions running on the same project conflict silently:
- No auto-loading of context between sessions
- No session identity management
- No coordination protocol for concurrent sessions
- Beads can be claimed by multiple sessions
- File reservations not respected across sessions

## Target Users

- Developers running 2+ Amp terminal sessions on the same codebase
- Use cases: parallel track work, coding + research sessions, orchestrator testing

## Functional Requirements

### FR1: Session Identity Management
- **FR1.1**: Generate unique session ID using format `{BaseAgent}-{timestamp}` (e.g., `BlueLake-1735689600`)
- **FR1.2**: Display human-readable name `{BaseAgent} (session HH:MM)` (e.g., `BlueLake (session 10:30)`)
- **FR1.3**: Persist identity in Agent Mail profile via `register_agent()`
- **FR1.4**: On ID collision, retry with incremented timestamp

### FR2: Active Session Detection
- **FR2.1**: On preflight trigger, call `fetch_inbox()` for messages from last 30 minutes
- **FR2.2**: Parse messages for `[SESSION START]`, `[HEARTBEAT]`, `[SESSION END]` subjects
- **FR2.3**: Build active session list including: session ID, track, beads claimed, files reserved, last seen
- **FR2.4**: Mark sessions as "stale" if no activity for >10 minutes

### FR3: Conflict Detection
- **FR3.1**: Detect track conflicts (same track as active session)
- **FR3.2**: Detect file reservation overlaps (glob matching)
- **FR3.3**: Detect bead conflicts (same bead claimed by multiple sessions)
- **FR3.4**: Display conflicts with actionable options

### FR4: Session Lifecycle
- **FR4.1**: Send `[SESSION START]` message on preflight completion
- **FR4.2**: Send `[HEARTBEAT]` message every 5 minutes during active work
- **FR4.3**: Send `[SESSION END]` message on session completion
- **FR4.4**: Release file reservations on session end

### FR5: Stale Session Handling
- **FR5.1**: Detect stale sessions (>10 min since last activity)
- **FR5.2**: Display takeover prompt with options: [T]ake over, [W]ait, [I]gnore
- **FR5.3**: On takeover: release reservations, reset beads to `open`
- **FR5.4**: Warn about potential uncommitted work before takeover

### FR6: Preflight Triggers
- **FR6.1**: Trigger on `/conductor-implement`, `/conductor-orchestrate`
- **FR6.2**: Skip preflight for `ds` (design sessions always fresh)
- **FR6.3**: Skip preflight for query commands: `bd ready`, `bd show`, `bd list`

### FR7: Python Scripts (claudekit-skills pattern)
- **FR7.1**: Create `preflight.py` with CLI interface and JSON output
- **FR7.2**: Create `session_identity.py` for ID generation/parsing
- **FR7.3**: Create `session_cleanup.py` for stale session cleanup
- **FR7.4**: Scripts use stdlib only, under 200 lines each

## Non-Functional Requirements

### NFR1: Performance
- Preflight should complete within 3 seconds (Agent Mail timeout)
- If timeout, proceed with warning (graceful degradation)

### NFR2: Reliability
- Agent Mail is required for coordination; fallback to no-coordination mode if unavailable
- Scripts must be stateless (read input, output JSON, exit)

### NFR3: Compatibility
- Maintain backward compatibility with existing Orchestrator workflow
- Phase 0 inserts before existing Phase 1

## Acceptance Criteria

- [ ] **AC1**: Session 1 starts → registers identity, shows "no active sessions"
- [ ] **AC2**: Session 2 starts → shows Session 1 context (track, beads, files)
- [ ] **AC3**: Session 2 on same track → warns "track conflict" with options
- [ ] **AC4**: Session 2 claims same bead → shows "claimed by BlueLake-xxx"
- [ ] **AC5**: Session 1 inactive >10 min → Session 2 sees takeover prompt
- [ ] **AC6**: Takeover accepted → beads reset to `open`, reservations released
- [ ] **AC7**: `ds` command → skips preflight entirely
- [ ] **AC8**: Agent Mail slow (>3s) → warns, proceeds without coordination
- [ ] **AC9**: All scripts execute with JSON output

## Out of Scope

- Cross-machine coordination (different computers)
- Real-time file locking at OS level
- Automatic conflict resolution (user decides)
- Background heartbeat daemon
- User authentication

## Technical Constraints

- Scripts must use Python 3.9+ stdlib only
- Agent Mail MCP must be available for full functionality
- Session identity stored in Agent Mail profile (not local file)
- Graceful degradation required when Agent Mail unavailable

## Dependencies

- Agent Mail MCP (`mcp__mcp_agent_mail__*` tools)
- Existing Orchestrator skill structure
- Beads CLI (`bd` commands)
