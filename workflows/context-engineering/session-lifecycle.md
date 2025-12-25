<!-- session-lifecycle v1 -->

# Session Lifecycle

Context management across session boundaries using RECALL and ROUTE phases.

## Overview

Every session follows a lifecycle:

```
┌─────────┐     ┌─────────┐     ┌─────────────┐
│ RECALL  │ ──► │  ROUTE  │ ──► │  Execution  │
└─────────┘     └─────────┘     └─────────────┘
```

---

## RECALL Phase

Load prior session context from `.conductor/session-context.md`.

### Step 1: Check File Existence

```bash
if [ -f ".conductor/session-context.md" ]; then
    # Load context
else
    # Cold start
fi
```

### Step 2: Load and Validate Context Contract

Parse the context file for required fields:

| Field | Required | Description |
|-------|----------|-------------|
| `Intent` | Yes | What the session is trying to accomplish |
| `Track ID` | Yes | Current track identifier |
| `Decisions` | Yes | Key decisions made in prior sessions |

**Validation:**
- All required fields must be present
- Track ID must match an existing track in `conductor/tracks/`
- Intent must be non-empty

### Step 3: Display Token Budget

Calculate and display context utilization:

```
┌─────────────────────────────────────┐
│          Token Budget               │
├─────────────────┬───────────────────┤
│ Available       │ 128,000           │
│ Prompt          │  42,000           │
│ Reserved        │  10,000           │
│ Usable          │  76,000 (59%)     │
└─────────────────┴───────────────────┘
```

### Step 4: Token Threshold Warnings

| Usable % | Action |
|----------|--------|
| ≥20% | Proceed normally |
| <20% | ⚠️ WARN: Low context budget. Consider compacting session-context.md |
| <10% | 🛑 FORCE: Compress context before proceeding. Archive stale decisions. |

### Step 5: Cold Start (Missing Context)

If `.conductor/session-context.md` does not exist, create skeleton:

```markdown
# Session Context

## Intent
<!-- Describe what this session should accomplish -->

## Track ID
<!-- Current track: e.g., feature-auth_20251226 -->

## Decisions
<!-- Key decisions from prior sessions -->
- None yet

## Notes
<!-- Session-specific context -->
```

---

## ROUTE Phase

Decision tree for routing to design vs execution workflows.

### Routing Logic

```
┌──────────────────────┐
│   Analyze Intent     │
└──────────┬───────────┘
           │
     ┌─────▼─────┐
     │ Has spec? │
     └─────┬─────┘
           │
    No ────┴──── Yes
    │             │
    ▼             ▼
┌────────┐   ┌──────────────┐
│ DESIGN │   │ Has beads?   │
└────────┘   └──────┬───────┘
                    │
             No ────┴──── Yes
             │             │
             ▼             ▼
        ┌────────┐   ┌───────────┐
        │ DESIGN │   │ EXECUTION │
        └────────┘   └───────────┘
```

### Design Routing

When intent requires exploration, design, or specification:

→ Reference: [design-routing-heuristics.md](design-routing-heuristics.md)

**Triggers:**
- No spec.md exists for the intent
- User explicitly requests design session (`ds`)
- Intent contains exploratory language ("how should we...", "what's the best way...")
- Significant ambiguity in requirements

### Execution Routing

When intent has clear implementation path:

→ Reference: [execution-routing.md](execution-routing.md)

**Triggers:**
- spec.md and plan.md exist
- Beads are filed and ready
- Intent matches existing track work
- User explicitly requests implementation

---

## Integration Points

### preflight-beads.md: RECALL Hook

The RECALL phase integrates with preflight as the first step:

```
Preflight Sequence:
1. RECALL ← Load session-context.md
2. Mode detect (SA/MA)
3. Validate bd availability
4. Create/update session state
```

Location: [../conductor/preflight-beads.md](../conductor/preflight-beads.md)

### implement.md: ROUTE Evaluation at Phase 2b

During implementation, ROUTE is re-evaluated at Phase 2b to ensure correct path:

```
/conductor-implement Phases:
├── Phase 1: Preflight (includes RECALL)
├── Phase 2a: Load track context
├── Phase 2b: ROUTE evaluation ← Verify execution path is correct
├── Phase 3: TDD execution
└── Phase 4: Close and sync
```

**Phase 2b Checks:**
- Confirm spec/plan alignment with intent
- Detect if design revision needed (→ /conductor-revise)
- Validate beads are filed and ready

Location: [../implement.md](../implement.md)

---

## Session Context File Format

`.conductor/session-context.md` structure:

```markdown
# Session Context

## Intent
Implement user authentication with OAuth2 providers.

## Track ID
auth-oauth2_20251226

## Decisions
- Use Google and GitHub as initial providers
- Store tokens in encrypted session storage
- Implement refresh token rotation

## Prior Sessions
| Date | Thread | Summary |
|------|--------|---------|
| 2025-12-25 | T-abc123 | Designed OAuth flow |
| 2025-12-26 | T-def456 | Implemented Google provider |

## Notes
- Blocked: Waiting on API keys for GitHub
- Next: Complete GitHub provider once unblocked
```

---

## Error Handling

| Error | Recovery |
|-------|----------|
| Corrupted session-context.md | Backup and recreate skeleton |
| Track ID not found | Prompt user to select valid track |
| Missing required fields | Fill from track metadata if available |
| Token budget exceeded | Force archive of old decisions |
