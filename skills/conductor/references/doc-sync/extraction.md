# Thread Extraction Pipeline

Extract topics and knowledge from Amp threads for documentation synchronization.

## Pipeline Overview

```
REQUEST → THREADS → TOPICS → CODE
```

| Phase | Action | Tools |
|-------|--------|-------|
| 1. Discover | Find threads by query/time | `find_thread` |
| 2. Extract | Parallel topic extraction | `Task` + `read_thread` |
| 3. Verify | Ground topics in code | `gkg`, `finder` |

## Phase 1: Discover Threads

Start from user request and find relevant threads:

### Query Patterns

```bash
# Time-based: "Document last 2 weeks"
find_thread after:2w

# Topic-based: "Summarize auth work"  
find_thread "authentication"

# File-based: "What touched the SDK?"
find_thread file:packages/sdk

# Combined filters
find_thread "refactor" after:1w file:packages/api
```

### Query Examples

| User Request | Query |
|--------------|-------|
| "Document recent changes" | `find_thread after:7d` |
| "Summarize auth refactor" | `find_thread "auth" after:14d` |
| "What touched the API?" | `find_thread file:src/api` |
| "Epic 0abc work" | `find_thread "epic-0abc"` |

### Output

Returns list of thread IDs: `[T-abc, T-def, T-ghi]`

## Phase 2: Extract Topics

Spawn parallel `Task` agents (2-3 threads each) for efficient extraction.

### Task Agent Prompt Template

```
Task prompt:
"Read threads [T-xxx, T-yyy] using read_thread.
Goal: 'Extract topics, decisions, changes'

Return JSON:
{
  'topics': [{
    'name': 'topic name',
    'threads': ['T-xxx'],
    'summary': '1-2 sentences',
    'decisions': ['...'],
    'patterns': ['...'],
    'changes': ['...']
  }]
}"
```

### Parallelization Strategy

| Thread Count | Task Agents | Threads per Agent |
|--------------|-------------|-------------------|
| 1-3 | 1 | All |
| 4-6 | 2 | 2-3 each |
| 7-9 | 3 | 2-3 each |
| 10+ | 4-5 | Even split |

### Oracle Synthesis

Collect outputs from all Task agents → Oracle synthesizes:

```
Oracle prompt:
"Cluster these extractions. Deduplicate. 
Latest thread wins conflicts. Output unified topic list."
```

### Output Schema

```json
{
  "topics": [
    {
      "name": "JWT Migration",
      "threads": ["T-abc", "T-def"],
      "summary": "Migrated from session tokens to JWT for API auth",
      "decisions": [
        "Use RS256 for token signing",
        "15-minute access token expiry"
      ],
      "patterns": [
        "Middleware validates tokens before route handlers"
      ],
      "changes": [
        "packages/auth/jwt.ts - New JWTService class",
        "packages/api/middleware.ts - Token validation"
      ]
    }
  ]
}
```

## Phase 3: Verify Against Code

For each extracted topic, verify claims against actual codebase state.

### Verification Flow

```
Topic: "Added retry logic to API client"

1. finder "retry logic API client"
   → finds src/api/client.ts

2. gkg__search_codebase_definitions "retry"
   → RetryPolicy class at L45

3. gkg__get_references "RetryPolicy"
   → 12 usages across 4 files

→ Confirmed: topic matches code
```

### Verification by Claim Type

| Claim Type | Primary Tool | Secondary Tool |
|------------|--------------|----------------|
| "Added X" | `gkg__search_codebase_definitions "X"` | `finder "X"` |
| "Refactored Y" | `finder "Y"` | `gkg__get_references` |
| "Changed pattern" | `warpgrep "pattern"` | `finder "pattern implementation"` |
| "Updated config" | `gkg__repo_map` on config paths | `Read` config file |

### GKG Tool Reference

| Tool | Purpose | Example |
|------|---------|---------|
| `gkg__search_codebase_definitions` | Find where symbol is defined | `"JWTService"` |
| `gkg__get_references` | Find all usages of symbol | `"RetryPolicy"` |
| `gkg__repo_map` | Get structure overview | `"packages/auth"` |
| `finder` | Semantic code search | `"retry logic implementation"` |
| `warpgrep` | Pattern-based search | `"authentication middleware"` |

### Verification Results

Mark each topic claim:

| Status | Meaning | Action |
|--------|---------|--------|
| ✅ Confirmed | Code matches claim | Proceed to doc update |
| ⚠️ Partial | Some aspects match | Note discrepancy |
| ❌ Not Found | Claim not in code | Mark as "planned" or "removed" |
| 🔄 Changed | Code differs from claim | Use code as truth |

### Output

Verified topics with code citations:

```json
{
  "topic": "JWT Migration",
  "verified": true,
  "citations": [
    {
      "claim": "JWTService class added",
      "file": "packages/auth/jwt.ts",
      "line": 45,
      "status": "confirmed"
    },
    {
      "claim": "Middleware validates tokens",
      "file": "packages/api/middleware.ts", 
      "line": 23,
      "status": "confirmed"
    }
  ]
}
```

## Troubleshooting

| Issue | Solution |
|-------|----------|
| Thread not found | Try topic keywords, widen date range |
| Too many threads | Add `file:` filter, narrow dates |
| Topic ≠ code | Code is truth; note as "planned" or "historical" |
| GKG returns nothing | Fall back to `finder` semantic search |

---

*See [SKILL.md](../../SKILL.md) for full doc-sync workflow.*
