---
name: atlas
description: Interview-driven planning workflow that asks clarifying questions before generating implementation plans. Use for structured planning with requirements clarification.
---

# Atlas Workflow

> Interview-driven planning with Task()-based delegation (Claude Code compatible)

## Triggers

| Trigger | Action |
|---------|--------|
| `/ap <request>` | Start Prometheus interview mode |
| `/atlas-work` | Load plan and execute via orchestrator |
| `/ralph-loop` | Start autonomous completion loop |
| `/cancel-ralph` | Stop Ralph loop |
| `@plan`, `ultraplan` | Prometheus interview mode |
| `@oracle` | Delegate to strategic advisor |
| `@explore` | Delegate to codebase search |
| `@librarian` | Delegate to external research |
| `@momus` | Delegate to plan reviewer |
| `@metis` | Delegate to pre-planning consultant |
| `@review` | Delegate to code reviewer |
| `@docs` | Delegate to document writer |
| `@tdd` | Delegate to TDD implementation (atlas-kraken) |
| `ultrawork`, `ulw` | High-priority thoroughness mode |

## Core Principles

1. **Interview First** - Prometheus asks questions before generating plans
2. **Consult Before Planning** - Metis identifies gaps before plan generation
3. **Validate Plans** - Momus reviews until "OKAY"
4. **Delegate All Work** - Orchestrator never works directly
5. **Leviathan Executes** - atlas-leviathan does actual implementation
6. **Wisdom Accumulates** - Notepads persist learnings

## State Directory

All workflow state lives in `.atlas/`:
- `plans/` - Committed work plans
- `drafts/` - Interview drafts
- `notepads/` - Wisdom per plan
- `boulder.json` - Active execution state

## References

- [Routing & Keywords](references/workflows/router.md)
- [Planning Workflow](references/workflows/prometheus.md)
- [Execution Workflow](references/workflows/execution.md)
- [Delegation Categories](references/guides/delegation.md)
- [Ralph Loop](references/workflows/ralph.md)
- [Wisdom Accumulation](references/guides/wisdom.md)

## Agent Prompt Templates

> **Note**: All Atlas agents are self-contained in `skills/atlas/references/agents/` and symlinked to `.claude/agents/` for Claude Code compatibility.

**All Atlas Agents** (self-contained in this skill):
- [atlas-prometheus](references/agents/atlas-prometheus.md) - Strategic planner, interview mode
- [atlas-orchestrator](references/agents/atlas-orchestrator.md) - Master delegator, never works directly
- [atlas-leviathan](references/agents/atlas-leviathan.md) - Focused task executor
- [atlas-kraken](references/agents/atlas-kraken.md) - TDD implementation specialist
- [atlas-spark](references/agents/atlas-spark.md) - Quick fix specialist
- [atlas-oracle](references/agents/atlas-oracle.md) - Strategic advisor (opus)
- [atlas-explore](references/agents/atlas-explore.md) - Codebase search specialist
- [atlas-librarian](references/agents/atlas-librarian.md) - External docs/research
- [atlas-metis](references/agents/atlas-metis.md) - Pre-planning consultant
- [atlas-momus](references/agents/atlas-momus.md) - Plan reviewer
- [atlas-code-reviewer](references/agents/atlas-code-reviewer.md) - Code quality review
- [atlas-document-writer](references/agents/atlas-document-writer.md) - Technical documentation

---

## Entry Points

### /ap <request>

When triggered with `/ap`, enter Prometheus interview mode:

1. **Load Agent**: Read [atlas-prometheus](references/agents/atlas-prometheus.md)
2. **Create Draft**: Initialize `.atlas/drafts/{topic}.md`
3. **Interview Mode**: Ask clarifying questions, record to draft
4. **Clearance Check**: After each response, check if all requirements are clear
5. **If Clear**: Consult Metis, generate plan
6. **Optional Review**: If high accuracy requested, invoke Momus loop

### /atlas-work

When triggered with `/atlas-work`:

1. **Load Plan**: Find most recent plan in `.claude/plans/`
2. **Initialize Boulder**: Create execution state in `.atlas/boulder.json`
3. **Load Orchestration Skill**: Read [skills/orchestration/SKILL.md](../orchestration/SKILL.md)
4. **Execute**: Orchestrator delegates ALL work to atlas agents
5. **Complete**: Update plan checkboxes, save wisdom

### /ralph-loop

When triggered with `/ralph-loop`:

1. **Load Ralph State**: Check `.atlas/ralph-loop.local.md`
2. **Continue Work**: Execute until `<promise>DONE</promise>` detected
3. **Auto-Continue**: Self-invoke to continue after each task

### /cancel-ralph

When triggered with `/cancel-ralph`:

1. **Clear State**: Reset `.atlas/ralph-loop.local.md` to inactive state
2. **Stop Continuation**: Do NOT self-invoke or continue execution
3. **Notify User**: Confirm cancellation with status

**Implementation**: Run `${CLAUDE_PLUGIN_ROOT}/scripts/cancel-ralph.sh`

**Response Template**:

```
[OK] Ralph loop cancelled
  - State cleared: .atlas/ralph-loop.local.md
  - No further autonomous execution will occur

To resume work manually, use `/atlas-work`.
```

**Note**: This command takes effect immediately. Any in-flight atlas agents will complete their current task but no new tasks will be delegated.

---

## Component Registry

> **Central Reference**: This registry is the single source of truth for all Atlas components. All agents, skills, and commands reference this registry.

### Agents (spawn via Task)

> **Note**: All agents including prometheus are proper subagents spawned via Task tool. The `@plan` keyword triggers spawning prometheus as a subagent, not a "mode" of main Claude.

| Agent | Purpose | Skills Loaded | Model | Chains To |
|-------|---------|---------------|-------|-----------|
| `atlas-prometheus` | Strategic planner, interview mode | atlas | sonnet | atlas-metis, atlas-momus, atlas-oracle |
| `atlas-orchestrator` | Master delegator, never works directly | atlas | sonnet | atlas-leviathan, atlas-explore, atlas-librarian, atlas-oracle, atlas-document-writer, atlas-kraken, atlas-spark, atlas-code-reviewer |
| `atlas-leviathan` | Focused task executor | atlas, git-master, playwright | sonnet | (terminal - no delegation) |
| `atlas-oracle` | Strategic advisor, high-IQ reasoning | atlas | opus | (terminal - read-only) |
| `atlas-explore` | Codebase search specialist | atlas | sonnet | (terminal - read-only) |
| `atlas-librarian` | External docs/research | atlas | sonnet | (terminal - read-only) |
| `atlas-metis` | Pre-planning consultant, gap analysis | atlas | sonnet | (terminal - read-only) |
| `atlas-momus` | Plan reviewer, ruthless critic | atlas | sonnet | (terminal - read-only) |
| `atlas-code-reviewer` | Code quality review (@review) | atlas | sonnet | (terminal - read-only) |
| `atlas-document-writer` | Technical documentation (@docs) | atlas | sonnet | (terminal - implements) |
| `atlas-kraken` | TDD implementation, heavy refactor (@tdd) | atlas, git-master | sonnet | (terminal - implements) |
| `atlas-spark` | Quick fixes, simple changes (orchestrator-only) | atlas, git-master | sonnet | (terminal - implements) |

### Skills (invoke via Skill tool or /skill)

| Skill | Purpose | Chains To |
|-------|---------|-----------|
| `atlas` | Workflow orchestration | prometheus, orchestrator, all commands |
| `orchestration` | Orchestration mode - delegate to specialized agents | (loaded by /atlas-work) |
| `git-master` | Git operations | - |
| `playwright` | Browser automation | - |

### Commands (invoke via /atlas:command)

| Command | Purpose | Spawns Agent | Uses Skills |
|---------|---------|--------------|-------------|
| `/atlas:atlas-plan` | Interview-driven planning | atlas-prometheus (DIRECT_GENERATE) | atlas |
| `/atlas:atlas-work` | Execute plan | - | atlas, orchestration |
| `/atlas:ralph-loop` | Autonomous loop | atlas-orchestrator | atlas |
| `/atlas:cancel-ralph` | Stop loop | - | - |
| `/atlas:refactor` | Refactor workflow | atlas-kraken | atlas, git-master |
| `/atlas:init-deep` | Generate AGENTS.md files | atlas-explore | atlas |

### Hooks (auto-triggered)

| Hook | Trigger | Purpose |
|------|---------|---------|
| `keyword-detector.sh` | UserPromptSubmit | Routes @keywords to spawn agents |
| `orchestrator-guard.sh` | PreToolUse(Write/Edit) | Blocks orchestrator from direct edits |
| `verification-injector.sh` | PostToolUse(Task) | Reminds to verify task results |
| `ralph-loop-handler.sh` | Stop | Continues Ralph loop if active |
| `registry-injector.sh` | UserPromptSubmit | Injects component registry context |

---

## Agent Selection Matrix

When `/atlas-work` loads the orchestration skill, apply this selection logic:

### Selection Rules (in order of precedence)

| Condition | Agent | Rationale |
|-----------|-------|-----------|
| Task mentions "TDD", "test-driven", "refactor" | `atlas-kraken` | Requires red-green-refactor cycle |
| Task mentions "heavy", "complex", "multi-file" | `atlas-kraken` | Needs structured approach |
| Task is < 10 lines, single file | `atlas-spark` | Quick fix, minimal overhead |
| Task mentions "fix typo", "update config", "small" | `atlas-spark` | Simple change |
| Default (no keywords match) | `atlas-leviathan` | General implementation |

### Implementation

The orchestrator (when loaded with atlas skill) parses the task description for keywords:

```javascript
function selectAtlasAgent(taskDescription) {
  const desc = taskDescription.toLowerCase();

  // Kraken patterns (TDD, heavy work)
  if (/\b(tdd|test-driven|refactor|heavy|complex|multi-file)\b/.test(desc)) {
    return 'atlas-kraken';
  }

  // Spark patterns (quick fixes)
  if (/\b(typo|config|small|simple|quick|minor)\b/.test(desc)) {
    return 'atlas-spark';
  }

  // Default: Leviathan
  return 'atlas-leviathan';
}
```

### Manual Override

Orchestrator can override auto-selection by specifying agent in 7-section prompt:
```
## REQUIRED SKILLS
Use agent: atlas-kraken (override auto-selection)
```

---

## Self-Chaining Patterns

### Planning Chain
```
@plan → atlas-prometheus → atlas-metis (gap analysis) → atlas-momus (review loop) → plan file
```

### Execution Chain
```
/atlas-work → atlas-orchestrator → atlas-leviathan/atlas-explore/atlas-librarian → verification → wisdom
```

### Autonomous Chain
```
/ralph-loop → atlas-orchestrator → atlas-leviathan → verification-injector → ralph-loop-handler → continue
```

### Research Chain
```
@oracle → atlas-oracle (opus, strategic)
@explore → atlas-explore (codebase search)
@librarian → atlas-librarian (external docs)
```

### Review Chain
```
@momus → atlas-momus → OKAY or feedback → atlas-prometheus revise
```

### Implementation Specialists
```
atlas-orchestrator → atlas-kraken (TDD, heavy work)
atlas-orchestrator → atlas-spark (quick fixes)
atlas-orchestrator → atlas-document-writer (docs)
atlas-orchestrator → atlas-code-reviewer (quality review)
```

---

## Chaining Rules

### Who Can Chain to Whom

| From | Can Chain To |
|------|--------------|
| `atlas-prometheus` | atlas-metis, atlas-momus, atlas-oracle (consultation only) |
| `atlas-orchestrator` | ALL implementing agents (atlas-leviathan, atlas-kraken, atlas-spark, atlas-document-writer) + ALL read-only agents (atlas-code-reviewer, atlas-explore, atlas-librarian, atlas-oracle) |
| `atlas-leviathan` | NONE (terminal executor) |
| `atlas-kraken` | NONE (terminal executor) |
| `atlas-spark` | NONE (terminal executor) |
| Read-only agents | NONE (atlas-oracle, atlas-explore, atlas-librarian, atlas-metis, atlas-momus, atlas-code-reviewer) |

### Skills Auto-Loading

When an agent spawns, the `skills:` field in its YAML frontmatter auto-loads skill knowledge:
- `atlas` → This registry and workflow context
- `git-master` → Git operations and commit patterns
- `playwright` → Browser automation and testing

---

### Agent Location

**All Atlas agents** are defined in `skills/atlas/references/agents/`:
- All 12 agents: `atlas-prometheus.md`, `atlas-orchestrator.md`, `atlas-leviathan.md`, `atlas-kraken.md`, `atlas-spark.md`, `atlas-oracle.md`, `atlas-explore.md`, `atlas-librarian.md`, `atlas-metis.md`, `atlas-momus.md`, `atlas-code-reviewer.md`, `atlas-document-writer.md`
- Symlinked to `.claude/agents/` for Claude Code compatibility
- Each agent has YAML frontmatter with `name`, `description`, `tools`, `model`
- The `skills: atlas` field auto-loads this workflow context when the agent is spawned
