# Implementation Plan: Integrate agent_mail MCP into Workflow

## Epic 1: Create Workflow Container

Create the `workflows/agent-coordination/` directory structure with core files.

### Tasks

- [ ] 1.1 Create `workflows/agent-coordination/workflow.md`
  - Core protocol definition (~60 lines)
  - When it applies, core protocol steps, patterns list, failure modes
  - Include verification section
  
- [ ] 1.2 Create `workflows/agent-coordination/patterns/parallel-dispatch.md`
  - File detection heuristics table
  - Reserve/dispatch/release flow
  - Visible feedback templates
  
- [ ] 1.3 Create `workflows/agent-coordination/patterns/subagent-prompt.md`
  - Coordination block template
  - Example injection
  
- [ ] 1.4 Create `workflows/agent-coordination/patterns/session-lifecycle.md`
  - AGENTS.md guidance for session start/end
  - Handoff message template
  - Best-effort note
  
- [ ] 1.5 Create `workflows/agent-coordination/patterns/graceful-fallback.md`
  - Timeout strategy
  - Failure responses table
  - Warning format
  - Recovery notes

- [ ] 1.6 Create `workflows/agent-coordination/examples/dispatch-three-agents.md`
  - Annotated example with code blocks
  - Shows reserve, dispatch, release flow

## Epic 2: Integrate into Skills

Update existing skills to reference the workflow patterns.

### Tasks

- [ ] 2.1 Update `skills/dispatching-parallel-agents/SKILL.md`
  - Add "Coordination (Optional)" section
  - Link to parallel-dispatch pattern
  - Link to graceful-fallback pattern
  
- [ ] 2.2 Update `skills/subagent-driven-development/SKILL.md`
  - Add "Coordination" section
  - Link to parallel-dispatch pattern

## Epic 3: Update Documentation

Update project documentation to reflect new coordination capability.

### Tasks

- [ ] 3.1 Update `AGENTS.md`
  - Add "Agent Coordination" section
  - Session start/end guidance
  - Parallel dispatch reference
  - Failure handling guidance

- [ ] 3.2 Update `README.md`
  - Add "Multi-Agent Coordination" section
  - Brief description and link to workflow

- [ ] 3.3 Update `workflows/README.md`
  - Add agent-coordination to directory structure
  - Note in pipeline description

- [ ] 3.4 Update `conductor/CODEMAPS/overview.md`
  - Add to "Key Entry Points" table
  - Add to "Common Tasks" table

## Epic 4: Verification

Verify the integration works as designed.

### Tasks

- [ ] 4.1 Manual test: Dispatch 2 agents to same file
  - Verify one agent warns about conflict
  - Verify visible feedback appears
  
- [ ] 4.2 Manual test: MCP failure
  - Stop agent_mail MCP
  - Run dispatch skill
  - Verify workflow continues with warning

- [ ] 4.3 Manual test: Session handoff
  - End session with handoff message
  - Start new session
  - Verify inbox contains message

## Dependencies

```
Epic 1 (Workflow) ─┬─► Epic 2 (Skills)
                   │
                   └─► Epic 3 (Docs)
                   
Epic 2 + Epic 3 ──────► Epic 4 (Verification)
```

## Estimates

| Epic | Complexity | Estimate |
|------|------------|----------|
| Epic 1: Workflow Container | Medium | 30 min |
| Epic 2: Skill Integration | Low | 15 min |
| Epic 3: Documentation | Low | 15 min |
| Epic 4: Verification | Low | 15 min |
| **Total** | | **~1.5 hours** |
