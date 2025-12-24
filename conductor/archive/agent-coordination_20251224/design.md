# Design: Integrate agent_mail MCP into Workflow

**Track ID:** agent-coordination_20251224  
**Status:** Design Complete  
**Date:** 2024-12-24

---

## Problem Statement

> "Integrate agent_mail MCP invisibly into existing Maestro skills so that parallel agents automatically avoid file collisions and new sessions inherit context from previous ones - without requiring users to learn new commands or change their workflow."

## Context

### Pain Points (DISCOVER)
- Parallel agents frequently collide on file edits (overwrites, race conditions)
- Context lost between sessions
- No visibility into what agents are doing
- Beads notes insufficient for rich communication
- Mixed agent types: Task subagents, terminal sessions, git worktrees

### Constraints
- Zero ceremony tolerance - must be invisible to user
- Graceful degradation - workflow continues if MCP down
- Beads Village (`bv`) never used - no migration needed
- agent_mail MCP already installed and working

## Decisions (DEFINE)

| Decision | Value |
|----------|-------|
| **Container** | `workflows/agent-coordination/` |
| **Structure** | workflow.md → patterns/ → examples/ |
| **Skill integration** | Link to patterns, auto-trigger |
| **Session hooks** | AGENTS.md guidance (best-effort) |
| **Handoff format** | Template + agent discretion |
| **Reservation model** | Hybrid: coordinator pre-reserves known files, subagent can add |
| **File detection** | Best-effort parse from task description + subagent fallback |
| **TTL** | 1h default, user can override |
| **Subagent context** | Coordination block injected in prompt |
| **Contention** | Warn + skip, don't release |
| **Failure handling** | 3s timeout, warn, proceed without coordination |

## Design (DEVELOP)

### Workflow Structure

```
workflows/agent-coordination/
├── workflow.md                    # Core protocol (~60 lines)
├── patterns/
│   ├── parallel-dispatch.md       # Coordinator reservation flow
│   ├── subagent-prompt.md         # Coordination block template
│   ├── session-lifecycle.md       # AGENTS.md guidance
│   └── graceful-fallback.md       # Timeout + warn + proceed
└── examples/
    └── dispatch-three-agents.md   # Annotated example
```

### workflow.md

```markdown
# Agent Coordination Workflow

Enable parallel agents to avoid file collisions and share context.

## When This Applies
- Dispatching parallel subagents via Task tool
- Multiple terminal sessions on same codebase
- Handoff between sessions

## Core Protocol
1. Coordinator reserves files before dispatch
2. Subagents receive coordination block in prompt
3. Conflicts = warn + skip (optimistic)
4. Coordinator releases on completion
5. Session end = handoff message to inbox

## Patterns
- [parallel-dispatch](patterns/parallel-dispatch.md)
- [subagent-prompt](patterns/subagent-prompt.md)
- [session-lifecycle](patterns/session-lifecycle.md)
- [graceful-fallback](patterns/graceful-fallback.md)

## Failure Modes
- MCP unreachable: warn, proceed without coordination
- Reservation conflict: warn, skip file
- Stale reservation: TTL expires, auto-releases

## Verification

After implementing coordination:
1. Dispatch 2 agents to same file - verify one warns about conflict
2. Kill MCP mid-session - verify workflow continues with warning
3. End session, start new - verify inbox has handoff message
4. Check `🔒 Reserved` and `🔓 Released` feedback appears
```

### patterns/parallel-dispatch.md

```markdown
# Parallel Dispatch Pattern

## When to Use
Before dispatching 2+ subagents via Task tool that may touch overlapping files.

## File Detection Heuristics

Parse task descriptions for file patterns:

| Pattern | Example | Extracts |
|---------|---------|----------|
| Explicit path | "Edit skills/beads/SKILL.md" | `skills/beads/SKILL.md` |
| Directory reference | "Update the beads skill" | `skills/beads/**` |
| File type | "Fix the test file" | `**/*.test.{ts,js}` |
| Component name | "Modify the conductor workflow" | `workflows/**/`, `skills/conductor/**` |
| Quoted paths | "`src/api/users.ts`" | `src/api/users.ts` |
| Backtick code | "Change the `UserService` class" | Search for file containing `UserService` |

**Fallback:** If no patterns detected, don't reserve. Subagent self-reserves if needed.

## Flow

1. **Parse tasks for files** (using heuristics above)
   - Extract explicit paths from task descriptions
   - Infer from patterns ("edit the beads skill" → `skills/beads/**`)
   - Best-effort; subagents can reserve extras

2. **Reserve files** (3s timeout)
   ```
   file_reservation_paths(
     project_key: <workspace>,
     agent_name: <coordinator>,
     paths: [<file patterns>],
     ttl_seconds: 3600,  # 1h default
     exclusive: true
   )
   ```
   On timeout/failure: log warning, proceed without reservation.

3. **Inject coordination block** into each Task prompt
   See [subagent-prompt.md](subagent-prompt.md)

4. **Dispatch subagents** via Task tool

5. **Release on completion**
   ```
   release_file_reservations(
     project_key: <workspace>,
     agent_name: <coordinator>
   )
   ```
   On failure: log warning (TTL expires anyway)

## Visible Feedback
Show user:
```text
🔒 Reserved: skills/foo/SKILL.md, skills/bar/SKILL.md (1h)
Dispatching 3 agents...
```
```text
🔓 Released reservations
```

### patterns/subagent-prompt.md

```markdown
# Subagent Coordination Block

Inject this into Task prompts when dispatching coordinated subagents.

## Template

---
**Coordination:**
- Working inside reservation: {file_patterns}
- If you need files outside this, call register_agent then file_reservation_paths
- On conflict with unreserved file: warn + skip
- Do NOT release reservations; coordinator handles cleanup
---

## Example

---
**Coordination:**
- Working inside reservation: skills/beads/SKILL.md, skills/beads/references/*
- If you need files outside this, call register_agent then file_reservation_paths
- On conflict with unreserved file: warn + skip
- Do NOT release reservations; coordinator handles cleanup
---
```

### patterns/session-lifecycle.md

```markdown
# Session Lifecycle Pattern

Guidance for AGENTS.md to enable session handoff.

## Add to AGENTS.md

### Session Start
On first response, register and check for handoff:
1. Call `register_agent` with project path
2. Check inbox for handoff messages from previous sessions
3. Summarize any relevant context before proceeding

### Session End
Before ending (user says bye, task complete, etc.):
1. Send handoff message summarizing:
   - What was decided/completed
   - What remains to be done
   - Any context the next session needs
2. Release any file reservations you hold

### Handoff Message Template (adapt as needed)
Subject: Session handoff - {date}
Body:
- Completed: {list}
- Decisions: {list}
- Next steps: {list}
- Open questions: {list}

## Note
Session lifecycle is best-effort. Agent compliance varies.
Parallel dispatch coordination (file reservations) is more reliable.
```

### patterns/graceful-fallback.md

```markdown
# Graceful Fallback Pattern

Handle agent_mail MCP failures without blocking workflow.

## Timeout Strategy
All agent_mail calls should use 3-second mental timeout:
- If no response in ~3s, assume failure
- Log warning, proceed without coordination

## Failure Responses

| Operation | On Failure | User Sees |
|-----------|------------|-----------|
| `ensure_project` | Proceed | ⚠️ Coordination unavailable |
| `register_agent` | Proceed | (silent) |
| `file_reservation_paths` | Proceed uncoordinated | ⚠️ Could not reserve files |
| `release_file_reservations` | Proceed (TTL handles) | (silent) |
| `send_message` | Proceed | ⚠️ Handoff not sent |
| `fetch_inbox` | Proceed | (silent) |

## Warning Format
```
⚠️ Agent coordination unavailable - proceeding without file locks
```

## Recovery
If MCP becomes available mid-session:
- Next coordination call will succeed
- No need to retry failed calls
```

### Skill Integration

#### dispatching-parallel-agents/SKILL.md (add section)

```markdown
## Coordination (Optional)

When agent_mail MCP is available, coordinate parallel agents:

Follow [parallel-dispatch pattern](../../workflows/agent-coordination/patterns/parallel-dispatch.md):
1. Reserve files before dispatching
2. Inject coordination block into Task prompts
3. Release after all subagents complete

If MCP unavailable, proceed without coordination per [graceful-fallback](../../workflows/agent-coordination/patterns/graceful-fallback.md).
```

#### subagent-driven-development/SKILL.md (add section)

```markdown
## Coordination

When dispatching subagents, follow [parallel-dispatch pattern](../../workflows/agent-coordination/patterns/parallel-dispatch.md).
```

### AGENTS.md Updates

```markdown
## Agent Coordination

This project uses agent_mail MCP for multi-agent coordination.

### Session Start
On first response:
1. Register: `register_agent(project_key="/path/to/workspace", program="amp", model="<model>")`
2. Check inbox: `fetch_inbox(project_key, agent_name)` for handoff from previous sessions
3. Summarize relevant context before proceeding

### Session End
Before ending:
1. Send handoff via `send_message`:
   - Subject: "Session handoff - <date>"
   - Body: Completed, Decisions, Next steps, Open questions
2. Release reservations if any held

### Parallel Dispatch
When spawning parallel subagents:
- Follow [parallel-dispatch pattern](workflows/agent-coordination/patterns/parallel-dispatch.md)
- Reserve files → inject coordination block → dispatch → release

### Failure Handling
If agent_mail MCP unavailable:
- Log `⚠️ Coordination unavailable`
- Proceed without coordination
```

### Documentation Updates

1. **README.md** - Add multi-agent coordination section
2. **conductor/CODEMAPS/overview.md** - Update as follows:
   - Add to "Key Entry Points" table: `| workflows/agent-coordination/ | Multi-agent file locking + session handoff |`
   - Add to "Common Tasks" table: `| Coordinate parallel agents | Follow [parallel-dispatch pattern](workflows/agent-coordination/patterns/parallel-dispatch.md) |`
3. **workflows/README.md** - Update pipeline diagram to show coordination between COORDINATOR and WORKERS

### Skill Integration Notes

The current skills (`dispatching-parallel-agents`, `subagent-driven-development`) have no coordination logic. This design specifies what to add:
- Add "Coordination" section linking to workflow patterns
- Skills remain unchanged until implementation phase

## Success Criteria

| # | Criterion | Measurable |
|---|-----------|------------|
| 1 | No file collisions | Parallel subagents never overwrite each other's edits |
| 2 | Context flows | New session sees summary of previous session's decisions |
| 3 | Zero new commands | User's workflow unchanged |
| 4 | Graceful degradation | If MCP down, workflow continues with warning |
| 5 | Visible feedback | User sees what's reserved/released |

## Deliverables

| # | Deliverable |
|---|-------------|
| 1 | `workflows/agent-coordination/workflow.md` |
| 2 | `workflows/agent-coordination/patterns/parallel-dispatch.md` |
| 3 | `workflows/agent-coordination/patterns/subagent-prompt.md` |
| 4 | `workflows/agent-coordination/patterns/session-lifecycle.md` |
| 5 | `workflows/agent-coordination/patterns/graceful-fallback.md` |
| 6 | `workflows/agent-coordination/examples/dispatch-three-agents.md` |
| 7 | Update `skills/dispatching-parallel-agents/SKILL.md` |
| 8 | Update `skills/subagent-driven-development/SKILL.md` |
| 9 | Update `AGENTS.md` |
| 10 | Update `README.md` |
| 11 | Update `workflows/README.md` |

## Party Mode Insights

### Key Learnings from Design Session

1. **agent_mail complements Beads** - coordination vs work tracking, don't replace
2. **Zero ceremony = invisible integration** - bake into skills, no new commands
3. **Hybrid reservation model** - coordinator pre-reserves, subagent can add
4. **Workflow as container** - single source of truth, skills reference patterns
5. **Subagents don't need to register** if working inside coordinator's reservation
6. **Session handoff is best-effort** - agent compliance varies
7. **Graceful degradation is critical** - timeout + warn + proceed

### Agents Consulted
- 🏗️ Winston (Architect) - System design, layering
- 💻 Amelia (Developer) - Implementation pragmatism
- 🧪 Murat (QA) - Testing, failure modes
- 🔬 Dr. Quinn (Solver) - Problem decomposition
- 📚 Paige (Docs) - Documentation, teachability
- 🎯 Maya (Design Thinking) - User experience, framing
- ⚡ Victor (Strategist) - Strategic choices
- 🎨 Sally (UX) - User feedback, visibility
- 📋 John (PM) - User stories, product lens
- 🧠 Carson (Brainstorm) - Alternative approaches
- 📖 Sophia (Storyteller) - Narrative structure
