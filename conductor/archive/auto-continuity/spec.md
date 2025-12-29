# Spec: Auto-Continuity for Hookless Agents

## Overview

Make session continuity automatic for hookless agents (Amp, Gemini CLI, Codex) by embedding ledger operations into Conductor workflow entry points.

## Problem

Users on hookless agents must manually run `continuity load`, `continuity save`, and `continuity handoff` commands. These are easily forgotten, causing context loss across sessions.

## Solution

Remove manual commands. Ledger operations trigger automatically at Conductor workflow entry points:

| Entry Point | Ledger Action |
|-------------|---------------|
| `ds` | Load prior context |
| `/conductor-implement` | Load + bind track/bead |
| `/conductor-finish` | Handoff + archive |

## Scope

### In Scope

- Update documentation to reflect automatic behavior
- Update maestro-core with session lifecycle documentation
- Deprecate manual continuity instructions
- Update GLOBAL_CONFIG.md with unified "Session Lifecycle" section

### Out of Scope

- Changes to Conductor workflow logic (already implemented)
- Claude Code hooks (already automatic)
- New skill creation

## Requirements

### R1: Documentation Updates

| ID | Requirement |
|----|-------------|
| R1.1 | `docs/GLOBAL_CONFIG.md` MUST have "Session Lifecycle (All Agents)" section |
| R1.2 | `docs/GLOBAL_CONFIG.md` MUST NOT have "Amp-Specific Continuity" manual commands |
| R1.3 | `AGENTS.md` MUST NOT contain manual `continuity load/save/handoff` instructions |
| R1.4 | `conductor/AGENTS.md` MUST NOT contain gotcha about manual commands |

### R2: Skill Updates

| ID | Requirement |
|----|-------------|
| R2.1 | `skills/maestro-core/SKILL.md` MUST have "Session Lifecycle" section |
| R2.2 | `skills/maestro-core/references/hierarchy.md` MUST document ledger check in loading order |
| R2.3 | `skills/conductor/references/ledger/amp-setup.md` MUST have deprecation notice |
| R2.4 | `skills/conductor/references/workflows/setup.md` MUST NOT prompt for manual protocol |

### R3: Behavior

| ID | Requirement |
|----|-------------|
| R3.1 | `ds` MUST load LEDGER.md if exists and <24h old |
| R3.2 | `/conductor-implement` MUST load and bind to track/bead |
| R3.3 | `/conductor-finish` MUST archive LEDGER.md |
| R3.4 | Non-Conductor workflows MUST skip ledger operations (no overhead) |

## Acceptance Criteria

| # | Criteria | Verification |
|---|----------|--------------|
| AC-1 | No user-facing docs say "run continuity load manually" | `grep -r "continuity load" docs/ AGENTS.md` returns 0 matches for manual instructions |
| AC-2 | maestro-core has Session Lifecycle section | Read SKILL.md, verify section exists |
| AC-3 | GLOBAL_CONFIG.md has unified section | Read file, verify "Session Lifecycle (All Agents)" exists |
| AC-4 | amp-setup.md is deprecated | Read file, verify deprecation notice at top |
| AC-5 | `ds` starts with context | Start design session in project with LEDGER, verify context displayed |
| AC-6 | `/conductor-implement` binds | Run implement, verify LEDGER.md bound_track updated |

## Dependencies

- [x] skill-integration track (merged continuity into conductor)
- [x] maestro-core track (created orchestration skill)
- [ ] None blocking

## Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Agents ignore AGENTS.md | Low | Medium | Document per-agent behavior in GLOBAL_CONFIG |
| Users confused by change | Low | Low | Clear deprecation notices with "why" |

## Effort

**Total: 2.5 hours**

| Task | Hours |
|------|-------|
| Update docs/GLOBAL_CONFIG.md | 0.5 |
| Update maestro-core | 0.5 |
| Update AGENTS.md files | 0.5 |
| Deprecate amp-setup.md | 0.25 |
| Update setup workflow | 0.25 |
| Verification | 0.5 |
