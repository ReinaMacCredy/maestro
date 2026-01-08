# Planning Pipeline (Phases 5-10)

> **6-phase automated pipeline from validated design to execution-ready state.**

The `pl` trigger activates when `design.md` exists and runs phases 5-10 of the unified pipeline.

## Overview Diagram

```
┌──────────────────────────────────────────────────────────────────────────────────────────┐
│                           PLANNING PIPELINE (6 PHASES)                                   │
├──────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                          │
│       ┌───────────────┐     ┌───────────────┐     ┌───────────────┐                     │
│       │   DISCOVERY   │ ──► │  SYNTHESIS    │ ──► │ VERIFICATION  │                     │
│       │   (Phase 5)   │     │   (Phase 6)   │     │   (Phase 7)   │                     │
│       │      🔬       │     │      🧬       │     │      ⚗️       │                     │
│       └───────┬───────┘     └───────┬───────┘     └───────┬───────┘                     │
│               │                     │                     │                              │
│         finder, web_search    Oracle gap analysis   Task() spikes                        │
│         Librarian (15s)       risk-map.md           spike learnings                      │
│               │                spec.md                    │                              │
│               ▼                     ▼                     ▼                              │
│       ┌───────────────┐     ┌───────────────┐     ┌───────────────┐                     │
│       │ DECOMPOSITION │ ──► │  VALIDATION   │ ──► │TRACK PLANNING │                     │
│       │   (Phase 8)   │     │   (Phase 9)   │     │  (Phase 10)   │                     │
│       │      📦       │     │      🔍       │     │      📋       │                     │
│       └───────┬───────┘     └───────┬───────┘     └───────┬───────┘                     │
│               │                     │                     │                              │
│         bd create               bv --robot-*        plan.md Track                        │
│         file-beads              Oracle review       Assignments table                    │
│         embed learnings             │                     │                              │
│               │                     │                     │                              │
│               └─────────────────────┴─────────────────────┘                              │
│                                     │                                                    │
│                                     ▼                                                    │
│                          ┌───────────────────┐                                          │
│                          │       READY       │                                          │
│                          │   [O] Orchestrate │                                          │
│                          │   [S] Sequential  │                                          │
│                          └───────────────────┘                                          │
│                                                                                          │
└──────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## Phase 5: DISCOVERY

| Aspect | Value |
|--------|-------|
| **Purpose** | Parallel research to gather implementation context |
| **Inputs** | design.md sections 1-4 |
| **Outputs** | Research context, file mappings, external references |
| **Timeout** | 15 seconds per agent |
| **Fallback** | Graceful degradation to manual research |

### Parallel Agents

Three agents spawn concurrently:

| Agent | Tool | Purpose | Output |
|-------|------|---------|--------|
| Locator | `finder` | Find relevant codebase files | File paths, line numbers |
| Librarian | `finder` | Identify existing patterns | Pattern examples, utilities |
| Web | `web_search` | External documentation | Library docs, examples |

### Agent Dispatch

```python
# Spawn all three in parallel
Task(description="Locator: Find files for X", prompt="...")
Task(description="Librarian: Find patterns for X", prompt="...")
Task(description="Web: Search docs for X", prompt="...")
```

### Timeout and Fallback Behavior

| Condition | Action |
|-----------|--------|
| All agents complete | Aggregate results, continue |
| Partial timeout | Use available results + log warning |
| All timeout | DEGRADE: prompt for manual research |

```
⚠️ Discovery timeout - partial results available:
• Locator: ✅ 12 files found
• Librarian: ⏱️ timed out
• Web: ✅ 3 references found

[C] Continue with partial results
[M] Manual research
```

---

## Phase 6: SYNTHESIS

| Aspect | Value |
|--------|-------|
| **Purpose** | Gap analysis and risk assessment via Oracle |
| **Inputs** | Discovery results, design.md |
| **Outputs** | risk-map.md, spec.md |
| **Tool** | Oracle |

### Oracle Gap Analysis

```python
oracle(
    task="Gap analysis and risk assessment",
    context="""
    Analyze design.md against discovery results:
    1. Identify gaps between current state and required state
    2. Assess risk level for each component (LOW/MEDIUM/HIGH)
    3. Generate risk-map.md with verification strategy
    4. Draft spec.md with implementation requirements
    """,
    files=[
        "conductor/tracks/<id>/design.md",
        "<discovery_results>"
    ]
)
```

### Risk Assessment Criteria

| Risk Level | Criteria | Verification |
|------------|----------|--------------|
| **LOW** | Pattern exists in codebase | Proceed directly |
| **MEDIUM** | Variation of existing pattern | Interface sketch |
| **HIGH** | Novel or external integration | Spike required |

### Outputs

**risk-map.md:**
```markdown
# Risk Map: <Track ID>

| Component | Risk | Reason | Verification |
|-----------|------|--------|--------------|
| Stripe SDK | HIGH | New external dependency | Spike |
| User entity | LOW | Follows User pattern | Proceed |
| Webhook handler | MEDIUM | Variant of existing | Interface sketch |
```

**spec.md:**
```markdown
# Spec: <Track ID>

## Requirements
- ...

## Interfaces
- ...

## Dependencies
- ...
```

---

## Phase 7: VERIFICATION

| Aspect | Value |
|--------|-------|
| **Purpose** | Validate HIGH risk items via spikes |
| **Inputs** | risk-map.md with HIGH items |
| **Outputs** | Spike results, updated design.md Section 5 |
| **Tool** | `Task()` |
| **Timeout** | 30 minutes default (escalate on timeout) |

### Spike Execution

For each HIGH risk item, spawn a Task():

```python
Task(
    description=f"Spike: {risk_item.question}",
    prompt=f"""
    Time-box: 30 minutes
    Output location: conductor/spikes/<track>/<spike-id>/
    
    Question to answer: {risk_item.question}
    
    Success criteria:
    - Working throwaway code demonstrating feasibility
    - Answer documented: YES (approach works) or NO (blocker found)
    - Learnings captured in LEARNINGS.md
    
    On completion:
    bd close <spike-bead-id> --reason "YES: <approach>" or "NO: <blocker>"
    """
)
```

### Spike Timeout Behavior

| Duration | Use Case |
|----------|----------|
| 15 min | Simple API test |
| 30 min | Integration test (default) |
| 60 min | Complex external integration |

| Result | Action |
|--------|--------|
| **YES** | Proceed with validated approach |
| **NO (alternative)** | Update design with alternative |
| **NO (blocker)** | HALT for user decision |
| **TIMEOUT** | Escalate to user |

```
⏱️ Spike timeout after 30 minutes:
• Question: Can Stripe SDK work with Node 18?
• Partial findings: SDK installs, basic import works

[C] Continue with partial (risky)
[E] Extend time-box (+15 min)
[A] Abort spike, require human review
```

### Spike Learnings Capture

After spike completion, update design.md Section 5:

```markdown
## 5. Spike Results

### Spike: Can Stripe SDK work with Node 18?
- **Result**: YES
- **Bead**: bd-12
- **Learnings**: 
  - Must use `stripe.webhooks.constructEvent()` for signature verification
  - Raw body required (not parsed JSON)
- **Code reference**: conductor/spikes/<track>/spike-001/
- **Impact on approach**: Confirmed, no design changes needed
```

---

## Phase 8: DECOMPOSITION

| Aspect | Value |
|--------|-------|
| **Purpose** | Create beads from design with embedded spike learnings |
| **Inputs** | Validated design, spec.md, spike results |
| **Outputs** | Filed beads in `.beads/` |
| **Tool** | `bd create` |
| **Command** | `fb` (file beads) |

### File-Beads with Learnings

When filing beads, embed spike learnings:

```markdown
# Implement Stripe webhook handler

## Context
Spike bd-12 validated: Stripe SDK works with our Node version.
See `conductor/spikes/billing/spike-001/` for working example.

## Learnings from Spike
- Must use `stripe.webhooks.constructEvent()` for signature verification
- Webhook secret stored in `STRIPE_WEBHOOK_SECRET` env var
- Raw body required (not parsed JSON)

## Acceptance Criteria
- [ ] Webhook endpoint at `/api/webhooks/stripe`
- [ ] Signature verification implemented
- [ ] Events: `checkout.session.completed`, `invoice.paid`
```

### Integration Flow

```
Spike completes → bd close with result
      ↓
Update design.md Section 5
      ↓
fb (file-beads) embeds learnings in beads
      ↓
Worker prompts receive learnings
      ↓
Implementation uses validated approach
```

---

## Phase 9: VALIDATION

| Aspect | Value |
|--------|-------|
| **Purpose** | Dependency validation + Oracle review of beads |
| **Inputs** | Filed beads from Phase 8 |
| **Outputs** | Validated dependencies, Oracle approval |
| **Tools** | `bv --robot-*`, Oracle |

### Bead Validation Commands

```bash
# Check for dependency cycles
bv --robot-check

# Validate bead structure
bv --robot-validate

# Preview dependency graph
bv --robot-graph
```

### Oracle Beads Review

```python
oracle(
    task="Beads review for implementation readiness",
    context="""
    Review filed beads for:
    1. COMPLETENESS - All design components have beads
    2. DEPENDENCIES - Proper ordering, no cycles
    3. SCOPE - Each bead is appropriately sized
    4. CONTEXT - Spike learnings embedded where needed
    
    Return: APPROVED or NEEDS_REVISION
    """,
    files=[".beads/*.md"]
)
```

### Validation Results

| Result | Action |
|--------|--------|
| **APPROVED** | Continue to Phase 10 |
| **NEEDS_REVISION** | Fix beads, re-validate |
| **Cycle detected** | Auto-fix or HALT |

---

## Phase 10: TRACK PLANNING

| Aspect | Value |
|--------|-------|
| **Purpose** | Agent assignment and plan.md generation |
| **Inputs** | Validated beads |
| **Outputs** | plan.md with Track Assignments table |
| **Tool** | `bv --robot-plan` |

### Track Assignment

```bash
# Generate plan with track assignments
bv --robot-plan
```

### plan.md Track Assignments Table

```markdown
## Track Assignments

| Track | Agent | Beads | File Scope | Dependencies |
|-------|-------|-------|------------|--------------|
| A | BlueLake | bd-10, bd-11 | `packages/sdk/**` | None |
| B | GreenCastle | bd-12, bd-13 | `packages/cli/**` | bd-11 |
| C | OrangePond | bd-14 | `schemas/**` | None |
```

### Execution Prompt

After track planning:

```
✅ Planning complete. Ready for execution.

Tracks: 3
Beads: 5
Estimated time: 2h

[O] Orchestrate (parallel execution)
[S] Sequential (single agent)

Default: [O] after 30s
```

---

## Tool Dependencies

| Phase | Tools |
|-------|-------|
| DISCOVERY | `finder`, Librarian, `web_search` |
| SYNTHESIS | Oracle |
| VERIFICATION | `Task()` |
| DECOMPOSITION | `bd create` |
| VALIDATION | `bv --robot-*`, Oracle |
| TRACK PLANNING | `bv --robot-plan` |

---

## Timeout and Fallback Summary

| Phase | Timeout | Fallback |
|-------|---------|----------|
| DISCOVERY | 15s per agent | Graceful degradation, use partial results |
| SYNTHESIS | 30s | HALT (Oracle required) |
| VERIFICATION | 30min per spike | Escalate to user |
| DECOMPOSITION | None | Manual `fb` command |
| VALIDATION | 30s | HALT (validation required) |
| TRACK PLANNING | None | Manual assignment |

---

## State Transitions

```
DISCOVERY ──► SYNTHESIS ──► VERIFICATION ──► DECOMPOSITION ──► VALIDATION ──► TRACK_PLANNING
    │             │              │                │                │                │
    ▼             ▼              ▼                ▼                ▼                ▼
 context      risk-map       spikes           beads         validated          plan.md
 gathered     spec.md        complete         filed          beads           generated
```

### metadata.json Tracking

```json
{
  "planning": {
    "state": "track_planned",
    "phases_completed": ["discovery", "synthesis", "verification", "decomposition", "validation", "track_planning"],
    "spikes": [
      {
        "id": "spike-001",
        "question": "Can Stripe SDK work with Node 18?",
        "result": "YES",
        "path": "conductor/spikes/<track>/spike-001/"
      }
    ]
  }
}
```

---

## Related

| File | Purpose |
|------|---------|
| [spikes.md](spikes.md) | Detailed spike workflow |
| [design-template.md](design-template.md) | Unified design.md template |
| [../pipeline.md](../pipeline.md) | Full 10-phase DS+PL pipeline |
| [../../conductor/SKILL.md](../../conductor/SKILL.md) | Track execution |
| [../../orchestrator/SKILL.md](../../orchestrator/SKILL.md) | Parallel dispatch |
