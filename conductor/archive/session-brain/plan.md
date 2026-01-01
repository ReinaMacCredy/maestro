# Orchestrator Session Brain - Implementation Plan

## Overview

Add Phase 0 (Preflight) to Orchestrator for multi-session coordination.

**Estimated Duration**: 8 hours  
**Methodology**: TDD (test-driven development)

---

## Phase 1: Scripts Setup (2h)

### 1.1 Create scripts directory structure
- [ ] 1.1.1 Create `skills/orchestrator/scripts/` directory
- [ ] 1.1.2 Create `skills/orchestrator/scripts/__init__.py`
- [ ] 1.1.3 Create `skills/orchestrator/scripts/requirements.txt` (empty - stdlib only)

### 1.2 Implement session_identity.py
- [ ] 1.2.1 Write failing tests for ID generation
- [ ] 1.2.2 Implement `generate_session_id(base_agent: str) -> str`
- [ ] 1.2.3 Implement `format_display_name(session_id: str) -> str`
- [ ] 1.2.4 Implement `parse_session_id(session_id: str) -> dict`
- [ ] 1.2.5 Add CLI interface with argparse

### 1.3 Implement preflight.py
- [ ] 1.3.1 Write failing tests for session detection
- [ ] 1.3.2 Implement `detect_sessions(inbox_json: list) -> list`
- [ ] 1.3.3 Implement `check_conflicts(my_session: dict, active_sessions: list) -> dict`
- [ ] 1.3.4 Implement `format_active_sessions(sessions: list) -> str`
- [ ] 1.3.5 Implement `format_conflicts(conflicts: dict) -> str`
- [ ] 1.3.6 Add CLI interface with subcommands: `detect`, `format-sessions`, `format-conflicts`

### 1.4 Implement session_cleanup.py
- [ ] 1.4.1 Write failing tests for stale detection
- [ ] 1.4.2 Implement `find_stale_sessions(sessions: list, threshold_min: int) -> list`
- [ ] 1.4.3 Implement `format_takeover_prompt(session: dict) -> str`
- [ ] 1.4.4 Add CLI interface

---

## Phase 2: Reference Documentation (1h)

### 2.1 Create preflight.md reference
- [ ] 2.1.1 Document Phase 0 protocol (4 steps)
- [ ] 2.1.2 Document trigger conditions and skip rules
- [ ] 2.1.3 Include code examples for Agent Mail integration
- [ ] 2.1.4 Document error handling and timeouts

### 2.2 Create session-identity.md reference
- [ ] 2.2.1 Document identity format (internal vs display)
- [ ] 2.2.2 Document collision handling (retry with incremented timestamp)
- [ ] 2.2.3 Document Agent Mail profile persistence

### 2.3 Update session-lifecycle.md
- [ ] 2.3.1 Add multi-session awareness section
- [ ] 2.3.2 Document SESSION START/HEARTBEAT/SESSION END messages
- [ ] 2.3.3 Add stale detection and takeover flow

---

## Phase 3: Orchestrator Integration (3h)

### 3.1 Update workflow.md
- [ ] 3.1.1 Insert Phase 0 before Mode Selection (Pre-Phase)
- [ ] 3.1.2 Document 4-step preflight protocol
- [ ] 3.1.3 Add skip conditions for ds and query commands
- [ ] 3.1.4 Document integration with existing Phase 1-7

### 3.2 Update SKILL.md
- [ ] 3.2.1 Add "Phase 0: Preflight" section to workflow overview
- [ ] 3.2.2 Update 7-phase → 8-phase in architecture diagram
- [ ] 3.2.3 Document session identity format
- [ ] 3.2.4 Document conflict detection behavior
- [ ] 3.2.5 Add scripts/ directory to file structure

### 3.3 Update agents/README.md
- [ ] 3.3.1 Document Orchestrator as "session brain"
- [ ] 3.3.2 Add preflight responsibility to orchestrator role

### 3.4 Update AGENTS.md (project root)
- [ ] 3.4.1 Add session protocol section
- [ ] 3.4.2 Document session identity format
- [ ] 3.4.3 Add preflight trigger documentation

---

## Phase 4: Testing & Verification (2h)

### 4.1 Unit tests for scripts
- [ ] 4.1.1 Test session_identity.py (ID generation, parsing, display)
- [ ] 4.1.2 Test preflight.py (detection, conflicts, formatting)
- [ ] 4.1.3 Test session_cleanup.py (stale detection, formatting)

### 4.2 Manual integration test
- [ ] 4.2.1 Create `docs/testing/multi-session-test.md` with step-by-step
- [ ] 4.2.2 Test: Session 1 starts, Session 2 detects it
- [ ] 4.2.3 Test: Track conflict warning
- [ ] 4.2.4 Test: Stale session takeover

### 4.3 Verification
- [ ] 4.3.1 Run all script tests
- [ ] 4.3.2 Verify ds skips preflight
- [ ] 4.3.3 Verify bd ready skips preflight
- [ ] 4.3.4 Verify /conductor-implement runs preflight

---

## Automated Verification

```bash
# Run script tests
python -m pytest skills/orchestrator/scripts/ -v

# Verify scripts are executable
python skills/orchestrator/scripts/session_identity.py --help
python skills/orchestrator/scripts/preflight.py --help
python skills/orchestrator/scripts/session_cleanup.py --help

# Verify JSON output format
echo '[]' | python skills/orchestrator/scripts/preflight.py detect --inbox-json -
```

---

## Track Assignments

| Track | Agent | Tasks | File Scope | Depends On |
|-------|-------|-------|------------|------------|
| 1 | Worker-A | 1.1.*, 1.2.*, 1.3.*, 1.4.* | skills/orchestrator/scripts/** | - |
| 2 | Worker-B | 2.1.*, 2.2.*, 2.3.* | skills/orchestrator/references/** | - |
| 3 | Worker-C | 3.1.*, 3.2.*, 3.3.*, 3.4.* | skills/orchestrator/SKILL.md, AGENTS.md | 1.*, 2.* |
| 4 | Worker-D | 4.1.*, 4.2.*, 4.3.* | docs/testing/**, skills/orchestrator/scripts/** | 1.*, 2.*, 3.* |

### Cross-Track Dependencies
- Track 3 waits for Track 1 (scripts must exist before SKILL.md references them)
- Track 3 waits for Track 2 (references must exist before SKILL.md links them)
- Track 4 waits for all tracks (testing requires complete implementation)

---

## Risk Mitigation

| Risk | Mitigation |
|------|------------|
| Agent Mail unavailable | Graceful fallback with 3s timeout |
| Script complexity | Keep each under 200 lines, stdlib only |
| Breaking existing workflow | Phase 0 is additive, no changes to Phase 1-7 |
