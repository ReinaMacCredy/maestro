# Orchestrator Agent Directory

> Specialized agents for multi-agent parallel execution workflows.

## Agent Index

| Agent | Category | Purpose |
|-------|----------|---------|
| [codebase-locator](research/codebase-locator.md) | research | Find WHERE files/code exist in codebase |
| [codebase-analyzer](research/codebase-analyzer.md) | research | Understand HOW code works |
| [pattern-finder](research/pattern-finder.md) | research | Find existing conventions and patterns |
| [impact-assessor](research/impact-assessor.md) | research | Assess change impact on codebase |
| [web-researcher](research/web-researcher.md) | research | External docs and API research |
| [github-researcher](research/github-researcher.md) | research | GitHub issues, PRs, and repo research |
| [security-reviewer](review/security-reviewer.md) | review | Security vulnerability analysis |
| [code-reviewer](review/code-reviewer.md) | review | Code quality and best practices |
| [pr-reviewer](review/pr-reviewer.md) | review | Pull request review |
| [spec-reviewer](review/spec-reviewer.md) | review | Specification validation |
| [oracle](review/oracle.md) | review | 6-dimension design audit at CP4 |
| [plan-agent](planning/plan-agent.md) | planning | Create implementation plans |
| [validate-agent](planning/validate-agent.md) | planning | Validate plans and specs |
| [implement-agent](execution/implement-agent.md) | execution | Execute implementation tasks |
| [worker-agent](execution/worker-agent.md) | execution | Autonomous parallel worker |
| [debug-agent](debug/debug-agent.md) | debug | Root cause analysis and debugging |

## Category Overview

### Research (`research/`)
Agents that gather information before implementation:
- **codebase-locator**: Finds relevant files using grep, glob, finder
- **codebase-analyzer**: Deep analysis of code structure and flow
- **pattern-finder**: Identifies conventions to follow
- **impact-assessor**: Predicts ripple effects of changes
- **web-researcher**: External documentation lookup
- **github-researcher**: GitHub-specific research (issues, PRs)

### Review (`review/`)
Agents that validate work quality:
- **security-reviewer**: OWASP, vulnerability scanning
- **code-reviewer**: Style, patterns, maintainability
- **pr-reviewer**: Full PR review workflow
- **spec-reviewer**: Spec completeness and consistency
- **oracle**: 6-dimension design audit at CP4 (VERIFY)

### Planning (`planning/`)
Agents that create and validate plans:
- **plan-agent**: Generates phased implementation plans
- **validate-agent**: Validates specs and plans against codebase

### Execution (`execution/`)
Agents that perform implementation work:
- **implement-agent**: Primary implementation worker
- **worker-agent**: Autonomous parallel worker (orchestrator-spawned)

### Debug (`debug/`)
Agents that investigate and fix issues:
- **debug-agent**: Root cause analysis, systematic debugging

## Agent Mail Integration

All agents include an Agent Mail section for coordination:

```markdown
## Agent Mail

### Reporting Progress
\`\`\`
send_message(
  project_key="/path/to/project",
  sender_name="AgentName",
  to=["PurpleSnow"],  # Orchestrator
  subject="[Track N] Task completed: <task-id>",
  body_md="...",
  thread_id="<epic-thread>"
)
\`\`\`

### Requesting Help
\`\`\`
send_message(
  project_key="/path/to/project",
  sender_name="AgentName",
  to=["PurpleSnow"],
  subject="[Track N] BLOCKED: <reason>",
  body_md="...",
  importance="high",
  thread_id="<epic-thread>"
)
\`\`\`
```

## Usage

Agents are referenced in plan.md Track Assignments:

```markdown
## Track Assignments

| Track | Agent | Beads | File Scope | Depends On |
|-------|-------|-------|------------|------------|
| 1 | BlueLake | bd-101 | src/api/** | - |
| 2 | GreenCastle | bd-201 | src/web/** | bd-101 |
```

Each worker loads the appropriate agent prompts for their task type.

## Session Brain Role

The Orchestrator acts as the "session brain" for multi-session coordination:

- Detects active sessions via Agent Mail inbox analysis
- Manages session identity registration
- Coordinates file reservations and bead claiming
- Handles stale session takeover

This enables multiple Amp sessions to work on the same project without conflicts.
