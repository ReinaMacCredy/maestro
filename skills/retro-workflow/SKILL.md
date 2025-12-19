---
name: retro-workflow
description: Use after completing tracks, significant milestones, or incident resolution - captures lessons learned for future reference
---

# Retrospective Workflow

## Overview

Capture lessons learned immediately after significant work. Promotes insights to memory for future sessions.

## When to Use

- After completing a Conductor track
- After significant milestones
- After incident resolution
- After failed attempts (especially valuable)
- Before context compaction

## Process

### 1. Trigger Retro

When work completes or after incidents, run retro to capture learnings while fresh.

### 2. Gather Context

Review:
- Completed tasks and their actual vs. estimated complexity
- Blockers encountered and how resolved
- Code review feedback received
- Test failures and their root causes
- Decisions made and their outcomes

### 3. Create Retro Document

Create `history/retros/<date>-<topic>.md`:

```markdown
# Retro: [Topic/Track ID]

**Date:** YYYY-MM-DD
**Duration:** [Actual time spent]
**Outcome:** [Success | Partial | Failed]

## What Went Well
- Specific wins with context
- Patterns that worked
- Tools/approaches that helped

## What Didn't Go Well
- Specific challenges
- Time sinks
- Approaches that failed

## Lessons Learned

### Technical
- Code patterns discovered
- Architecture insights
- Tool learnings

### Process
- Workflow improvements
- Communication insights
- Planning accuracy

## Action Items
- [ ] Concrete improvements for next time
- [ ] Memory updates needed
- [ ] Documentation to write

## Quotes / Key Moments
> Notable insights or breakthroughs worth remembering
```

### 4. Promote to Memory

For significant learnings, update `.memory/project.md`:

```markdown
## Learnings

### [Date] - [Topic]
- Key insight that should persist across sessions
```

## Integration with Conductor

After track completion:
1. Mark track complete in `conductor/tracks.md`
2. Run retro
3. Link retro in track's final status
4. Optionally archive track folder

## Anti-Patterns

- **Skipping retros** - "No time" means losing learnings
- **Vague retros** - "It went well" teaches nothing
- **Blame focus** - Focus on systems, not individuals
- **No action items** - Retros without changes are theater

## Memory Promotion Criteria

Promote to `.memory/project.md` when:
- Learning applies to multiple future tasks
- Insight saves significant time if remembered
- Pattern should become standard practice
- Gotcha could cause repeat failures

## Trigger

```
retro
```

Or after completing track: `retro track-001`
