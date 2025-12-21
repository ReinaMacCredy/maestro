# Workflow-Aware Skills Design

**Date**: 2025-12-21  
**Status**: Ready for implementation

## Problem

After a skill completes (e.g., `bs`, `/conductor-newtrack`), the agent stops without suggesting the next step. Users must manually remember the pipeline and trigger the next skill.

## Solution

Make skills "workflow-aware" — after completing, each skill:
1. Checks what artifacts exist
2. Suggests the logical next step
3. Waits for user approval before continuing

## Workflow Chain

```
bs/conductor → plan.md → "Review plan?" → approved → suggest fb
fb (parallel) → files beads → suggest rb
rb → review/refine → approved → output HANDOFF block
---session break---
paste HANDOFF → execute plan (tdd)
```

## Skill Transitions

| After this... | Suggest this... |
|---------------|-----------------|
| `bs` / `/conductor-newtrack` | "Review the plan?" |
| Plan approved | "Say `fb` to convert into beads issues" |
| `fb` completes | "Say `rb` to review filed beads" |
| `rb` approved | Output HANDOFF block |

## Files to Modify

### 1. skills/brainstorming/SKILL.md

Add to end of skill:

```markdown
## After Completion

After writing the design to `docs/plans/` or `conductor/tracks/<id>/plan.md`:

1. Ask: "Review the plan?"
2. Address any feedback
3. When approved, say: "Plan approved. Say `fb` to convert into beads issues."
```

### 2. skills/conductor/SKILL.md

Add to `/conductor-newtrack` completion section:

```markdown
## After Track Creation

After creating spec.md and plan.md:

1. Present the plan for review
2. Address any feedback
3. When approved, say: "Plan approved. Say `fb` to convert into beads issues."
```

### 3. skills/beads/file-beads/SKILL.md

Add to end of skill:

```markdown
## After Completion

After parallel agents finish filing beads:

1. Summarize what was created (epic ID, issue count)
2. Say: "Beads filed. Say `rb` to review and refine."
```

### 4. skills/beads/review-beads/SKILL.md

Add to end of skill:

```markdown
## After Approval

When user approves the reviewed beads:

1. Generate HANDOFF block:

\`\`\`
=== HANDOFF ===
Epic: bd-XXX
Track: conductor/tracks/<id>/
Plan: conductor/tracks/<id>/plan.md
Ready issues: [list of bd-XXX IDs]
================
\`\`\`

2. Say: "Ready for execution. Copy this HANDOFF for next session, or continue now with `ct` (claim task)."
```

## Implementation Notes

- Minimal changes: only add "After Completion" sections
- No new skills or orchestration layer needed
- `fb` continues to use parallel agents (existing behavior)
- HANDOFF block provides all context needed to resume
