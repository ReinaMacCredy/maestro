# Task Type Guidance

## Overview

The orchestrator delegates tasks using Claude Code's `Task(description, prompt)` syntax.

## Task Types

| Task Type | Description Pattern | Prompt Focus |
|-----------|---------------------|--------------|
| UI/Frontend | `"implement [component] UI"` | Visual patterns, CSS, layout |
| Backend/Logic | `"implement [feature] logic"` | Architecture, algorithms |
| Research/Explore | `"explore [topic]"` | Search patterns, codebase analysis |
| External Docs | `"research [library] docs"` | Documentation, examples |
| Documentation | `"document [feature]"` | README, API docs, guides |
| Debugging | `"debug [issue]"` | Error analysis, root cause |

## Specialized Agent Types

| Subagent Type | Use For |
|---------------|---------|
| `atlas-leviathan` | General implementation (default) |
| `atlas-kraken` | TDD implementation, heavy refactoring |
| `atlas-spark` | Quick fixes, simple changes |
| `explore` | Codebase exploration, pattern discovery |
| `oracle` | External research, documentation lookup |

## Decision Matrix

| Task Type | Agent |
|-----------|-------|
| TDD, refactoring, complex | `atlas-kraken` |
| Quick fix, simple change | `atlas-spark` |
| General implementation | `atlas-leviathan` |
