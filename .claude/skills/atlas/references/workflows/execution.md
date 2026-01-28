# Execution Workflow

## Overview

After planning is complete, `/atlas-work` triggers the execution phase.

```
/atlas-work
    ↓
┌─────────────────┐
│  LOAD PLAN      │  ← Read .claude/plans/{name}.md
└────────┬────────┘
         ↓
┌─────────────────┐
│ INIT BOULDER    │  ← Create boulder.json state
└────────┬────────┘
         ↓
┌─────────────────┐
│  ORCHESTRATOR   │  ← Delegation-only mode
└────────┬────────┘
         │
         ├──→ Task() → ATLAS-LEVIATHAN (implementation)
         ├──→ Task() → ATLAS-KRAKEN (TDD work)
         └──→ Task() → ATLAS-SPARK (quick fixes)
         ↓
┌─────────────────┐
│    VERIFY       │  ← Orchestrator verifies all work
└────────┬────────┘
         ↓
┌─────────────────┐
│   COMPLETE      │  ← Update plan, sync boulder
└─────────────────┘
```

## Agent Selection

| Task Characteristics | Agent | Rationale |
|---------------------|-------|-----------|
| TDD, refactoring, complex | `atlas-kraken` | Red-Green-Refactor cycle |
| Quick fix, simple change | `atlas-spark` | Minimal overhead |
| General implementation | `atlas-leviathan` | Default executor |

## State Files

| File | Purpose |
|------|---------|
| `.atlas/boulder.json` | Active execution state |
| `.atlas/notepads/{plan}/` | Wisdom accumulation |

## Triggers

| Trigger | Action |
|---------|--------|
| `/atlas-work` | Begin execution |
| `/atlas-work {plan}` | Execute specific plan |

## Verification

Orchestrator verifies EVERY task:
- Read modified files
- Run test commands
- Validate acceptance criteria

**CRITICAL**: Subagents lie. Never trust "I'm done."
