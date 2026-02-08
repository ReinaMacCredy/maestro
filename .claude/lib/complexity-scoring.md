---
name: complexity-scoring
description: Complexity scoring system for model tier selection. Used by the orchestrator to route tasks to appropriate model tiers.
type: internal
---

# Complexity Scoring

Scoring system for the orchestrator to determine which model tier to assign to each worker task. Apply these signals to the task description before spawning a worker.

## Lexical Signals

| Signal | Condition | Score |
|--------|-----------|-------|
| Long description | Word count > 200 | +2 |
| Multi-file | File path count >= 2 | +1 |
| Architecture keywords | refactor, redesign, architect, migrate, rewrite | +3 |
| Debug keywords | root cause, investigate, debug, diagnose | +2 |
| Simple keywords | find, list, show, rename, move | -2 |
| Risk keywords | production, critical, migration, security | +2 |

## Structural Signals

| Signal | Condition | Score |
|--------|-----------|-------|
| Many subtasks | Estimated subtasks > 3 | +3 |
| Cross-file dependencies | Task touches files in different modules | +2 |
| System-wide impact | Changes affect shared interfaces or configs | +3 |

## Scoring Thresholds

| Score | Complexity | Model Tier | Route To |
|-------|-----------|-----------|----------|
| >= 8 | HIGH | opus | oracle for analysis, kraken for implementation |
| >= 4 | MEDIUM | sonnet | kraken (default) |
| < 4 | LOW | haiku | spark |

## Usage

The orchestrator reads each task description and applies the scoring before spawning a worker:

1. Count lexical signals in the task description
2. Assess structural signals from the task's file list and dependencies
3. Sum the scores
4. Route to the appropriate model tier based on thresholds

This is guidance, not enforcement. The orchestrator uses judgment and may override the score when context warrants it.
