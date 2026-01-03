---
name: design
description: Design Session - collaborative brainstorming to turn ideas into designs using Double Diamond methodology OR 6-phase planning pipeline. Use when user types "ds", "pl", "/plan", or wants to explore/design a feature before implementation. MUST load maestro-core skill first for routing.
---

# Design & Planning

Turn ideas into fully-formed designs through collaborative dialogue or structured planning.

## Entry Points

| Trigger | Action |
|---------|--------|
| `ds` | Start design session (Double Diamond) |
| `/conductor-design` | Start design session (alias) |
| `pl`, `/plan` | Start planning pipeline (6-phase) |
| "design a feature" | Start design session |
| "let's think through X" | Start design session |
| "plan feature X" | Start planning pipeline |

## Quick Reference

### Double Diamond (ds)

| Phase | Purpose | Exit Criteria |
|-------|---------|---------------|
| DISCOVER | Explore problem | Problem articulated |
| DEFINE | Frame problem | Approach selected |
| DEVELOP | Explore solutions | Interfaces defined |
| DELIVER | Finalize design | Design verified |

### Planning Pipeline (pl)

| Phase | Tool | Output |
|-------|------|--------|
| 1. Discovery | Parallel Task() agents | design.md Section 2 |
| 2. Synthesis | Oracle | design.md Section 3 (Gap + Risk Map) |
| 3. Verification | Spikes via Task() | design.md Section 5 |
| 4. Decomposition | fb (file-beads) | .beads/*.md |
| 5. Validation | bv + Oracle | Validated dependency graph |
| 6. Track Planning | bv --robot-plan | plan.md Track Assignments |

See [planning/pipeline.md](references/planning/pipeline.md) for full details.

## Core Principles

- **One question at a time** - Don't overwhelm
- **Multiple choice preferred** - Easier to answer
- **YAGNI ruthlessly** - Remove unnecessary features
- **Explore alternatives** - Always propose 2-3 approaches
- **Research everything** - Verify with parallel agents before finalizing

## Session Flow

0. **Load Core** - Load [maestro-core](../maestro-core/SKILL.md) for routing table and fallback policies
1. **Initialize** - Load handoffs, CODEMAPS, verify conductor setup → [session-init.md](references/session-init.md)
2. **Research** - Spawn research agents BEFORE DISCOVER (mandatory) → [research-verification.md](references/research-verification.md)
3. **Route** - Score complexity (< 4 = SPEED, > 6 = FULL) → [design-routing-heuristics.md](references/design-routing-heuristics.md)
4. **Execute** - Double Diamond phases with A/P/C checkpoints → [double-diamond.md](references/double-diamond.md)
5. **Validate** - Progressive validation at each checkpoint (CP1-4); **Oracle audit at CP4** → [validation/lifecycle.md](../conductor/references/validation/lifecycle.md)
6. **Handoff** - Suggest next steps: `cn` (newtrack), `ci` (implement), `fb` (file beads)

### Research & Validation Triggers

| Trigger Point | Research | Validation |
|---------------|----------|------------|
| Session start | discover-hook (Locator + Pattern + CODEMAPS) | - |
| CP1 (DISCOVER) | - | WARN (product alignment) |
| CP2 (DEFINE) | - | WARN (problem clarity) |
| CP3 (DEVELOP) | grounding-hook (Locator + Analyzer + Pattern) | WARN (tech-stack) |
| CP4 (DELIVER) | Full + impact scan + **Oracle audit** | SPEED=WARN, FULL=HALT |

## Adaptive A/P/C System

A/P/C checkpoints now work **adaptively** across the entire workflow, not just in FULL DS mode.

### State Ladder

```
INLINE → MICRO_APC → NUDGE → DS_FULL → DS_BRANCH → BRANCH_MERGE
```

| State | Description | Trigger |
|-------|-------------|---------|
| **INLINE** | Normal flow (conductor/beads) | Default |
| **MICRO_APC** | Lightweight checkpoint at boundaries | End of spec/plan section |
| **NUDGE** | Suggest upgrade to DS | 3+ design iterations |
| **DS_FULL** | Full Double Diamond with A/P/C | `ds` command or upgrade |
| **DS_BRANCH** | DS attached to design branch | Design rethink in track |
| **BRANCH_MERGE** | Apply branch changes | Branch complete |

### Micro A/P/C (Outside DS)

At natural checkpoint boundaries (end of spec section, plan step, etc.):

```
Design checkpoint:
[A] Advanced – deeper exploration (upgrades to DS)
[P] Party – multi-perspective feedback (upgrades to DS)
[C] Continue inline
```

### Design Mode Nudge

After 3+ iterations on the same design topic without resolution:

```
We've iterated on this flow several times.
Want to switch into a structured Design Session with A/P/C checkpoints?

[Start Design Session] (recommended)
[Not now]
```

### Branch-aware DS

When in implementation (`ci`) and design needs major rethink:

```
This change diverges from the original design.
[A] Explore alternatives in a design branch
[P] Get opinions first
[C] Keep current plan
```

Branch merge options at completion:
- **[M1]** Replace current design/plan
- **[M2]** Create new implementation track
- **[M3]** Keep as documented alternative

### A/P/C in DS (FULL mode)

At end of each phase:

- **[A] Advanced** - Phase-specific deep dive
- **[P] Party** - Multi-agent feedback (BMAD v6) → [bmad/](references/bmad/)
- **[C] Continue** - Proceed to next phase
- **[↩ Back]** - Return to previous phase

### Priority Rules

1. **Explicit commands** (`ds`) always win
2. **Active DS/Branch** blocks passive triggers
3. **Branch safety** preferred when in implementation
4. **Micro A/P/C** at checkpoint boundaries
5. **Nudge** after 3+ iterations

See [apc-checkpoints.md](references/apc-checkpoints.md) and [adaptive-apc-system.ts](references/adaptive-apc-system.ts) for implementation details.

## Mode Comparison

| Aspect | SPEED (< 4) | FULL (> 6) |
|--------|-------------|------------|
| Phases | 1 (quick) | 4 (all) |
| A/P/C | No | Yes |
| Verification | Advisory | Mandatory |
| Use `[E]` to escalate | Yes | N/A |

## Anti-Patterns

- ❌ Jumping to solutions before understanding the problem
- ❌ Skipping verification at DELIVER phase
- ❌ Asking multiple questions at once
- ❌ Over-engineering simple features (use SPEED mode)

## Next Steps (after design.md created)

| Command | Description |
|---------|-------------|
| `cn` | `/conductor-newtrack` - Create spec + plan from design |
| `ci` | `/conductor-implement` - Execute track |
| `fb` | File beads from plan |

See [maestro-core](../maestro-core/SKILL.md) for full routing table.

## Dependencies

**Auto-loads:** [maestro-core](../maestro-core/SKILL.md) for routing and fallback policies.

## Related

- [conductor](../conductor/SKILL.md) - Track creation and implementation
- [beads](../beads/SKILL.md) - Issue tracking after design
