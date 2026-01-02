# Implementation Plan: Orchestrator v2 - Gas Town Philosophy

**Track ID:** orchestrator-v2-gastown
**Estimated Effort:** 4 phases (~4 weeks)

## Phase 1: Foundation (P0) - Message Protocol

### Epic 1.1: Protocol Implementation

#### Task 1.1.1: Create message parser module
**Files:** `skills/orchestrator/references/protocol/parser.py`
**Acceptance Criteria:**
- [ ] `parse_message(body_md)` extracts YAML frontmatter
- [ ] Returns `{"meta": {...}, "content": "..."}`
- [ ] Handles missing frontmatter → `type: UNKNOWN`
- [ ] Handles malformed YAML gracefully
- [ ] `build_message(type, content, **fields)` creates formatted message

#### Task 1.1.2: Create message types module
**Files:** `skills/orchestrator/references/protocol/types.py`
**Acceptance Criteria:**
- [ ] `MessageType` enum with all 11 types
- [ ] `IMPORTANCE_MAP` for type → importance mapping
- [ ] TypedDict schemas for each payload type
- [ ] Validation helpers for required fields

#### Task 1.1.3: Create message templates documentation
**Files:** `skills/orchestrator/references/protocol/templates.md`
**Acceptance Criteria:**
- [ ] Template for each of 11 message types
- [ ] YAML frontmatter examples with all required fields
- [ ] Human-readable content examples
- [ ] Subject pattern conventions

#### Task 1.1.4: Create protocol __init__.py
**Files:** `skills/orchestrator/references/protocol/__init__.py`
**Acceptance Criteria:**
- [ ] Export parse_message, build_message
- [ ] Export MessageType enum
- [ ] Export IMPORTANCE_MAP

### Epic 1.2: Worker Protocol v2

#### Task 1.2.1: Create worker protocol v2 documentation
**Files:** `skills/orchestrator/references/worker-protocol-v2.md`
**Acceptance Criteria:**
- [ ] 7-step startup sequence documented
- [ ] Inbox check → beads query → execute flow
- [ ] Heartbeat protocol (5min interval)
- [ ] PING/PONG handling
- [ ] BLOCKED behavior documented

#### Task 1.2.2: Create worker prompt v2 template
**Files:** `skills/orchestrator/references/worker-prompt-v2.md`
**Acceptance Criteria:**
- [ ] Self-propulsion instructions
- [ ] macro_start_session() as first step
- [ ] Inbox check before beads query
- [ ] File reservation before work
- [ ] Mandatory COMPLETED message before exit
- [ ] Heartbeat reminders

#### Task 1.2.3: Update SKILL.md with v2 references
**Files:** `skills/orchestrator/SKILL.md`
**Acceptance Criteria:**
- [ ] Add protocol layer documentation
- [ ] Reference new protocol/ directory
- [ ] Update worker protocol reference
- [ ] Add message catalog summary

---

## Phase 2: Beads Integration (P0)

### Epic 2.1: Beads Assignee Support

#### Task 2.1.1: Document assignee field requirements
**Files:** `skills/orchestrator/references/beads-assignee.md`
**Acceptance Criteria:**
- [ ] Schema change documented (assignee, assigned_at fields)
- [ ] CLI interface documented (--assignee flag)
- [ ] Query patterns documented (--assignee=self)
- [ ] Integration points with orchestrator

#### Task 2.1.2: Document stale detection requirements
**Files:** `skills/orchestrator/references/beads-stale.md`
**Acceptance Criteria:**
- [ ] Stale query documented (--stale=30m)
- [ ] Calculation logic documented
- [ ] Use cases documented
- [ ] Integration with witness patrol

#### Task 2.1.3: Document conditional update requirements
**Files:** `skills/orchestrator/references/beads-atomic-claim.md`
**Acceptance Criteria:**
- [ ] --expect-status flag documented
- [ ] Race condition handling documented
- [ ] Failure scenarios documented
- [ ] Worker claiming protocol

### Epic 2.2: Orchestrator Dispatch Update

#### Task 2.2.1: Update workflow.md with typed dispatch
**Files:** `skills/orchestrator/references/workflow.md`
**Acceptance Criteria:**
- [ ] Phase 4 updated: Assign in Beads before dispatch
- [ ] Typed ASSIGN message format
- [ ] Epic thread creation
- [ ] Worker pre-registration

#### Task 2.2.2: Update auto-routing with assignee check
**Files:** `skills/orchestrator/references/auto-routing.md`
**Acceptance Criteria:**
- [ ] Check assignee field in bead triage
- [ ] Skip already-assigned tasks
- [ ] Handle reassignment scenarios

---

## Phase 3: Monitoring (P1)

### Epic 3.1: Wisp Support

#### Task 3.1.1: Document wisp requirements
**Files:** `skills/orchestrator/references/wisps.md`
**Acceptance Criteria:**
- [ ] Schema change documented (ephemeral field)
- [ ] CLI interface documented (--wisp, bd burn, bd squash)
- [ ] Lifecycle documented (create → use → burn/squash)
- [ ] Git exclusion behavior documented

### Epic 3.2: Witness Patrol

#### Task 3.2.1: Create witness patrol documentation
**Files:** `skills/orchestrator/references/witness-patrol.md`
**Acceptance Criteria:**
- [ ] 4-check patrol cycle documented
- [ ] Backoff strategy documented
- [ ] Wisp usage for patrol
- [ ] Reassignment protocol

#### Task 3.2.2: Update workflow.md Phase 6 with patrol
**Files:** `skills/orchestrator/references/workflow.md`
**Acceptance Criteria:**
- [ ] Monitor loop includes patrol
- [ ] Stale detection integrated
- [ ] Unblock detection integrated
- [ ] Load balance check integrated

#### Task 3.2.3: Create /conductor-patrol command
**Files:** `skills/conductor/references/commands/patrol.toml`, `skills/conductor/references/workflows/patrol.md`
**Acceptance Criteria:**
- [ ] Command definition in patrol.toml
- [ ] Workflow documented in patrol.md
- [ ] Session scan logic
- [ ] Stale bead detection
- [ ] Takeover/cleanup options

### Epic 3.3: Heartbeat Protocol

#### Task 3.3.1: Document heartbeat requirements
**Files:** `skills/orchestrator/references/heartbeat.md`
**Acceptance Criteria:**
- [ ] Beads --heartbeat flag documented
- [ ] last_heartbeat field documented
- [ ] --heartbeat-stale query documented
- [ ] Worker heartbeat interval (5min)
- [ ] Stale threshold (10min without heartbeat)

---

## Phase 4: Optimization (P2)

### Epic 4.1: Work Stealing

#### Task 4.1.1: Document work stealing protocol
**Files:** `skills/orchestrator/references/work-stealing.md`
**Acceptance Criteria:**
- [ ] STEAL message format
- [ ] Load imbalance detection
- [ ] Reassignment via Beads
- [ ] Worker handling of STEAL

### Epic 4.2: Refinery Integration

#### Task 4.2.1: Document refinery role
**Files:** `skills/orchestrator/references/refinery.md`
**Acceptance Criteria:**
- [ ] Post-completion review flow
- [ ] Integration with rb (review beads)
- [ ] Quality gate checks
- [ ] Merge integration

### Epic 4.3: Observability

#### Task 4.3.1: Document metrics requirements
**Files:** `skills/orchestrator/references/observability.md`
**Acceptance Criteria:**
- [ ] Metrics to track (beads closed, blockers, retries)
- [ ] State file additions (implement_state.json)
- [ ] Patrol log format
- [ ] Summary generation

---

## Track Assignments

| Track | Worker | Tasks | Files |
|-------|--------|-------|-------|
| A: Protocol | Worker-Protocol | 1.1.1, 1.1.2, 1.1.3, 1.1.4 | `skills/orchestrator/references/protocol/*` |
| B: Worker | Worker-Template | 1.2.1, 1.2.2, 1.2.3 | `skills/orchestrator/references/worker-*.md`, `SKILL.md` |
| C: Beads-Docs | Worker-Beads | 2.1.1, 2.1.2, 2.1.3 | `skills/orchestrator/references/beads-*.md` |
| D: Dispatch | Worker-Dispatch | 2.2.1, 2.2.2 | `skills/orchestrator/references/workflow.md`, `auto-routing.md` |
| E: Patrol | Worker-Patrol | 3.1.1, 3.2.1, 3.2.2, 3.2.3, 3.3.1 | `skills/orchestrator/references/witness-*.md`, `wisps.md`, `heartbeat.md`, `skills/conductor/references/*/patrol.*` |
| F: Optimize | Worker-Optimize | 4.1.1, 4.2.1, 4.3.1 | `skills/orchestrator/references/work-stealing.md`, `refinery.md`, `observability.md` |

---

## Dependencies

```
Phase 1 (Foundation)
├── 1.1.1 parser.py
├── 1.1.2 types.py (depends on 1.1.1)
├── 1.1.3 templates.md (depends on 1.1.2)
├── 1.1.4 __init__.py (depends on 1.1.1, 1.1.2)
├── 1.2.1 worker-protocol-v2.md (depends on 1.1.3)
├── 1.2.2 worker-prompt-v2.md (depends on 1.2.1)
└── 1.2.3 SKILL.md update (depends on 1.1.4, 1.2.2)

Phase 2 (Beads Integration) - depends on Phase 1
├── 2.1.1 beads-assignee.md
├── 2.1.2 beads-stale.md
├── 2.1.3 beads-atomic-claim.md
├── 2.2.1 workflow.md update (depends on 2.1.*)
└── 2.2.2 auto-routing.md update (depends on 2.2.1)

Phase 3 (Monitoring) - depends on Phase 2
├── 3.1.1 wisps.md
├── 3.2.1 witness-patrol.md (depends on 3.1.1)
├── 3.2.2 workflow.md Phase 6 (depends on 3.2.1)
├── 3.2.3 /conductor-patrol (depends on 3.2.1)
└── 3.3.1 heartbeat.md

Phase 4 (Optimization) - depends on Phase 3
├── 4.1.1 work-stealing.md
├── 4.2.1 refinery.md
└── 4.3.1 observability.md
```

---

## Verification

| Phase | Verification |
|-------|--------------|
| Phase 1 | Protocol parser tests, message format validation |
| Phase 2 | Beads CLI simulation (mock), dispatch flow walkthrough |
| Phase 3 | Patrol loop simulation, recovery scenario testing |
| Phase 4 | Load balance scenarios, refinery flow validation |
