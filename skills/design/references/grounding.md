---
description: Verify patterns against current truth before implementation
argument-hint: <question-or-pattern>
---

# Ground — Tiered Verification Protocol

Verify truth is **in the repo**, **on the web**, or **in prior sessions** before implementation.

---

## Overview

Grounding prevents designs based on outdated or hallucinated information by:
- **Automatic execution** at phase transitions
- **Tiered intensity** based on mode and phase
- **Enforcement levels** from advisory to mandatory
- **Cascading sources** with fallback logic

---

## Tiered System

| Mode | Phase Transition | Tier | Enforcement |
|------|------------------|------|-------------|
| SPEED | Any | Light | Advisory ⚠️ |
| FULL | DISCOVER→DEFINE | Mini | Advisory ⚠️ |
| FULL | DEFINE→DEVELOP | Mini | Advisory ⚠️ |
| FULL | DEVELOP→DELIVER | Standard | Gatekeeper 🚫 |
| FULL | DELIVER→Complete | Full + Impact Scan | Mandatory 🔒 |

See [grounding/tiers.md](grounding/tiers.md) for detailed tier definitions.

---

## Enforcement Levels

### Advisory ⚠️
- Grounding skip is logged
- Warning displayed, proceed allowed
- Use: Early exploration

### Gatekeeper 🚫
- Grounding must be run
- Blocks if skipped
- Low confidence still proceeds with warning
- Use: Pre-delivery validation

### Mandatory 🔒
- Grounding must run AND pass
- Blocks if: not run, all fail, or low confidence
- **No skip allowed**; `MANUAL_VERIFY` requires explicit user confirmation with justification
- Use: Final delivery gate

---

## Source Routing

Priority chain: **repo → web → history**

| Source | Best For | Tools |
|--------|----------|-------|
| repo | Patterns, conventions, existing code | Grep, finder, Read |
| web | APIs, libraries, current documentation | web_search, read_web_page |
| history | Past decisions, context | find_thread, git log |

See [grounding/router.md](grounding/router.md) for cascading logic.

---

## Usage

### Manual Command

```
/ground <question-or-pattern>
```

### Automatic Triggers

Grounding runs automatically at phase transitions in design sessions.

---

## Output Format

```
┌─ GROUNDING RESULT ─────────────────────┐
│ Tier: standard                         │
│ Phase: DEVELOP→DELIVER                 │
│ Duration: 1.2s                         │
├────────────────────────────────────────┤
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

---

## Conflict Handling

When sources disagree:
1. Use highest confidence answer
2. Display conflict summary
3. Recommend review before DELIVER
4. Log conflict for audit

---

## Error Catalog

| Code | Message | Action |
|------|---------|--------|
| GR-001 | All sources failed | Manual verification required |
| GR-002 | Timeout exceeded | Retry or skip (if advisory) |
| GR-003 | Low confidence | Additional verification needed |
| GR-004 | Conflict detected | Review conflict summary |
| GR-005 | Query sanitized | Sensitive content removed |

---

## Performance Limits

All limits are **soft** (warn + continue):

| Tier | Target | Hard Limit |
|------|--------|------------|
| Light | 3s | 5s |
| Mini | 5s | 8s |
| Standard | 10s | 15s |
| Full | 45s | 60s |

---

## Session Cache

- TTL: 5 minutes
- Hash-based key from normalized query
- Prevents duplicate queries in same session
- Invalidates on conflict or low confidence

See [grounding/cache.md](grounding/cache.md) for caching logic.

---

## Query Sanitization

Before external queries:
- Remove secrets (API keys, passwords, tokens)
- Anonymize internal paths
- Log sanitization events (GR-005)

See [grounding/sanitization.md](grounding/sanitization.md) for patterns.

---

## Resilience

| Scenario | Behavior |
|----------|----------|
| Primary times out | Try next source |
| Primary fails | Try next source |
| All timeout | Return partial + warning |
| All fail | Block + manual verify |
| Network unavailable | Repo-only mode |

---

## Examples

### Example 1: Library API (Web Source)

```
/ground how to create Stripe customer with new API
```

```
GROUNDING: Stripe customer creation API
SOURCE: web (stripe.com/docs)
CONFIDENCE: HIGH
PATTERN: stripe.customers.create({ email, metadata })
```

### Example 2: Project Convention (Repo Source)

```
/ground how do we handle errors in this codebase
```

```
GROUNDING: Error handling pattern
SOURCE: repo (src/lib/errors.ts)
CONFIDENCE: HIGH
PATTERN: throw new AppError(code, message, { cause })
```

### Example 3: Prior Decision (History Source)

```
/ground did we decide on auth strategy
```

```
GROUNDING: Authentication strategy decision
SOURCE: history (find_thread)
CONFIDENCE: HIGH
PATTERN: JWT with refresh tokens, 15min access / 7day refresh
```

---

## Track-Level Storage

Grounding results are stored per-track:

```
conductor/tracks/{track-id}/grounding/
├── discover-define.json
├── define-develop.json
├── develop-deliver.json
├── deliver-complete.json
└── impact-scan.md
```

---

## Related Documentation

- [grounding/tiers.md](grounding/tiers.md) - Tier definitions and decision matrix
- [grounding/router.md](grounding/router.md) - Cascading router logic
- [grounding/cache.md](grounding/cache.md) - Session cache specification
- [grounding/sanitization.md](grounding/sanitization.md) - Query sanitization rules
- [grounding/impact-scan-prompt.md](grounding/impact-scan-prompt.md) - Impact scan subagent
