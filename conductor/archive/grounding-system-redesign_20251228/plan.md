# Grounding System Redesign - Implementation Plan

## Overview

Implement tiered grounding system with enforcement, cascading router, impact scan, and supporting infrastructure.

**Delivery Strategy:** 2 PRs (Foundation + Enforcement)

---

## PR1: Foundation

### Epic 1: Grounding Infrastructure

#### Phase 1.1: Folder Structure & Schema

- [ ] **1.1.1** Create `skills/design/references/grounding/` directory
- [ ] **1.1.2** Create `grounding/tiers.md` - Define Light/Mini/Standard/Full tiers
  - Document each tier: sources, timeout, enforcement level
  - Include decision matrix table
- [ ] **1.1.3** Create `grounding/router.md` - Cascading router logic
  - Document route_grounding() algorithm
  - Document execute_cascade() with fallback
  - Include external reference detection rules
- [ ] **1.1.4** Create `grounding/cache.md` - Session cache specification
  - TTL: 5 minutes
  - Hash algorithm for cache keys
  - Cache invalidation rules
- [ ] **1.1.5** Create `grounding/sanitization.md` - Query sanitization rules
  - Regex patterns for secrets
  - Logging requirements (GR-005)

#### Phase 1.2: Core Grounding Rewrite

- [ ] **1.2.1** Rewrite `grounding.md` - Integrate tiered system
  - depends: 1.1.2, 1.1.3, 1.1.4, 1.1.5
  - Replace flat decision table with tiered routing
  - Add enforcement section
  - Add output format with conflict visibility
  - Add error catalog (GR-001 to GR-005)
- [ ] **1.2.2** Create `grounding/schema.json` - Result schema v1.1
  - Include routing section (sources_tried, sources_succeeded, fallback_used)
  - Include queries array with confidence
  - Include conflicts array

#### Phase 1.3: Verification (PR1)

- [ ] **1.3.1** Review all new grounding/ files for consistency
- [ ] **1.3.2** Verify cross-references between files
- [ ] **1.3.3** Create PR1: Foundation

---

## PR2: Enforcement

### Epic 2: Enforcement & Integration

#### Phase 2.1: Impact Scan

- [ ] **2.1.1** Create `grounding/impact-scan-prompt.md` - Subagent template
  - Input: design summary, grounding results
  - Output: files affected, change types, dependencies, risk, order
  - Constraints: 30s timeout, 100 files max, 4000 tokens

#### Phase 2.2: Design Skill Integration

- [ ] **2.2.1** Update `design/SKILL.md` - Add tiered grounding to phase transitions
  - depends: 1.2.1
  - DISCOVER→DEFINE: Mini grounding (repo)
  - DEFINE→DEVELOP: Mini grounding (web verify)
  - DEVELOP→DELIVER: Standard grounding (gatekeeper)
  - DELIVER→Complete: Full grounding + Impact scan (mandatory)
- [ ] **2.2.2** Update `design/SKILL.md` - Add enforcement section
  - Document enforcement levels and actions
  - Document blocking behavior

#### Phase 2.3: Conductor Integration

- [ ] **2.3.1** Update `conductor/references/commands/design.toml`
  - depends: 2.2.1
  - Add enforcement checkpoints to DELIVER phase
  - Update FULL GROUNDING section with new requirements
  - Add grounding checklist with blocking
- [ ] **2.3.2** Update `conductor/SKILL.md` - Add grounding references
  - Link to new grounding documentation
  - Document tiered system overview
- [ ] **2.3.3** Update `conductor-design-workflow.md` - Final integration
  - depends: 2.3.1, 2.3.2
  - Update workflow to include grounding at transitions

#### Phase 2.4: Resilience

- [ ] **2.4.1** Add timeout handling to `grounding.md`
  - Soft limits with warning
  - Partial result return
- [ ] **2.4.2** Add fallback behavior documentation
  - Network failure → repo-only
  - Subagent timeout → grounding-only
  - All-fail → mandatory block

#### Phase 2.5: Verification (PR2)

- [ ] **2.5.1** Test phase transition triggers grounding
- [ ] **2.5.2** Test DELIVER blocks without grounding
- [ ] **2.5.3** Test conflict visibility output
- [ ] **2.5.4** Create PR2: Enforcement

---

## Epic 3: Documentation

#### Phase 3.1: User Documentation

- [ ] **3.1.1** Create `docs/grounding-user-guide.md`
  - How tiered grounding works
  - What to expect at each phase
  - How to interpret grounding output
- [ ] **3.1.2** Create `docs/grounding-migration.md`
  - Changes from old system
  - New behavior overview
  - FAQ
- [ ] **3.1.3** Create `grounding/troubleshooting.md`
  - Error code explanations (GR-001 to GR-005)
  - Common issues and fixes
  - Resilience scenarios

---

## Task Summary

| Epic | Phase | Tasks | Priority |
|------|-------|-------|----------|
| 1 | 1.1 | 5 | P0 |
| 1 | 1.2 | 2 | P0 |
| 1 | 1.3 | 3 | P0 |
| 2 | 2.1 | 1 | P0 |
| 2 | 2.2 | 2 | P0 |
| 2 | 2.3 | 3 | P0 |
| 2 | 2.4 | 2 | P1 |
| 2 | 2.5 | 4 | P0 |
| 3 | 3.1 | 3 | P2 |

**Total: 25 tasks**

---

## Dependencies Graph

```
1.1.2 ──┐
1.1.3 ──┼──▶ 1.2.1 ──▶ 2.2.1 ──▶ 2.3.1 ──▶ 2.3.3
1.1.4 ──┤                              │
1.1.5 ──┘                              ▼
                                     2.3.2
```

---

## Verification Checklist

- [ ] All grounding/ files created and consistent
- [ ] Schema v1.1 matches design
- [ ] SKILL.md has all integration points
- [ ] design.toml has enforcement checkpoints
- [ ] Cross-references validated
- [ ] PR1 merged before PR2 started
