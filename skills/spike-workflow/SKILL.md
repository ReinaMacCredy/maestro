---
name: spike-workflow
version: "1.0.0"
description: Use when exploring unknown technology, assessing feasibility, or before Complex (C) tier work - time-boxed technical research with structured output
argument-hint: <topic>
---

# Spike Workflow

## Overview

Time-boxed technical research for unknowns. Produces structured findings that inform implementation decisions.

## When to Use

- Unknown technology exploration
- Feasibility assessment before committing to approach
- Before Complex (C) tier work in Conductor
- Evaluating multiple technical options
- Investigating third-party integrations

## Process

### 1. Define Scope (5 min)

```markdown
## Spike: [Topic]
**Time Box:** [30min | 1hr | 2hr]
**Question:** What specific question are we answering?
**Success Criteria:** How will we know we have enough information?
```

### 2. Research Phase

- Read documentation, not just examples
- Build minimal proof-of-concept if needed
- Document dead ends (they're valuable)
- Stay within time box

### 3. Document Findings

Create `history/spikes/<topic>.md`:

```markdown
# Spike: [Topic]

**Date:** YYYY-MM-DD
**Time Spent:** Xh Ym
**Status:** [Concluded | Needs More Research | Blocked]

## Question
What were we trying to learn?

## Findings

### What Works
- Finding 1 with evidence
- Finding 2 with code snippet

### What Doesn't Work
- Attempted approach and why it failed

### Unknowns Remaining
- Questions still unanswered

## Recommendation
Clear recommendation based on findings.

## Next Steps
- [ ] Concrete action items
```

## Integration with Conductor

Spike findings feed into `/conductor-newtrack`:
- Reference spike in spec context
- Use findings to inform task breakdown
- Link spike doc in track's spec.md

## Anti-Patterns

- **Endless research** - Respect the time box
- **No documentation** - Undocumented spikes are wasted time
- **Scope creep** - Answer the specific question, not all questions
- **Implementation during spike** - Spike is research, not building

## Trigger

```
spike [topic]
```

Example: `spike websocket authentication`
