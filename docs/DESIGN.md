# maestro Design

## Core Concept

Agent-optimized development orchestrator -- structured memory, workflow guardrails, and a plan-first pipeline for AI coding agents.

```text
DISCOVERY  -->  RESEARCH  -->  PLANNING  -->  EXECUTION  -->  LEARNING
(why)           (what)         (how)          (do)           (improve)
```

All durable state lives under `.maestro/`.

## Data Structure

```text
.maestro/
  config.json                    # Project configuration
  doctrine/                      # Cross-feature operating rules (JSON)
    <name>.json                  # Structured rule with effectiveness metrics
  memory/                        # Global project-scoped memory files
  features/
    <feature>/
      feature.json               # Feature metadata and lifecycle
      plan.md                    # Implementation plan
      comments.json              # Plan review comments
      APPROVED                   # Plan approval marker
      memory/                    # Feature-scoped memory files (DCP-scored)
      tasks/
        <task>/
          status.json            # Task state, claims, revisions
          spec.md                # Compiled task specification
          report.md              # Completion summary
          verification.json      # Verification report
          doctrine-trace.json    # Doctrine injection trace
```

## Architecture

4-layer hexagonal architecture with immutable DI container:

```text
surfaces/  -->  app/     -->  domain/ports/  <--  infra/adapters/
(CLI, MCP,      (rules,       (interfaces)       (implementations)
 hooks)          DCP, skills)
```

```text
src/
  domain/       # Pure domain: ports, types, errors (zero infra deps)
  app/          # Use cases, DCP, doctrine, skills, workflow engine
  infra/        # Adapters, toolbox, settings, utilities, visual renderer
  surfaces/     # CLI (71 commands), MCP (26 tools), hooks (5 scripts)
  container.ts  # Immutable DI container (Object.freeze)
```

### Ports (11 interfaces)

| Port | Purpose |
|------|---------|
| TaskPort | Task CRUD, claims, specs, verification |
| FeaturePort | Feature lifecycle |
| PlanPort | Plan read/write/approval |
| MemoryPort | Feature-scoped and global memory |
| DoctrinePort | Cross-feature operating rules with effectiveness tracking |
| VerificationPort | Build/test verification |
| GraphPort | Dependency graph insights (optional, requires bv) |
| HandoffPort | Cross-agent handoff (optional, requires agent-mail) |
| SearchPort | Session search (optional, requires cass) |
| SettingsPort | Project configuration with merge semantics |
| HostPort | Host agent detection and backend resolution |

## Task Lifecycle

6 states:

```text
pending --> claimed --> done
                   \-> blocked --> (unblock) --> pending
                   \-> review --> revision --> claimed
```

Stale claims expire after `claimExpiresMinutes` (default 120) and auto-reset to `pending` on `task-next`.

## Pipeline

```text
discovery --> research --> planning --> approval --> execution --> done
```

Stages are skippable. Hooks inject pipeline context automatically.

## DCP (Dynamic Context Pruning)

Budget-conscious context injection for worker agents. Scores memories by 5 weighted factors (tag overlap, keyword overlap, category match, priority, recency) plus dependency proximity bonus. Greedy selection within configurable byte budgets.

## Doctrine Compiler

Cross-feature learning system that turns execution history into reusable operating rules:

1. **Plan-time awareness**: `plan_write` queries execution memories across features, surfaces failure patterns as historical pitfalls
2. **Doctrine injection**: Active doctrine items are scored by tag/keyword overlap and injected into workers via a separate budget pool (default 1024 bytes), independent of memory DCP
3. **Semi-automatic suggestion**: `feature_complete` analyzes cross-feature patterns (revisions, verification failures) and proposes doctrine candidates requiring human approval
4. **Effectiveness tracking**: Append-only doctrine-trace.json records injections per revision cycle. On task completion, records success/failure against each doctrine. Staleness detection surfaces idle doctrine.

Doctrine items are JSON files at `.maestro/doctrine/` with structured fields: rule, rationale, conditions (tags/file patterns), source features/memories, effectiveness metrics (injection count, success rate, override count), and schema version.

## Hooks

| Hook | Trigger | Purpose |
|------|---------|---------|
| SessionStart | Session begins | Inject pipeline state and recommended skills |
| PreToolUse:Bash | Before Bash tool | Remind agent to finish maestro tasks on git commit |
| PreToolUse:Agent | Before agent spawn | Inject task spec + DCP-scored memories + doctrine into worker prompt |
| PostToolUse | After tool execution | Track tool usage and state changes |
| PreCompact | Before context compaction | Preserve critical maestro state |

## Key Principles

- Pure MCP plugin -- Claude Code is the orchestrator, maestro is the filing cabinet with opinions
- No LLM calls in the pipeline -- all scoring and pattern detection is deterministic
- Human-in-the-loop for doctrine creation -- suggestions proposed, never auto-written
- Feature/task context survives agent restarts
- Best-effort hooks -- never block agent operations
