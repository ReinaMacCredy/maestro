# Research-Based Verification System

> The tiered grounding system has been replaced by the Research Protocol.
> See [conductor/references/research/protocol.md](../../conductor/references/research/protocol.md) for complete documentation.

## Overview

Research verification uses **parallel sub-agents** instead of sequential grounding:

| Mode | Phase Transition | Agents | Enforcement |
|------|------------------|--------|-------------|
| SPEED | Any | 1 (Locator) | Advisory ⚠️ |
| FULL | DISCOVER→DEFINE | 2 (Locator + Pattern) | Advisory ⚠️ |
| FULL | DEFINE→DEVELOP | 2 (Locator + Pattern) | Advisory ⚠️ |
| FULL | DEVELOP→DELIVER | 4 (All agents) | Gatekeeper 🚫 |
| FULL | DELIVER→Complete | 5 (All + Impact) | Mandatory 🔒 |

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

## DEVELOP → DELIVER Verification

**Gatekeeper enforcement (🚫):**

1. Spawn 4 agents in parallel (Locator, Analyzer, Pattern, Web)
2. Timeout: 15s max
3. Calculate confidence based on agent results
4. Display verification summary
5. **HALT if not run** - show verification required prompt
6. On skip: Display warning banner, log for audit, proceed

## DELIVER → Complete Verification

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

Track verification completion across phases in session memory:

```json
{
  "DISCOVER→DEFINE": { "completed": true, "confidence": "HIGH", "timestamp": "..." },
  "DEFINE→DEVELOP": { "completed": true, "confidence": "MEDIUM", "timestamp": "..." },
  "DEVELOP→DELIVER": null,
  "DELIVER→Complete": null
}
```

## Validation Gate: validate-design

After research verification passes, run the design validation gate:

1. Load gate: `conductor/references/validation/shared/validate-design.md`
2. Run validation: Check design vs product.md, tech-stack.md, CODEMAPS
3. Update metadata.json: Add to `validation.gates_passed` or `validation.last_failure`
4. Behavior by mode:
   - **SPEED mode**: WARN on failure, continue to A/P/C
   - **FULL mode**: HALT on failure, retry up to 2x, then escalate

## Documentation

- [Research Protocol](../../conductor/references/research/protocol.md) - Main documentation
- [Research agents](../../orchestrator/agents/research/) - Research-specific agents
- [Integration hooks](../../conductor/references/research/hooks/) - Hook integration points
