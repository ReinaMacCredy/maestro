---
name: design
description: Design Session - collaborative brainstorming to turn ideas into designs using Double Diamond methodology. Use when user types "ds" or wants to explore/design a feature before implementation.
---

# Design Session (ds)

Turn ideas into fully-formed designs through collaborative dialogue.

## Entry Points

| Trigger | Action |
|---------|--------|
| `ds` | Start design session |
| `/conductor-design` | Start design session (alias) |
| "design a feature" | Start design session |
| "let's think through X" | Start design session |

## Quick Reference

| Phase | Purpose | Exit Criteria |
|-------|---------|---------------|
| DISCOVER | Explore problem | Problem articulated |
| DEFINE | Frame problem | Approach selected |
| DEVELOP | Explore solutions | Interfaces defined |
| DELIVER | Finalize design | Design verified |

## Core Principles

- **One question at a time** - Don't overwhelm
- **Multiple choice preferred** - Easier to answer
- **YAGNI ruthlessly** - Remove unnecessary features
- **Explore alternatives** - Always propose 2-3 approaches
- **Research everything** - Verify with parallel agents before finalizing

## Session Flow

1. **Initialize** - Load handoffs, CODEMAPS, verify conductor setup → [session-init.md](references/session-init.md)
2. **Route** - Score complexity (< 4 = SPEED, > 6 = FULL) → [design-routing-heuristics.md](references/design-routing-heuristics.md)
3. **Execute** - Double Diamond phases with A/P/C checkpoints → [double-diamond.md](references/double-diamond.md)
4. **Verify** - Research verification at phase transitions → [research-verification.md](references/research-verification.md)
5. **Handoff** - "Run `/conductor-newtrack` to generate spec, plan, and file beads."

## A/P/C Checkpoints

At end of each phase (FULL mode only):

- **[A] Advanced** - Phase-specific deep dive
- **[P] Party** - Multi-agent feedback (BMAD v6) → [bmad/](references/bmad/)
- **[C] Continue** - Proceed to next phase
- **[↩ Back]** - Return to previous phase

See [apc-checkpoints.md](references/apc-checkpoints.md) for details.

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

## Related

- [conductor](../conductor/SKILL.md) - Track creation and implementation
- [beads](../beads/SKILL.md) - Issue tracking after design
- [maestro-core](../maestro-core/SKILL.md) - Routing policies in AGENTS.md
