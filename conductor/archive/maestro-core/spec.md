# Spec: maestro-core

## Summary

Create a central orchestration skill (`maestro-core`) that defines skill loading hierarchy, HALT/DEGRADE fallback policies, and trigger routing rules for the Maestro plugin ecosystem.

## Background

The Maestro plugin has grown to 6 skills with overlapping triggers and inconsistent error handling. When multiple skills could match a user request, there's no clear priority. Some failures HALT execution while others DEGRADE gracefully, but the rules are scattered across different skills.

## Requirements

### Functional Requirements

#### FR-1: Skill Hierarchy
- Define 5-level priority for skill loading:
  1. maestro-core (routing decisions)
  2. conductor (track orchestration)
  3. design (design sessions)
  4. beads (issue tracking)
  5. specialized (worktrees, sharing, writing)

#### FR-2: Fallback Policy
- HALT only for core dependencies that block ALL functionality
- DEGRADE for optional features with warning messages
- Standardize message formats:
  - HALT: `❌ Cannot proceed: [reason]. [fix instruction]`
  - DEGRADE: `⚠️ [feature] unavailable. [fallback behavior].`

#### FR-3: Trigger Routing
- Define disambiguation rules for overlapping triggers
- Context-aware routing (e.g., "track this work" routes differently based on conductor/ presence)
- Document Beads vs TodoWrite decision rules
- Document Worktree invocation points

#### FR-4: Prerequisites Pattern
- Establish `REQUIRED SUB-SKILL:` pattern for declaring dependencies
- All existing skills reference maestro-core as prerequisite
- Document pattern in writing-skills

#### FR-5: Design Standalone Mode
- Design skill must work without conductor/ directory
- Show warning, not HALT
- Skip CODEMAPS and product context gracefully

### Non-Functional Requirements

#### NFR-1: Token Budget
- maestro-core SKILL.md ≤100 lines
- Heavy content in references/

#### NFR-2: Compatibility
- Follow existing Maestro conventions
- No changes to skill loading infrastructure

## Acceptance Criteria

| ID | Criterion | Verification |
|----|-----------|--------------|
| AC-1 | `skills/maestro-core/SKILL.md` exists | `ls skills/maestro-core/SKILL.md` |
| AC-2 | SKILL.md ≤100 lines | `wc -l skills/maestro-core/SKILL.md` ≤ 100 |
| AC-3 | hierarchy.md has 5-level table | Manual inspection |
| AC-4 | routing.md has trigger table | Manual inspection |
| AC-5 | `ds` without conductor/ shows warning | Run `ds` in empty dir |
| AC-6 | 5 skills have Prerequisites | Grep for `maestro-core` |
| AC-7 | writing-skills has dependency docs | Manual inspection |

## Out of Scope

- JSON schema contracts (deferred)
- Validation script (deferred)
- Skill loader modifications
- CODEMAPS regeneration (happens via /conductor-finish)

## Dependencies

None - all work is internal to this repository.

## Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Skills ignore Prerequisites | Medium | Medium | Clear documentation, convention enforcement |
| Hierarchy edge cases | Low | Medium | routing.md covers common cases |
| Message format drift | Low | Low | Standardized in hierarchy.md |
