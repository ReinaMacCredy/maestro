# Session Cache

Prevents duplicate grounding queries within a session.

## Overview

- **TTL:** 5 minutes
- **Scope:** Session-level (not persistent)
- **Key:** Hash of normalized query
- **Purpose:** Reduce redundant API calls, improve performance

---

## Cache Structure

```python
import hashlib
import re
import time

class GroundingCache:
    def __init__(self, ttl_seconds: int = 300):
        self.cache: dict[str, CacheEntry] = {}
        self.ttl = ttl_seconds
    
    def get(self, query: str) -> GroundingResult | None:
        """Retrieve cached result if valid."""
        key = self.hash_query(query)
        
        if key not in self.cache:
            return None
        
        entry = self.cache[key]
        if self.is_expired(entry):
            del self.cache[key]
            return None
        
        return entry.result
    
    def set(self, query: str, result: GroundingResult) -> None:
        """Cache a grounding result."""
        key = self.hash_query(query)
        self.cache[key] = CacheEntry(
            result=result,
            timestamp=time.time(),
            hit_count=0
        )
    
    def is_expired(self, entry: CacheEntry) -> bool:
        """Check if cache entry has expired."""
        return (time.time() - entry.timestamp) > self.ttl
    
    def hash_query(self, query: str) -> str:
        """Generate cache key from query."""
        normalized = self.normalize_query(query)
        return hashlib.sha256(normalized.encode()).hexdigest()[:16]
    
    def normalize_query(self, query: str) -> str:
        """Normalize query for consistent hashing.
        
        Preserves code-significant symbols (++, ::, [], <>) to avoid
        collisions like C++ -> C or std::vector -> stdvector.
        """
        # Lowercase
        q = query.lower()
        # Remove extra whitespace
        q = ' '.join(q.split())
        # Preserve code-significant symbols, only strip trailing punctuation
        # that doesn't change meaning (e.g., "auth?" -> "auth")
        q = re.sub(r'[?!.,;:]+$', '', q)
        return q
```

---

## Cache Entry

```python
from dataclasses import dataclass

@dataclass
class CacheEntry:
    result: GroundingResult
    timestamp: float
    hit_count: int = 0
```

---

## TTL Configuration

| Context | TTL | Rationale |
|---------|-----|-----------|
| Default | 5 min | Balance freshness vs. performance |
| Quick mode | 10 min | Prioritize speed |
| Full grounding | 2 min | Higher freshness requirements |

---

## Invalidation Rules

Cache is invalidated when:

1. **TTL expires** - Entry older than configured TTL
2. **Phase changes** - Moving to new phase clears cache (optional)
3. **Manual clear** - User requests fresh grounding
4. **Conflict detected** - Previous result had conflicts

```python
def should_invalidate(entry: CacheEntry, context: GroundingContext) -> bool:
    """Determine if cache entry should be invalidated."""
    
    # TTL check
    if is_expired(entry):
        return True
    
    # Conflict invalidation
    if entry.result.conflicts:
        return True
    
    # Low confidence invalidation for mandatory tier
    if context.tier == "full" and entry.result.confidence == "low":
        return True
    
    return False
```

---

## Cache Key Generation

Query normalization ensures similar queries hit the same cache entry while preserving code semantics:

| Original Query | Normalized | Hash |
|----------------|------------|------|
| "How does auth work?" | "how does auth work" | `a1b2c3...` |
| "how does AUTH work" | "how does auth work" | `a1b2c3...` |
| "How does auth work??" | "how does auth work" | `a1b2c3...` |
| "C++ templates" | "c++ templates" | `d4e5f6...` |
| "std::vector usage" | "std::vector usage" | `g7h8i9...` |
| "array[] syntax" | "array[] syntax" | `j0k1l2...` |

---

## Cache Bypass

Bypass cache when:

```python
def should_bypass_cache(
    query: str,
    context: GroundingContext,
    cache: GroundingCache
) -> bool:
    """Determine if cache should be bypassed."""
    
    # User requested fresh
    if context.force_refresh:
        return True
    
    # Mandatory tier with low cached confidence
    if context.tier == "full":
        cached = cache.get(query)
        if cached and cached.confidence == "low":
            return True
    
    return False
```

---

## Cache Metrics

Track cache performance:

```python
@dataclass
class CacheMetrics:
    hits: int = 0
    misses: int = 0
    invalidations: int = 0
    
    @property
    def hit_rate(self) -> float:
        total = self.hits + self.misses
        return self.hits / total if total > 0 else 0.0
```

---

## Integration with Router

```python
def grounding_with_cache(
    query: str, 
    tier: str, 
    cache: GroundingCache,
    context: GroundingContext
) -> GroundingResult:
    """Execute grounding with cache check.
    
    Uses route_grounding() and execute_cascade() from router.md.
    """
    
    # Check cache first
    cached = cache.get(query)
    if cached and not should_bypass_cache(query, context):
        cached.cached = True
        return cached
    
    # Execute grounding
    sources = route_grounding(query, tier)
    result = execute_cascade(sources, query, tier)
    
    # Cache result
    cache.set(query, result)
    result.cached = False
    
    return result
```

---

## Schema Extension

Cache status in result:

```json
{
  "queries": [
    {
      "question": "How does auth work?",
      "cached": true,
      "cache_age_seconds": 120
    }
  ]
}
```

---

## Related

- [tiers.md](tiers.md) - Tier definitions
- [router.md](router.md) - Source routing
- [sanitization.md](sanitization.md) - Query sanitization
