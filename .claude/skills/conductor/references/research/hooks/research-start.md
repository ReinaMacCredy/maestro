# Research Start Hook

> **Trigger:** Phase 1 (DISCOVER) start  
> **Agents:** Locator + Pattern + CODEMAPS + Architecture  
> **Timeout:** 20s (hard limit, partial results acceptable)  
> **Mode:** Runs in BOTH SPEED and FULL modes

---

## Overview

The research-start hook runs at the very beginning of the unified DS pipeline, gathering codebase context before any user interaction. This hook consolidates the old `discover-hook` and PL Phase 1 Discovery into a single, parallel research operation.

### What This Replaces

| Old Hook | Now Merged Into |
|----------|-----------------|
| `discover-hook.md` | research-start |
| PL Discovery (Phase 1) | research-start |

---

## Agent Specification

All 4 agents run **in parallel** for maximum speed.

### 1. Locator Agent

**Purpose:** Find files in the codebase related to the user's request.

| Aspect | Value |
|--------|-------|
| Tools | `Grep`, `finder`, `glob` |
| Timeout | 5s soft / 20s hard |
| Output | List of relevant file paths |

**Search Strategy:**
1. Extract keywords from user request
2. Search file names and content
3. Rank by relevance (direct match > indirect)
4. Return top 20 files

### 2. Pattern Agent

**Purpose:** Find similar implementations or patterns in the codebase.

| Aspect | Value |
|--------|-------|
| Tools | `finder`, `Grep`, `Read` |
| Timeout | 5s soft / 20s hard |
| Output | List of similar features/patterns |

**Pattern Detection:**
1. Identify the type of feature requested
2. Search for similar existing features
3. Extract reusable patterns
4. Note conventions and standards

### 3. CODEMAPS Agent

**Purpose:** Load relevant architecture documentation modules.

| Aspect | Value |
|--------|-------|
| Tools | `Read` (conductor/CODEMAPS/) |
| Timeout | 3s soft / 20s hard |
| Output | Loaded CODEMAPS modules |

**CODEMAPS Loading:**
1. Always load `overview.md`
2. Identify relevant modules based on topic
3. Load up to 3 relevant module files
4. Extract key architectural constraints

### 4. Architecture Agent

**Purpose:** Analyze structural constraints and integration points.

| Aspect | Value |
|--------|-------|
| Tools | `Read`, `Grep`, package.json, tsconfig |
| Timeout | 5s soft / 20s hard |
| Output | Architecture notes and constraints |

**Analysis Scope:**
1. Package dependencies
2. Build configuration
3. Module boundaries
4. Entry points

---

## Output Schema

Results are stored in `pipeline_context.research.start`:

```json
{
  "completed": true,
  "duration_ms": 18000,
  "timestamp": "2026-01-08T10:00:00Z",
  
  "locator": {
    "files_found": [
      {
        "path": "src/auth/login.ts",
        "relevance": "HIGH",
        "reason": "Direct match for authentication"
      },
      {
        "path": "src/auth/types.ts",
        "relevance": "MEDIUM",
        "reason": "Related type definitions"
      }
    ],
    "duration_ms": 4500
  },
  
  "pattern": {
    "patterns_matched": [
      {
        "name": "Auth middleware pattern",
        "location": "src/middleware/auth.ts",
        "description": "JWT validation middleware"
      }
    ],
    "similar_features": [
      {
        "name": "OAuth integration",
        "location": "src/auth/oauth.ts"
      }
    ],
    "duration_ms": 5000
  },
  
  "codemaps": {
    "modules_loaded": [
      "overview.md",
      "auth.md",
      "api.md"
    ],
    "key_constraints": [
      "All auth must use JWT",
      "API follows REST conventions"
    ],
    "duration_ms": 2000
  },
  
  "architecture": {
    "dependencies": {
      "auth": ["jsonwebtoken", "bcrypt"]
    },
    "entry_points": [
      "src/index.ts",
      "src/api/routes.ts"
    ],
    "constraints": [
      "TypeScript strict mode",
      "ESM modules only"
    ],
    "duration_ms": 4000
  }
}
```

---

## Progressive Rendering

Results are displayed **as agents complete**, not waiting for all to finish:

```
┌─ RESEARCH CONTEXT ─────────────────────────┐
│ Topic: {extracted topic}                   │
│ Duration: Xs (in progress...)              │
├────────────────────────────────────────────┤
│ ✅ LOCATOR (4.5s):                         │
│ • src/auth/login.ts - Direct match         │
│ • src/auth/types.ts - Related types        │
│                                            │
│ ✅ PATTERN (5.0s):                         │
│ • Auth middleware pattern in middleware/   │
│                                            │
│ ⏳ CODEMAPS: Loading...                    │
│                                            │
│ ⏳ ARCHITECTURE: Analyzing...              │
└────────────────────────────────────────────┘
```

On completion:

```
┌─ RESEARCH CONTEXT ─────────────────────────┐
│ Topic: Authentication feature              │
│ Duration: 18s (complete)                   │
├────────────────────────────────────────────┤
│ RELATED CODE:                              │
│ • src/auth/login.ts - Direct match         │
│ • src/auth/oauth.ts - Similar feature      │
├────────────────────────────────────────────┤
│ PATTERNS:                                  │
│ • Auth middleware pattern                  │
│ • JWT validation pattern                   │
├────────────────────────────────────────────┤
│ CODEMAPS:                                  │
│ • auth.md - Auth architecture              │
│ • api.md - API conventions                 │
├────────────────────────────────────────────┤
│ CONSTRAINTS:                               │
│ • TypeScript strict mode                   │
│ • JWT required for all auth                │
└────────────────────────────────────────────┘
```

---

## Timeout Handling

| Scenario | Action |
|----------|--------|
| Agent completes < 5s | ✅ Include results |
| Agent completes 5-20s | ✅ Include results |
| Agent times out (20s) | ⚠️ Include partial results |
| All agents time out | ⚠️ Proceed with minimal context |

**Important:** Research NEVER blocks the pipeline. Partial results are better than no results.

---

## Execution Notes

1. **Always runs** - No skip conditions. Research always provides value.
2. **Parallel execution** - All 4 agents run simultaneously for speed.
3. **Progressive display** - Show results as they arrive, don't wait.
4. **Timeout is hard** - At 20s, stop waiting and use what we have.
5. **Minimal context OK** - If research fails, DISCOVER phase can still proceed.

---

## Integration with DISCOVER Phase

The research results feed directly into the DISCOVER phase:

1. **Locator results** → Show user relevant existing code
2. **Pattern results** → Suggest reusable patterns
3. **CODEMAPS results** → Provide architectural context
4. **Architecture results** → Inform technical constraints

This context enables more informed questions during DISCOVER.

---

## Migration from Old Hooks

### From discover-hook.md

| Old | New |
|-----|-----|
| 3 agents | 4 agents (added Architecture) |
| 10s timeout | 20s timeout |
| Single output | Structured per-agent output |

### From PL Discovery

| Old | New |
|-----|-----|
| Ran in PL Phase 1 | Runs at DS start |
| Task() based | Integrated parallel agents |
| ~30s duration | 20s max |

---

## Related

- [research-verify.md](research-verify.md) - Phase 3→4 verification hook
- [unified-pipeline.md](../../../design/references/unified-pipeline.md) - Full pipeline reference
- [session-init.md](../../../design/references/session-init.md) - Session initialization
