# Planning Workflow Reference

Complete reference for Atlas's interview-driven planning workflow.

## Overview

```
User Request
    ↓
┌─────────────────────────────────────────────────────────┐
│  PHASE 1: INTERVIEW MODE                                │
│  Prometheus asks questions, updates draft continuously  │
│  Runs clearance checklist after EVERY turn              │
└────────────────────────┬────────────────────────────────┘
                         ↓
              Clearance Check (all YES?)
                         ↓
┌─────────────────────────────────────────────────────────┐
│  PHASE 2: METIS CONSULTATION                            │
│  Gap analysis, guardrail suggestions, AI-slop detection │
└────────────────────────┬────────────────────────────────┘
                         ↓
              Incorporate findings silently
                         ↓
┌─────────────────────────────────────────────────────────┐
│  PHASE 3: PLAN GENERATION                               │
│  Generate to .claude/plans/{name}.md                     │
│  Present summary, offer high accuracy option            │
└────────────────────────┬────────────────────────────────┘
                         ↓
              High Accuracy? ─────────────────────┐
                   │                              │
                   ↓ NO                           ↓ YES
              Ready for                ┌──────────────────────┐
              /atlas-work              │  PHASE 4: MOMUS LOOP │
                                       │  Review → Fix → Loop │
                                       └──────────┬───────────┘
                                                  ↓
                                           OKAY? ─── NO → Fix & Resubmit
                                             │
                                             ↓ YES
                                        Ready for /atlas-work
```

---

## Phase 1: Interview Mode

Prometheus operates in **INTERVIEW** mode by default. The goal is to understand requirements before committing to a plan.

### Draft File

**Location**: `.atlas/drafts/{name}.md`

### Clearance Checklist

Run after **EVERY** turn. All must be YES to proceed:

```
CLEARANCE CHECKLIST:
- [ ] Core objective clearly defined?
- [ ] Scope boundaries established (IN/OUT)?
- [ ] No critical ambiguities remaining?
- [ ] Technical approach decided?
- [ ] Test strategy confirmed (TDD/manual)?
- [ ] No blocking questions outstanding?
```

---

## Phase 3: Plan Generation

### Output Location

`.claude/plans/{name}.md`

---

## Output Files

| File | Purpose | Lifecycle |
|------|---------|-----------|
| `.atlas/drafts/{name}.md` | Interview working memory | Created during interview, deleted after plan |
| `.claude/plans/{name}.md` | Final work plan | Created by Prometheus, consumed by Atlas |

---

## After Plan Completion

```
Plan saved to: .claude/plans/{plan-name}.md
Draft cleaned up: .atlas/drafts/{name}.md (deleted)

To begin execution, run:
  /atlas-work
```

---

## Triggers

| Trigger | Action |
|---------|--------|
| `/ap <request>` | Start interview mode |
| "High accuracy" | Enable Momus review loop |
