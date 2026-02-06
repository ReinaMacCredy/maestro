# Skill Integration Draft

> Interview COMPLETE - Plan generated

## Context

User wants Maestro to act as a composition layer that orchestrates skills from multiple sources under a unified workflow.

## Confirmed Requirements

### Integration Direction
**Composition layer** - Maestro acts as an abstraction that lets skills from any source work together under its orchestration.

### Skill Sources (Priority Order)
1. **Local Claude plugins** - Already installed plugins in the user's environment
2. **Anthropic marketplace** - Future integration with official marketplace

### Orchestration Method
**Auto-detection** - Prometheus automatically detects when to delegate based on:
- Keywords in the request (e.g., "frontend", "design")
- Capability matching against registered skills

### Composition Mode
**Full delegation** - External skill handles the entire task, returns result to Maestro. Maestro does not micro-manage the external skill's execution.

## Technical Decisions

1. **Discovery mechanism**: Scan standard Claude Code plugin locations for installed skills
2. **Registry format**: File-based JSON registry at `.maestro/registry/skills.json`
3. **Capability declaration**: Skills declare keywords and capabilities in their manifest
4. **Delegation**: Use Task() tool to spawn skill-specific agents when available

## Scope Boundaries

**IN**:
- Local plugin discovery and registration
- Auto-detection in Prometheus based on keywords/capabilities
- Full delegation to external skills
- Registry management (add, remove, refresh)

**OUT**:
- Marketplace browsing/installation (future phase)
- Partial delegation (skill assists but Maestro controls)
- Authentication/API key management for remote services
- Skill versioning and updates

## Plan Generated

See: `.maestro/plans/skill-integration.md`
