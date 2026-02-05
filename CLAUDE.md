# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**Maestro** is an AI agent workflow plugin for Claude Code. It provides interview-driven planning and team-based execution using Agent Teams.

**Core philosophy:** "Spend tokens once on a good plan; reuse it many times."

## Commands

- `/design <request>` — Interview-driven planning (team-based)
- `/work` — Execute plan with Agent Teams (parallel workers)

### Validation

```bash
cat .claude-plugin/plugin.json | jq .     # Validate plugin manifest
./scripts/validate-links.sh               # Validate documentation links
./scripts/validate-anchors.sh             # Validate markdown anchors
```

## Architecture

```
.claude/
├── agents/          # 6 agent definitions (prometheus, orchestrator, kraken, spark, oracle, explore)
├── commands/        # /design, /work (full workflows — source of truth)
└── skills/
    └── maestro/     # Skill manifest and reference

.maestro/            # Runtime state
├── plans/           # Work plans
├── drafts/          # Interview drafts
└── wisdom/          # Accumulated learnings

.claude-plugin/      # Plugin manifest
```

**Key principle**: Commands contain the full workflow. Agent definitions are lean (identity + constraints). No duplication between them.

## Critical Rules

1. **Both phases use Agent Teams** — `/design` and `/work` both create teams and spawn teammates
2. **Orchestrator never edits directly** — Always delegates to kraken/spark
3. **Workers self-coordinate** — All agents have TaskList/TaskUpdate/SendMessage for parallel work
4. **Verify teammate claims** — Always read files and run tests after delegation
5. **TDD by default** — Use kraken for new features
