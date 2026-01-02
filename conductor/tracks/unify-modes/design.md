/h# Design: Unify SA/MA into FULL Mode

## Summary

Merge Single-Agent (SA) and Multi-Agent (MA) modes into one unified FULL mode. All execution goes through orchestrator with Agent Mail coordination. Remove Village MCP entirely.

## Problem

Current workflow has two modes:
- **SA (Single-Agent):** Sequential TDD, direct `bd` CLI
- **MA (Multi-Agent):** Parallel dispatch, Village MCP coordination

This creates:
- Branching logic throughout the codebase
- Cognitive overhead (which mode am I in?)
- Two coordination mechanisms to maintain (Village + Agent Mail)

## Solution

**One mode: FULL**
- Always parallel-capable via orchestrator
- Always spawn orchestrator, even for 1 task (consistency > micro-optimization)
- Coordination via Agent Mail MCP only
- Remove Village MCP entirely

## Architecture

```
ci/implement
    │
    ▼
┌─────────────────────────────┐
│ Preflight (simplified)      │
│ - Check bd available        │
│ - Register with Agent Mail  │
│ - No mode detection         │
└─────────────────────────────┘
    │
    ▼
┌─────────────────────────────┐
│ Orchestrator (always)       │
│ - Analyze dependencies      │
│ - Spawn workers via Task    │
│ - Coordinate via Agent Mail │
│   • file_reservation_paths  │
│   • send_message/fetch_inbox│
└─────────────────────────────┘
    │
    ▼
┌─────────────────────────────┐
│ Workers (parallel)          │
│ - Claim bead (bd update)    │
│ - Reserve files (Agent Mail)│
│ - TDD cycle                 │
│ - Release reservation       │
│ - Report via send_message   │
└─────────────────────────────┘
```

## Agent Mail Coordination (Mode B: Autonomous Workers)

Reference: 

### Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              ORCHESTRATOR                                   │
│                              (Main Agent)                                   │
├─────────────────────────────────────────────────────────────────────────────┤
│  1. Read plan.md / metadata.json                                            │
│  2. Initialize Agent Mail (ensure_project, register_agent)                  │
│  3. Spawn worker subagents via Task()                                       │
│  4. Monitor progress via Agent Mail (fetch_inbox)                           │
│  5. Handle cross-track blockers                                             │
│  6. Announce completion                                                     │
└─────────────────────────────────────────────────────────────────────────────┘
           │
           │ Task() spawns parallel workers
           ▼
┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐
│  BlueLake        │  │  GreenCastle     │  │  RedStone        │
│  Track 1         │  │  Track 2         │  │  Track 3         │
├──────────────────┤  ├──────────────────┤  ├──────────────────┤
│  For each bead:  │  │  For each bead:  │  │  For each bead:  │
│  • Reserve files │  │  • Reserve files │  │  • Reserve files │
│  • Do work (TDD) │  │  • Do work (TDD) │  │  • Do work (TDD) │
│  • Report mail   │  │  • Report mail   │  │  • Report mail   │
│  • Next bead     │  │  • Next bead     │  │  • Next bead     │
└──────────────────┘  └──────────────────┘  └──────────────────┘
           │                   │                   │
           └───────────────────┼───────────────────┘
                               ▼
                    ┌─────────────────────┐
                    │     Agent Mail      │
                    │  ─────────────────  │
                    │  Epic Thread:       │
                    │  • Progress reports │
                    │  • Bead completions │
                    │  • Blockers         │
                    │                     │
                    │  Track Threads:     │
                    │  • Bead context     │
                    │  • Learnings        │
                    └─────────────────────┘
```

### Worker Protocol (per bead)

**Village → Agent Mail Mapping:**

| Step | Village (remove) | Agent Mail (use) |
|------|------------------|------------------|
| 1. Identity | `init(team, role)` | `register_agent(name, program, model)` |
| 2. Check messages | `inbox()` | `fetch_inbox()` |
| 3. Claim task | `claim()` (atomic) | `bd update <id> --status in_progress` |
| 4. Reserve files | `reserve(path)` | `file_reservation_paths(paths=[...])` |
| 5. Work | (same) | TDD cycle |
| 6a. Close task | `done(taskId, reason)` | `bd close <id> --reason completed` |
| 6b. Release files | (auto in done) | `release_file_reservations()` |
| 7. Notify | `msg(content)` | `send_message(to=[...], body_md=...)` |

**Full Worker Flow:**

```
┌────────────────────────────────────────────────────────────────────┐
│  1. REGISTER                                                        │
│     register_agent(name="{AGENT_NAME}", program="amp", model="...")│
│                                                                     │
│  2. CHECK INBOX                                                     │
│     fetch_inbox() → Check for messages from orchestrator           │
│                                                                     │
│  3. LOAD CONTEXT                                                    │
│     summarize_thread(thread_id="track:{AGENT_NAME}:{EPIC_ID}")     │
│                                                                     │
│  4. CLAIM BEAD                                                      │
│     bd update {BEAD_ID} --status in_progress                       │
│                                                                     │
│  5. RESERVE FILES                                                   │
│     file_reservation_paths(paths=["{FILE_SCOPE}"], reason="{BEAD}")│
│                                                                     │
│  6. WORK                                                            │
│     ┌─────────────────────────────────────────────────────────────┐│
│     │  TDD Cycle (RED → GREEN → REFACTOR)                         ││
│     │  Check inbox periodically for blockers                      ││
│     └─────────────────────────────────────────────────────────────┘│
│                                                                     │
│  7. CLOSE BEAD                                                      │
│     bd close {BEAD_ID} --reason completed                          │
│                                                                     │
│  8. RELEASE FILES                                                   │
│     release_file_reservations()                                    │
│                                                                     │
│  9. NOTIFY ORCHESTRATOR                                             │
│     send_message(                                                  │
│       to=["{ORCHESTRATOR}"],                                       │
│       thread_id="{EPIC_ID}",                                       │
│       subject="[{BEAD_ID}] COMPLETE",                              │
│       body_md="Status: completed\nFiles: ...\nNext: ..."          │
│     )                                                               │
│                                                                     │
│ 10. SAVE CONTEXT (for next bead)                                    │
│     send_message(                                                  │
│       to=["{AGENT_NAME}"],  ← self                                 │
│       thread_id="track:{AGENT_NAME}:{EPIC_ID}",                    │
│       subject="Context for next bead",                             │
│       body_md="Decisions: ...\nState: ..."                         │
│     )                                                               │
│                                                                     │
│ 11. NEXT BEAD                                                       │
│     → Loop back to step 3 (LOAD CONTEXT)                           │
└────────────────────────────────────────────────────────────────────┘
```

**Key Difference:** Village `done()` auto-released files. Agent Mail requires explicit `release_file_reservations()` call.

### Thread Structure

| Thread | Purpose | Participants |
|--------|---------|--------------|
| `{EPIC_ID}` | Epic-wide coordination | Orchestrator ↔ All workers |
| `track:{AGENT}:{EPIC_ID}` | Bead-to-bead context | Worker ↔ Self |

### Village Removal

| Village (remove) | Agent Mail (use) |
|------------------|------------------|
| `bv claim` | `bd update --status in_progress` + `file_reservation_paths` |
| `bv reserve` | `file_reservation_paths` |
| `bv release` | `release_file_reservations` |
| `bv msg` | `send_message` |
| `bv inbox` | `fetch_inbox` |
| `bv --robot-status` | `fetch_inbox` + parse |
| `.beads-village/` | Not needed |

## Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Single task handling | Orchestrator with 1 worker | Consistent flow, predictable |
| Coordination mechanism | Agent Mail only | Already robust, no Village maintenance |
| Mode detection | Remove entirely | No branching needed |
| Fallback on Agent Mail failure | HALT | Coordination is required for parallel |

## Migration Checklist (Detailed)

### 1. AGENTS.md (Root)

**File:** `AGENTS.md`
**Lines:** 22-29

**Remove:**
```markdown
### SA vs MA Mode

```
plan.md has "## Track Assignments"?
├─ YES → MA mode (load orchestrator skill)
└─ NO  → SA mode (sequential TDD)
```
```

**Replace with:**
```markdown
### Execution Mode

Always FULL mode via orchestrator. Even single tasks spawn 1 worker for consistency.
```

---

### 2. conductor/SKILL.md

**File:** `.claude/skills/conductor/SKILL.md`
**Lines:** 57-64

**Remove:**
```markdown
## Beads Integration

| Mode | When | Commands |
|------|------|----------|
| SA (Single-Agent) | Default | Direct `bd` CLI |
| MA (Multi-Agent) | Coordinated | Village MCP |
```

**Replace with:**
```markdown
## Beads Integration

Unified FULL mode - all execution goes through orchestrator with Agent Mail coordination.
Workers use `bd` CLI for beads, Agent Mail for file reservations and messaging.
```

---

### 3. conductor/references/preflight-beads.md

**File:** `.claude/skills/conductor/references/preflight-beads.md`

**Major rewrite:**
- Remove Village MCP check (Step 3 lines 43-45)
- Remove mode detection algorithm (lines 28-56)
- Simplify to: Check bd → Register with Agent Mail → Done

**New flow:**
```
1. Check bd available (HALT if not)
2. Register agent with Agent Mail
3. Check for active sessions (fetch_inbox)
4. Create session state in metadata.json
```

---

### 4. conductor/references/workflows/implement.md

**File:** `.claude/skills/conductor/references/workflows/implement.md`
**Lines:** 123-290 (Phase 2)

**Remove:**
- `SINGLE_AGENT` execution path
- Mode detection in Phase 2
- Fallback to sequential logic

**Replace with:**
- Always route to orchestrator
- Single bead = orchestrator with 1 worker

---

### 5. beads/references/VILLAGE.md

**File:** `.claude/skills/beads/references/VILLAGE.md`

**Action:** DELETE entire file (277 lines)

---

### 6. beads/references/workflow.md

**File:** `.claude/skills/beads/references/workflow.md`
**Lines:** 137-152

**Remove:**
```markdown
### Multi-Agent Session Start

When multiple agents work on the same codebase, use Village coordination.
```

**Replace with:**
```markdown
### Parallel Session Start

When spawning workers, use Agent Mail coordination:
1. register_agent(name=worker_name)
2. file_reservation_paths(paths=[scope])
3. bd update <id> --status in_progress
```

---

### 7. beads/references/conductor-integration.md

**File:** `.claude/skills/beads/references/conductor-integration.md`
**Lines:** 20-27

**Remove entire SA vs MA Mode section**

---

### 8. orchestrator/SKILL.md

**File:** `.claude/skills/orchestrator/SKILL.md`
**Line:** 52

**Remove:**
```markdown
| Complete | Verify via `bv`, send summary, `bd close epic` |
```

**Replace with:**
```markdown
| Complete | Verify via `bd list`, send summary, `bd close epic` |
```

---

### 9. orchestrator/references/workflow.md

**File:** `.claude/skills/orchestrator/references/workflow.md`
**Lines:** 517-519

**Remove:**
```python
# Check via beads
status = bash("bv --robot-triage --graph-root <epic-id> | jq '.quick_ref'")
```

**Replace with:**
```python
# Check via beads
open_count = bash("bd list --parent=<epic-id> --status=open --json | jq 'length'")
```

---

### 10. Global ~/.config/amp/AGENTS.md

**Remove entire section:**
```markdown
<!-- BEGIN maestro-village -->

### Beads Village

MCP server for multi-agent coordination via `npx beads-village`.

**Session Start:**
```bash
bv --robot-status  # Check team state
```

**Tools:** `init`, `claim`, `done`, `reserve`, `release`, `msg`, `inbox`, `status`

**Paths:** `.beads-village/`, `.reservations/`, `.mail/`

<!-- END maestro-village -->
```

---

### 11. Additional Files with Village References

**Found via grep - need updates:**

#### Skills (conductor/references/)
- `beads-session.md` - Lines 74-92, 140, 296, 578, 611, 731 (init, inbox, claim, reserve, done, msg)
- `beads-integration.md` - Lines 82, 89-90, 134, 471-486 (Village flow)
- `beads-facade.md` - Line 219 (done auto-release)
- `decompose-task.md` - Lines 230-232 (bv --robot-suggest/plan/alerts)
- `preflight-beads.md` - Line 132 (bv --robot-status check)
- `workflows/implement.md` - Line 345 (claim())

#### Skills (orchestrator/references/)
- `preparation.md` - Lines 19, 22, 90 (bv --robot-triage)
- `workflow.md` - Line 518 (bv --robot-triage)
- `monitoring.md` - Lines 17-18, 52 (bv --robot-triage)

#### Skills (beads/references/)
- `auto-orchestrate.md` - Lines 15, 26, 220 (bv --robot-triage)
- `workflow.md` - Lines 145, 163, 170, 849-860 (init, claim, reserve, inbox, msg)
- `WORKFLOWS.md` - Lines 546, 590, 596 (init)
- `FILE_BEADS.md` - Line 388 (bv --robot-triage)

#### Root Files
- `SETUP_GUIDE.md` - Lines 93, 107-143, 155, 181 (Village setup)
- `CLAUDE.md` - Line 134 (bv warning)
- `AGENTS.md` - Line 195 (--robot-* rule)
- `README.md` - Lines 62, 85 (Village reference)
- `REFERENCE.md` - Lines 88-94, 131, 202, 258 (Village commands)

#### Conductor Root
- `conductor/AGENTS.md` - Lines 31, 56-57 (bv commands in learnings)
- `conductor/CODEMAPS/overview.md` - Lines 73, 149 (bv commands)
- `conductor/tracks.md` - Lines 57, 63 (Village references)

#### Scripts
- `scripts/validate-anchors.sh` - Line 56 (.beads-village)
- `scripts/validate-links.sh` - Line 57 (.beads-village)

#### Archive (historical - skip or update minimally)
- `conductor/archive/` - Multiple files reference bv (can leave as historical)

---

### 12. Orchestrator LIGHT/FULL Mode (Also Remove)

The orchestrator also has LIGHT vs FULL mode - this should also be unified to FULL only.

**Files:**
- `orchestrator/references/workflow.md` - Lines 66-80, 207-209 (LIGHT mode definition)
- `orchestrator/references/coordination-modes.md` - Entire file defines LIGHT vs FULL
- `orchestrator/references/architecture.md` - Line 50 (Light mode reference)

**Current:**
```python
if not agent_mail_available:
    return "LIGHT"  # Fallback - no Agent Mail
elif all(estimate_duration(t) < 10 for t in TRACKS):
    return "LIGHT"  # Simple short tasks
else:
    return "FULL"   # Default for complex work
```

**New (FULL only):**
```python
if not agent_mail_available:
    HALT("Agent Mail required for orchestration")
return "FULL"  # Always FULL mode
```

---

### 13. Summary: Complete File Count

| Category | Files | Action |
|----------|-------|--------|
| **Delete** | 1 | VILLAGE.md |
| **Major rewrite** | 8 | preflight-beads, beads-session, beads-integration, implement, coordination-modes, workflow, auto-routing, execution-routing |
| **High ref count** | 9 | track-init-beads, beads-facade, finish-workflow, setup, finish, session-init, status-sync, migrate-beads, RESUMABILITY |
| **SA/MA/Mode refs** | 18 | Multiple conductor/beads/orchestrator references |
| **Village/bv refs** | 10 | orchestrator, conductor, root docs |
| **Scripts** | 2 | validate-anchors.sh, validate-links.sh |
| **Archive** | ~15 | Leave as historical (no changes) |

**Total: ~45 files to modify (excluding archive)**

---

### 15. Execution Routing (TIER 1/2 → Simplify)

Currently there's complex routing logic:
- **TIER 1**: Weighted scoring → SPEED/ASK/FULL
- **TIER 2**: Compound conditions → SINGLE_AGENT/PARALLEL_DISPATCH

**Files:**
- `.claude/skills/design/references/execution-routing.md` - SINGLE_AGENT reference
- `.claude/skills/orchestrator/references/auto-routing.md` - TIER 1/2 evaluation paths
- `.claude/skills/design/references/session-lifecycle.md` - Mode detect reference
- `.claude/skills/conductor/references/pipeline.md` - TIER references

**Simplify to:**
- Remove SINGLE_AGENT path
- Always route to orchestrator (even for single bead)
- Keep TIER 1 for design complexity (SPEED/ASK/FULL) - this is about *design*, not execution
- Remove TIER 2 - no longer needed (always parallel-capable)

---

### 16. Fallback Logic (Remove)

Multiple files have "fallback to sequential" logic that should become HALT:

**Files with fallback logic:**
1. `.claude/skills/beads/references/auto-orchestrate.md` - Line 394 "Fallback: Sequential Execution"
2. `.claude/skills/orchestrator/references/patterns/parallel-dispatch.md` - Line 14 "fallback to sequential"
3. `.claude/skills/orchestrator/references/preflight.md` - Lines 92, 133 "degrade to single-session"
4. `.claude/skills/orchestrator/references/workflow.md` - Lines 151, 617-628 "falling back to sequential"

**Change:**
```python
# Old
if not agent_mail_available:
    return implement_sequential(track_id)

# New  
if not agent_mail_available:
    HALT("Agent Mail required. Start agent_mail MCP server.")
```

---

### 17. Complete Updated File List

#### Delete (1 file)
1. `.claude/skills/beads/references/VILLAGE.md`

#### Major Rewrite (8 files) - Most references
1. `.claude/skills/conductor/references/preflight-beads.md` (31 refs)
2. `.claude/skills/conductor/references/beads-session.md` (29 refs)
3. `.claude/skills/conductor/references/beads-integration.md` (27 refs)
4. `.claude/skills/conductor/references/workflows/implement.md` (18 refs)
5. `.claude/skills/orchestrator/references/coordination-modes.md`
6. `.claude/skills/orchestrator/references/workflow.md` (9 refs)
7. `.claude/skills/orchestrator/references/auto-routing.md`
8. `.claude/skills/design/references/execution-routing.md`

#### High Reference Count (additional files)
1. `.claude/skills/conductor/references/track-init-beads.md` (16 refs)
2. `.claude/skills/conductor/references/beads-facade.md` (14 refs)
3. `.claude/skills/conductor/references/finish-workflow.md` (13 refs)
4. `.claude/skills/conductor/references/workflows/setup.md` (11 refs)
5. `.claude/skills/conductor/references/workflows/finish.md` (9 refs)
6. `.claude/skills/design/references/session-init.md` (7 refs)
7. `.claude/skills/conductor/references/status-sync-beads.md` (7 refs)
8. `.claude/skills/conductor/references/migrate-beads.md` (6 refs)
9. `.claude/skills/beads/references/RESUMABILITY.md` (6 refs)

#### Update SA/MA/Mode References (18 files)
1. `AGENTS.md` (root)
2. `CLAUDE.md`
3. `.claude/skills/conductor/SKILL.md`
4. `.claude/skills/conductor/references/remember.md`
5. `.claude/skills/conductor/references/validation/beads/checks.md`
6. `.claude/skills/conductor/references/pipeline.md`
7. `.claude/skills/beads/references/workflow.md`
8. `.claude/skills/beads/references/workflow-integration.md`
9. `.claude/skills/beads/references/conductor-integration.md`
10. `.claude/skills/beads/references/WORKFLOWS.md`
11. `.claude/skills/beads/references/auto-orchestrate.md`
12. `.claude/skills/beads/references/FILE_BEADS.md`
13. `.claude/skills/design/references/session-lifecycle.md`
14. `.claude/skills/orchestrator/references/preflight.md`
15. `.claude/skills/orchestrator/references/patterns/parallel-dispatch.md`
16. `.claude/skills/orchestrator/references/architecture.md`
17. `.claude/skills/conductor/references/doc-sync/integration.md`
18. `.claude/skills/orchestrator/references/examples/dispatch-three-agents.md`

#### Update Village/bv References (10 files)
1. `.claude/skills/orchestrator/SKILL.md`
2. `.claude/skills/orchestrator/references/preparation.md`
3. `.claude/skills/orchestrator/references/monitoring.md`
4. `.claude/skills/conductor/references/decompose-task.md`
5. `SETUP_GUIDE.md`
6. `README.md`
7. `REFERENCE.md`
8. `conductor/AGENTS.md`
9. `conductor/tech-stack.md`
10. `conductor/CODEMAPS/overview.md`

#### Scripts (2 files)
1. `scripts/validate-anchors.sh`
2. `scripts/validate-links.sh`

**Total: ~45 files to modify (excluding archive)**

### 18. metadata.json Session State (Update Schema)

The `metadata.json` file stores session state with mode field. This needs updating:

**Current schema (in session section):**
```json
{
  "session": {
    "mode": "SA",           // ← Remove this field
    "bound_bead": "...",
    "tdd_phase": "...",
    "last_activity": "..."
  }
}
```

**New schema:**
```json
{
  "session": {
    "bound_bead": "...",
    "tdd_phase": "...",
    "last_activity": "...",
    "agent_mail_registered": true   // ← Add: confirms Agent Mail setup
  }
}
```

**Files with metadata.json mode references:**
1. `.claude/skills/conductor/references/preflight-beads.md` - Lines 96, 100, 508
2. `.claude/skills/conductor/references/beads-session.md` - Line 634
3. `.claude/skills/conductor/references/beads-integration.md` - Line 304
4. `.claude/skills/conductor/references/beads-facade.md` - Lines 57, 152

---

### 19. Event Logging (Update)

Telemetry events reference mode:

**File:** `.claude/skills/beads/references/workflow-integration.md`
**Line 100:**
```json
{"event": "ma_attempt", "mode": "MA", "timestamp": "..."}
```

**Change to:**
```json
{"event": "orchestrator_init", "agent_mail": true, "timestamp": "..."}
```

---

### 20. Leader Mode (Village concept - Remove)

Village had "leader mode" for task assignment. This is removed.

**File:** `.claude/skills/beads/references/workflow.md`
- References "Leader mode" - remove or replace with orchestrator role

---

### 21. Command Files (.toml) - Minimal Changes

Command files use SPEED/FULL for *design complexity*, not execution mode. These are **not** SA/MA related and should be kept.

**No changes needed:**
- `design.toml` - SPEED/FULL is design routing, keep
- `handoff.toml` - CREATE/RESUME modes, keep
- `finish.toml` - SPEED/FULL enforcement, keep

---

### 22. Schema Files - Update

**File:** `.claude/skills/conductor/references/schemas/metadata.schema.json`
- No `session` field at root level (session state is in markdown refs)
- No changes needed to schema

---

### 23. Final Summary

| What | Action |
|------|--------|
| **SA/MA mode** | Remove entirely |
| **LIGHT/FULL orchestrator mode** | Remove LIGHT, always FULL |
| **Village MCP** | Remove entirely |
| **TIER 2 routing** | Remove (always orchestrator) |
| **TIER 1 routing** | Keep (design complexity) |
| **Fallback to sequential** | Change to HALT |
| **metadata.json session.mode** | Remove field |
| **Agent Mail** | Required (not optional) |

### 24. Additional Root/Doc Files

**Found via grep (by reference count):**

| File | Refs | Action |
|------|------|--------|
| `REFERENCE.md` | 21 | Update Village section, SA/MA refs |
| `conductor/AGENTS.md` | 17 | Update learnings (bv commands, SA mode) |
| `docs/ARCHITECTURE.md` | 13 | Update diagrams (remove SA/MA mode box) |
| `CHANGELOG.md` | 11 | No change (historical) |
| `conductor/CODEMAPS/overview.md` | 9 | Update SA/MA mode section |
| `conductor/tracks.md` | 7 | Update mode references |
| `SETUP_GUIDE.md` | 7 | Remove Village setup section |
| `CLAUDE.md` | 7 | Update fallback policy |
| `AGENTS.md` | 7 | Remove SA vs MA decision tree |
| `TUTORIAL.md` | 6 | Update if has mode references |
| `conductor/workflow.md` | 3 | Update if has mode references |
| `templates/workflow.md` | 2 | Template - check for mode refs |
| `conductor/CODEMAPS/skills.md` | 2 | Update orchestrator description |

**Global config:**
- `~/.config/amp/AGENTS.md` - Remove `<!-- BEGIN maestro-village -->` section

---

### 25. Oracle Audit - Additional Files Found

The Oracle identified these additional files not in previous lists:

#### Graceful Fallback Patterns (4 files)
1. `.claude/skills/orchestrator/references/patterns/graceful-fallback.md` - Lines 1, 11, 38, 66, 86
2. `.claude/skills/orchestrator/references/patterns/parallel-dispatch.md` - Lines 14, 84, 98
3. `.claude/skills/orchestrator/references/worker-prompt.md` - Lines 255, 259
4. `.claude/skills/maestro-core/references/glossary.md` - Lines 42, 46, 47, 54

#### Team/Role Concepts (beads workflow)
1. `.claude/skills/beads/references/workflow.md` - Lines 137-170 (Multi-Agent Session, team/role/leader)
2. `.claude/skills/beads/references/WORKFLOWS.md` - Lines 538-590 (Multi-Agent Workflows, team session)
3. `.claude/skills/beads/references/GIT_INTEGRATION.md` - Lines 402, 425 (Team Branch Pattern)
4. `.claude/skills/beads/references/CONFIG.md` - Line 398 (Team ID)
5. `.claude/skills/beads/references/LABELS.md` - Line 398 (team-prefixed labels)
6. `.claude/skills/beads/references/BOUNDARIES.md` - Lines 271, 291, 337 (role notes)

#### Validation & Structure
1. `.claude/skills/conductor/references/validation/lifecycle.md` - Lines 9, 44, 52 (HALT/DEGRADE)
2. `.claude/skills/writing-skills/references/skill-structure.md` - Lines 98, 109-123 (HALT/DEGRADE guidelines)

#### Maestro Core
1. `.claude/skills/maestro-core/SKILL.md` - Lines 50-56 (Fallback Policies)

---

### 26. Detailed Line-by-Line References (from Oracle)

#### beads-session.md (29 references)
- Lines 26, 30: SA Mode Flow heading/diagram
- Lines 64, 68: MA Mode Flow heading/diagram  
- Lines 71, 92, 139-140: init team/role, msg notify team
- Lines 101, 136, 165: SA/MA Mode Claim sections
- Lines 257, 293: SA/MA Mode Close sections
- Line 149: Atomic claim comment
- Line 731: init team example

#### beads-integration.md (27 references)
- Line 19: Preflight diagram with mode detect
- Lines 26-27: SA/MA table rows
- Lines 40-46: HALT conditions
- Lines 61, 66: Join team (MA) or claim task (SA)
- Lines 439-462: SA vs MA Mode Flows sections
- Lines 491-500: HALT vs Degrade section
- Lines 502, 516: Village MCP unavailable messages
- Line 685: Fallback row

#### preflight-beads.md (31 references)
- Lines 12, 24, 43-44: Village MCP check steps
- Lines 128-146: Step 3 Village MCP Availability
- Line 132: VILLAGE_STATUS=$(bv --robot-status)
- Lines 136, 140: Village MCP Available/Unavailable logs
- Line 700: Degraded mode message

#### implement.md (18 references)
- Line 47: Preflight example output
- Lines 123, 180, 210-212, 249, 289, 314: SINGLE_AGENT routing
- Line 337: SA Mode sequential execution path
- Line 345: claim() atomic claim comment

---

### 27. Updated Final File Count

| Category | Files | Total |
|----------|-------|-------|
| Delete | 1 | VILLAGE.md |
| Major rewrite | 8 | Core skill references |
| High ref count | 9 | Additional skill references |
| SA/MA/Mode refs | 18 | Skill updates |
| Village/bv refs | 10 | Skill updates |
| Graceful fallback | 4 | orchestrator/maestro-core patterns |
| Team/role concepts | 6 | beads references |
| Validation/structure | 2 | validation, writing-skills |
| Root docs | 13 | REFERENCE, ARCHITECTURE, SETUP_GUIDE, etc. |
| Templates | 2 | workflow.md, SETUP.md |
| Scripts | 2 | validate-*.sh |
| Global config | 1 | ~/.config/amp/AGENTS.md |

**Grand Total: ~55 files to modify (excluding archive)**

---

## Out of Scope

- Changing the orchestrator's wave execution logic
- Modifying TDD checkpoint behavior
- Changing beads CLI (`bd`) commands

## Next Steps

After design approval:
1. `cn` - Create spec + plan from this design
2. `fb` - File beads for each migration task
3. `ci` - Execute (ironically, in the new unified mode once implemented)
