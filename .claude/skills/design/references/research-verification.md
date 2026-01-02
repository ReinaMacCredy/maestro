# Research-Based Verification System

> The tiered grounding system has been replaced by the Research Protocol.
> See [conductor/references/research/protocol.md](../../conductor/references/research/protocol.md) for complete documentation.

## Overview

Research verification uses **parallel sub-agents** at specific trigger points (not every phase transition):

| Trigger Point | Agents | Enforcement |
|---------------|--------|-------------|
| Session start (before DISCOVER) | Locator + Pattern + CODEMAPS | Advisory ⚠️ |
| CP3 (DEVELOP) | Locator + Analyzer + Pattern | Gatekeeper 🚫 |
| CP4 (DELIVER) | All 5 + Impact scan | Mandatory 🔒 |

**Note:** Research does NOT run at every phase transition. Only at session start, CP3, and CP4.

## Enforcement Levels

| Level | Symbol | Behavior |
|-------|--------|----------|
| Advisory | ⚠️ | Log skip, warn, proceed |
| Gatekeeper | 🚫 | Block if verification not run |
| Mandatory | 🔒 | Block if fails or low confidence |

## Research Agents

Five specialized agents run in parallel:

| Agent | Purpose |
|-------|---------|
| **Locator** | Find all affected files |
| **Analyzer** | Deep interface/dependency analysis |
| **Pattern** | Verify patterns match conventions |
| **Impact** | Full scope assessment (files, modules, risk) |
| **Web** | Verify external API docs (if external deps) |

## Session Start Research (Before DISCOVER)

**Advisory enforcement (⚠️):**

Spawns automatically when `ds` starts, BEFORE entering DISCOVER phase:

1. Load: `conductor/references/research/hooks/discover-hook.md`
2. Spawn agents: Locator + Pattern + CODEMAPS loader
3. Timeout: 10s max
4. Display research summary
5. On skip: Display warning, proceed to DISCOVER

This provides context for the entire design session.

## CP3 (DEVELOP) Research

**Gatekeeper enforcement (🚫):**

1. Load: `conductor/references/research/hooks/grounding-hook.md`
2. Spawn 3 agents in parallel (Locator, Analyzer, Pattern)
3. Timeout: 15s max
4. Calculate confidence based on agent results
5. Display verification summary
6. **HALT if not run** - show verification required prompt
7. On skip: Display warning banner, log for audit, proceed

## CP4 (DELIVER) Full Verification

**Mandatory enforcement (🔒):**

1. Spawn ALL 5 research agents in parallel
2. Timeout: 20s total (parallel execution)
3. Calculate confidence:
   - All agents pass, no conflicts → HIGH
   - Minor conflicts or warnings → MEDIUM
   - Major conflicts or agent failures → LOW
4. **BLOCK if:**
   - Verification not run
   - Confidence = LOW
   - Major conflicts detected
5. Override requires explicit justification: `SKIP_VERIFICATION: <reason>`

## Research State Tracking

Track verification completion in session memory:

```json
{
  "session_start": { "completed": true, "confidence": "HIGH", "timestamp": "..." },
  "CP3_DEVELOP": { "completed": true, "confidence": "MEDIUM", "timestamp": "..." },
  "CP4_DELIVER": null
}
```

## Progressive Validation (runs at every checkpoint)

In addition to research, validation runs at EVERY checkpoint (CP1-4):

| Checkpoint | Validation Checks | Enforcement |
|------------|------------------|-------------|
| CP1 (DISCOVER) | Product alignment, no duplicate features | WARN |
| CP2 (DEFINE) | Problem clear, success measurable, scope explicit | WARN |
| CP3 (DEVELOP) | 3+ options, tech-stack alignment, risk analysis | WARN |
| CP4 (DELIVER) | Full validation + grounding + impact scan | SPEED=WARN, FULL=HALT |

See [validation/lifecycle.md](../../conductor/references/validation/lifecycle.md) for complete validation gate details.

## Documentation

- [Research Protocol](../../conductor/references/research/protocol.md) - Main documentation
- [Research agents](../../orchestrator/agents/research/) - Research-specific agents
- [Integration hooks](../../conductor/references/research/hooks/) - Hook integration points
- [Validation lifecycle](../../conductor/references/validation/lifecycle.md) - Per-checkpoint validation
