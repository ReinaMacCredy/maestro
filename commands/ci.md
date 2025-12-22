---
description: Start conductor implement (execute epics)
argument-hint: [track_id or "Start epic <epic-id>"]
---

# Conductor Implement (ci)

Load the `conductor-implement` skill and begin implementation using beads tracking.

**What this does:**
1. Loads the conductor-implement skill
2. Runs pre-flight checks (conductor files exist, jq installed)
3. Selects an epic to implement
4. Loads track context (design.md, spec.md, plan.md)
5. Executes tasks using TDD workflow
6. Closes tasks/epics in beads as they complete
7. Hands off to next epic or completes track

## Usage

Say `ci` to start implementing the current track, or `ci <track_id>` for a specific track.

To resume a specific epic: `ci Start epic <epic-id>`

## Example

```
User: ci
Agent: [loads conductor-implement skill]
       Checking conductor setup...
       Found track: auth-system
       Epic: Authentication (bd-101) - 4 tasks ready
       Starting implementation...
```

## After Implementation

When epic is complete:
- Say `rb` to review remaining beads
- Or hand off to next epic with `ci Start epic <next-id>`
