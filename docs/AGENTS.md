# docs

## Purpose
System-level documentation for the Maestro workflow architecture and testing procedures.

## Key Directories

| Directory | Purpose |
|-----------|---------|
| testing/ | Procedures for validating the agent workflow itself |

## Key Files

| File | Purpose |
|------|---------|
| ARCHITECTURE.md | System overview of Maestro workflow |
| testing/multi-session-test.md | Manual verification for session coordination |

## Architecture Concepts

From ARCHITECTURE.md:
- **Skill Hierarchy**: conductor -> orchestrator -> design -> beads
- **Workflow Pipeline**: Planning (Double Diamond) -> Spec (/conductor-newtrack) -> Execution (/conductor-implement)
- **Party Mode**: Specialized sub-agent roles (Architect, QA, etc.)
- **Handoffs**: Context preservation between sessions

## Patterns

- **Documentation as Specification**: Docs define expected behavior
- **Manual Verification**: Some workflows require human-in-the-loop testing
- **Skill Hierarchy**: Understanding the skill dependency chain is critical

## Dependencies

- **Internal**: References skills/ and .claude/ for implementation details
- **External**: None

## Notes for AI Agents

- ARCHITECTURE.md is the system map - read it to understand skill interactions
- testing/ contains verification procedures for the workflow itself
- When debugging multi-agent issues, check testing/multi-session-test.md
- Documentation should stay in sync with actual skill implementations
