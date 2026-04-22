---
name: maestro-mission
description: Use for planned multi-step work at the mission level in any project that uses maestro. Covers mission lifecycle, feature/milestone navigation, feature prompt reading with injected memory, feature progress reporting, memory corrections, and agent-friendly mission-control TUI rendering. Use when the user says "show missions", "what feature am I on", "report feature progress", "read feature prompt", or when the agent needs to orient at the mission layer. Activates whenever a maestro-initialized project is detected.
---

# Maestro Mission

You are orienting at or working inside a maestro mission. Missions are the planned-execution layer: milestones, features, assertions, checkpoints, handoff launches.

The task system is separate. Use `maestro-task` for the daily queue.

---

## When to activate

- "Show missions", "what missions are in flight", "what am I working on"
- "What feature am I on", "list features for mission X"
- "Report feature progress", "mark feature X as passed/blocked"
- "Read the prompt for feature X"
- Agent needs to render the mission-control TUI for the user

## Top-level orientation

```bash
maestro status --json
```

Prints session id, active missions, current task counts, and mission-control summary. Always a safe first call.

## Mission layer

```bash
maestro mission list --json
maestro mission show <missionId>
```

Mission lifecycle transitions (`draft -> approved -> executing -> completed`), milestones, checkpoints, and assertion semantics live in `./reference/mission-lifecycle.md`.

## Feature layer

```bash
maestro feature list --mission <id> --json
maestro feature prompt <featureId> --mission <id>
maestro feature update <featureId> --mission <id> \
  --status <passed|failed|blocked|waived> \
  --report @report.json
```

`feature prompt` returns the briefing a feature-scoped agent reads before execution, with memory context auto-injected. Use it to understand what a feature expects, or to re-render after memory changes.

Assertion states (`passed|failed|blocked|waived`) and how reports feed back into a mission: `./reference/assertions.md`.

## Memory corrections

Capture a correction rule for future sessions:

```bash
maestro memory-correct "use bun not npm" --trigger "package,install,npm"
```

`--trigger` is a comma-separated list of keywords. When a future agent's work matches a trigger, the correction is injected as context.

## Mission-control TUI

`maestro mission-control` is a read-only TUI. It has an agent-friendly preview mode that prints plain-text frames to stdout.

```bash
maestro mission-control --preview --size 120x40 --format plain
maestro mission-control --preview all --size 120x40 --format plain
maestro mission-control --render-check --size 120x40
maestro mission-control --preview --size 120x40 --format json
```

Preview modes:
- `--preview`: renders the current default screen as plain text.
- `--preview all`: renders every screen sequentially.
- `--render-check`: validates the TUI renders without errors, returns non-zero on failure.
- `--format plain|json`: pick plain-text for agent-consumable frames or JSON for structured snapshots.

**Read-only contract.** Preview, JSON, and render-check paths must not mutate state. Use for inspection only. Full flag reference: `./reference/mission-control-tui.md`.

## Reference

- `./reference/mission-lifecycle.md`: draft/approved/executing transitions, checkpoint semantics
- `./reference/mission-control-tui.md`: flag reference, `--screen` options, format matrix
- `./reference/assertions.md`: assertion state machine and update rules
