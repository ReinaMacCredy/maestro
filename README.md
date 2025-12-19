# My Workflow

Personal AI agent workflow skills for Claude Code - a curated collection of skills for structured development workflows.

## Overview

This plugin provides a comprehensive set of skills for:
- **Brainstorming & Planning** - Structured exploration before implementation
- **Test-Driven Development** - RED-GREEN-REFACTOR methodology
- **Systematic Debugging** - Four-phase debugging with root cause analysis
- **Code Review** - Both requesting and receiving reviews with technical rigor
- **Multi-Session Tracking** - Beads issue tracking for complex, long-running work

## Skills Included

| Skill | Description |
|-------|-------------|
| `brainstorming` | Deep exploration and creative design before implementation |
| `test-driven-development` | TDD workflow with verification enforcement |
| `systematic-debugging` | Four-phase debugging methodology |
| `requesting-code-review` | Request reviews with structured requirements |
| `receiving-code-review` | Handle feedback with technical verification |
| `writing-plans` | Create implementation plans from specs |
| `executing-plans` | Execute plans with review checkpoints |
| `verification-before-completion` | Ensure evidence before success claims |
| `using-git-worktrees` | Isolated feature development |
| `finishing-a-development-branch` | Complete and integrate work |
| `dispatching-parallel-agents` | Parallel task execution |
| `subagent-driven-development` | Subagent coordination |
| `beads/*` | Multi-session issue tracking with dependencies |
| `using-superpowers` | Session initialization and skill discovery |
| `writing-skills` | Create and test new skills |

## Installation

```bash
# Add the marketplace
/plugin marketplace add ReinaMacCredy/my-workflow

# Install the plugin
/plugin install my-workflow
```

## Usage

Skills are automatically loaded based on trigger phrases:

```
bs, brainstorm     → brainstorming skill
debug, investigate → systematic-debugging skill
tdd                → test-driven-development skill
review code        → requesting-code-review skill
write plan         → writing-plans skill
fb, file beads     → file-beads skill
bd status          → beads skill
```

## License

MIT
