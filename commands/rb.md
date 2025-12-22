---
description: Review and refine beads issues
---

# Review Beads (rb)

Load the `review-beads` skill to review, proofread, and polish filed Beads issues.

**What this does:**
1. Loads the review-beads skill
2. Dispatches a subagent to review all issues
3. Checks clarity, completeness, dependencies, scope, priority
4. Fixes common issues (vague titles, missing context, etc.)
5. Validates dependency graph
6. Outputs HANDOFF block for execution session

## Usage

Say `rb` after filing beads with `fb`.

## Example

```
User: rb
Agent: [loads review-beads skill]
       [reviews and refines issues]
       Issues reviewed. Run `/conductor-implement` to start execution.
       
       ## HANDOFF
       **Command:** `Start epic bd-42`
       ...
```

## After Review

When review is complete:
- Copy the HANDOFF block for the execution session
- Run `/conductor-implement` to start implementation
- Or run `bd ready` to see ready issues
