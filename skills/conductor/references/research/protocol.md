# Research Protocol — Parallel Sub-Agent Codebase Research

## Overview

Research protocol spawns parallel sub-agents to comprehensively document the codebase AS-IS. Used at three integration points in the Maestro workflow.

## Critical Rule

**DOCUMENT, DON'T EVALUATE**

- DO NOT suggest improvements or changes
- DO NOT perform root cause analysis
- DO NOT propose future enhancements
- DO NOT critique the implementation
- DO NOT recommend refactoring
- ONLY describe what exists, where it exists, how it works

## Integration Points

| Hook | Trigger | Purpose | Agents |
|------|---------|---------|--------|
| [discover-hook](hooks/discover-hook.md) | `ds` starts | Ground context before DISCOVER | Locator + Pattern + CODEMAPS |
| [grounding-hook](hooks/grounding-hook.md) | DEVELOP→DELIVER | Verify design against codebase | Locator + Analyzer + Pattern + Web |
| [newtrack-hook](hooks/newtrack-hook.md) | `/conductor-newtrack` | Research before spec generation | All 5 agents |

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

**No skip conditions exist. Research ALWAYS runs.**

## Research Agents

| Agent | Purpose | When to Use |
|-------|---------|-------------|
| [codebase-locator](../../../orchestrator/agents/research/codebase-locator.md) | Find WHERE files/components live | First - discovery |
| [codebase-analyzer](../../../orchestrator/agents/research/codebase-analyzer.md) | Understand HOW code works | After locator finds targets |
| [pattern-finder](../../../orchestrator/agents/research/pattern-finder.md) | Find existing patterns | When looking for conventions |
| [web-researcher](../../../orchestrator/agents/research/web-researcher.md) | External docs/APIs | Only if explicitly needed |

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
│     ├─► Wait for ALL agents to complete                     │
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
| HIGH | 3+ matches, clear patterns |
| MEDIUM | 1-3 matches, some ambiguity |
| LOW | 0 matches or conflicting info |

## Integration with Existing Systems

### Replaces Grounding at DEVELOP→DELIVER

Old grounding:
```
finder → Grep → web_search (sequential)
```

New research:
```
┌─────────────┬─────────────┬─────────────┐
│  Locator    │  Analyzer   │  Pattern    │  (parallel)
└─────────────┴─────────────┴─────────────┘
         ↓
    Synthesize
```

### Enhances DISCOVER Phase

Before asking user questions, auto-research:
- Existing similar features
- Related patterns
- Affected components

### Pre-Spec Research for Newtrack

Before generating spec.md:
- Research affected files
- Find existing patterns to follow
- Identify dependencies

## Performance

| Hook | Target Duration | Max Agents |
|------|-----------------|------------|
| discover | 10s | 3 |
| verification | 15s | 5 |
| newtrack | 20s | 5 |

## Error Handling

| Scenario | Action |
|----------|--------|
| Agent timeout (30s) | Continue with partial results |
| Agent error | Log, continue with other agents |
| All agents fail | Fallback to manual research |
| No results | Display "No matches found" + proceed |

## Related

- [Research agents](../../../orchestrator/agents/research/) - Research agent definitions
- [hooks/](hooks/) - Integration hook specifications
