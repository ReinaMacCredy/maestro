# Cascading Router

Routes grounding queries to appropriate sources with fallback logic.

## Overview

The router selects sources based on:
1. Grounding tier (determines available sources)
2. Question type (determines priority order)
3. Fallback chain (when primary source fails)

---

## Source Priority

Default priority chain: **repo → web → history**

| Source | Description | Best For |
|--------|-------------|----------|
| repo | Local codebase search | Patterns, conventions, existing code |
| web | External web search | APIs, libraries, documentation |
| history | Thread/git history | Past decisions, context |

---

## Routing Algorithm

```python
def route_grounding(question: str, tier: str) -> list[Source]:
    """Select sources based on tier and question type."""
    
    if tier == "light":
        return [Source.REPO]
    
    if tier == "mini":
        if contains_external_ref(question):
            return [Source.WEB, Source.REPO]
        return [Source.REPO, Source.WEB]
    
    if tier in ["standard", "full"]:
        return [Source.REPO, Source.WEB, Source.HISTORY]
    
    return [Source.REPO]
```

---

## External Reference Detection

Questions are routed to web first when they contain external references:

```python
def contains_external_ref(question: str) -> bool:
    """Detect if question references external resources."""
    
    patterns = [
        r'https?://',           # URLs
        r'\bAPI\b',             # API references
        r'\blibrary\b',         # Library mentions
        r'\bpackage\b',         # Package mentions
        r'\bdocumentation\b',   # Doc requests
        r'\bversion\s+\d',      # Version numbers
        r'\blatest\b',          # Freshness indicators
        r'\brelease\b',         # Release mentions
    ]
    
    for pattern in patterns:
        if re.search(pattern, question, re.I):
            return True
    return False
```

---

## Cascade Execution

```python
def execute_cascade(
    sources: list[Source], 
    question: str,
    tier: str
) -> GroundingResult:
    """Execute sources in order with fallback on failure."""
    
    results = []
    timeouts = get_timeouts(tier)
    
    for source in sources:
        try:
            result = query_source(
                source=source,
                question=question,
                timeout=timeouts[source]
            )
            results.append(result)
            
            # Early exit on high confidence
            if result.confidence == "high":
                break
                
        except TimeoutError:
            log_warning(f"Source {source} timed out, trying fallback")
            continue
            
        except SourceError as e:
            log_warning(f"Source {source} failed: {e}, trying fallback")
            continue
    
    if not results:
        return GroundingResult(
            confidence="none",
            all_sources_failed=True,
            blocking=True,
            notes="All sources failed. Manual grounding required."
        )
    
    return merge_results(results)
```

---

## Source-Specific Tools

### repo Source

| Tool | Use Case |
|------|----------|
| Grep | Exact text/pattern matching |
| finder | Semantic code search |
| Read | File content inspection |
| glob | File pattern matching |

**Query strategy:**
1. Extract key terms from question
2. Run Grep for exact matches
3. Use finder for semantic search if Grep yields low results
4. Read relevant files for context

### web Source

| Tool | Use Case |
|------|----------|
| web_search | Find relevant pages |
| read_web_page | Extract content from URLs |

**Query strategy:**
1. Formulate search query from question
2. Run web_search with objective
3. Read top results for verification

### history Source

| Tool | Use Case |
|------|----------|
| find_thread | Search past Amp threads |
| git log | Search commit history |

**Query strategy:**
1. Search threads for related discussions
2. Check git history for related changes
3. Extract relevant context

---

## Timeout Configuration

| Tier | repo | web | history |
|------|------|-----|---------|
| light | 3s | - | - |
| mini | 3s | 5s | - |
| standard | 5s | 8s | 5s |
| full | 10s | 15s | 10s |

---

## Result Merging

When multiple sources return results:

```python
def merge_results(results: list[SourceResult]) -> GroundingResult:
    """Merge results from multiple sources."""
    
    # Sort by confidence
    sorted_results = sorted(
        results, 
        key=lambda r: confidence_rank(r.confidence),
        reverse=True
    )
    
    primary = sorted_results[0]
    conflicts = detect_conflicts(sorted_results)
    
    return GroundingResult(
        queries=[r.query for r in results],
        primary_answer=primary.answer,
        overall_confidence=primary.confidence,
        conflicts=conflicts,
        routing={
            "sources_tried": [r.source for r in results],
            "sources_succeeded": [r.source for r in results if r.success],
            "fallback_used": len(results) > 1
        }
    )

def confidence_rank(confidence: str) -> int:
    return {"high": 3, "medium": 2, "low": 1, "none": 0}.get(confidence, 0)
```

---

## Conflict Detection

```python
def detect_conflicts(results: list[SourceResult]) -> list[Conflict]:
    """Detect when sources disagree."""
    
    conflicts = []
    answers = [(r.source, r.answer) for r in results if r.answer]
    
    for i, (src1, ans1) in enumerate(answers):
        for src2, ans2 in answers[i+1:]:
            if not answers_agree(ans1, ans2):
                conflicts.append(Conflict(
                    source1=src1,
                    source2=src2,
                    answer1=ans1,
                    answer2=ans2,
                    recommendation="Review before DELIVER"
                ))
    
    return conflicts
```

---

## Fallback Behavior

| Scenario | Behavior |
|----------|----------|
| Primary times out | Try next source in chain |
| Primary fails | Try next source in chain |
| All sources timeout | Return partial results + warning |
| All sources fail | Return blocking result |
| Network unavailable | Fall back to repo-only |

---

## Related

- [tiers.md](tiers.md) - Tier definitions and enforcement
- [cache.md](cache.md) - Query caching
- [sanitization.md](sanitization.md) - Query sanitization before external calls
