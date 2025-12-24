# Specification: Integrate agent_mail MCP into Workflow

## Overview

Integrate agent_mail MCP invisibly into existing Maestro skills so that parallel agents automatically avoid file collisions and new sessions inherit context from previous ones - without requiring users to learn new commands or change their workflow.

## Functional Requirements

### FR1: Workflow Container
- Create `workflows/agent-coordination/` directory with:
  - `workflow.md` - Core protocol definition
  - `patterns/` - Reusable pattern files
  - `examples/` - Annotated usage examples

### FR2: Parallel Dispatch Coordination
- Before dispatching 2+ parallel subagents, coordinator must:
  - Parse task descriptions for file patterns using defined heuristics
  - Reserve identified files via `file_reservation_paths` with 1h TTL
  - Inject coordination block into each Task prompt
  - Release reservations after all subagents complete
- Show visible feedback: `🔒 Reserved: <files>` and `🔓 Released reservations`

### FR3: Subagent Coordination Block
- Subagents receive prompt injection with:
  - List of reserved files they can work on
  - Instructions to register and reserve if needing additional files
  - Conflict behavior: warn + skip
  - Instruction not to release (coordinator handles)

### FR4: Session Lifecycle (Best-Effort)
- On session start: register agent, fetch inbox for handoff messages
- On session end: send handoff message with completed/decisions/next steps
- Guidance added to AGENTS.md (not enforced in code)

### FR5: Graceful Degradation
- All agent_mail calls use 3s mental timeout
- On failure: log warning, proceed without coordination
- Workflow must never block on MCP unavailability

### FR6: Skill Integration
- Update `dispatching-parallel-agents/SKILL.md` to link to parallel-dispatch pattern
- Update `subagent-driven-development/SKILL.md` to link to parallel-dispatch pattern

### FR7: Documentation
- Update `AGENTS.md` with coordination guidance
- Update `README.md` with coordination section
- Update `workflows/README.md` pipeline diagram
- Update `conductor/CODEMAPS/overview.md` with new entry

## Non-Functional Requirements

### NFR1: Zero Ceremony
- Users must not learn new commands
- Coordination is automatic when skills trigger it
- No additional steps in existing workflow

### NFR2: Visibility Without Action
- Users see what's happening (reserved/released feedback)
- Users don't need to act on it

### NFR3: Failure Tolerance
- MCP failure never blocks workflow
- Stale reservations auto-expire via TTL

## Acceptance Criteria

| # | Criterion | Test |
|---|-----------|------|
| AC1 | No file collisions | Dispatch 2 agents to same file → one warns |
| AC2 | Context flows | End session, start new → inbox has handoff |
| AC3 | Zero new commands | User workflow unchanged from before |
| AC4 | Graceful degradation | Kill MCP → workflow continues with warning |
| AC5 | Visible feedback | User sees 🔒/🔓 messages |

## Out of Scope

- Cross-project coordination (future)
- Contact approval workflows (not needed for internal agents)
- Full agent_mail feature coverage (only using subset)
- Replacing Beads CLI (`bd`) for work tracking
- UI/dashboard for visibility
- Automated tests (manual verification only)
