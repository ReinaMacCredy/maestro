---
description: File beads epics and issues from a plan
---

# File Beads (fb)

Load the `file-beads` skill to convert a plan into Beads epics and issues.

**What this does:**
1. Loads the file-beads skill
2. Analyzes the plan (from context or conductor/tracks/)
3. Creates all epics first (sequential, for stable IDs)
4. Dispatches parallel subagents to fill each epic with child issues
5. Links cross-epic dependencies
6. Summarizes what was created
7. Suggests `rb` to review and refine

## Usage

Say `fb` after completing a design session or when you have a plan ready.

## Example

```
User: fb
Agent: [loads file-beads skill]
       Analyzing plan from conductor/tracks/...
       [creates epics and issues]
       Beads filed. Say `rb` to review and refine.
```

## After Filing

When beads are filed:
- Say `rb` to review and refine the issues
- Or run `bd ready` to see what's ready to work on
