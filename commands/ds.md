---
description: Start a design session (brainstorm)
---

# Design Session (ds)

Load the `design` skill and begin collaborative brainstorming.

**What this does:**
1. Loads the design skill
2. Understands project context (docs, files, recent commits)
3. Asks questions one at a time to refine the idea
4. Proposes 2-3 approaches with trade-offs
5. Presents design in small sections for validation
6. Grounds decisions against current reality (REQUIRED)
7. Suggests `fb` to convert into beads issues

## Usage

Just say `ds` to start a design session.

## Example

```
User: ds
Agent: [loads design skill]
       What would you like to design today?
```

## After Design

When design is approved:
- Run `/conductor-newtrack` to generate spec + plan + file beads + review (automatic)
- Use `fb` manually if you need to re-file beads from an existing plan
