---
description: File beads epics and issues from a plan
---

# File Beads (fb)

Load the `file-beads` skill to convert a plan into Beads epics and issues.

**What this does:**
1. Loads the file-beads skill
2. Analyzes the plan (from context or conductor/tracks/)
3. Dispatches sequential subagents to create epics
4. Links cross-epic dependencies
5. Summarizes what was created
6. Suggests `rb` to review and refine

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
