# Maestro Plugin - Architecture Overview

AI workflow plugin for structured development: planning (Conductor), issue tracking (Beads), and execution (TDD).

## Key Entry Points

| Entry | Purpose |
|-------|---------|
| `ds` / `/conductor-design` | Start Double Diamond design session |
| `pl` / `/plan` | Start 6-phase risk-based planning pipeline |
| `/conductor-newtrack` | Generate spec + plan + beads from design |
| `/conductor-implement` | Execute tasks with TDD (sequential) |
| `/conductor-orchestrate` | Execute tracks in parallel with workers |
| `/conductor-finish` | Complete track, extract learnings, archive |
| `/doc-sync` | Sync documentation with code changes |
| `fb` / `rb` | File/review beads from plan |

## Directory Structure

```
maestro/
├── skills/           # 9 skill directories (SKILL.md + references/)
│   ├── tracking/references/         # Issue tracking workflows
│   ├── conductor/references/     # Planning + execution (absorbed 9 skills)
│   │   ├── research/             # Research protocol (replaces grounding)
│   │   ├── planning/             # 6-phase planning pipeline (pl command)
│   │   ├── prompts/              # Agent prompts
│   │   ├── coordination/         # Multi-agent coordination
│   │   ├── tdd/                  # TDD cycle + gates
│   │   ├── verification/         # Pre-completion gates
│   │   ├── doc-sync/             # Doc sync workflows
│   │   ├── handoff/              # Session handoff system
│   │   └── finish/               # Branch completion
│   ├── designing/references/        # Double Diamond + Party Mode
│   │   └── bmad/                 # Multi-agent design personas
│   ├── orchestrator/references/  # Multi-agent parallel execution
│   │   ├── patterns/             # Parallel dispatch, lifecycle, fallback
│   │   └── examples/             # Three-agent dispatch example
│   ├── maestro-core/             # Routing and fallback policies
│   ├── handoff/                  # Session cycling and context preservation
│   ├── using-git-worktrees/      # Isolated dev environments
│   ├── creating-skills/          # Skill authoring guide
│   └── sharing-skills/           # Upstream contribution
├── conductor/        # Project context + tracks
│   ├── product.md, tech-stack.md, workflow.md
│   ├── CODEMAPS/     # Architecture documentation (this directory)
│   ├── spikes/       # Research spikes (pl output)
│   └── tracks/<id>/  # design.md, spec.md, plan.md per track
├── agents/           # Agent persona definitions
└── .beads/           # Issue tracker storage
```

## Data Flow

```
ds → design.md ─┬→ /conductor-newtrack → spec.md + plan.md
                │                              ↓
pl → spike.md ──┘                       fb → .beads/ (epics + issues)
                                               ↓
                                     [auto-orchestrate?]
                                      ╱             ╲
                                    YES              NO
                                    ↓                ↓
                          auto-analyze graph    manual choice
                                    ↓                ↓
                       spawn parallel workers   /conductor-implement
                                    ↓                ↓
                            workers complete    TDD cycle
                                    ↓                ↓
                                rb review         done
                                    ↓                ↓
                               /conductor-finish → LEARNINGS.md → archive
```

### Auto-Orchestration (New)

After `fb` completes filing beads, Phase 6 triggers automatically:
1. Query graph: `bd list --json` to get ready/blocked beads
2. Generate Track Assignments from dependency analysis
3. Spawn workers via Task() for parallel execution
4. After workers complete, spawn `rb` sub-agent for final review

Idempotency: `metadata.json.beads.orchestrated = true` prevents re-running.

## Beads-Conductor Integration

Zero manual `bd` commands in the happy path:

| Point | Conductor Command | Beads Action |
|-------|-------------------|--------------|
| Preflight | All | Validate bd CLI |
| Claim | `/conductor-implement` | `bd update --status in_progress` |
| Close | `/conductor-implement` | `bd close --reason completed` |
| Sync | All (session end) | `bd sync` with retry |
| Compact | `/conductor-finish` | AI summaries for closed issues |

## Core Skills

| Skill | Trigger | Role |
|-------|---------|------|
| `conductor` | `/conductor-*`, `/research` | Planning + execution + **research protocol** |
| `orchestrator` | `/conductor-orchestrate`, "spawn workers" | **Multi-agent parallel execution** |
| `designing` | `ds`, `cn` | Double Diamond + Party Mode + Research verification |
| `tracking` | `bd`, `fb`, `rb` | Issue tracking, file/review beads |
| `maestro-core` | All Maestro skills | Routing and fallback policies |
| `handoff` | `ho`, `/conductor-finish` | Session cycling and context preservation |
| `creating-skills` | "create skill", "write skill" | Skill authoring guide |
| `using-git-worktrees` | - | Isolated dev environments |
| `sharing-skills` | - | Upstream contribution |

## Common Tasks

| Task | How |
|------|-----|
| Start new feature | `ds` → design → `/conductor-newtrack` |
| Find work | `bd ready --json` |
| Execute task (sequential) | `/conductor-implement` (auto-claims from beads) |
| Execute tracks (parallel) | `/conductor-orchestrate` (spawns worker agents) |
| Complete track | `/conductor-finish` (extracts learnings, archives) |
| Add a skill | Create `skills/<name>/SKILL.md` with frontmatter + `references/` |
| Add workflow docs | Add to `skills/<skill>/references/*.md` |
| Regenerate CODEMAPS | `/conductor-finish` (Phase 6: CODEMAPS Regeneration) |
| Coordinate parallel agents | `/conductor-orchestrate` or `skills/orchestrator/references/` |

## Validation Gates

5 validation gates integrated into the Maestro lifecycle:

| Gate | Trigger | Enforcement |
|------|---------|-------------|
| `design` | After DELIVER phase | SPEED=WARN, FULL=HALT |
| `spec` | After spec.md generation | WARN (both modes) |
| `plan-structure` | After plan.md generation | WARN (both modes) |
| `plan-execution` | After TDD REFACTOR | SPEED=WARN, FULL=HALT |
| `completion` | Before /conductor-finish | SPEED=WARN, FULL=HALT |

Gate files: `skills/conductor/references/validation/shared/*.md`
Lifecycle routing: `skills/conductor/references/validation/lifecycle.md`

Validation state (metadata.json):
```yaml
validation:
  gates_passed: [design, spec, plan-structure]
  current_gate: plan-execution
  retries: 0
  last_failure: null
```

Max 2 retries before escalating to human review.

## Gotchas

- `bd` should use `--json` for structured output
- Skills require YAML frontmatter with `name` and `description`
- Never write production code without a failing test first (TDD iron law)
- CODEMAPS are auto-regenerated by `/conductor-finish`
