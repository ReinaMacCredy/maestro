# Design Routing Heuristics

## COMPLEXITY_EXPLAINER for Design Routing

This document defines the weighted scoring system that determines whether a design session uses **SPEED MODE** (1 phase, quick spec) or **FULL MODE** (4 phases, A/P/C checkpoints).

---

## 1. Weighted Scoring Criteria

Maximum possible score: **18 points**

| Factor | Weight | Description |
|--------|--------|-------------|
| Multiple epics | +3 | Work spans multiple epics |
| Cross-module | +2 | Changes touch multiple modules/domains |
| New abstractions | +3 | Creating new patterns/interfaces |
| External deps | +2 | New external dependencies |
| Files > 5 | +1 | Touching more than 5 files |
| Unclear scope | +2 | Scope not well-defined |
| Security/auth | +2 | Involves security or authentication |
| Data migration | +3 | Database or data migration |

---

## 2. Routing Rules

| Score | Route | Description |
|-------|-------|-------------|
| < 4 | **SPEED MODE** | 1 phase, quick spec |
| 4-6 | **ASK USER** | Soft zone, default FULL after 2 prompts |
| > 6 | **FULL MODE** | 4 phases, A/P/C checkpoints |

---

## 3. COMPLEXITY_EXPLAINER Display Format

```
┌─ COMPLEXITY EXPLAINER ─────────────────┐
│ Factor              │ Score │          │
│ Multiple epics      │   0   │          │
│ Cross-module        │   2   │ ✓        │
│ New abstractions    │   0   │          │
│ External deps       │   0   │          │
│ Files > 5           │   1   │ ✓        │
│ Unclear scope       │   2   │ ✓        │
│ Security/auth       │   0   │          │
│ Data migration      │   0   │          │
├─────────────────────┼───────┼──────────┤
│ TOTAL               │   5   │ ASK USER │
└─────────────────────────────────────────┘
```

---

## 4. Soft Zone (Score 4-6) Behavior

When the score falls in the soft zone:

1. **Prompt user**: "Score is X (soft zone). [S]peed or [F]ull?"
2. **Wait for response**
3. **After 2 prompts without response**: Default to FULL MODE

This ensures user agency while preventing analysis paralysis.

---

## 5. Escalation Rules

### During SPEED Mode

- User can type `[E]` at any time to **escalate to FULL MODE**
- Escalation **preserves current progress** (no work lost)
- Session continues from equivalent phase in FULL MODE

### Escalation Flow

```
SPEED MODE (Phase 1)
       │
       ▼ User types [E]
       │
FULL MODE (Phase 2 - DEFINE)
       │
       ▼ Continue with remaining phases
```

---

## 6. Examples

### Example 1: Simple (SPEED)

**Request**: "Add logging to existing function"

```
┌─ COMPLEXITY EXPLAINER ─────────────────┐
│ Factor              │ Score │          │
│ Multiple epics      │   0   │          │
│ Cross-module        │   0   │          │
│ New abstractions    │   0   │          │
│ External deps       │   0   │          │
│ Files > 5           │   0   │          │
│ Unclear scope       │   0   │          │
│ Security/auth       │   0   │          │
│ Data migration      │   0   │          │
├─────────────────────┼───────┼──────────┤
│ TOTAL               │   1   │ SPEED    │
└─────────────────────────────────────────┘
```

**Route**: → SPEED MODE

---

### Example 2: Medium (ASK USER)

**Request**: "Add new API endpoint with auth"

```
┌─ COMPLEXITY EXPLAINER ─────────────────┐
│ Factor              │ Score │          │
│ Multiple epics      │   0   │          │
│ Cross-module        │   0   │          │
│ New abstractions    │   0   │          │
│ External deps       │   0   │          │
│ Files > 5           │   1   │ ✓        │
│ Unclear scope       │   2   │ ✓        │
│ Security/auth       │   2   │ ✓        │
│ Data migration      │   0   │          │
├─────────────────────┼───────┼──────────┤
│ TOTAL               │   5   │ ASK USER │
└─────────────────────────────────────────┘
```

**Route**: → ASK USER ("Score is 5 (soft zone). [S]peed or [F]ull?")

---

### Example 3: Complex (FULL)

**Request**: "New authentication system"

```
┌─ COMPLEXITY EXPLAINER ─────────────────┐
│ Factor              │ Score │          │
│ Multiple epics      │   3   │ ✓        │
│ Cross-module        │   2   │ ✓        │
│ New abstractions    │   3   │ ✓        │
│ External deps       │   0   │          │
│ Files > 5           │   1   │ ✓        │
│ Unclear scope       │   0   │          │
│ Security/auth       │   2   │ ✓        │
│ Data migration      │   0   │          │
├─────────────────────┼───────┼──────────┤
│ TOTAL               │  11   │ FULL     │
└─────────────────────────────────────────┘
```

**Route**: → FULL MODE (4 phases with A/P/C checkpoints)

---

### Example 4: Data-Heavy (FULL)

**Request**: "Migrate user data to new schema"

```
┌─ COMPLEXITY EXPLAINER ─────────────────┐
│ Factor              │ Score │          │
│ Multiple epics      │   0   │          │
│ Cross-module        │   2   │ ✓        │
│ New abstractions    │   0   │          │
│ External deps       │   0   │          │
│ Files > 5           │   1   │ ✓        │
│ Unclear scope       │   2   │ ✓        │
│ Security/auth       │   0   │          │
│ Data migration      │   3   │ ✓        │
├─────────────────────┼───────┼──────────┤
│ TOTAL               │   8   │ FULL     │
└─────────────────────────────────────────┘
```

**Route**: → FULL MODE

---

## Quick Reference

| Score Range | Route | Phases | Checkpoints |
|-------------|-------|--------|-------------|
| 0-3 | SPEED | 1 | None |
| 4-6 | ASK | Depends | Depends |
| 7+ | FULL | 4 | A/P/C at each |
