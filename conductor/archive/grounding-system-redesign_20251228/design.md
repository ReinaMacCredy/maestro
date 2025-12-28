---
track_id: grounding-system-redesign_20251228
created: 2025-12-28T11:00:00Z
status: approved
complexity_score: 6
mode: FULL
---

# Grounding System Redesign

## Problem Statement

Grounding within the Design skill is currently optional, reactive, and single-source — this leads to designs based on outdated or hallucinated information, with errors only discovered in the final (DELIVER) phase after significant effort has already been spent.

## Success Criteria

| Metric | Target |
|--------|--------|
| Grounding coverage | 100% phase transitions có grounding |
| Detection rate | Agent skip grounding → blocked (at DELIVER) |
| Source accuracy | Đúng source cho đúng question type ≥90% |
| Impact scan quality | List files chính xác ≥95% |
| Performance | Light ≤5s, Standard ≤10s, Full ≤45s |

## Out of Scope

- Real-time grounding during conversation
- External MCP integration
- Custom grounding plugins
- Keyword triggers (Phase 2)
- Confidence scoring (Phase 2)
- Intent classification layer (Phase 2)

## Chosen Approach

**Hybrid Tiered Model** combining:
- Cascading Router (priority chain + fallback)
- Tiered Enforcement (advisory → gatekeeper → mandatory)
- Impact Scan Subagent (parallel with full grounding)

---

## Design

### Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                    GROUNDING SYSTEM                             │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌─────────┐     ┌──────────────┐     ┌───────────────┐         │
│  │ TRIGGER │────▶│ MODE CHECK   │────▶│ TIER SELECT   │         │
│  └─────────┘     └──────────────┘     └───────┬───────┘         │
│                                               │                 │
│         ┌─────────────────────────────────────┼─────────────┐   │
│         │                                     │             │   │
│         ▼                                     ▼             ▼   │
│  ┌─────────────┐                    ┌─────────────┐  ┌─────────┐│
│  │    LIGHT    │                    │  STANDARD   │  │  FULL   ││
│  │  (SPEED)    │                    │  (FULL)     │  │+IMPACT  ││
│  │             │                    │             │  │  SCAN   ││
│  │ 1 source    │                    │ cascade     │  │ parallel││
│  │ 3s timeout  │                    │ repo→web    │  │ 30s each││
│  └──────┬──────┘                    └──────┬──────┘  └────┬────┘│
│         │                                  │              │     │
│         ▼                                  ▼              ▼     │
│  ┌─────────────┐                    ┌─────────────┐  ┌─────────┐│
│  │  ADVISORY   │                    │ GATEKEEPER  │  │MANDATORY││
│  │  (log only) │                    │ (can block) │  │ (block) ││
│  └─────────────┘                    └─────────────┘  └─────────┘│
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

### Tiered Grounding Matrix

| Mode | Phase Transition | Grounding Type | Enforcement |
|------|------------------|----------------|-------------|
| SPEED | Any | Light (1 source, 3s max) | Advisory ⚠️ |
| FULL | DISCOVER→DEFINE | Mini (repo check) | Advisory ⚠️ |
| FULL | DEFINE→DEVELOP | Mini (web verify) | Advisory ⚠️ |
| FULL | DEVELOP→DELIVER | Standard (cascade) | Gatekeeper 🚫 |
| FULL | DELIVER→Complete | Full + Impact Scan | Mandatory 🔒 |

### Folder Structure

```
conductor/tracks/{track-id}/
├── design.md
├── spec.md
├── plan.md
└── grounding/
    ├── discover-define.json
    ├── define-develop.json
    ├── develop-deliver.json
    ├── deliver-complete.json
    └── impact-scan.md

skills/design/references/grounding/
├── tiers.md
├── router.md
├── cache.md
├── sanitization.md
└── impact-scan-prompt.md
```

### Schema v1.1

```json
{
  "$schema": "grounding-result-v1.1",
  "phase_transition": "DISCOVER→DEFINE",
  "grounding_type": "mini|standard|full",
  "timestamp": "2025-12-28T10:30:00Z",
  "duration_ms": 1200,
  
  "routing": {
    "sources_tried": ["repo", "web"],
    "sources_succeeded": ["repo"],
    "fallback_used": true
  },
  
  "queries": [
    {
      "intent": "pattern_check",
      "question": "How does this repo handle auth?",
      "source": "repo",
      "tool": "Grep",
      "result_summary": "Found JWT middleware in src/auth/",
      "confidence": "high|medium|low",
      "cached": false
    }
  ],
  
  "conflicts": [],
  "overall_confidence": "high",
  "all_sources_failed": false,
  "blocking": false,
  "notes": ""
}
```

### Cascading Router

```python
def route_grounding(question: str, grounding_type: str) -> list[Source]:
    if grounding_type == "light":
        return [Source.REPO]
    
    if grounding_type == "mini":
        if contains_external_ref(question):
            return [Source.WEB, Source.REPO]
        else:
            return [Source.REPO, Source.WEB]
    
    if grounding_type in ["standard", "full"]:
        return [Source.REPO, Source.WEB, Source.HISTORY]
    
    return [Source.REPO]

def execute_cascade(sources: list[Source], question: str) -> GroundingResult:
    results = []
    for source in sources:
        try:
            result = query_source(source, question, timeout=get_timeout(source))
            results.append(result)
            if result.confidence == "high":
                break
        except TimeoutError:
            continue
    
    if not results:
        return GroundingResult(
            confidence="none",
            blocking=True,
            notes="All sources failed. Manual grounding required."
        )
    
    return merge_results(results)
```

### Merge Protocol (Grounding + Impact Scan)

```python
CONFIDENCE_LEVELS = ["none", "low", "medium", "high"]

def confidence_rank(level: str) -> int:
    """Convert confidence level to numeric rank for comparison."""
    return {"low": 1, "medium": 2, "high": 3}.get(level, 0)

def confidence_from_rank(rank: int) -> str:
    """Convert numeric rank back to confidence level string."""
    return CONFIDENCE_LEVELS[rank]

def merge_grounding_and_impact(
    grounding: GroundingResult,
    impact: ImpactScanResult
) -> DeliverPhaseResult:
    combined_rank = min(confidence_rank(grounding.confidence), confidence_rank(impact.confidence))
    combined_confidence = confidence_from_rank(combined_rank)
    return DeliverPhaseResult(
        grounding=grounding,
        impact=impact,
        combined_confidence=combined_confidence,
        blocking=grounding.blocking or impact.has_high_risk_files
    )
```

### Enforcement Mechanism

```python
class GroundingEnforcer:
    def check_transition(self, from_phase, to_phase, mode, grounding_result):
        level = self.get_enforcement_level(from_phase, to_phase, mode)
        
        if level == "mandatory":
            if grounding_result is None:
                return EnforcementResult(allowed=False, action="RUN_GROUNDING")
            if grounding_result.all_sources_failed:
                return EnforcementResult(allowed=False, action="MANUAL_VERIFY")
            if grounding_result.overall_confidence == "low":
                return EnforcementResult(allowed=False, action="RETRY_GROUNDING")
        
        elif level == "gatekeeper":
            if grounding_result is None:
                return EnforcementResult(allowed=False, action="RUN_GROUNDING")
        
        elif level == "advisory":
            if grounding_result is None:
                return EnforcementResult(allowed=True, warning="⚠️ Grounding skipped.")
        
        return EnforcementResult(allowed=True)
    
    def get_enforcement_level(self, from_phase, to_phase, mode):
        if mode == "SPEED":
            return "advisory"
        
        matrix = {
            ("DELIVER", "COMPLETE"): "mandatory",
            ("DEVELOP", "DELIVER"): "gatekeeper",
            ("DEFINE", "DEVELOP"): "advisory",
            ("DISCOVER", "DEFINE"): "advisory",
        }
        return matrix.get((from_phase, to_phase), "advisory")
```

### Sanitization

```python
def sanitize_grounding_query(query: str) -> str:
    patterns_to_remove = [
        r'API[_-]?KEY[=:]\s*\S+',
        r'SECRET[=:]\s*\S+',
        r'PASSWORD[=:]\s*\S+',
        r'TOKEN[=:]\s*\S+',
    ]
    sanitized = query
    for pattern in patterns_to_remove:
        sanitized = re.sub(pattern, '[REDACTED]', sanitized, flags=re.I)
    return sanitized
```

### Session Cache

```python
class GroundingCache:
    def __init__(self, ttl_seconds=300):
        self.cache = {}
        self.ttl = ttl_seconds
    
    def get(self, query_hash):
        if query_hash in self.cache:
            entry = self.cache[query_hash]
            if not self.is_expired(entry):
                return entry["result"]
        return None
    
    def set(self, query_hash, result):
        self.cache[query_hash] = {
            "result": result,
            "timestamp": time.time()
        }
```

### Conflict Visibility Output

```
┌─ GROUNDING RESULT ─────────────────────┐
│ Source: repo (Grep)                    │
│ Answer: JWT middleware in src/auth/    │
│ Confidence: HIGH                       │
├────────────────────────────────────────┤
│ ⚠️ CONFLICT DETECTED                   │
│ Web source suggests: OAuth2 flow       │
│ Using: repo (higher confidence)        │
│ Review recommended before DELIVER      │
└────────────────────────────────────────┘
```

### Error Catalog

| Code | Message | Action |
|------|---------|--------|
| GR-001 | All sources failed | Manual verification required |
| GR-002 | Timeout exceeded | Retry or skip |
| GR-003 | Low confidence | Additional verification needed |
| GR-004 | Conflict detected | Review conflict summary |
| GR-005 | Query sanitized | Sensitive content removed |

---

## Testing Strategy

### Acceptance Tests

| # | Criterion | Test Method |
|---|-----------|-------------|
| 1 | Phase transitions trigger grounding | Unit: mock transition → expect call |
| 2 | DELIVER blocks without grounding | Integration: skip → expect error |
| 3 | Router selects correct source | Fixture: 20 questions → ≥90% correct |
| 4 | Impact scan returns file list | Subagent: known design → expected files |
| 5 | Cache prevents duplicate queries | Unit: same query 2x → 1 API call |
| 6 | Sanitization removes secrets | Regex: API_KEY → [REDACTED] |

### Resilience Tests

| # | Scenario | Expected Behavior |
|---|----------|-------------------|
| R1 | Grounding timeout | Return partial + warning |
| R2 | Subagent timeout | Use grounding-only, skip impact |
| R3 | All sources fail | Block + manual verify prompt |
| R4 | Network failure | Fallback to repo-only |

---

## Implementation Plan

### Phased Delivery

**PR1: Foundation**
1. Create `grounding/` folder structure
2. `tiers.md` - tier definitions
3. `router.md` - cascading router
4. `cache.md` - session cache
5. `sanitization.md` - query sanitization
6. Update `grounding.md` - integrate new modules

**PR2: Enforcement**
1. `impact-scan-prompt.md` - subagent template
2. Update `design/SKILL.md` - integration points
3. Update `design.toml` - enforcement checkpoints
4. Update `conductor/SKILL.md` - references
5. Add resilience handling

### Implementation Order

1. `grounding/tiers.md` (foundation)
2. `grounding/router.md` (core logic)
3. `grounding/cache.md` (optimization)
4. `grounding/sanitization.md` (security)
5. `grounding/impact-scan-prompt.md` (subagent)
6. `grounding.md` (rewrite)
7. `design/SKILL.md` (integration)
8. `design.toml` (enforcement)
9. `conductor/SKILL.md` (references)
10. `conductor-design-workflow.md` (final)

---

## Documentation Deliverables

1. `docs/grounding-user-guide.md` - End user instructions
2. `docs/grounding-migration.md` - Upgrade path
3. `grounding/troubleshooting.md` - Error resolution
4. `grounding/api-reference.md` - Schema + functions

---

## Risks & Open Questions

| Risk | Mitigation |
|------|------------|
| Breaking existing design sessions | Feature flag / gradual rollout |
| Performance regression | Benchmark before/after |
| Subagent timeout in large repos | Chunked scan with early return |

| Decision Made | Value |
|---------------|-------|
| Track-level grounding folder | Yes (isolation) |
| Error catalog format | Markdown (human-readable) |
| Cache TTL | 5 minutes |
| Performance limits | Soft (warn + continue) |
| Conflict handling | Prefer confidence + show visibility |

---

## Grounding Notes

- Verified against existing `grounding.md`, `SKILL.md`, `design.toml`
- No conflicting patterns found in codebase
- Impact scan identified 11 files (6 create, 5 modify)
- Party Mode review: APPROVED with conditions (all incorporated)

---

## Approvals

| Reviewer | Verdict | Conditions |
|----------|---------|------------|
| John (PM) | ✅ | Phased delivery |
| Winston (Architect) | ✅ | Merge protocol |
| Murat (Test Architect) | ✅ | Resilience tests |
| Paige (Tech Writer) | ✅ | Doc plan |

**Final Status: ✅ APPROVED**
