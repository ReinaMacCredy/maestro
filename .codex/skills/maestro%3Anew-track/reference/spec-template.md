# Specification Template

## Writing Guidance

A spec answers "what" and "why" -- never "how." If you catch yourself describing implementation, stop and rewrite as a requirement. Every section below has a purpose; do not leave sections blank or write "TBD."

---

## Structure

```markdown
# Specification: {title}

## Overview
{One paragraph: what this track delivers, why it matters, and what success looks like.
Good: "Add email verification to the signup flow so unverified accounts cannot access paid features. Success: no unverified user reaches the dashboard."
Bad: "Add email verification." (no why, no success criteria)}

## Type
{feature | bug | chore}

## Requirements

### Functional Requirements
1. {FR-1}: {description -- verb phrase, testable}
2. {FR-2}: {description}
3. {FR-3}: {description}

{Each requirement must be independently testable. If you can't write a test for it, it's not a requirement -- it's a wish.}

### User Interaction
- Interaction type: {UI | API | CLI | Background}
- Entry point: {where the user triggers this}
- Output: {what the user sees/receives}
- Error feedback: {how failures are communicated to the user}

### Non-Functional Requirements
- Performance: {latency/throughput expectations, or "standard"}
- Security: {auth requirements, data sensitivity, or "standard"}
- Compatibility: {browser/OS/version requirements, or "N/A"}

## Edge Cases & Error Handling
1. {Edge case 1}: {expected behavior}
2. {Edge case 2}: {expected behavior}
3. {Error scenario}: {recovery strategy}

{Minimum 3 items. If you wrote "none" -- you didn't think hard enough. Every feature has edge cases: empty input, concurrent access, network failure, permission denied, rate limits.}

## Out of Scope
- {Thing 1 this track explicitly does NOT cover}
- {Thing 2}

{Critical section. Prevents scope creep during implementation. If a reviewer asks "what about X?" and X is out of scope, this section is the answer.}

## Acceptance Criteria
- [ ] {Criterion 1 -- testable, specific, binary pass/fail}
- [ ] {Criterion 2}
- [ ] {Criterion 3}

{Each criterion must be verifiable by running a command, clicking a button, or observing a measurable outcome. "Works correctly" is not a criterion. "Returns 200 with JSON body containing `verified: true`" is.}
```

---

## Type-Specific Variations

### Bug Specs

Bug specs replace `Functional Requirements` and `User Interaction` with:

```markdown
### Reproduction
- Steps to reproduce: {numbered steps}
- Expected behavior: {what should happen}
- Actual behavior: {what happens instead}
- Frequency: {always | intermittent | environment-specific}
- Affected versions/environments: {list}

### Root Cause (if known)
{Description of the underlying issue, or "To be determined during implementation."}

### Fix Requirements
1. {Fix-1}: {what the fix must achieve}
2. {Fix-2}: {additional fix requirements}

### Regression Prevention
- {How to prevent this class of bug from recurring}
- {Test coverage gap this exposed}
```

### Chore Specs

Chore specs replace `User Interaction` with:

```markdown
### Scope of Change
- Files/modules affected: {list or pattern}
- Public API changes: {none | additions | breaking}
- Migration required: {yes/no, describe if yes}

### Backward Compatibility
- Compatibility stance: {fully compatible | breaking with migration | internal only}
- Deprecation period: {N/A | version range}
```

---

## Quality Checklist

Before presenting the spec for approval, verify:

- [ ] Overview states the "why" -- not just the "what"
- [ ] Every functional requirement is independently testable
- [ ] Edge cases include at least: empty/null input, concurrent access, error recovery
- [ ] Out of Scope has at least 2 items (forces you to think about boundaries)
- [ ] Acceptance criteria are binary pass/fail (no "should be fast" -- use numbers)
- [ ] No implementation details leaked into requirements
- [ ] Bug specs include reproduction steps that someone else can follow
- [ ] Chore specs state the backward compatibility stance

---

## Examples: Good vs. Mediocre

### Feature Spec -- Good

```markdown
# Specification: API Rate Limiting

## Overview
Add per-user rate limiting to the REST API to prevent abuse and ensure fair usage.
Success: no single user can degrade service for others; offenders receive clear 429 responses.

## Requirements

### Functional Requirements
1. FR-1: Enforce a configurable per-user request limit (default: 100 req/min)
2. FR-2: Return HTTP 429 with `Retry-After` header when limit exceeded
3. FR-3: Track limits by authenticated user ID; unauthenticated requests share a global pool
4. FR-4: Expose current usage via `X-RateLimit-Remaining` response header

### User Interaction
- Interaction type: API
- Entry point: Every authenticated API endpoint
- Output: Normal responses when under limit; 429 with retry guidance when over
- Error feedback: JSON body with `error: "rate_limit_exceeded"` and `retry_after_seconds`

## Edge Cases & Error Handling
1. Clock skew between servers: Use sliding window, not fixed window
2. Rate limit store unavailable: Fail open (allow request) and log warning
3. User hits limit mid-batch: Return 429 for the exceeding request; prior requests succeed
4. Token refresh during rate window: Limit follows user ID, not token

## Out of Scope
- IP-based rate limiting (separate track)
- Rate limit dashboard UI
- Per-endpoint different limits (v2 feature)

## Acceptance Criteria
- [ ] User exceeding 100 req/min receives 429 within 1 second of limit breach
- [ ] `X-RateLimit-Remaining` header is accurate to within 1 request
- [ ] Rate limit store failure does not block requests
- [ ] Limits reset correctly at window boundary
```

### Feature Spec -- Mediocre (do not produce specs like this)

```markdown
# Specification: API Rate Limiting

## Overview
Add rate limiting to the API.

## Requirements
1. Rate limit users
2. Return errors when limit hit
3. Use Redis for storage

## Edge Cases
None identified.

## Acceptance Criteria
- [ ] Rate limiting works
- [ ] Tests pass
```

**What's wrong:** No "why." Requirement 3 is an implementation detail (Redis). No edge cases means no thought was given. "Works" and "tests pass" are not testable criteria.

### Bug Spec -- Good

```markdown
# Specification: Fix Login Timeout on Slow Connections

## Overview
Users on connections >500ms RTT experience a timeout error during login because the
auth token exchange has a 2-second hard timeout. Fix: make the timeout configurable
with a higher default.

## Reproduction
1. Connect through a network proxy adding 600ms latency
2. Navigate to /login
3. Enter valid credentials and submit
4. Observe: "Connection timed out" error after 2 seconds
- Expected: Login succeeds (takes ~3 seconds on slow connections)
- Frequency: Always, on connections >500ms RTT

## Fix Requirements
1. FR-1: Increase default auth timeout from 2s to 10s
2. FR-2: Make timeout configurable via AUTH_TIMEOUT_MS environment variable
3. FR-3: Show "Connecting..." spinner instead of hanging silently

## Regression Prevention
- Add integration test with simulated 1000ms latency
- Add timeout value to health check output for monitoring

## Acceptance Criteria
- [ ] Login succeeds on 600ms RTT connections within 10 seconds
- [ ] AUTH_TIMEOUT_MS=5000 caps timeout at 5 seconds
- [ ] Spinner appears within 500ms of form submission
```
