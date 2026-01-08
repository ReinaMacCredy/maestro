# Beads Village Integration Plan

**Created**: 2025-06-20
**Source**: [mcp-beads-village](https://github.com/LNS2905/mcp-beads-village)
**Goal**: Merge multi-agent coordination features into existing beads skill

---

## Summary

Integrate mcp-beads-village MCP server into my-workflow plugin to enable:
- Atomic task claiming with role-based filtering
- File locking to prevent conflicts
- Agent messaging and team coordination
- Leader/worker role support

**Design decision**: MCP is required (not optional fallback). Village becomes the default coordination layer.

---

## Phase 1: Documentation Updates

### Task 1.1: Update SETUP_GUIDE.md
**File**: `SETUP_GUIDE.md`

Add to Step 4 (MCP Servers):
- Prerequisites: `pip install beads`, Node.js 16+
- Installation commands for Claude Code, Amp, Codex
- Link to source repo
- Verification steps
- Tool reference table (init, claim, done, reserve, msg, etc.)

### Task 1.2: Update TUTORIAL.md
**File**: `TUTORIAL.md`

Changes:
- Update core workflow diagram: `Conductor → Beads + Village → TDD`
- Add "Multi-Agent Mode" section explaining when/why
- Update "What Problem Does Each Skill Solve?" table
- Add Village to Quick Reference triggers

---

## Phase 2: Skill Merge

### Task 2.1: Update beads SKILL.md Overview
**File**: `skills/beads/SKILL.md`

Add to Overview section:
```markdown
**Multi-Agent Mode**: When working in a team (multiple Claude instances on 
same codebase), bd coordinates through file reservations, task claiming, 
and agent messaging to prevent conflicts.
```

### Task 2.2: Add Multi-Agent Session Protocol
**File**: `skills/beads/SKILL.md`

New section after existing Session Start Protocol:
- `init` as first command (join workspace, set team/role)
- `claim` instead of manual task selection
- `status` to check team state
- `inbox` to read messages

### Task 2.3: Add File Reservation Section
**File**: `skills/beads/SKILL.md`

New section:
- `reserve` before editing files
- `release` when done (or auto-release via `done`)
- Conflict resolution workflow (check reservations → msg owner → wait or claim different task)
- TTL explanation (default 10 min)
- When to reserve table (editing=yes, creating=no, reading=no)

### Task 2.4: Add Task Completion Workflow
**File**: `skills/beads/SKILL.md`

Update Issue Lifecycle / add new section:
- `done id="<id>" msg="summary"` instead of just `bd close`
- Auto-releases all file reservations
- Broadcasts completion to team
- Best practice: 1 task = 1 session

### Task 2.5: Add Agent Communication Section
**File**: `skills/beads/SKILL.md`

New section:
- `msg` for direct and broadcast messages
- `inbox` to read messages (local and global)
- `status` to see online agents and locks
- When to communicate (blocked, need file, handoff)

### Task 2.6: Add Role-Based Coordination
**File**: `skills/beads/SKILL.md`

New section or integrate into Session Protocol:
- Role values: `fe`, `be`, `devops`, `docs`, custom
- Leader mode: `leader=true` in init
- Leader-only tools: `assign`
- How `claim` filters by role

---

## Phase 3: Reference Updates

### Task 3.1: Create Village Reference
**File**: `skills/beads/references/VILLAGE.md`

Comprehensive reference for:
- Complete tool specifications with all parameters
- State directories (`.reservations/`, `.mail/`, `~/.beads-village/`)
- Conflict resolution patterns
- Team coordination protocols
- Troubleshooting

### Task 3.2: Update WORKFLOWS.md
**File**: `skills/beads/references/WORKFLOWS.md`

Add multi-agent workflow patterns:
- Team session start checklist
- Handoff between agents
- Parallel task execution
- Conflict resolution

---

## Phase 4: Verification

### Task 4.1: Validate JSON/Markdown
```bash
cat .claude-plugin/plugin.json | jq .
```

### Task 4.2: Test Skill Loading
Verify skill loads correctly in Claude Code/Amp session.

### Task 4.3: Document in AGENTS.md
Update if needed to reflect new beads capabilities.

---

## File Change Summary

| File | Action |
|------|--------|
| `SETUP_GUIDE.md` | Add Village MCP installation (Step 4) |
| `TUTORIAL.md` | Update workflow diagram, add Village section |
| `skills/beads/SKILL.md` | Merge all Village patterns |
| `skills/beads/references/VILLAGE.md` | New reference file |
| `skills/beads/references/WORKFLOWS.md` | Add multi-agent workflows |

---

## Acceptance Criteria

- [ ] SETUP_GUIDE.md has complete Village installation for Claude/Amp/Codex
- [ ] TUTORIAL.md explains multi-agent coordination
- [ ] beads SKILL.md includes init/claim/done/reserve/msg workflows
- [ ] References document all Village tools and parameters
- [ ] Plugin JSON validates
- [ ] Skill loads successfully in agent session
