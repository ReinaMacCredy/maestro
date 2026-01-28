# Maestro Reference

Quick-lookup for commands, triggers, and technical details.

---

## Commands

| Command | Alias | Description |
|---------|-------|-------------|
| `/atlas-plan` | `@plan` | Start Prometheus interview mode |
| `/atlas-work` | - | Execute plan via orchestrator |
| `/ralph-loop` | - | Autonomous execution loop |
| `/cancel-ralph` | - | Stop Ralph loop |

---

## Triggers

| Trigger | Agent | Action |
|---------|-------|--------|
| `@plan`, `ultraplan` | atlas-prometheus | Interview-driven planning |
| `/atlas-work` | atlas-orchestrator | Execute plan via delegation |
| `/ralph-loop` | atlas-orchestrator | Autonomous loop |
| `@oracle` | atlas-oracle | Strategic advice (opus) |
| `@explore` | atlas-explore | Codebase search |
| `@librarian` | atlas-librarian | External docs research |
| `@metis` | atlas-metis | Pre-planning gap analysis |
| `@momus` | atlas-momus | Plan review (approve with OKAY) |
| `@tdd` | atlas-kraken | TDD implementation |
| `@review` | atlas-code-reviewer | Code quality review |
| `@docs` | atlas-document-writer | Technical documentation |
| `fb` | - | File beads from plan |
| `rb` | - | Review beads |
| `finish branch` | - | Finalize and merge/PR |

---

## Skills

4 skills organized by workflow phase:

### Core Workflow

| Skill | Description |
|-------|-------------|
| **atlas** | Interview-driven planning and orchestrated execution. All `@keyword` triggers and `/atlas-*` commands. |
| **orchestration** | Task()-based delegation for `/atlas-work`. Orchestrator never works directly. |

### Utility Skills

| Skill | Description |
|-------|-------------|
| **git-master** | Git operations, atomic commits, rebasing. Loaded by implementation agents. |
| **playwright** | Browser automation, E2E testing. Loaded by atlas-leviathan when needed. |

---

## Atlas Agents

12 specialized agents spawned via Task():

| Agent | Purpose | Model | Trigger |
|-------|---------|-------|---------|
| `atlas-prometheus` | Strategic planner, interview mode | sonnet | `@plan` |
| `atlas-orchestrator` | Master delegator (never works directly) | sonnet | `/atlas-work` |
| `atlas-leviathan` | Focused task executor | sonnet | (orchestrator) |
| `atlas-kraken` | TDD implementation, heavy refactors | sonnet | `@tdd` |
| `atlas-spark` | Quick fixes, simple changes | sonnet | (orchestrator) |
| `atlas-oracle` | Strategic advisor | opus | `@oracle` |
| `atlas-explore` | Codebase search | sonnet | `@explore` |
| `atlas-librarian` | External docs research | sonnet | `@librarian` |
| `atlas-metis` | Pre-planning consultant | sonnet | `@metis` |
| `atlas-momus` | Plan reviewer | sonnet | `@momus` |
| `atlas-code-reviewer` | Code quality review | sonnet | `@review` |
| `atlas-document-writer` | Technical documentation | sonnet | `@docs` |

### Agent Chaining

| From | Can Chain To |
|------|--------------|
| `atlas-prometheus` | atlas-metis, atlas-momus, atlas-oracle |
| `atlas-orchestrator` | ALL implementing + read-only agents |
| `atlas-leviathan` | NONE (terminal executor) |
| `atlas-kraken` | NONE (terminal executor) |
| `atlas-spark` | NONE (terminal executor) |
| Read-only agents | NONE (atlas-oracle, atlas-explore, atlas-librarian, atlas-metis, atlas-momus, atlas-code-reviewer) |

### Agent Selection (Orchestrator)

| Task Type | Agent | Rationale |
|-----------|-------|-----------|
| TDD, refactor, heavy, complex, multi-file | `atlas-kraken` | Red-green-refactor cycle |
| Typo, config, small, simple, quick, minor | `atlas-spark` | Quick fix, minimal overhead |
| Default | `atlas-leviathan` | General implementation |

---

## bd CLI Cheatsheet

### Essential Commands

| Command | Action |
|---------|--------|
| `bd ready --json` | Find available work |
| `bd show <id>` | Read task context |
| `bd update <id> --status in_progress` | Claim task |
| `bd update <id> --notes "..."` | Add notes |
| `bd close <id> --reason completed` | Close (completed/skipped/blocked) |
| `bd status` | Show ready + in_progress |
| `bd sync` | Sync to git |

### Session Pattern

```bash
# Start
bd ready --json                      # Find work
bd show <id>                         # Read context
bd update <id> --status in_progress  # Claim

# During (heartbeat every 5 min)
# ...work...

# End
bd update <id> --notes "COMPLETED: X. NEXT: Y"
bd close <id> --reason completed
bd sync
```

### bv (Beads Validation)

```bash
bv --robot-stdout    # Machine-readable (NEVER run bare `bv`)
bv --robot-status    # Status code only
```

**WARNING:** Bare `bv` hangs. Always use `--robot-*` flags.

---

## Directory Structure

```
.atlas/
├── plans/                    # Committed work plans
├── drafts/                   # Interview drafts
├── notepads/                 # Wisdom per plan
└── boulder.json              # Active execution state

.claude/
├── agents/                   # Agent definitions (symlinks)
├── commands/                 # Slash commands
├── hooks/                    # Hook configuration
├── plans/                    # Generated execution plans
├── scripts/                  # Hook scripts
└── skills/
    └── atlas/                # Main workflow skill
        └── references/
            └── agents/       # Atlas agent definitions

.beads/
├── beads.db                  # SQLite database
└── beads.jsonl               # Export format

toolboxes/
└── agent-mail/               # CLI wrapper for Agent Mail
```

---

## Fallback Policies

| Condition | Action | Message |
|-----------|--------|---------|
| `bd` unavailable | **HALT** | `Cannot proceed: bd CLI required` |
| `.atlas/` missing | **DEGRADE** | `Standalone mode - limited features` |
| Agent Mail unavailable | **HALT** | `Cannot proceed: Agent Mail required` |

### Decision Tree: bd vs TodoWrite

```
bd available?
├─ YES → Use bd CLI
└─ NO  → HALT (do NOT use TodoWrite as fallback)
```

---

## Session Protocol

### First Message

1. Check `.atlas/plans/` for active plans
2. If found: display plan status and available work
3. Skip if: "fresh start", no `.atlas/`, or plans stale

### Ralph (Autonomous Mode)

| Phase | Action |
|-------|--------|
| Start | `/ralph-loop` activates autonomous execution |
| During | Iterates tasks, delegates to agents, verifies results |
| End | Detection of `<promise>DONE</promise>` stops loop |

**Exclusive Lock:** Manual commands blocked while Ralph is active.

---

## MCPorter Toolboxes

CLI tools generated from MCP servers via [MCPorter](https://github.com/steipete/mcporter).

### Location

`toolboxes/<tool>/<tool>.js`

### Available Tools

| CLI | Source | Description |
|-----|--------|-------------|
| `agent-mail/agent-mail.js` | mcp-agent-mail | Agent coordination and messaging |

### Usage

```bash
# From project root
toolboxes/agent-mail/agent-mail.js <command> [args...]

# Example: health check
toolboxes/agent-mail/agent-mail.js health-check

# Example: send message
toolboxes/agent-mail/agent-mail.js send_message to:BlueLake subject:"Hello"
```

### Argument Syntax

```bash
# Colon-delimited
agent-mail.js send_message to:BlueLake subject:"Hello"

# Equals-delimited
agent-mail.js send_message to=BlueLake subject="Hello"

# Function-call style
agent-mail.js 'send_message(to: "BlueLake", subject: "Hello")'
```

---

## Orchestrator Protocol

### Execution Flow

| Phase | Action |
|-------|--------|
| 1. Load Plan | Find most recent plan in `.claude/plans/` |
| 2. Initialize | Create execution state in `.atlas/boulder.json` |
| 3. Delegate | Task() to specialized agents (leviathan/kraken/spark) |
| 4. Verify | Verify subagent results (agents can make mistakes) |
| 5. Complete | Update plan checkboxes, extract wisdom |

### 7-Section Prompt Format

When spawning agents, orchestrator uses this format:

```markdown
## CONTEXT
[Background and current state]

## OBJECTIVE
[Clear goal statement]

## SCOPE
[What's in/out of scope]

## REQUIREMENTS
[Specific acceptance criteria]

## REQUIRED SKILLS
[Skills and agent to use]

## CONSTRAINTS
[Technical/process constraints]

## VERIFICATION
[How to verify success]
```

---

## Troubleshooting

| Problem | Cause | Solution |
|---------|-------|----------|
| `bv` hangs | Missing `--robot-*` flag | Always use `bv --robot-stdout` |
| Task not found | Stale beads cache | Run `bd sync` |
| Agent Mail error | CLI not available | Check `toolboxes/agent-mail/agent-mail.js health-check` |
| Orchestrator editing directly | Bug | Must always delegate via Task() |
| Skill not loading | Wrong trigger | Check trigger table above |

### Validation Checklist

```bash
# Verify beads CLI
bd --version

# Verify Agent Mail
toolboxes/agent-mail/agent-mail.js health-check

# Verify project structure
ls .atlas/
ls .beads/

# Validate plugin manifest
cat .claude-plugin/plugin.json | jq .
```

---

## Critical Rules

1. Use `--json` with `bd` for structured output
2. Use `--robot-*` with `bv` (bare `bv` hangs)
3. Never write production code without failing test first (TDD)
4. Always commit `.beads/` with code changes
5. Orchestrator NEVER edits directly - always delegates via Task()
6. Always verify subagent claims - agents can make mistakes
7. Use 7-section prompts for Task() calls (50-200 lines)

---

## See Also

| Topic | Path |
|-------|------|
| Full tutorial | [TUTORIAL.md](TUTORIAL.md) |
| Agent guidance | [AGENTS.md](AGENTS.md) |
| Atlas workflow | [.claude/skills/atlas/SKILL.md](.claude/skills/atlas/SKILL.md) |
| Agent definitions | [.claude/skills/atlas/references/agents/](.claude/skills/atlas/references/agents/) |
| Router & keywords | [.claude/skills/atlas/references/workflows/router.md](.claude/skills/atlas/references/workflows/router.md) |
| Delegation guide | [.claude/skills/atlas/references/guides/delegation.md](.claude/skills/atlas/references/guides/delegation.md) |
