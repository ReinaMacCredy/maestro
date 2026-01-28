# Architecture

System architecture, agent hierarchy, and workflow pipeline for Maestro.

## System Overview

```mermaid
flowchart TB
    subgraph WORKFLOW["MAESTRO WORKFLOW"]
        direction TB
        PLAN["@plan"] --> INTERVIEW["Interview"]
        INTERVIEW --> METIS["@metis (Gap Analysis)"]
        METIS --> MOMUS["@momus (Review)"]
        MOMUS --> WORK["/atlas-work"]
        WORK --> EXECUTE["Execution"]
    end

    subgraph ARTIFACTS["ARTIFACTS"]
        PLANS[".claude/plans/"]
        DRAFTS[".atlas/drafts/"]
        WISDOM[".atlas/notepads/"]
    end

    INTERVIEW --> DRAFTS
    MOMUS --> PLANS
    EXECUTE --> WISDOM
```

## Agent Hierarchy

12 specialized agents with clear delegation patterns:

| Agent | Purpose | Model | Chains To |
|-------|---------|-------|-----------|
| `atlas-prometheus` | Strategic planner, interview mode | sonnet | metis, momus, oracle |
| `atlas-orchestrator` | Master delegator (never works directly) | sonnet | ALL implementing + read-only agents |
| `atlas-leviathan` | Focused task executor | sonnet | (terminal) |
| `atlas-kraken` | TDD implementation | sonnet | (terminal) |
| `atlas-spark` | Quick fixes | sonnet | (terminal) |
| `atlas-oracle` | Strategic advisor | opus | (read-only) |
| `atlas-explore` | Codebase search | sonnet | (read-only) |
| `atlas-librarian` | External docs | sonnet | (read-only) |
| `atlas-metis` | Pre-planning consultant | sonnet | (read-only) |
| `atlas-momus` | Plan reviewer | sonnet | (read-only) |
| `atlas-code-reviewer` | Code quality review | sonnet | (read-only) |
| `atlas-document-writer` | Technical documentation | sonnet | (terminal) |

### Agent Chaining Rules

```
atlas-prometheus → atlas-metis, atlas-momus, atlas-oracle (consultation only)
         ↓
    [plan file]
         ↓
atlas-orchestrator → atlas-leviathan/kraken/spark + ALL read-only agents
         ↓
     [terminal executors - no further delegation]
```

- **Prometheus** chains to consultants during planning
- **Orchestrator** delegates ALL work (never edits directly)
- **Terminal agents** (leviathan, kraken, spark) do actual implementation
- **Read-only agents** (oracle, explore, librarian, metis, momus, code-reviewer) only analyze

## Complete Pipeline

```mermaid
flowchart TB
    subgraph PLANNING["PLANNING (@plan)"]
        PL_START["@plan"] --> PL_INTERVIEW["Interview"]
        PL_INTERVIEW --> PL_DRAFT["Draft Plan"]
        PL_DRAFT --> PL_METIS["Metis: Gap Analysis"]
        PL_METIS --> PL_MOMUS["Momus: Review Loop"]
        PL_MOMUS --> PL_OKAY{{"OKAY?"}}
        PL_OKAY -->|"No"| PL_DRAFT
        PL_OKAY -->|"Yes"| PL_DONE["Plan Ready"]
    end

    subgraph EXECUTION["/atlas-work"]
        EX_LOAD["Load Plan"] --> EX_ORCH["Orchestrator"]
        EX_ORCH --> EX_SELECT["Select Agent"]
        EX_SELECT --> EX_TDD["TDD Cycle"]
        EX_TDD --> EX_VERIFY["Verify Results"]
        EX_VERIFY --> EX_MORE{{"More Tasks?"}}
        EX_MORE -->|"Yes"| EX_SELECT
        EX_MORE -->|"No"| EX_WISDOM["Extract Wisdom"]
    end

    PL_DONE --> EX_LOAD
```

### Agent Selection Logic

```mermaid
flowchart TD
    TASK["Task Description"] --> CHECK_TDD{{"Contains TDD/refactor/heavy?"}}
    CHECK_TDD -->|"Yes"| KRAKEN["atlas-kraken"]
    CHECK_TDD -->|"No"| CHECK_SIMPLE{{"Contains typo/simple/quick?"}}
    CHECK_SIMPLE -->|"Yes"| SPARK["atlas-spark"]
    CHECK_SIMPLE -->|"No"| LEVIATHAN["atlas-leviathan"]
```

### TDD Micro-Loop

```mermaid
flowchart LR
    RED["RED: Failing Test"] --> GREEN["GREEN: Make Pass"]
    GREEN --> REFACTOR["REFACTOR: Clean"]
    REFACTOR --> RED
```

**Iron Law**: No production code without a failing test first.

## Planning Chain

```mermaid
flowchart LR
    TRIGGER["@plan"] --> PROMETHEUS["atlas-prometheus"]
    PROMETHEUS --> METIS["atlas-metis"]
    METIS --> MOMUS["atlas-momus"]
    MOMUS --> PLAN["Plan File"]
```

### Phase Details

| Phase | Agent | Purpose | Output |
|-------|-------|---------|--------|
| Interview | atlas-prometheus | Ask clarifying questions | `.atlas/drafts/` |
| Gap Analysis | atlas-metis | Identify hidden requirements | Feedback |
| Review Loop | atlas-momus | Validate plan quality | "OKAY" or revisions |
| Finalize | atlas-prometheus | Generate plan | `.claude/plans/` |

## Execution Chain

```mermaid
flowchart LR
    WORK["/atlas-work"] --> ORCH["atlas-orchestrator"]
    ORCH --> TASK["Task()"]
    TASK --> AGENT["leviathan/kraken/spark"]
    AGENT --> VERIFY["Verify"]
    VERIFY --> WISDOM["Wisdom"]
```

### Orchestrator Protocol

| Phase | Action |
|-------|--------|
| 1. Load | Find most recent plan in `.claude/plans/` |
| 2. Initialize | Create `.atlas/boulder.json` execution state |
| 3. Delegate | Task() to specialized agents with 7-section prompts |
| 4. Verify | Verify subagent claims (agents can make mistakes) |
| 5. Complete | Update plan checkboxes, extract wisdom to notepads |

### 7-Section Prompt Format

When spawning agents, orchestrator uses:

```markdown
## CONTEXT
## OBJECTIVE
## SCOPE
## REQUIREMENTS
## REQUIRED SKILLS
## CONSTRAINTS
## VERIFICATION
```

## Autonomous Chain (Ralph Loop)

```mermaid
flowchart LR
    RALPH["/ralph-loop"] --> ORCH["atlas-orchestrator"]
    ORCH --> EXECUTE["Execute Task"]
    EXECUTE --> CHECK{{"DONE?"}}
    CHECK -->|"No"| ORCH
    CHECK -->|"Yes"| COMPLETE["<promise>DONE</promise>"]
```

| Phase | Action |
|-------|--------|
| Start | `/ralph-loop` activates autonomous execution |
| During | Orchestrator iterates tasks, delegates, verifies |
| End | Detection of `<promise>DONE</promise>` stops loop |

## Session Flow

```mermaid
flowchart TB
    subgraph S1["SESSION 1: Planning"]
        S1_PLAN["@plan"] --> S1_INTERVIEW["Interview"]
        S1_INTERVIEW --> S1_READY["Plan Ready"]
    end

    subgraph S2["SESSION 2+: Execution"]
        S2_WORK["/atlas-work"]
        S2_EXECUTE["Execution"]
        S2_WORK --> S2_EXECUTE
    end

    subgraph S3["SESSION N: Autonomous"]
        S3_RALPH["/ralph-loop"]
        S3_DONE["<promise>DONE</promise>"]
        S3_RALPH --> S3_DONE
    end

    S1_READY -.->|"handoff"| S2_WORK
    S2_EXECUTE -.->|"handoff"| S3_RALPH
```

### Handoff Mechanism

| Artifact | Preserves |
|----------|-----------|
| `.claude/plans/` | Plan structure, task status |
| `.atlas/notepads/` | Accumulated wisdom |
| `.atlas/boulder.json` | Execution state |
| `.beads/` | Issue state, notes |

**At session end**: `bd update --notes "COMPLETED: X. NEXT: Y."`
**At session start**: `bd ready --json` → `bd show <id>`

## Directory Structure

```
.atlas/
├── plans/                    # Committed work plans
├── drafts/                   # Interview drafts
├── notepads/                 # Wisdom per plan
├── boulder.json              # Active execution state
└── ralph-loop.local.md       # Ralph autonomous loop state

.claude/
├── agents/                   # Agent definitions (symlinks)
├── commands/                 # Slash commands (/atlas-plan, etc.)
├── hooks/                    # Hook configuration
├── plans/                    # Generated execution plans
├── scripts/                  # Hook scripts
└── skills/
    └── atlas/                # Main workflow skill
        └── references/
            ├── agents/       # Atlas agent definitions
            ├── workflows/    # Workflow documentation
            └── guides/       # Usage guides

.beads/
├── beads.db                  # SQLite database
└── beads.jsonl               # Export format

toolboxes/
└── agent-mail/               # CLI wrapper for Agent Mail
```

## Quick Reference

| Component | Trigger | Purpose |
|-----------|---------|---------|
| Planning | `@plan` | Interview-driven planning |
| Gap Analysis | `@metis` | Pre-planning consultation |
| Review | `@momus` | Plan validation |
| Execution | `/atlas-work` | Orchestrated execution |
| Autonomous | `/ralph-loop` | Run until complete |
| Strategic | `@oracle` | High-IQ advice (opus) |
| Search | `@explore` | Codebase search |
| Research | `@librarian` | External docs |
| TDD | `@tdd` | Test-driven implementation |

### Fallback Policy

| Condition | Action |
|-----------|--------|
| `bd` unavailable | HALT |
| `.atlas/` missing | DEGRADE (standalone) |
| Agent Mail unavailable | HALT |

## Related

- [README.md](../README.md) — Overview and installation
- [TUTORIAL.md](../TUTORIAL.md) — Complete workflow guide
- [.claude/skills/atlas/SKILL.md](../.claude/skills/atlas/SKILL.md) — Atlas workflow skill
- [.claude/skills/atlas/references/agents/](../.claude/skills/atlas/references/agents/) — Agent definitions
