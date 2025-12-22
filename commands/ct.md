---
description: Initialize conductor setup
---

# Conductor Setup (ct)

Load the `conductor-setup` skill and initialize context-driven development.

**What this does:**
1. Loads the conductor-setup skill
2. Detects project type (brownfield vs greenfield)
3. Creates conductor/ directory structure
4. Generates context files (product.md, tech-stack.md, workflow.md)
5. Initializes tracks.md
6. Optionally enhances AGENTS.md with beads instructions
7. Commits setup

## Usage

Say `ct` to initialize conductor in this project.

## Example

```
User: ct
Agent: [loads conductor-setup skill]
       Existing project detected...
       Analyzing README.md, package.json...
       Creating conductor/product.md...
       Setup complete. Run `ci` to start implementing.
```

## After Setup

When setup is complete:
- Run `/conductor-newtrack <description>` to create a track
- Or say `ds` to start a design session
