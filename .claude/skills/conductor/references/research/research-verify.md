# Research Verify Hook

> **Trigger:** Phase 3→4 (DEVELOP→VERIFY transition)  
> **Agents:** Analyzer + Pattern + Impact + Web  
> **Timeout:** 15s (hard limit, partial results acceptable)  
> **Mode:** SKIPPED in SPEED mode

---

## ⚡ SPEED Mode Behavior

> **IMPORTANT:** This hook is **SKIPPED entirely** in SPEED mode.
> 
> SPEED mode runs only `research-start` hook, not `research-verify`.
> This is by design—SPEED mode prioritizes velocity over verification.
>
> ```
> SPEED mode pipeline:
>   research-start → [develop] → SKIP research-verify → [verify]
>
> THOROUGH mode pipeline:
>   research-start → [develop] → research-verify → [verify]
> ```

---

## Overview

The research-verify hook runs parallel verification agents before entering the VERIFY phase. It validates proposed changes against codebase conventions, assesses impact, and gathers external references for complex patterns.

### When This Runs

| Condition | Action |
|-----------|--------|
| Mode = THOROUGH | ✅ Run all 4 agents |
| Mode = BALANCED | ✅ Run (may skip Web agent if confident) |
| Mode = SPEED | ❌ **SKIP ENTIRELY** |

---

## Agent Specification

### 1. Analyzer Agent

**Purpose:** Deep code analysis of proposed changes

```yaml
agent: AnalyzerAgent
role: research
scope: read-only
focus:
  - Static analysis of change locations
  - Function signature validation
  - Type compatibility checking
  - Import/export graph analysis
timeout: 5s
```

**Responsibilities:**
- Analyze proposed change locations
- Validate function signatures and types
- Check import/export dependencies
- Identify breaking changes

**Output:**
```json
{
  "analysis_complete": true,
  "locations_validated": 5,
  "type_issues": [],
  "breaking_changes": [],
  "confidence": "HIGH"
}
```

---

### 2. Pattern Agent

**Purpose:** Verify patterns match codebase conventions

```yaml
agent: PatternAgent
role: research
scope: read-only
focus:
  - Code style consistency
  - Naming convention adherence
  - Architectural pattern compliance
  - Anti-pattern detection
timeout: 4s
```

**Responsibilities:**
- Match against `code_styleguides/` rules
- Check naming conventions (files, functions, variables)
- Validate architectural patterns (e.g., component structure)
- Flag anti-patterns from CODEMAPS

**Output:**
```json
{
  "patterns_checked": 12,
  "matches": [
    {"pattern": "hook-naming", "match": true, "location": "hooks/useAuth.ts"}
  ],
  "violations": [],
  "recommendations": []
}
```

---

### 3. Impact Agent

**Purpose:** Assess blast radius and affected files

```yaml
agent: ImpactAgent
role: research
scope: read-only
focus:
  - Dependency graph traversal
  - Affected file enumeration
  - Blast radius calculation
  - Risk categorization
timeout: 4s
```

**Responsibilities:**
- Trace downstream dependencies
- Enumerate all affected files
- Calculate blast radius (LOW/MEDIUM/HIGH)
- Identify test coverage gaps

**Blast Radius Levels:**

| Level | Criteria |
|-------|----------|
| LOW | ≤5 files affected, no public APIs |
| MEDIUM | 6-15 files OR public API changes |
| HIGH | >15 files OR breaking changes OR core system |

**Output:**
```json
{
  "files_affected": ["src/api/auth.ts", "src/hooks/useAuth.ts"],
  "file_count": 2,
  "blast_radius": "LOW",
  "public_api_changes": false,
  "test_coverage": {
    "covered": 2,
    "uncovered": 0
  }
}
```

---

### 4. Web Agent

**Purpose:** Search for external patterns/documentation

```yaml
agent: WebAgent
role: research
scope: external
focus:
  - Library documentation lookup
  - Best practice verification
  - Security advisory checking
  - Version compatibility confirmation
timeout: 6s
```

**Responsibilities:**
- Search for library-specific patterns
- Verify against official documentation
- Check for security advisories
- Confirm version compatibility

**Output:**
```json
{
  "sources_checked": 3,
  "external_references": [
    {
      "source": "react-query docs",
      "url": "https://...",
      "relevance": "HIGH",
      "summary": "Confirms mutation pattern usage"
    }
  ],
  "security_advisories": [],
  "version_compatible": true
}
```

---

## Output Schema

Results stored in `pipeline_context.research.verify`:

```json
{
  "completed": true,
  "duration_ms": 12000,
  "agents_completed": 4,
  "agents_timed_out": 0,
  
  "analysis_results": [
    {
      "agent": "Analyzer",
      "locations_validated": 5,
      "issues": [],
      "confidence": "HIGH"
    }
  ],
  
  "pattern_matches": [
    {
      "pattern": "hook-naming",
      "match": true,
      "location": "hooks/useAuth.ts"
    },
    {
      "pattern": "component-structure",
      "match": true,
      "location": "components/AuthForm.tsx"
    }
  ],
  
  "impact_assessment": {
    "files_affected": [
      "src/api/auth.ts",
      "src/hooks/useAuth.ts",
      "src/components/AuthForm.tsx"
    ],
    "blast_radius": "LOW",
    "public_api_changes": false,
    "breaking_changes": []
  },
  
  "external_references": [
    {
      "source": "React Query Docs",
      "url": "https://tanstack.com/query/...",
      "relevance": "HIGH",
      "summary": "Mutation pattern validated"
    }
  ],
  
  "confidence": "HIGH",
  "skip_reason": null
}
```

### When Skipped (SPEED mode)

```json
{
  "completed": false,
  "skipped": true,
  "skip_reason": "SPEED_MODE",
  "duration_ms": 0,
  "confidence": null
}
```

---

## Confidence Levels

| Level | Criteria | Action |
|-------|----------|--------|
| **HIGH** | All 4 agents confirm, patterns match, blast radius LOW/MEDIUM | Proceed to VERIFY phase |
| **MEDIUM** | Partial confirmation, some unknowns, or Web agent timeout | Proceed with warning |
| **LOW** | Major unknowns, pattern violations, or conflicts detected | Trigger Oracle review |

### Confidence Calculation

```python
def calculate_confidence(results):
    # Start HIGH, degrade based on issues
    confidence = "HIGH"
    
    # Degrade to MEDIUM
    if results.agents_timed_out > 0:
        confidence = "MEDIUM"
    if len(results.pattern_matches.violations) > 0:
        confidence = "MEDIUM"
    if results.impact_assessment.blast_radius == "HIGH":
        confidence = "MEDIUM"
    
    # Degrade to LOW
    if results.agents_completed < 3:
        confidence = "LOW"
    if len(results.analysis_results.breaking_changes) > 0:
        confidence = "LOW"
    if results.external_references.security_advisories:
        confidence = "LOW"
    
    return confidence
```

---

## Timeout Handling

**Hard Limit:** 15 seconds total

```
Timeline:
[0s]──────[5s]──────[10s]──────[15s]
 │         │          │          │
 │         │          │          └── HARD STOP
 │         │          └── Warning: agents still running
 │         └── Check: ≥2 agents complete?
 └── Start all 4 agents in parallel
```

### Partial Results

If timeout reached before all agents complete:

1. Collect available results
2. Mark timed-out agents in output
3. Degrade confidence to MEDIUM (if ≥2 complete) or LOW (if <2 complete)
4. Proceed with available data

```json
{
  "completed": true,
  "duration_ms": 15000,
  "agents_completed": 3,
  "agents_timed_out": 1,
  "timed_out_agents": ["WebAgent"],
  "confidence": "MEDIUM"
}
```

---

## Integration with VERIFY Phase

### Oracle Integration

Results feed directly into Oracle audit:

```yaml
oracle_input:
  from_research_verify:
    - confidence_level
    - impact_assessment
    - pattern_violations
    - external_references
```

### Risk Escalation

| Condition | Action |
|-----------|--------|
| `blast_radius: HIGH` | Flag for Oracle deep review |
| `confidence: LOW` | Trigger warning, consider HALT |
| `breaking_changes: [...]` | Mandatory Oracle review |
| `security_advisories: [...]` | HALT for security review |

### Spike Task Spawning

HIGH risk items trigger spike `Task()` spawning:

```python
if results.impact_assessment.blast_radius == "HIGH":
    spawn_task(
        type="spike",
        title=f"Investigate blast radius for {feature}",
        context=results.impact_assessment,
        output="conductor/spikes/{track}/blast-radius-analysis.md"
    )

if results.confidence == "LOW":
    spawn_task(
        type="spike",
        title=f"Resolve confidence issues for {feature}",
        context={
            "unknowns": results.unknowns,
            "conflicts": results.conflicts
        },
        output="conductor/spikes/{track}/confidence-resolution.md"
    )
```

### Mode-Based Behavior

| Mode | Confidence LOW | Confidence MEDIUM | Confidence HIGH |
|------|----------------|-------------------|-----------------|
| THOROUGH | HALT + spike | Warning + proceed | Proceed |
| BALANCED | Warning + proceed | Proceed | Proceed |
| SPEED | N/A (hook skipped) | N/A | N/A |

---

## Execution Flow

```
┌─────────────────────────────────────────────────────────────┐
│  CHECK MODE                                                 │
│  ─────────                                                  │
│  if mode == SPEED:                                          │
│    return { skipped: true, skip_reason: "SPEED_MODE" }      │
└─────────────────────────────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────┐
│  SPAWN AGENTS (parallel)                                    │
│  ───────────────────────                                    │
│  Task(AnalyzerAgent)  ──┐                                   │
│  Task(PatternAgent)   ──┼── await all (15s timeout)         │
│  Task(ImpactAgent)    ──┤                                   │
│  Task(WebAgent)       ──┘                                   │
└─────────────────────────────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────┐
│  AGGREGATE RESULTS                                          │
│  ─────────────────                                          │
│  - Collect all agent outputs                                │
│  - Handle timeouts (partial results OK)                     │
│  - Calculate confidence                                     │
└─────────────────────────────────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────┐
│  STORE & INTEGRATE                                          │
│  ────────────────                                           │
│  pipeline_context.research.verify = results                 │
│  → Feed to Oracle audit                                     │
│  → Spawn spikes if HIGH risk                                │
│  → HALT if confidence LOW + mode THOROUGH                   │
└─────────────────────────────────────────────────────────────┘
```

---

## Related

- [research-start.md](./research-start.md) - Initial research hook
- [protocol.md](../protocol.md) - Research protocol specification
- [Oracle agent](../../../../orchestrator/agents/review/oracle.md) - Audit integration
- [Impact Assessor](../../../../orchestrator/agents/research/impact-assessor.md) - Impact agent details
