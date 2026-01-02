# A/P/C Checkpoints

The Adaptive A/P/C system provides design checkpoints across the entire workflow, not just in FULL DS mode.

## State Machine Overview

See [adaptive-apc-system.ts](adaptive-apc-system.ts) for full implementation.

```
INLINE ──┬──[checkpoint]──→ MICRO_APC ──[A/P]──→ DS_FULL
         │                      │                   │
         │                    [C]                   │
         │                      ↓                   │
         ├──[3+ iterations]──→ NUDGE ──[accept]────→┤
         │                                          │
         ├──[design rethink]────────────────→ DS_BRANCH
         │                                          │
         │←─────────────[complete]─────────────────←┤
```

---

## Micro A/P/C (INLINE → MICRO_APC)

Lightweight checkpoints shown at natural boundaries **outside** of a full Design Session.

### When to Show

- End of spec section
- After summarizing requirements
- After generating plan steps
- After inline design discussion

### Prompt Template

```
Design checkpoint:
[A] Advanced – deeper exploration (upgrades to DS)
[P] Party – multi-perspective feedback (upgrades to DS)
[C] Continue inline
```

### With Branch Option (in implementation context)

```
Design checkpoint (changes diverge from current design):
[A] Explore alternatives in a design branch
[P] Get opinions first
[C] Continue as-is
```

### Behavior

| Choice | Action |
|--------|--------|
| **[A]** | Transition to `DS_FULL`, start at appropriate phase, run Advanced check |
| **[P]** | Transition to `DS_FULL`, start Party Mode immediately |
| **[C]** | Stay in `INLINE`, set micro cooldown (3 turns) |

---

## Design Mode Nudge (INLINE → NUDGE)

Passive suggestion after repeated design iterations without resolution.

### Trigger Conditions

- 3+ user turns tagged `ux_flow` or `design_iteration` in last 6 turns
- Not in active DS
- Not in nudge cooldown (10 turns)

### Detection Patterns

```
/try (another|different)/i
/what if/i
/iterate/i
/rework/i
/still not (sure|right)/i
/this flow feels wrong/i
```

### Prompt Template

```
We've iterated on this flow several times.
Want to switch into a structured Design Session with A/P/C checkpoints?

[Start Design Session] (recommended)
[Not now]
```

### Behavior

| Choice | Action |
|--------|--------|
| **Accept** | Transition to `DS_FULL`, seed with conversation context |
| **Decline** | Stay in `INLINE`, set nudge cooldown (10 turns) |

---

## Branch-aware DS (DS_BRANCH)

Design exploration that's attached to a branch, keeping original track intact.

### Trigger Conditions

- In implementation (`ci`) with active track
- User expresses design rethink intent:
  - "this flow feels wrong"
  - "we need to rethink the UX"
  - "designed this wrong"

### Prompt Template (on detection)

```
You're signaling the existing design doesn't feel right.
[A] Start a design branch – explore alternatives safely
[P] Get opinions first
[C] Keep current plan
```

### Branch Lifecycle

1. **Create**: Fork design branch linked to `activeTrackId`
2. **Execute**: Full Double Diamond with A/P/C (same as DS_FULL)
3. **Merge**: Choose how to apply changes

### Merge Options (BRANCH_MERGE)

```
Design branch complete. How to apply?

[M1] Replace current design/plan for this track
[M2] Create new implementation track
[M3] Keep as documented alternative (no changes yet)
[Cancel] Discard branch
```

| Choice | Action |
|--------|--------|
| **M1** | Overwrite spec/plan, tag affected beads for review |
| **M2** | Create new track via `cn`, link both tracks |
| **M3** | Save as `design/track-{id}-branch-{branch_id}.md` |
| **Cancel** | Discard branch, return to original track |

---

## A/P/C in DS (DS_FULL / DS_BRANCH)

At the end of each phase, present the checkpoint menu:

```
📍 End of [PHASE] phase.

Choose:
[A] Advanced - deeper analysis, assumption audit
[P] Party - multi-perspective feedback from expert agents
[C] Continue - proceed to next phase
[↩ Back] - return to previous phase
```

### [A] Advanced Mode

Phase-specific deep dives:

| Phase | Focus Areas |
|-------|-------------|
| **DISCOVER** | Challenge assumptions, explore biases, consider alternative users |
| **DEFINE** | Stress-test scope, challenge metrics, identify hidden dependencies |
| **DEVELOP** | Deep-dive components, explore alternatives, security/performance review |
| **DELIVER** | Edge case audit, security check, documentation completeness |

### [P] Party Mode

Invokes multi-agent collaborative review using BMAD v6 integration. See [bmad/workflows/party-mode/workflow.md](bmad/workflows/party-mode/workflow.md).

#### 25 Agents Available

- **Core (1):** BMad Master (🧙) - Orchestrator
- **BMM (9):** PM, Analyst, Architect, Developer, Scrum Master, Test Architect, UX Designer, Tech Writer, Quick Flow Solo Dev
- **CIS (6):** Brainstorming Coach, Problem Solver, Design Thinking Coach, Innovation Strategist, Presentation Master, Storyteller
- **BMB (3):** Agent Builder, Module Builder, Workflow Builder
- **BMGD (6):** Game Architect, Game Designer, Game Dev, Game QA, Game Scrum Master, Game Solo Dev

#### Agent Selection

BMad Master selects 2-3 agents based on topic relevance:
- **Primary:** Best expertise match
- **Secondary:** Complementary perspective
- **Tertiary:** Devil's advocate

### [C] Continue

Proceed to the next phase. **Validation runs at each checkpoint; research runs at session start and CP3.**

### CP4 (DELIVER) Special Behavior

At CP4, **Oracle audit runs automatically before showing the A/P/C menu**:

1. Detect platform (Amp or Claude Code/Gemini/Codex)
2. Invoke Oracle review (6-dimension audit)
3. Oracle appends `## Oracle Audit` section to design.md
4. Based on verdict:
   - **APPROVED**: Show A/P/C menu as normal
   - **NEEDS_REVISION + FULL mode**: HALT - display issues, prompt to fix before proceeding
   - **NEEDS_REVISION + SPEED mode**: WARN - log issues, show A/P/C menu

```
📍 End of DELIVER phase.

Oracle Audit: ✅ APPROVED (or ⚠️ NEEDS_REVISION)

[Show A/P/C menu]
```

See [validation/lifecycle.md](../../conductor/references/validation/lifecycle.md) for checkpoint-specific validation.

### [↩ Back]

Return to previous phase. When looping back:
1. Summarize what was established
2. Ask what to reconsider
3. Update subsequent phases if decisions change

---

## Priority Rules

When multiple triggers are eligible, apply this priority (highest first):

1. **Explicit commands** (`ds`, `/conductor-design`) → always win
2. **Active DS/Branch** → block all passive triggers
3. **Branch safety** (in `ci` + design shift) → prefer DS_BRANCH
4. **Micro A/P/C** → at checkpoint boundaries, not in cooldown
5. **Nudge** → after 3+ iterations, not in cooldown
6. **Inline hints** → suppressed if any above pending

---

## Cooldowns

| Type | Duration | Scope |
|------|----------|-------|
| Micro | 3 turns | Per topic |
| Nudge | 10 turns | Per topic |

Cooldowns are topic-scoped so different workstreams don't interfere.
