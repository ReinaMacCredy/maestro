# Design Session: Continuous-Claude-v2 Integration

## Overview

Integrate Continuous-Claude-v2 patterns into Maestro workflow, adapted for Amp (no hooks).

## Source Repository

https://github.com/parcadei/Continuous-Claude-v2

## Key Decisions

### 1. Agent Mail as Primary Storage

- Handoffs stored in Agent Mail (not files)
- Markdown export as secondary (for git)
- FTS5 search via `search_messages()`

### 2. Orchestrator as Central Hub

- Specialized agents for different tasks
- Main thread stays clean (routing + summaries only)
- Sub-agents save full context to Agent Mail

### 3. Token-Efficient Architecture (Amp-Specific)

- Thin router in AGENTS.md (~50 lines)
- Agent prompts load IN sub-agent context (not main)
- No hooks - sub-agents responsible for saving own context

### 4. Agent Directory Structure

```
skills/orchestrator/agents/
├── research/      ← Observation agents (migrated from conductor)
├── review/        ← Evaluation agents (NEW)
├── planning/      ← Planning agents (from C-C-v2)
├── execution/     ← Task execution agents
└── debug/         ← Debugging agents (from C-C-v2)
```

### 5. Delegation Pattern

| Main Thread | Sub-Agent |
|-------------|-----------|
| Understand intent | File reading |
| Route to agent | Code analysis |
| Display summaries | Implementation |
| Confirm actions | Security review |
| Handle errors | Research |

## What's IN Scope

- [x] Agent directory with specialized agents
- [x] Orchestrator routing table
- [x] Agent Mail integration for context persistence
- [x] Thin router for token efficiency
- [x] Sub-agent summary protocol
- [x] Manual handoff commands (`/create_handoff`)

## What's OUT of Scope (Amp Limitations)

- [ ] Lifecycle hooks (SessionStart, PreCompact, etc.)
- [ ] Automatic context capture on compaction
- [ ] PostToolUse file tracking
- [ ] Compound learnings extraction (deferred)

## Integration Points

### From C-C-v2

| Feature | Adaptation |
|---------|------------|
| Ledger system | → Agent Mail messages |
| SubagentStop hook | → Agent saves own context |
| thoughts/handoffs/ | → Agent Mail threads |
| implement_plan orchestrator | → orchestrator skill |
| Research agents | → Migrate to orchestrator/agents/ |

### Amp-Specific

| Pattern | Implementation |
|---------|----------------|
| First-message context load | `fetch_inbox()` in AGENTS.md |
| Sub-agent context save | Agent calls `send_message()` before return |
| Manual handoff | `/create_handoff` command |

## Complexity Score

| Factor | Score |
|--------|-------|
| Multiple epics | 3 |
| Cross-module | 2 |
| New abstractions | 2 |
| External deps | 1 |
| Files > 5 | 1 |
| **Total** | **9** (FULL MODE) |

## Session Info

- Date: 2024-12-31
- Mode: FULL (4-phase Double Diamond)
- Platform: Amp (no hooks)
