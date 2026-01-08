# Research Protocol — Consolidated 2-Hook System

## Overview

Research protocol spawns parallel sub-agents to comprehensively document the codebase AS-IS. The system uses **2 hooks** (down from 5) for faster, more focused research.

### Hook Comparison

| Hook | Trigger | Agents | Timeout | Mode | Purpose |
|------|---------|--------|---------|------|---------|
| [research-start](./research-start.md) | Phase 1 start | 4 (Locator+Pattern+CODEMAPS+Architecture) | 20s | ALL | Ground context before DISCOVER |
| [research-verify](./research-verify.md) | Phase 3→4 | 4 (Analyzer+Pattern+Impact+Web) | 15s | FULL only | Verify design against codebase |

### Timing Improvement

| Metric | Old (5 hooks) | New (2 hooks) |
|--------|---------------|---------------|
| Worst case | ~95s | 35s max |
| SPEED mode | ~45s | 20s max |
| Hooks executed | 5 | 2 (or 1 in SPEED) |

## Critical Rule

**DOCUMENT, DON'T EVALUATE**

- DO NOT suggest improvements or changes
- DO NOT perform root cause analysis
- DO NOT propose future enhancements
- DO NOT critique the implementation
- DO NOT recommend refactoring
- ONLY describe what exists, where it exists, how it works

## 2-Hook System

### Hook 1: research-start

> **Trigger:** Phase 1 (DISCOVER) start  
> **Runs in:** ALL modes (SPEED, BALANCED, THOROUGH)

Consolidates old `discover-hook` and PL Phase 1 Discovery.

```
┌───────────────────────────────────────────────────────────────┐
│                    RESEARCH-START (20s)                       │
├───────────────────────────────────────────────────────────────┤
│  ┌─────────┐  ┌─────────┐  ┌──────────┐  ┌──────────────┐    │
│  │ Locator │  │ Pattern │  │ CODEMAPS │  │ Architecture │    │
│  │  (5s)   │  │  (5s)   │  │  (3s)    │  │    (5s)      │    │
│  └────┬────┘  └────┬────┘  └────┬─────┘  └──────┬───────┘    │
│       │            │            │               │             │
│       └────────────┴────────────┴───────────────┘             │
│                          │                                    │
│                     SYNTHESIZE                                │
│                          │                                    │
│              pipeline_context.research.start                  │
└───────────────────────────────────────────────────────────────┘
```

| Agent | Purpose | Timeout |
|-------|---------|---------|
| Locator | Find WHERE files/components live | 5s soft |
| Pattern | Find existing patterns/conventions | 5s soft |
| CODEMAPS | Load architecture documentation | 3s soft |
| Architecture | Analyze structural constraints | 5s soft |

### Hook 2: research-verify

> **Trigger:** Phase 3→4 (DEVELOP→VERIFY transition)  
> **Runs in:** FULL modes only (BALANCED, THOROUGH)  
> **SKIPPED in:** SPEED mode

```
┌───────────────────────────────────────────────────────────────┐
│                   RESEARCH-VERIFY (15s)                       │
├───────────────────────────────────────────────────────────────┤
│  ┌──────────┐  ┌─────────┐  ┌────────┐  ┌─────────┐          │
│  │ Analyzer │  │ Pattern │  │ Impact │  │   Web   │          │
│  │   (5s)   │  │  (4s)   │  │  (4s)  │  │  (6s)   │          │
│  └─────┬────┘  └────┬────┘  └───┬────┘  └────┬────┘          │
│        │            │           │            │                │
│        └────────────┴───────────┴────────────┘                │
│                          │                                    │
│                     AGGREGATE                                 │
│                          │                                    │
│             pipeline_context.research.verify                  │
└───────────────────────────────────────────────────────────────┘
```

| Agent | Purpose | Timeout |
|-------|---------|---------|
| Analyzer | Deep code analysis of change locations | 5s |
| Pattern | Verify patterns match conventions | 4s |
| Impact | Assess blast radius, affected files | 4s |
| Web | External docs/best practices | 6s |

## Mode-Based Execution

```
SPEED mode:
  research-start (20s) → DISCOVER → DEVELOP → SKIP → VERIFY
                                              └── research-verify skipped

BALANCED mode:
  research-start (20s) → DISCOVER → DEVELOP → research-verify (15s) → VERIFY

THOROUGH mode:
  research-start (20s) → DISCOVER → DEVELOP → research-verify (15s) → VERIFY
                                              └── + Oracle deep review if LOW confidence
```

## CRITICAL: Always Spawn Agents

**Sub-agents are ALWAYS spawned. No skip conditions.**

```
┌─────────────────────────────────────────────────────────────┐
│                 MANDATORY AGENT DISPATCH                    │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ❌ OLD: Skip if "quick" or timeout or SPEED mode          │
│  ✅ NEW: ALWAYS spawn parallel agents                       │
│                                                             │
│  Rationale:                                                 │
│  - Research is fast (parallel execution)                   │
│  - Context is always valuable                              │
│  - Prevents hallucination and outdated assumptions         │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

**Exception:** `research-verify` is skipped in SPEED mode by design.

## Research Agents

| Agent | Purpose | When to Use | Hook |
|-------|---------|-------------|------|
| [codebase-locator](../../../orchestrator/agents/research/codebase-locator.md) | Find WHERE files/components live | First - discovery | start |
| [pattern-finder](../../../orchestrator/agents/research/pattern-finder.md) | Find existing patterns | When looking for conventions | both |
| [codebase-analyzer](../../../orchestrator/agents/research/codebase-analyzer.md) | Understand HOW code works | After locator finds targets | verify |
| [impact-assessor](../../../orchestrator/agents/research/impact-assessor.md) | Assess blast radius | Before VERIFY phase | verify |
| [web-researcher](../../../orchestrator/agents/research/web-researcher.md) | External docs/APIs | For library patterns | verify |

## Execution Flow

```
┌─────────────────────────────────────────────────────────────┐
│                    RESEARCH PROTOCOL                        │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  1. RECEIVE QUERY                                           │
│     └─► Read any directly mentioned files FULLY first       │
│                                                             │
│  2. DECOMPOSE                                               │
│     └─► Break into composable research areas                │
│     └─► Create TodoWrite plan                               │
│                                                             │
│  3. SPAWN PARALLEL AGENTS                                   │
│     ├─► Locator agents (find what exists)                   │
│     ├─► Analyzer agents (understand how it works)           │
│     └─► Pattern agents (find conventions)                   │
│                                                             │
│  4. WAIT & SYNTHESIZE                                       │
│     ├─► Wait for ALL agents to complete (or timeout)        │
│     ├─► Prioritize live codebase over docs                  │
│     └─► Connect findings across components                  │
│                                                             │
│  5. OUTPUT                                                  │
│     └─► Structured research document                        │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

## Agent Dispatch Rules

### Parallel Dispatch (Safe)

Run these agents in parallel when searching different areas:

```
┌─────────────┬─────────────┬─────────────┐
│  Locator 1  │  Locator 2  │  Locator 3  │
│  (auth/)    │  (api/)     │  (db/)      │
└─────────────┴─────────────┴─────────────┘
```

### Sequential Dispatch (Required)

Run these in sequence when there are dependencies:

```
Locator → finds files → Analyzer (on found files)
```

### Agent Instructions

When spawning agents via Task tool:

1. **Be specific** about what to find/analyze
2. **Remind agents** they are documentarians, not critics
3. **Specify output format** (file paths, line numbers)
4. **Set scope** (directories, file patterns)

## Output Format

### Research Summary Block

```
┌─ RESEARCH RESULT ──────────────────────────┐
│ Query: [original query]                    │
│ Agents: [N] spawned, [N] completed         │
│ Duration: [X]s                             │
├────────────────────────────────────────────┤
│ FINDINGS:                                  │
│ • [file.ts:L10-L50] - Description          │
│ • [another.ts:L25] - Description           │
├────────────────────────────────────────────┤
│ CONNECTIONS:                               │
│ • ComponentA → calls → ComponentB          │
│ • PatternX used in: file1, file2, file3    │
└────────────────────────────────────────────┘
```

### Confidence Levels

| Level | Criteria |
|-------|----------|
| HIGH | All agents complete, patterns match, no conflicts |
| MEDIUM | Partial completion or some unknowns |
| LOW | Major unknowns, violations, or agent failures |

## Performance

| Hook | Target Duration | Max Agents | Mode |
|------|-----------------|------------|------|
| research-start | 20s | 4 | ALL |
| research-verify | 15s | 4 | FULL only |
| **Total (SPEED)** | **20s** | **4** | - |
| **Total (FULL)** | **35s** | **8** | - |

## Error Handling

| Scenario | Action |
|----------|--------|
| Agent timeout | Continue with partial results |
| Agent error | Log, continue with other agents |
| All agents fail | Fallback to minimal context |
| No results | Display "No matches found" + proceed |

**Important:** Research NEVER blocks the pipeline. Partial results are better than no results.

## Migration from Old Hooks

### Consolidated Hooks

| Old Hook | New Hook |
|----------|----------|
| `discover-hook.md` | research-start |
| PL Discovery (Phase 1) | research-start |
| `grounding-hook.md` | research-verify |
| `newtrack-hook.md` | Split between both |

### Key Changes

| Aspect | Old | New |
|--------|-----|-----|
| Total hooks | 5 | 2 |
| Max duration | ~95s | 35s |
| SPEED mode research | ~45s | 20s (1 hook) |
| Agent reuse | Minimal | Maximum |

## Related

- [research-start.md](./research-start.md) - Phase 1 research hook
- [research-verify.md](./research-verify.md) - Phase 3→4 verification hook
- [Research agents](../../../orchestrator/agents/research/) - Agent definitions
