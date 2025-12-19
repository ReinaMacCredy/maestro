# My Workflow

Personal AI agent workflow skills for Claude Code - a batteries-included collection for structured development workflows.

## Overview

This plugin bundles everything needed for context-driven development:

- **Conductor** - Automated planning flow with specs and plans
- **Beads** - Multi-session issue tracking with dependencies
- **Superpowers** - TDD, debugging, code review, and more

## Skills (26 total)

### Core Workflow

| Skill | Trigger | Description |
|-------|---------|-------------|
| `conductor` | `/conductor-*` | Context-driven development methodology |
| `beads` | `bd status` | Multi-session issue tracking |
| `beads/file-beads` | `fb` | Convert plans to beads issues |
| `beads/review-beads` | `rb` | Review filed beads issues |

### Planning & Execution

| Skill | Trigger | Description |
|-------|---------|-------------|
| `brainstorming` | `bs` | Deep exploration before implementation |
| `writing-plans` | `write plan` | Create implementation plans |
| `executing-plans` | `execute plan` | Execute plans with checkpoints |
| `spike-workflow` | `spike [topic]` | Time-boxed technical research |
| `retro-workflow` | `retro` | Capture lessons learned |

### Development

| Skill | Trigger | Description |
|-------|---------|-------------|
| `test-driven-development` | `tdd` | RED-GREEN-REFACTOR methodology |
| `testing-anti-patterns` | - | Avoid common testing mistakes |
| `using-git-worktrees` | - | Isolated feature development |
| `finishing-a-development-branch` | - | Complete and integrate work |
| `subagent-driven-development` | - | Subagent coordination |
| `dispatching-parallel-agents` | `dispatch` | Parallel task execution |

### Debugging

| Skill | Trigger | Description |
|-------|---------|-------------|
| `systematic-debugging` | `debug` | Four-phase debugging methodology |
| `root-cause-tracing` | `trace` | Trace bugs backward through stack |
| `condition-based-waiting` | `flaky` | Replace timeouts with polling |
| `defense-in-depth` | - | Multi-layer validation |

### Code Review

| Skill | Trigger | Description |
|-------|---------|-------------|
| `requesting-code-review` | `review code` | Request structured reviews |
| `receiving-code-review` | - | Handle feedback with rigor |

### Meta

| Skill | Trigger | Description |
|-------|---------|-------------|
| `using-superpowers` | - | Session initialization |
| `verification-before-completion` | - | Evidence before assertions |
| `writing-skills` | `write skill` | Create new skills |
| `testing-skills-with-subagents` | - | Test skills before deployment |
| `sharing-skills` | `share skill` | Contribute skills upstream |

## Installation

```bash
# Add the marketplace
/plugin marketplace add ReinaMacCredy/my-workflow

# Install the plugin
/plugin install my-workflow
```

## Workflow Pipeline

```
PLANNING PHASE
  /conductor-setup (once per project)
       │
  /conductor-newtrack [description]
       │
       ├── Clarifying questions
       ├── Generate spec.md
       └── Generate plan.md
                │
                ▼
  fb (file beads) → bd issues created
                │
EXECUTION PHASE │
                ▼
  bd ready → claim issue → execute with TDD
                │
                ▼
  bd checkpoint → finishing-a-development-branch
                │
RETROSPECTIVE   │
                ▼
  bd close → retro → history/retros/
```

## Manual Specialist Tools

Outside the automated flow:
- `bs` (brainstorm) - Deep exploration for complex unknowns
- `spike [topic]` - Time-boxed research
- `debug` - Systematic debugging
- `retro` - Capture lessons learned

## Credits

Built on foundations from:
- [superpowers](https://github.com/obra/superpowers) by Jesse Vincent
- [conductor](https://github.com/anthropics/conductor) 
- [beads](https://github.com/anthropics/beads)

## License

MIT
