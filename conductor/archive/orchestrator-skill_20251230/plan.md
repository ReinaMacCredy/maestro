# Implementation Plan: Orchestrator Skill

## Overview

Create orchestrator skill for multi-agent parallel execution with autonomous workers.

**Estimated Total: 7 hours**

---

## Orchestration Config

epic_id: pending
max_workers: 3
mode: autonomous

## Track Assignments

| Track | Agent | Tasks | File Scope | Depends On |
|-------|-------|-------|------------|------------|
| 1 | BlueLake | 1.1.*, 1.2.* | skills/orchestrator/** | - |
| 2 | GreenCastle | 2.1.*, 2.2.* | skills/maestro-core/** | 1.2.3 |
| 3 | RedStone | 3.1.* | conductor/CODEMAPS/** | 2.2.2 |

### Cross-Track Dependencies
- Track 2 waits for 1.2.3 (SKILL.md complete)
- Track 3 waits for 2.2.2 (maestro-core routing complete)

---

## Phase 1: Skill Setup (1h)

### Epic 1.1: Directory Structure

- [ ] **1.1.1** Create `skills/orchestrator/` directory
- [ ] **1.1.2** Create `skills/orchestrator/references/` directory
- [ ] **1.1.3** Create `skills/orchestrator/references/patterns/` directory

**Acceptance Criteria:**
- Directory structure matches design

### Epic 1.2: Core Skill Files

- [ ] **1.2.1** Create `skills/orchestrator/SKILL.md` with frontmatter and overview
- [ ] **1.2.2** Create `skills/orchestrator/references/workflow.md` (6-phase protocol)
- [ ] **1.2.3** Create `skills/orchestrator/references/worker-prompt.md` (worker template)

**Acceptance Criteria:**
- SKILL.md has correct frontmatter (name, version, description)
- workflow.md covers all 6 phases
- worker-prompt.md has complete protocol

---

## Phase 2: maestro-core Integration (1.5h)

### Epic 2.1: Skill Hierarchy Update

- [ ] **2.1.1** Update `skills/maestro-core/SKILL.md` - add orchestrator to hierarchy table
- [ ] **2.1.2** Update `skills/maestro-core/references/hierarchy.md` - add Level 3 orchestrator

**Acceptance Criteria:**
- Orchestrator appears at Level 3 in hierarchy

### Epic 2.2: Command Routing Update

- [ ] **2.2.1** Update `skills/maestro-core/SKILL.md` - add `/conductor-orchestrate` to routing table
- [ ] **2.2.2** Update `skills/maestro-core/references/routing.md` - add orchestrator routing
- [ ] **2.2.3** Add trigger disambiguation for "run parallel", "spawn workers"

**Acceptance Criteria:**
- `/conductor-orchestrate` routes to orchestrator skill
- Natural language triggers recognized

---

## Phase 3: Supporting References (2h)

### Epic 3.1: Workflow References

- [ ] **3.1.1** Create `skills/orchestrator/references/preparation.md` (bv --robot-triage)
- [ ] **3.1.2** Create `skills/orchestrator/references/monitoring.md` (Agent Mail monitoring)

**Acceptance Criteria:**
- preparation.md covers "dọn cỗ" with bv robot flags
- monitoring.md covers fetch_inbox, search_messages, blocker handling

### Epic 3.2: Pattern Migration

- [ ] **3.2.1** Copy `coordination/patterns/parallel-dispatch.md` → `orchestrator/references/patterns/`
- [ ] **3.2.2** Copy `coordination/patterns/session-lifecycle.md` → `orchestrator/references/patterns/`
- [ ] **3.2.3** Copy `coordination/patterns/graceful-fallback.md` → `orchestrator/references/patterns/`
- [ ] **3.2.4** Copy `coordination/examples/dispatch-three-agents.md` → `orchestrator/references/examples/`

**Acceptance Criteria:**
- All pattern files copied and adapted for orchestrator context

---

## Phase 4: Documentation & CODEMAPS (1.5h)

### Epic 4.1: CODEMAPS Update

- [ ] **4.1.1** Update `conductor/CODEMAPS/overview.md` - add orchestrator to structure
- [ ] **4.1.2** Update `conductor/CODEMAPS/skills.md` - add orchestrator skill docs

**Acceptance Criteria:**
- Orchestrator appears in CODEMAPS

### Epic 4.2: Integration Docs

- [ ] **4.2.1** Update `AGENTS.md` - add orchestrator to skill list
- [ ] **4.2.2** Add plan.md extended format documentation to orchestrator/references/

**Acceptance Criteria:**
- AGENTS.md references orchestrator skill
- plan.md format documented

---

## Phase 5: Testing (1h)

### Epic 5.1: Manual Testing

- [ ] **5.1.1** Test `/conductor-orchestrate` command routing
- [ ] **5.1.2** Test plan.md Track Assignments parsing
- [ ] **5.1.3** Test worker spawn with mock beads
- [ ] **5.1.4** Test Agent Mail integration (register, send, fetch)
- [ ] **5.1.5** Test graceful fallback when Agent Mail unavailable

**Acceptance Criteria:**
- All manual tests pass
- No errors in orchestrator workflow

---

## Task Dependencies

```
Phase 1 (Setup)
├── 1.1.* (directories) ─┬─► 1.2.* (core files)
                         │
                         └─► Phase 2 (maestro-core)
                              ├── 2.1.* (hierarchy)
                              └── 2.2.* (routing) ─► Phase 3 (references)
                                                      ├── 3.1.* (workflow refs)
                                                      └── 3.2.* (patterns) ─► Phase 4 (docs)
                                                                                └── Phase 5 (testing)
```

---

## Summary

| Phase | Epics | Tasks | Est. Hours |
|-------|-------|-------|------------|
| 1. Skill Setup | 2 | 6 | 1h |
| 2. maestro-core Integration | 2 | 5 | 1.5h |
| 3. Supporting References | 2 | 6 | 2h |
| 4. Documentation | 2 | 4 | 1.5h |
| 5. Testing | 1 | 5 | 1h |
| **Total** | **9** | **26** | **7h** |
