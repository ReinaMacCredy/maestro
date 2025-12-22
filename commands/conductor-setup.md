---
description: Initialize project with Conductor context-driven development
---

# Conductor Setup

Initialize this project with context-driven development. Follow this workflow:

## 1. Check Existing Setup

- If `conductor/setup_state.json` exists with `"last_successful_step": "complete"`, inform user setup is done
- If partial state, offer to resume or restart

## 2. Detect Project Type

**Brownfield** (existing project): Has `.git`, `package.json`, `requirements.txt`, `go.mod`, or `src/`
**Greenfield** (new project): Empty or only README.md

## 3. For Brownfield Projects

1. Announce: "Existing project detected"
2. Analyze: README.md, package.json/requirements.txt/go.mod, directory structure
3. Infer: tech stack, architecture, project goals
4. Present findings for confirmation

## 4. For Greenfield Projects

1. Ask: "What do you want to build?"
2. Initialize git if needed: `git init`

## 5. Create Conductor Directory

```bash
mkdir -p conductor/code_styleguides
```

## 6. Generate Context Files (Interactive)

For each file, ask 2-3 targeted questions, then generate:

- **product.md** - Product vision, users, goals, features
- **tech-stack.md** - Languages, frameworks, databases, tools
- **workflow.md** - Use the default TDD workflow from `templates/workflow.md`

Copy relevant code styleguides from `templates/code_styleguides/` based on tech stack.

## 7. Initialize Tracks File

Create `conductor/tracks.md`:
```markdown
# Project Tracks

This file tracks all major work items. Each track has its own spec and plan.

---
```

## 8. Generate Initial Track

1. Based on project context, propose an initial track (MVP for greenfield, first feature for brownfield)
2. On approval, create track using the newtrack workflow

## 9. Enhance AI Agent Integration

Check if AGENTS.md (or CLAUDE.md) exists and offer to add beads workflow instructions:

1. **Check for existing file:**
   ```bash
   if [ -f "AGENTS.md" ] || [ -f "CLAUDE.md" ]; then
     AGENT_FILE=$([ -f "AGENTS.md" ] && echo "AGENTS.md" || echo "CLAUDE.md")
   fi
   ```

2. **Check if beads instructions exist:**
   ```bash
   grep -q "beads" "$AGENT_FILE" 2>/dev/null
   ```

3. **If file exists but no beads instructions:**
   
   Present to user:
   ```
   We found $AGENT_FILE in this project but it doesn't include beads_viewer instructions.
   
   Adding these helps AI coding agents understand how to use your issue tracking workflow.
   
   Preview of content to add:
   
   ## Beads Workflow Integration
   
   This project uses [beads_viewer](https://github.com/Dicklesworthstone/beads_viewer) for issue tracking. 
   Issues are stored in `.beads/` and tracked in git.
   
   ### Key Commands
   - `bd ready --json` - Find available tasks
   - `bd update <id> --status in_progress` - Claim a task
   - `bd close <id> --reason "summary"` - Complete a task
   - `bd list --json` - List all issues
   
   ### Rules
   - Always commit `.beads/` with code changes
   - Use `--json` flag for structured output
   - Beads is source of truth for task status
   
   Add this to $AGENT_FILE? [Yes, add it / No thanks / Don't ask again]
   ```

4. **If user approves:** Append content to agent file
5. **If "Don't ask again":** Add `"skip_beads_integration": true` to `conductor/setup_state.json`

## 10. Finalize

1. Write `conductor/setup_state.json`: `{"last_successful_step": "complete"}`
2. Commit: `git add conductor && git commit -m "conductor(setup): Initialize conductor"`
3. Announce: "Setup complete. Run `/conductor-implement` to start."
