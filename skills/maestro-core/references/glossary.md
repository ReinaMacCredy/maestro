# Glossary

Key terms for Maestro workflow.

## Workflow Terms

| Term | Definition |
|------|------------|
| **Track** | A feature/bug/improvement work unit with design, spec, plan |
| **Epic** | Top-level bead grouping related tasks |
| **Bead** | Single trackable task with status and dependencies |
| **Phase** | Workflow stage: DESIGN → SPEC → PLAN → IMPLEMENT |

## Commands

| Command | Meaning |
|---------|---------|
| `ds` | Design Session (Double Diamond) |
| `fb` | File Beads from plan |
| `rb` | Review Beads |
| `ci` | Conductor Implement |
| `co` | Conductor Orchestrate |
| `bd` | Beads CLI |

## Double Diamond

| Phase | Type | Purpose |
|-------|------|---------|
| DISCOVER | Diverge | Explore problem space |
| DEFINE | Converge | Frame the problem |
| DEVELOP | Diverge | Explore solutions |
| DELIVER | Converge | Finalize design |

## A/P/C Checkpoints

| Choice | Meaning |
|--------|---------|
| **[A]** Advanced | Deeper analysis, assumption audit |
| **[P]** Party | Multi-agent feedback (BMAD) |
| **[C]** Continue | Proceed to next phase |

## Fallback Policies

| Policy | Meaning |
|--------|---------|
| **HALT** | Stop execution, require fix |
| **DEGRADE** | Continue with reduced functionality |

## Modes

| Mode | Description |
|------|-------------|
| **SA** | Single-Agent: Direct bd CLI |
| **MA** | Multi-Agent: Village MCP coordination |
| **SPEED** | Fast design path (score < 4) |
| **FULL** | Complete Double Diamond (score > 6) |
