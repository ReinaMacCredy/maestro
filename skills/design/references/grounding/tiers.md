# Grounding Tiers

Defines grounding intensity levels based on mode and phase transition.

## Tier Definitions

### Light Tier

- **When:** SPEED mode, any phase transition
- **Sources:** 1 (repo only)
- **Timeout:** 3 seconds
- **Enforcement:** Advisory ⚠️
- **Tools:** Grep, Read

Use for quick exploration where speed matters more than verification depth.

### Mini Tier

- **When:** FULL mode, early phase transitions (DISCOVER→DEFINE, DEFINE→DEVELOP)
- **Sources:** 1-2 (repo or web, based on question type)
- **Timeout:** 5 seconds
- **Enforcement:** Advisory ⚠️
- **Tools:** Grep, Read, web_search (conditional)

Light verification to catch obvious misalignments without blocking flow.

### Standard Tier

- **When:** FULL mode, DEVELOP→DELIVER transition
- **Sources:** Cascade (repo → web → history)
- **Timeout:** 10 seconds
- **Enforcement:** Gatekeeper 🚫
- **Tools:** Grep, Read, finder, web_search

Full cascade with fallback. Blocks if grounding not run.

### Full Tier

- **When:** FULL mode, DELIVER→Complete transition
- **Sources:** Cascade + Impact Scan (parallel)
- **Timeout:** 45 seconds (30s grounding + 30s impact scan, parallel)
- **Enforcement:** Mandatory 🔒
- **Tools:** All sources + Impact Scan subagent

Complete verification. Blocks on failure or low confidence.

---

## Decision Matrix

| Mode | Phase Transition | Tier | Enforcement |
|------|------------------|------|-------------|
| SPEED | Any | Light | Advisory ⚠️ |
| FULL | DISCOVER→DEFINE | Mini | Advisory ⚠️ |
| FULL | DEFINE→DEVELOP | Mini | Advisory ⚠️ |
| FULL | DEVELOP→DELIVER | Standard | Gatekeeper 🚫 |
| FULL | DELIVER→Complete | Full | Mandatory 🔒 |

---

## Enforcement Levels

### Advisory ⚠️

- Grounding skip is logged
- Warning displayed to user
- Phase transition proceeds
- Use case: Early exploration where speed matters

### Gatekeeper 🚫

- Grounding must be run
- Phase transition blocked if skipped
- Low confidence results still allow proceed with warning
- Use case: Pre-delivery validation

### Mandatory 🔒

- Grounding must be run AND pass
- Blocks if: not run, all sources fail, or confidence too low
- Requires manual override or retry
- Use case: Final delivery gate

---

## Timeout Strategy

All timeouts are **soft limits** (warn + continue):

| Tier | Soft Limit | Hard Limit | On Timeout |
|------|------------|------------|------------|
| Light | 3s | 5s | Return partial + warning |
| Mini | 5s | 8s | Return partial + warning |
| Standard | 10s | 15s | Return partial + warning |
| Full | 45s | 60s | Block + manual verify |

---

## Source Configuration

| Source | Tier Availability | Priority | Tools |
|--------|-------------------|----------|-------|
| repo | All tiers | 1 (highest) | Grep, Read, finder |
| web | Mini, Standard, Full | 2 | web_search, read_web_page |
| history | Standard, Full | 3 | find_thread, git log |

---

## Tier Selection Algorithm

```python
def select_tier(mode: str, from_phase: str, to_phase: str) -> str:
    if mode == "SPEED":
        return "light"
    
    transition = (from_phase, to_phase)
    
    tier_map = {
        ("DISCOVER", "DEFINE"): "mini",
        ("DEFINE", "DEVELOP"): "mini",
        ("DEVELOP", "DELIVER"): "standard",
        ("DELIVER", "COMPLETE"): "full",
    }
    
    return tier_map.get(transition, "mini")
```

---

## Related

- [router.md](router.md) - Cascading source routing
- [cache.md](cache.md) - Session-level caching
- [sanitization.md](sanitization.md) - Query sanitization
