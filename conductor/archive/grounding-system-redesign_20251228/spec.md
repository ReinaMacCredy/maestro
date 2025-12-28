# Grounding System Redesign - Specification

## Overview

Redesign the grounding system for the Design skill to be proactive, tiered, and enforced rather than optional and reactive.

### Background

Current grounding is:
- **Optional**: Agent can skip without consequence
- **Reactive**: Only triggered manually via `/ground`
- **Single-source**: No cascading or fallback logic
- **Late detection**: Issues found at DELIVER phase when effort already spent

### Goals

1. Make grounding automatic at phase transitions
2. Implement tiered grounding (Light/Standard/Full) based on mode
3. Add enforcement mechanism with advisory/gatekeeper/mandatory levels
4. Create impact scan subagent for DELIVER phase
5. Add caching, sanitization, and conflict visibility

---

## Functional Requirements

### FR-1: Tiered Grounding System

| Mode | Phase Transition | Grounding Type | Enforcement |
|------|------------------|----------------|-------------|
| SPEED | Any | Light (1 source, 3s max) | Advisory |
| FULL | DISCOVER→DEFINE | Mini (repo check) | Advisory |
| FULL | DEFINE→DEVELOP | Mini (web verify) | Advisory |
| FULL | DEVELOP→DELIVER | Standard (cascade) | Gatekeeper |
| FULL | DELIVER→Complete | Full + Impact Scan | Mandatory |

### FR-2: Cascading Router

- Route questions to appropriate source based on type
- Priority chain: repo → web → history
- Fallback when primary source fails or times out
- Stop early when confidence is "high"

### FR-3: Enforcement Mechanism

- **Advisory**: Log skip, allow proceed
- **Gatekeeper**: Block if grounding not run
- **Mandatory**: Block if grounding fails or confidence too low

### FR-4: Impact Scan Subagent

- Run in parallel with full grounding at DELIVER→Complete
- Scan codebase to identify affected files
- Return: file list, change type, dependencies, risk, suggested order
- Merge with grounding result before enforcement

### FR-5: Session Cache

- Cache grounding results for 5 minutes (TTL)
- Hash query to create cache key
- Prevent duplicate queries in same session

### FR-6: Query Sanitization

- Remove secrets (API keys, passwords, tokens) before external queries
- Log when sanitization occurs (GR-005)

### FR-7: Conflict Visibility

- When multiple sources disagree, use highest confidence
- Display conflict summary in output
- Recommend review before DELIVER

---

## Non-Functional Requirements

### NFR-1: Performance

- Light grounding: ≤5s
- Standard grounding: ≤10s
- Full grounding + impact scan: ≤45s

### NFR-2: Reliability

- Soft timeout limits (warn + continue)
- Graceful fallback on source failure
- Repo-only mode when network unavailable

### NFR-3: Security

- Sanitize queries before external calls
- No secrets in grounding results
- Track-level isolation for grounding data

---

## Acceptance Criteria

### AC-1: Phase Transitions Trigger Grounding
- **Given** a FULL mode design session
- **When** transitioning between phases
- **Then** appropriate grounding type runs automatically

### AC-2: DELIVER Blocks Without Grounding
- **Given** FULL mode at DEVELOP→DELIVER transition
- **When** grounding is not run
- **Then** phase transition is blocked with action "RUN_GROUNDING"

### AC-3: Router Selects Correct Source
- **Given** 20 test questions with known expected sources
- **When** routing each question
- **Then** ≥90% are routed to correct primary source

### AC-4: Impact Scan Returns File List
- **Given** a known design
- **When** impact scan runs
- **Then** returns ≥95% of files that will actually be changed

### AC-5: Cache Prevents Duplicate Queries
- **Given** same query asked twice within 5 minutes
- **When** second query executes
- **Then** only 1 API call is made (cache hit)

### AC-6: Sanitization Removes Secrets
- **Given** a query containing "API_KEY=abc123"
- **When** sanitization runs
- **Then** query becomes "API_KEY=[REDACTED]"

### AC-7: Conflict Visibility Works
- **Given** repo and web return different answers
- **When** results are merged
- **Then** output shows conflict summary with recommendation

### AC-8: Resilience on Timeout
- **Given** grounding source times out
- **When** cascade continues
- **Then** partial results returned with warning

### AC-9: Resilience on All-Fail
- **Given** all sources fail
- **When** enforcement checks
- **Then** mandatory level blocks with "MANUAL_VERIFY"

---

## Out of Scope

- Real-time grounding during conversation
- External MCP integration
- Custom grounding plugins
- Keyword triggers (Phase 2)
- Confidence scoring (Phase 2)
- Intent classification layer (Phase 2)
- Skip syntax + audit trail (Phase 2)

---

## Dependencies

- Existing `grounding.md` file (will be rewritten)
- `design/SKILL.md` (will be modified)
- `design.toml` (will be modified)
- `conductor/SKILL.md` (will be modified)
- `finder` tool availability (fallback to Grep)
- `web_search` rate limits (caching mitigates)

---

## Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Breaking existing sessions | Medium | High | Feature flag / gradual rollout |
| Performance regression | Low | Medium | Benchmark before/after |
| Subagent timeout | Medium | Medium | Chunked scan + early return |
