# Planning Pipeline (Phases 5-10)

> **6-phase automated pipeline from validated design through track completion.**

The `pl` trigger runs phases 5-10 with flexible input sources.

## ⚠️ MANDATORY: Input Detection & Track Bootstrap

**Before running Phase 5, YOU MUST detect input and bootstrap track if needed:**

```python
# REQUIRED - Input detection at pl entry:
def detect_pl_input():
    # Priority 1: design.md from ds (phases 1-4)
    if file_exists("conductor/tracks/<track-id>/design.md"):
        return "ALIAS", "conductor/tracks/<track-id>/design.md"
    
    # Priority 2: PRD file (user-provided)
    if file_exists("conductor/tracks/<track-id>/prd.md"):
        return "STANDALONE", "conductor/tracks/<track-id>/prd.md"
    
    # Priority 3: User provided file in message
    if user_provided_file:
        return "STANDALONE", user_provided_file
    
    # Priority 4: No input - prompt user and bootstrap
    return "BOOTSTRAP", None

mode, input_file = detect_pl_input()

if mode == "BOOTSTRAP":
    # Prompt user for planning context
    print("""
    📋 Planning Pipeline (phases 5-10)
    
    What do you want to plan?
    
    Provide:
    - Feature description
    - Requirements/goals
    - Any constraints
    """)
    
    # Wait for user input
    user_description = get_user_input()
    
    # Create track with artifacts
    track_id = generate_track_id()
    create_track_artifacts(track_id, user_description)
```

## Track Bootstrap (STANDALONE Mode)

When no input exists, **create minimal artifacts before Phase 5**:

```python
def create_track_artifacts(track_id, user_description):
    """Create conductor track with minimal artifacts for pl pipeline."""
    
    track_dir = f"conductor/tracks/{track_id}"
    mkdir(track_dir)
    
    # 1. Create design.md (minimal - from user description)
    create_file(f"{track_dir}/design.md", f"""
# Design: {extract_title(user_description)}

## 1. Problem Statement
{user_description}

## 2. Goals
- [To be refined in Phase 5]

## 3. Approach
- [To be determined after validation]

## 4. Risks
- [To be assessed]
""")
    
    # 2. Create metadata.json
    create_file(f"{track_dir}/metadata.json", {
        "id": track_id,
        "created_at": now(),
        "source": "pl-bootstrap",
        "planning": {
            "state": "decompose",
            "phases_completed": []
        }
    })
    
    # 3. spec.md and plan.md created in later phases
    
    return track_id
```

## Input Source Handling

| Source | Mode | Track Exists? | Phase 5 (DECOMPOSE) |
|--------|------|---------------|---------------------|
| **design.md** | ALIAS | Yes | Use existing design |
| **PRD file** | STANDALONE | Yes | Parse PRD |
| **User description** | BOOTSTRAP | **Create** | Run full pipeline |

### Phase Flow by Mode

```
ALIAS mode (design.md exists):
  Phase 5 → Phase 6 → Phase 7 → Phase 8 → Phase 9 → Phase 10

STANDALONE mode (PRD exists):
  Phase 5 → Phase 6 → Phase 7 → Phase 8 → Phase 9 → Phase 10

BOOTSTRAP mode (no input):
  Prompt → Create artifacts → Phase 5 → Phase 6 → Phase 7 → Phase 8 → Phase 9 → Phase 10
```

**Anti-pattern:** Do NOT skip input detection. Always validate or bootstrap before Phase 5.

## Overview Diagram

```
┌──────────────────────────────────────────────────────────────────────────────────────────┐
│                           PLANNING PIPELINE (6 PHASES)                                   │
├──────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                          │
│       ┌───────────────┐     ┌───────────────┐     ┌───────────────┐     ┌───────────────┐│
│       │  DECOMPOSE    │ ──► │   VALIDATE    │ ──► │    ASSIGN     │ ──► │    READY      ││
│       │   (Phase 5)   │     │   (Phase 6)   │     │   (Phase 7)   │     │   (Phase 8)   ││
│       │      📦       │     │      🔍       │     │      📋       │     │      🚀       ││
│       └───────┬───────┘     └───────┬───────┘     └───────┬───────┘     └───────┬───────┘│
│               │                     │                     │                     │        │
│         fb (file beads)       bv + Oracle           track planning        [O]/[S]/[R] prompt │
│         embed learnings       dependency check      plan.md generation                   │
│                                                                                          │
│       ┌───────────────┐     ┌───────────────┐                                            │
│       │   EXECUTE     │ ──► │    FINISH     │                                            │
│       │   (Phase 9)   │     │  (Phase 10)   │                                            │
│       │      ⚙️       │     │      📁       │                                            │
│       └───────┬───────┘     └───────┬───────┘                                            │
│               │                     │                                                    │
│         ci/co (TDD)         /conductor-finish                                            │
│         implement beads     archive + learnings                                          │
│                                                                                          │
└──────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## Phase 5: DECOMPOSE

| Aspect | Value |
|--------|-------|
| **Purpose** | Create beads from design with embedded spike learnings |
| **Inputs** | Validated design, spec.md, spike results |
| **Outputs** | Filed beads in `.beads/` |
| **Skill** | `tracking` |
| **Command** | `fb` (file beads) |

### ⚠️ MANDATORY: Load Tracking Skill First

**Before running `fb`, YOU MUST load the tracking skill:**

```python
# REQUIRED - Load tracking skill before file-beads:
skill("tracking")  # Loads fb, rb, bd commands

# Then file beads with spike learnings
fb  # Creates beads from plan with embedded learnings
```

**Why?** The tracking skill provides:
- `fb` command for filing beads
- `rb` command for reviewing beads
- `bd` commands for bead operations
- Spike learnings injection into bead descriptions

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

## Phase 6: VALIDATE

| Aspect | Value |
|--------|-------|
| **Purpose** | Dependency validation + Oracle review of beads |
| **Inputs** | Filed beads from Phase 5 |
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
| **APPROVED** | Continue to Phase 7 |
| **NEEDS_REVISION** | Fix beads, re-validate |
| **Cycle detected** | Auto-fix or HALT |

---

## Phase 7: ASSIGN

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

---

## Phase 8: READY

| Aspect | Value |
|--------|-------|
| **Purpose** | Handoff to implementation |
| **Inputs** | Assigned tracks |
| **Outputs** | Execution prompt |

### Execution Prompt

After track planning:

```
✅ Planning complete. Ready for execution.

Tracks: 3
Beads: 5
Estimated time: 2h

[O] Orchestrate (parallel execution)
[S] Sequential (single agent)
[R] Ralph (autonomous loop - ca)

Default: [O] after 30s
```

> **Note:** `[R]` is available when `ralph.enabled == true` in track `metadata.json`.

---

## Tool Dependencies

| Phase | Tools |
|-------|-------|
| DECOMPOSE | `fb`, `bd create` |
| VALIDATE | `bv --robot-*`, Oracle |
| ASSIGN | `bv --robot-plan` |
| READY | None (prompt only) |
| EXECUTE | `ci`, `co`, `bd update/close` |
| FINISH | `/conductor-finish`, `bd sync` |

---

## Timeout and Fallback Summary

| Phase | Timeout | Fallback |
|-------|---------|----------|
| DECOMPOSE | None | Manual `fb` command |
| VALIDATE | 30s | HALT (validation required) |
| ASSIGN | None | Manual assignment |
| READY | None | N/A |
| EXECUTE | None | Manual `ci` per track |
| FINISH | None | Manual archive |

---

## State Transitions

```
DECOMPOSE ──► VALIDATE ──► ASSIGN ──► READY ──► EXECUTE ──► FINISH
    │             │           │          │          │          │
    ▼             ▼           ▼          ▼          ▼          ▼
  beads       validated    tracks     [O]/[S]/[R]    beads      track
  filed        beads      assigned    prompt    completed   archived
```

### metadata.json Tracking

```json
{
  "planning": {
    "state": "finish",
    "phases_completed": ["decompose", "validate", "assign", "ready", "execute", "finish"],
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
