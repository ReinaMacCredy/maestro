# Plan: Unified DS Pipeline

## Epic Structure

```
EPIC-1: Core Pipeline Infrastructure
├── TASK-1.1: Create unified-pipeline.md reference doc
├── TASK-1.2: Update SKILL.md with 8-phase model
├── TASK-1.3: Add pipeline section to metadata schema
└── TASK-1.4: Update session-init.md for INIT preflight

EPIC-2: Research Consolidation
├── TASK-2.1: Create research-start.md hook
├── TASK-2.2: Create research-verify.md hook
├── TASK-2.3: Deprecate old hooks (discover-hook, grounding-hook)
└── TASK-2.4: Update research protocol.md

EPIC-3: Phase Transitions & A/P/C
├── TASK-3.1: Update apc-checkpoints.md for phases 1-4
├── TASK-3.2: Implement auto-planning gate (before Phase 5)
├── TASK-3.3: Implement Oracle revision loop
└── TASK-3.4: Add phase progress indicator

EPIC-4: Legacy & Compatibility
├── TASK-4.1: Deprecate double-diamond.md (redirect)
├── TASK-4.2: Deprecate planning/pipeline.md (redirect)
├── TASK-4.3: Update `pl` as alias for phases 5-8
└── TASK-4.4: Update maestro-core routing table

EPIC-5: Validation & Testing
├── TASK-5.1: Test SPEED mode flow (1→2→4→8)
├── TASK-5.2: Test FULL mode flow (1→8)
├── TASK-5.3: Test `pl` standalone with existing design.md
└── TASK-5.4: Test Oracle revision loop
```

## Implementation Order

### Wave 1: Foundation (Independent)

| Task | File | Scope | Dependencies |
|------|------|-------|--------------|
| TASK-1.1 | `skills/design/references/unified-pipeline.md` | NEW | None |
| TASK-1.3 | `skills/conductor/references/schemas/metadata.schema.json` | ADD pipeline section | None |
| TASK-2.1 | `skills/conductor/references/research/hooks/research-start.md` | NEW | None |
| TASK-2.2 | `skills/conductor/references/research/hooks/research-verify.md` | NEW | None |

### Wave 2: Core Updates (Depends on Wave 1)

| Task | File | Scope | Dependencies |
|------|------|-------|--------------|
| TASK-1.2 | `skills/design/SKILL.md` | UPDATE | TASK-1.1 |
| TASK-1.4 | `skills/design/references/session-init.md` | UPDATE | TASK-1.1 |
| TASK-2.4 | `skills/conductor/references/research/protocol.md` | UPDATE | TASK-2.1, TASK-2.2 |
| TASK-3.1 | `skills/design/references/apc-checkpoints.md` | UPDATE | TASK-1.1 |

### Wave 3: Transitions & UX (Depends on Wave 2)

| Task | File | Scope | Dependencies |
|------|------|-------|--------------|
| TASK-3.2 | `skills/design/references/unified-pipeline.md` | ADD section | TASK-1.2 |
| TASK-3.3 | `skills/design/references/unified-pipeline.md` | ADD section | TASK-1.2 |
| TASK-3.4 | `skills/design/references/unified-pipeline.md` | ADD section | TASK-1.2 |

### Wave 4: Deprecation & Routing (Depends on Wave 2)

| Task | File | Scope | Dependencies |
|------|------|-------|--------------|
| TASK-2.3 | `skills/conductor/references/research/hooks/` | DEPRECATE | TASK-2.4 |
| TASK-4.1 | `skills/design/references/double-diamond.md` | DEPRECATE | TASK-1.1 |
| TASK-4.2 | `skills/design/references/planning/pipeline.md` | DEPRECATE | TASK-1.1 |
| TASK-4.3 | `skills/design/SKILL.md` | UPDATE pl section | TASK-1.2 |
| TASK-4.4 | `skills/maestro-core/SKILL.md` | UPDATE routing | TASK-1.2 |

### Wave 5: Validation (Depends on Wave 4)

| Task | Scope | Dependencies |
|------|-------|--------------|
| TASK-5.1 | Manual test: `ds` in SPEED mode | All prior |
| TASK-5.2 | Manual test: `ds` in FULL mode | All prior |
| TASK-5.3 | Manual test: `pl` standalone | TASK-4.3 |
| TASK-5.4 | Manual test: Oracle NEEDS_REVISION | TASK-3.3 |

## Task Details

### TASK-1.1: Create unified-pipeline.md

**File:** `skills/design/references/unified-pipeline.md`

**Content:**
- 8-phase overview diagram
- Phase-by-phase details (purpose, inputs, outputs, checkpoints)
- Mode comparison (SPEED vs FULL)
- Context flow specification
- State machine transitions

**Acceptance:**
- [ ] All 8 phases documented
- [ ] SPEED/FULL paths clear
- [ ] Context object defined

---

### TASK-1.2: Update SKILL.md

**File:** `skills/design/SKILL.md`

**Changes:**
- Replace "Double Diamond (ds)" section with "Unified Pipeline"
- Update phase table (4 phases → 8 phases)
- Update entry points (remove separate pl trigger)
- Update "Session Flow" to reference unified-pipeline.md
- Add mode routing (complexity scoring)

**Acceptance:**
- [ ] 8 phases listed
- [ ] unified-pipeline.md referenced
- [ ] pl described as alias

---

### TASK-1.3: Add pipeline to metadata schema

**File:** `skills/conductor/references/schemas/metadata.schema.json`

**Changes:**
- Add `pipeline` object with:
  - `version`, `current_phase`, `mode`
  - `preflight_completed`, `started_at`
  - `research.start`, `research.verify`
  - `validation.checkpoints_passed`, `oracle_verdict`, `retries`

**Acceptance:**
- [ ] Schema validates sample pipeline object
- [ ] All fields documented

---

### TASK-1.4: Update session-init.md

**File:** `skills/design/references/session-init.md`

**Changes:**
- Rename section to "PREFLIGHT (INIT)"
- Add complexity scoring step
- Add mode determination (SPEED/FULL)
- Add pipeline context initialization

**Acceptance:**
- [ ] INIT is preflight, not phase 0
- [ ] Mode routing documented

---

### TASK-2.1: Create research-start.md

**File:** `skills/conductor/references/research/hooks/research-start.md`

**Content:**
- Trigger: Phase 1 (DISCOVER) start
- Agents: Locator + Pattern + CODEMAPS + Architecture
- Timeout: 20s
- Output format: `pipeline_context.research.start`
- Merge of: discover-hook + PL Discovery

**Acceptance:**
- [ ] All 4 agents documented
- [ ] Output schema defined
- [ ] Progressive rendering mentioned

---

### TASK-2.2: Create research-verify.md

**File:** `skills/conductor/references/research/hooks/research-verify.md`

**Content:**
- Trigger: Phase 3→4 (DEVELOP→VERIFY)
- Agents: Analyzer + Pattern + Impact + Web
- Timeout: 15s
- Skip in: SPEED mode
- Output format: `pipeline_context.research.verify`
- Confidence levels: HIGH/MEDIUM/LOW

**Acceptance:**
- [ ] Confidence enforcement documented
- [ ] SPEED mode skip noted

---

### TASK-3.1: Update apc-checkpoints.md

**File:** `skills/design/references/apc-checkpoints.md`

**Changes:**
- Update phase numbers (1-4 instead of DS phases)
- Remove references to separate PL
- Add Phase 4 Oracle-before-A/P/C behavior
- Add "Phases 5-8 automated" note

**Acceptance:**
- [ ] Phases 1-4 have A/P/C
- [ ] Phases 5-8 noted as automated

---

### TASK-3.2: Add auto-planning gate

**File:** `skills/design/references/unified-pipeline.md`

**Add section:**
- Gate shown before Phase 5
- Options: [C] Continue, [M] Manual, [P] Preview
- Default: [C] after 30s

---

### TASK-3.3: Add Oracle revision loop

**File:** `skills/design/references/unified-pipeline.md`

**Add section:**
- NEEDS_REVISION behavior
- [R] Revise, [S] Skip, [A] Abort options
- Max 2 retries
- Line reference display format

---

### TASK-4.4: Update maestro-core routing

**File:** `skills/maestro-core/SKILL.md`

**Changes:**
- Update `ds` description: "Unified 8-phase pipeline"
- Update `pl` description: "Alias for phases 5-8 (requires design.md)"
- Add deprecation note for pl standalone

**Acceptance:**
- [ ] Routing table updated
- [ ] Deprecation noted

## Track Assignments

| Track | Agent | Tasks | File Scope |
|-------|-------|-------|------------|
| A | BlueLake | 1.1, 1.2, 1.4 | `skills/design/**` |
| B | GreenCastle | 1.3, 2.1, 2.2, 2.3, 2.4 | `skills/conductor/references/**` |
| C | RedStone | 3.1, 3.2, 3.3, 3.4 | `skills/design/references/apc-*.md`, `unified-pipeline.md` |
| D | PurpleBear | 4.1, 4.2, 4.3, 4.4 | Deprecation + routing |
| E | Manual | 5.1, 5.2, 5.3, 5.4 | Testing (no files) |

## Definition of Done

- [ ] All 10 files modified/created per design.md Section 5
- [ ] `ds` runs unified pipeline by default
- [ ] SPEED mode works (phases 1,2,4,8)
- [ ] FULL mode works (all 8 phases)
- [ ] `pl` works as alias for phases 5-8
- [ ] Research completes in < 40s
- [ ] Oracle revision loop functional
- [ ] All tests pass (Wave 5)
