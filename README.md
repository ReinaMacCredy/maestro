# Maestro

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

This plugin is my **personal source of truth** for structured AI-assisted development.
It contains the exact skills, workflows, and patterns I use across _every_ project — continuously updated as I discover better ways to work.

Think of this as the **official, up-to-date playbook** for context-driven development:
how I plan features, how I track work across sessions, how I debug systematically, and how the whole system stays reliable.

If you're exploring structured AI coding workflows, this plugin shows the stack that actually works:
a combination of persistent memory (Beads), structured planning (Conductor), TDD methodology, and systematic debugging.
These workflows are not theoretical — they're refined daily through real builds.

Everything here evolves as I refine my process.
If a skill is in this plugin, it's because I actively use it.

## Install

### Claude Code

**Plugin install (recommended):**

```
/plugin install https://github.com/ReinaMacCredy/maestro
```

**Or via agent prompt:**

```
Follow https://raw.githubusercontent.com/ReinaMacCredy/maestro/main/SETUP_GUIDE.md to install Maestro.
```

### OpenAI Codex

**Quick install/update (one command):**

```bash
curl -fsSL https://raw.githubusercontent.com/ReinaMacCredy/maestro/main/scripts/install-codex.sh | bash
```

**Install (recommended, updateable):**

```bash
git clone https://github.com/ReinaMacCredy/maestro.git ~/.codex/skills/maestro
```

**Update:**

```bash
git -C ~/.codex/skills/maestro pull
```

**Install a specific skill only (optional):**

```bash
python ~/.codex/skills/.system/skill-installer/scripts/install-skill-from-github.py --repo ReinaMacCredy/maestro --path skills/conductor
```

Restart Codex to pick up new skills.

### Amp

```bash
amp skill add https://github.com/ReinaMacCredy/maestro --global
```

### Cursor / Windsurf / Other tools

Tell your AI agent:

```
Follow https://raw.githubusercontent.com/ReinaMacCredy/maestro/main/SETUP_GUIDE.md to install Maestro.
```

### Manual Setup

See [SETUP_GUIDE.md](./SETUP_GUIDE.md) for detailed instructions.

---

## Table of Contents

- [Quick Start for Agents](#quick-start-for-agents)
- [The Skills](#the-skills)
- [Skill Reference](#skill-reference)
  - [Conductor (Planning)](#conductor-planning)
  - [Beads (Issue Tracking)](#beads-issue-tracking)
  - [TDD (Execution)](#tdd-execution)
- [Workflow Pipeline](#workflow-pipeline)
- [Understanding Handoff](#understanding-handoff)
- [Slash Commands](#slash-commands)
- [Documentation](#documentation)
- [Troubleshooting](#troubleshooting)

---

## Quick Start for Agents

### The Complete Workflow

```
/conductor-setup                   # 1. Initialize project (once)
ds                                 # 2. Design session → design.md
/conductor-newtrack                # 3. Create spec + plan + beads + review
/conductor-implement               # 4. Execute with TDD
```

### Quick Start (Existing Project)

```bash
# Start
bd ready --json                    # What's available?
/conductor-implement               # Execute next task (auto-claims from beads)

# Work (with TDD)
tdd                                # Enter TDD mode
# RED → GREEN → REFACTOR

# Session ends automatically tracked via beads
```

### When Stuck

```
debug                              # Systematic debugging (external: superpowers)
trace                              # Root cause tracing (external: superpowers)
/conductor-design                  # Design alternatives
```

### Rules

- **Always** use `--robot-*` flags with `bv` (bare `bv` launches TUI and will hang)
- **Always** use `--json` flags with `bd` for structured output
- **Never** write production code without a failing test first (TDD)
- **Always** commit `.beads/` with your code changes

---

## The Skills

| Category          | Skills                                                                                                        |
| ----------------- | ------------------------------------------------------------------------------------------------------------- |
| **Core Workflow** | conductor, design (Double Diamond + Party Mode), beads                                                        |
| **Development**   | test-driven-development, using-git-worktrees, finishing-a-development-branch                                  |
| **Utilities**     | dispatching-parallel-agents, subagent-driven-development, [agent-coordination](workflows/agent-coordination/) |
| **Meta**          | verification-before-completion, writing-skills, sharing-skills                                                |

---

## Skill Reference

### Conductor (Planning & Design)

**What it does**: Structured design and planning flow that turns ideas into `design.md`, `spec.md` and `plan.md`.

**Design Sessions (Double Diamond)**: `/conductor-design` (or `ds` trigger) runs a Double Diamond design session with four phases: DISCOVER → DEFINE → DEVELOP → DELIVER. Each phase ends with A/P/C checkpoints:

- **[A] Advanced**: Deeper analysis, assumption audit
- **[P] Party**: Multi-agent collaborative review (see `workflows/party-mode/`)
- **[C] Continue**: Proceed to next phase

```mermaid
flowchart TB
    subgraph PIPELINE["WORKFLOW PIPELINE"]
        direction TB

        subgraph SETUP["SETUP"]
            TRIGGER["ds / /conductor-design"]
            CHECK["Verify conductor/<br/>(product.md, tech-stack.md, workflow.md)"]
        end

        subgraph DIAMOND1["DIAMOND 1: UNDERSTAND PROBLEM"]
            DISCOVER["DISCOVER (Diverge)<br/>• Explore problem space<br/>• 5 Whys, edge cases<br/>• Mini-ground: codebase check"]
            APC1{"A/P/C"}
            DEFINE["DEFINE (Converge)<br/>• Problem statement<br/>• Success criteria<br/>• YAGNI filtering"]
            APC2{"A/P/C"}
        end

        subgraph DIAMOND2["DIAMOND 2: DESIGN SOLUTION"]
            DEVELOP["DEVELOP (Diverge)<br/>• 3+ approaches<br/>• Trade-off analysis<br/>• Wild/10x option"]
            APC3{"A/P/C"}
            DELIVER["DELIVER (Converge)<br/>• Architecture, Components<br/>• Data Model, User Flow<br/>• FULL GROUNDING required"]
            APC4{"A/P/C"}
        end

        subgraph HANDOFF["HANDOFF"]
            DESIGND["design.md saved to<br/>conductor/tracks/{id}/"]
            NEXT["Next: /conductor-newtrack {track_id}<br/>(spec + plan + beads + review)"]
        end
    end

    subgraph AGENTS["PARTY MODE: 12 AGENTS (BMAD v6)"]
        subgraph PRODUCT["Product Module"]
            PM["John (PM)"]
            ANALYST["Mary (Analyst)"]
            UX["Sally (UX)"]
        end

        subgraph TECHNICAL["Technical Module"]
            ARCH["Winston (Architect)"]
            DEV["Amelia (Developer)"]
            QA["Murat (QA)"]
            DOCS["Paige (Docs)"]
        end

        subgraph CREATIVE["Creative Module"]
            STORY["Sophia (Storyteller)"]
            BRAIN["Carson (Brainstorm)"]
            DESIGN["Maya (Design Thinking)"]
            STRAT["Victor (Strategist)"]
            SOLVER["Dr. Quinn (Solver)"]
        end
    end

    subgraph VALIDATION["VALIDATION (Phase 0)"]
        VALIDATE["/conductor-validate"]
        V_FLOW["Checks: path → dir → files → JSON → state → track_id → staleness"]
        V_OUT{{"PASS / HALT / Auto-repair"}}
    end

    TRIGGER --> CHECK
    CHECK --> DISCOVER
    DISCOVER --> APC1
    APC1 -->|C| DEFINE
    APC1 -.->|Back| DISCOVER
    DEFINE --> APC2
    APC2 -->|C| DEVELOP
    APC2 -.->|Back| DISCOVER
    DEVELOP --> APC3
    APC3 -->|C| DELIVER
    APC3 -.->|Back| DEFINE
    DELIVER --> APC4
    APC4 -->|C| DESIGND
    APC4 -.->|Back| DEVELOP
    DESIGND --> NEXT

    APC1 & APC2 & APC3 & APC4 -.->|P| AGENTS
    AGENTS -.->|"Synthesize"| APC1 & APC2 & APC3 & APC4

    VALIDATE --> V_FLOW --> V_OUT
    NEXT -.->|"Phase 0"| VALIDATE
```

**Triggers**:

```
/conductor-setup                   # Initialize project (once)
/conductor-design "description"    # Design through dialogue → design.md
/conductor-newtrack                # Create spec.md + plan.md from design
/conductor-implement               # Execute tasks using beads
/conductor-status                  # View progress
/conductor-revert                  # Git-aware revert of work
/conductor-revise                  # Update spec/plan mid-track
/conductor-finish                  # Complete track: learnings, context refresh, archive
```

**Output structure**:

```
conductor/
├── product.md              # Product vision
├── tech-stack.md           # Technology choices
├── workflow.md             # Development standards
├── tracks.md               # Master track list
├── AGENTS.md               # Learnings hub (auto-updated by /conductor-finish)
├── CODEMAPS/               # Token-aware architecture docs
│   ├── overview.md         # System architecture overview
│   └── <module>.md         # Per-module codemaps
├── archive/                # Completed tracks
└── tracks/<track_id>/
    ├── design.md           # High-level design (from /conductor-design)
    ├── spec.md             # Requirements + acceptance
    ├── plan.md             # Phased task list
    └── LEARNINGS.md        # Track learnings (created by /conductor-finish)
```

**Key insight**: Spend tokens once on a good plan; reuse it many times.

### Beads (Issue Tracking)

**What it does**: Persistent issue tracking across sessions with dependency graphs. **Integrated with Conductor** via facade pattern for zero-manual-bd-commands workflow.

**Conductor Integration** (automatic, no manual bd required):
- `/conductor-newtrack` → creates epic + issues from plan.md
- `/conductor-implement` → claims, TDD checkpoints, closes tasks
- `/conductor-finish` → compacts summaries, cleans up old issues

**Commands** (requires `bd` CLI):

```bash
# Finding work
bd ready --json              # What's unblocked?
bd blocked --json            # What's waiting?
bd list --status in_progress # What's active?

# Working
bd update bd-123 --status in_progress   # Claim task
bd show bd-123                          # Read context
bd close bd-123 --reason "Done"         # Complete

# Dependencies
bd dep add bd-child bd-blocker --type blocks
bd dep tree bd-123
```

**Skill triggers**:

```
fb                          # File beads from plan (parallel subagents)
rb                          # Review filed beads (parallel + cross-epic validation)
bd status                   # Check project status
```

**Key insight**: Beads survive context compaction; chat history doesn't.

### TDD (Execution)

**What it does**: RED-GREEN-REFACTOR methodology for safe implementation.

**Trigger**: Say `tdd` to enter TDD mode.

**The cycle**:

```
RED     → Write one failing test (watch it fail)
GREEN   → Write minimal code to pass (watch it pass)
REFACTOR → Clean up (stay green)
REPEAT  → Next failing test
```

**Iron law**: No production code without a failing test first.

**Key insight**: If you didn't watch the test fail, you don't know if it tests the right thing.

---

## Workflow Pipeline

### Complete Workflow Architecture

```mermaid
flowchart TB
    subgraph PIPELINE["COMPLETE PIPELINE WORKFLOW"]
        direction TB

        subgraph PREFLIGHT["PREFLIGHT (All Commands)"]
            PF_START["Session Start"]
            PF_MODE["Mode Detection<br/>(SA/MA)"]
            PF_BD["Validate bd CLI"]
            PF_STATE["Create Session State"]
        end

        subgraph PLANNING["PLANNING LOOP"]
            DS["ds (Design Session)"]
            DISCOVER["DISCOVER<br/>Explore Problem"]
            DEFINE["DEFINE<br/>Frame Problem"]
            DEVELOP["DEVELOP<br/>Explore Solutions"]
            DELIVER["DELIVER<br/>Finalize Design"]
            APC{{"A/P/C"}}
            DESIGND["design.md"]
        end

        subgraph SPEC["SPEC GENERATION + BEADS"]
            NEWTRACK["/conductor-newtrack"]
            SPECMD["spec.md"]
            PLANMD["plan.md"]
            AUTO_FB["Auto: Create Epic + Issues"]
            FB_PROGRESS[".fb-progress.json<br/>(planTasks mapping)"]
        end

        subgraph AGENT_LOOP["AGENT EXECUTION LOOP"]
            READY["bd ready"]
            AUTO_CLAIM["Auto: bd update --status in_progress"]

            subgraph TDD["TDD CYCLE (--tdd flag)"]
                RED["RED: Write Failing Test<br/>(checkpoint)"]
                GREEN["GREEN: Make It Pass<br/>(checkpoint)"]
                REFACTOR["REFACTOR: Clean Up<br/>(checkpoint)"]
            end

            AUTO_CLOSE["Auto: bd close --reason completed"]
            AUTO_SYNC["Auto: bd sync (with retry)"]
        end

        subgraph DISPATCH["PARALLEL AGENT DISPATCH"]
            COORDINATOR["Coordinator Agent"]

            subgraph WORKERS["WORKER AGENTS (read-only bd)"]
                W1["Agent 1"]
                W2["Agent 2"]
                WN["Agent N"]
            end

            MERGE["Merge Results"]
        end

        subgraph FINISH["COMPLETION"]
            VERIFY["Verification"]
            BRANCH["finish branch"]
            FINISH_CMD["/conductor-finish"]
            COMPACT["Auto: Compact closed issues"]
            CLEANUP["Auto: Cleanup >150 closed"]
        end
    end

    subgraph FACADE["BEADS-CONDUCTOR FACADE"]
        direction LR
        SA["SA Mode<br/>Direct bd CLI"]
        MA["MA Mode<br/>Village MCP"]
        HEARTBEAT["Heartbeat<br/>(5 min updates)"]
        PENDING["Pending Ops<br/>(crash recovery)"]
    end

    subgraph BMAD["PARTY MODE: 12 BMAD AGENTS"]
        subgraph PRODUCT["Product Module"]
            PM["John (PM)"]
            ANALYST["Mary (Analyst)"]
            UX["Sally (UX)"]
        end

        subgraph TECHNICAL["Technical Module"]
            ARCH["Winston (Architect)"]
            DEV["Amelia (Developer)"]
            QA["Murat (QA)"]
            DOCS["Paige (Docs)"]
        end

        subgraph CREATIVE["Creative Module"]
            STORY["Sophia (Storyteller)"]
            BRAIN["Carson (Brainstorm)"]
            DESIGN["Maya (Design Thinking)"]
            STRAT["Victor (Strategist)"]
            SOLVER["Dr. Quinn (Solver)"]
        end
    end

    subgraph VALIDATION["VALIDATION SYSTEM (Phase 0)"]
        direction TB
        VALIDATE["/conductor-validate"]

        subgraph CHECKS["Validation Checks"]
            V01["0.1 Resolve track path"]
            V02["0.2 Check directory"]
            V03["0.3 File existence matrix"]
            V04["0.4 Validate JSON + beads"]
            V05["0.5 Auto-create state"]
            V06["0.6 Auto-fix track_id"]
            V07["0.7 Staleness + sync detection"]
        end

        OUTCOMES{{"PASS / HALT / Auto-repair"}}
    end

    PF_START --> PF_MODE --> PF_BD --> PF_STATE
    PF_STATE --> DS

    DS --> DISCOVER
    DISCOVER --> DEFINE
    DEFINE --> DEVELOP
    DEVELOP --> DELIVER
    DELIVER --> APC
    APC -->|"C"| DESIGND
    APC -->|"P"| BMAD
    BMAD -->|"Synthesize"| APC
    DESIGND --> NEWTRACK

    NEWTRACK --> SPECMD
    SPECMD --> PLANMD
    PLANMD --> AUTO_FB
    AUTO_FB --> FB_PROGRESS
    FB_PROGRESS --> READY

    READY --> AUTO_CLAIM
    AUTO_CLAIM --> COORDINATOR
    COORDINATOR --> W1 & W2 & WN
    W1 & W2 & WN --> MERGE
    MERGE --> RED
    RED --> GREEN
    GREEN --> REFACTOR
    REFACTOR -->|"More tests?"| RED
    REFACTOR -->|"Done"| AUTO_CLOSE
    AUTO_CLOSE --> AUTO_SYNC
    AUTO_SYNC -->|"More issues?"| READY
    AUTO_SYNC -->|"All done"| VERIFY

    VERIFY --> BRANCH
    BRANCH --> FINISH_CMD
    FINISH_CMD --> COMPACT --> CLEANUP

    VALIDATE --> V01 --> V02 --> V03 --> V04 --> V05 --> V06 --> V07 --> OUTCOMES

    NEWTRACK -.->|"Phase 0"| VALIDATE
    AUTO_FB -.->|"Phase 0"| VALIDATE
    READY -.->|"Phase 0"| VALIDATE

    PF_MODE -.-> FACADE
    AUTO_CLAIM -.-> FACADE
    AUTO_CLOSE -.-> FACADE
    AUTO_SYNC -.-> FACADE

    classDef preflight fill:#1e3a5f,stroke:#60a5fa,color:#e2e8f0
    classDef planning fill:#1a365d,stroke:#63b3ed,color:#e2e8f0
    classDef spec fill:#234e52,stroke:#4fd1c5,color:#e2e8f0
    classDef beads fill:#553c9a,stroke:#b794f4,color:#e2e8f0
    classDef dispatch fill:#742a2a,stroke:#fc8181,color:#e2e8f0
    classDef agent fill:#744210,stroke:#f6ad55,color:#e2e8f0
    classDef tdd fill:#2d3748,stroke:#a0aec0,color:#e2e8f0
    classDef finish fill:#22543d,stroke:#68d391,color:#e2e8f0
    classDef facade fill:#4c1d95,stroke:#a78bfa,color:#e2e8f0
    classDef product fill:#285e61,stroke:#4fd1c5,color:#e2e8f0
    classDef technical fill:#2c5282,stroke:#63b3ed,color:#e2e8f0
    classDef creative fill:#744210,stroke:#f6ad55,color:#e2e8f0
    classDef validation fill:#4a1d6e,stroke:#9f7aea,color:#e2e8f0

    class PF_START,PF_MODE,PF_BD,PF_STATE preflight
    class DS,DISCOVER,DEFINE,DEVELOP,DELIVER,APC,DESIGND planning
    class NEWTRACK,SPECMD,PLANMD,AUTO_FB,FB_PROGRESS spec
    class COORDINATOR,W1,W2,WN,MERGE dispatch
    class READY,AUTO_CLAIM,AUTO_CLOSE,AUTO_SYNC agent
    class RED,GREEN,REFACTOR tdd
    class VERIFY,BRANCH,FINISH_CMD,COMPACT,CLEANUP finish
    class SA,MA,HEARTBEAT,PENDING facade
    class PM,ANALYST,UX product
    class ARCH,DEV,QA,DOCS technical
    class STORY,BRAIN,DESIGN,STRAT,SOLVER creative
    class VALIDATE,V01,V02,V03,V04,V05,V06,V07,OUTCOMES validation
```

### Beads-Conductor Lifecycle (Zero Manual Commands)

| Phase | Conductor Command | Beads Action (Automatic) |
|-------|-------------------|--------------------------|
| Preflight | All commands | Mode detect (SA/MA), validate `bd`, create session state |
| Track Init | `/conductor-newtrack` | Create epic + issues from plan.md, wire dependencies |
| Claim | `/conductor-implement` | `bd update --status in_progress` |
| TDD Checkpoints | `--tdd` flag | `bd update --notes "RED/GREEN/REFACTOR..."` |
| Close | `/conductor-implement` | `bd close --reason completed\|skipped\|blocked` |
| Sync | All (session end) | `bd sync` with retry, pending ops recovery |
| Compact | `/conductor-finish` | AI summaries for closed issues |
| Cleanup | `/conductor-finish` | Remove oldest when >150 closed |

For detailed pipeline documentation, see [docs/PIPELINE_ARCHITECTURE.md](./docs/PIPELINE_ARCHITECTURE.md).

### Session-Based Flow

```mermaid
flowchart LR
    subgraph SESSION1["SESSION 1 (Planning)"]
        direction TB
        setup["/conductor-setup"]
        design["ds → Double Diamond design"]
        ground["ground decisions"]
        newtrack["/conductor-newtrack<br/>(spec + plan + beads + review)"]
        handoff["outputs HANDOFF block"]

        setup --> design
        design --> ground
        ground --> newtrack
        newtrack --> handoff
    end

    subgraph SESSION2["SESSION 2 (Epic 1)"]
        direction TB
        paste["User pastes HANDOFF"]
        implement["/conductor-implement"]
        tdd["claims tasks → TDD → verify"]
        choice{"Epic complete"}
        rb2["rb → review beads"]
        handoff2["HANDOFF to next epic"]

        paste --> implement
        implement --> tdd
        tdd --> choice
        choice -->|"fewer mistakes"| rb2
        choice -->|"continue"| handoff2
    end

    subgraph SESSION3["SESSION 3 (Epic 2...)"]
        direction TB
        paste2["User pastes HANDOFF"]
        implement2["/conductor-implement"]
        tdd2["claims tasks → TDD → verify"]
        complete["track complete"]

        paste2 --> implement2
        implement2 --> tdd2
        tdd2 --> complete
    end

    handoff -.-> paste
    rb2 -.-> handoff2
    handoff2 -.-> paste2
```

### Epic Completion: Quality Gate

After completing each epic, `/conductor-implement` presents an explicit choice:

1. **`rb` (recommended)** — Review remaining beads to catch mistakes before they propagate. Uses more tokens but reduces errors.
2. **Handoff** — Continue directly to next epic with `Start epic <next-epic-id>`

This prevents auto-continuation and gives you control between epics.

---

## Understanding Handoff

**Handoff** is how work survives between AI agent sessions. It's the structured transfer of context that ensures no progress is lost when a session ends.

### Why Handoff Matters

AI coding assistants have a fundamental limitation: **sessions end, but projects continue**. Without handoff:

- Context windows fill up and compact, losing conversation history
- Sessions crash, timeout, or simply get closed
- Tomorrow's session has no memory of today's decisions

### How Maestro Handles Handoff

Every artifact in Maestro is a handoff checkpoint:

| Artifact    | What It Preserves                      |
| ----------- | -------------------------------------- |
| `design.md` | Architecture decisions and trade-offs  |
| `spec.md`   | Requirements and acceptance criteria   |
| `plan.md`   | Step-by-step tasks with status markers |
| `.beads/`   | Issue state, dependencies, and notes   |

**The handoff flow:**

```
Session 1 (Planning):
  ds → design.md
  /conductor-newtrack → spec.md + plan.md + beads
  rb → reviewed beads
  → HANDOFF

Session 2+ (Execution):
  /conductor-implement → execute Epic 1 → HANDOFF
  /conductor-implement → execute Epic 2 → HANDOFF
  ...one epic per session
```

### Handoff in Practice

**At session end:**

```bash
bd update <id> --notes "COMPLETED: X. NEXT: Y."
git add -A && git commit -m "progress"
git push
```

**At session start:**

```bash
bd ready --json          # What's unblocked?
bd show <id>             # Read context from notes
```

The notes field in beads is your session-to-session memory. Write it like you're leaving instructions for yourself in two weeks.

### Manual Specialist Tools

Outside the automated flow (external: superpowers plugin):

- `debug` — Systematic debugging
- `trace` — Root cause tracing

---

## Slash Commands

| Command                          | Description                                                            |
| -------------------------------- | ---------------------------------------------------------------------- |
| `/conductor-setup`               | Initialize Conductor for project                                       |
| `/conductor-design [desc]`       | Design through Double Diamond dialogue (A/P/C checkpoints, Party Mode) |
| `ds`                             | Start design session (alias for `/conductor-design`)                   |
| `/conductor-newtrack [id]`       | Create spec + plan from design                                         |
| `/conductor-implement [id]`      | Execute ONE EPIC from track's plan                                     |
| `/conductor-status`              | View progress                                                          |
| `/conductor-revert`              | Git-aware revert of work                                               |
| `/conductor-revise`              | Update spec/plan when implementation reveals issues                    |
| `/conductor-finish [id]`         | Complete track: learnings, context refresh, archive (6 phases)         |
| `/conductor-validate [id]`       | Validate track health and state consistency                            |
| `/conductor-block [id] [reason]` | Mark a task as blocked                                                 |
| `/conductor-skip [id] [reason]`  | Skip a task with documented reason                                     |
| `/ground <pattern>`              | Verify patterns against current truth                                  |
| `/decompose-task <phase>`        | Break phases into atomic beads                                         |
| `/compact`                       | Checkpoint and compact session                                         |
| `review code`                    | Request code review (external: superpowers plugin)                     |

---

## Documentation

### Start Here

| If you want to...                      | Read                                                             |
| -------------------------------------- | ---------------------------------------------------------------- |
| Understand the philosophy and workflow | [TUTORIAL.md](./TUTORIAL.md)                                     |
| Set up a new project                   | [SETUP_GUIDE.md](./SETUP_GUIDE.md)                               |
| Configure global agent                 | [docs/GLOBAL_CONFIG.md](./docs/GLOBAL_CONFIG.md)                 |
| Understand the pipeline architecture   | [docs/PIPELINE_ARCHITECTURE.md](./docs/PIPELINE_ARCHITECTURE.md) |
| Use commands manually without skills   | [docs/manual-workflow-guide.md](./docs/manual-workflow-guide.md) |
| See all skills at a glance             | [Skills table above](#the-skills)                                |

### Repository Structure

```
maestro/
├── README.md              # This file
├── SETUP_GUIDE.md         # Installation guide
├── TUTORIAL.md            # Complete workflow guide
├── AGENTS.md              # Agent instructions
├── skills/                # Skill directories (conductor, design, beads, tdd, etc.)
│   ├── conductor/         # Planning methodology
│   ├── design/            # Design sessions (ds trigger)
│   ├── beads/             # Issue tracking (fb, rb triggers)
│   ├── test-driven-development/
│   └── ...                # See SETUP_GUIDE.md for full list
├── commands/              # 25+ slash commands
├── agents/                # Agent definitions
├── workflows/             # Workflow definitions
├── hooks/                 # Lifecycle hooks
├── lib/                   # Shared utilities
└── templates/             # Templates
```

---

## Troubleshooting

### Common Issues

| Issue                         | Fix                                                                |
| ----------------------------- | ------------------------------------------------------------------ |
| Skills not loading            | Run `/plugin list` to verify installation                          |
| `bd: command not found`       | Install via Agent Mail installer (see SETUP_GUIDE.md)              |
| `bv` hangs                    | You forgot `--robot-*` flag. Kill and restart with flag            |
| Agent ignores workflow        | Use trigger phrase explicitly: `tdd`, `debug`, `/conductor-design` |
| Tests pass immediately        | You wrote code first. Delete it. Start with failing test.          |
| Context compacted, lost state | Run `bd show <issue-id>` — notes field has recovery context        |
| Plan seems incomplete         | Use `rb` (review beads) to check and refine issues                 |

### Tips & Tricks

| Tip                              | Details                                                                                            |
| -------------------------------- | -------------------------------------------------------------------------------------------------- |
| **Plan before each epic**        | Switch to plan mode before `/conductor-implement`. Claude Code: `Shift+Tab`, Codex: `/create-plan` |
| **Handoff in Amp**               | Use handoff command (command palette) or reference threads with `@T-<id>`                          |
| **Handoff in Claude Code/Codex** | Run `/compact` before session end — beads notes survive, conversation doesn't                      |

### Agent-Specific Rules

**Critical**: These tools have TUI modes that will hang AI agents:

- `bv` → Always use `bv --robot-*` flags
- `cass` → Always use `cass --robot` or `--json` flags

### Without CLI Tools

The plugin still provides value without `bd`:

- Skills work as mental models and methodologies
- Use `TodoWrite` for session-local task tracking
- Track issues manually in GitHub Issues or markdown
- Full TDD, debugging, and code review workflows still apply

**The skills are the methodology; the CLIs are the persistence layer.**

---

## Credits

Built on foundations from:

- [conductor](https://github.com/NguyenSiTrung/conductor) by NguyenSiTrung
- [beads](https://github.com/steveyegge/beads) by Steve Yegge
- [beads-village](https://github.com/LNS2905/mcp-beads-village) by LNS2905
- [Knowledge & Vibes](https://github.com/kyleobrien91/knowledge-and-vibes) methodology

## License

MIT
