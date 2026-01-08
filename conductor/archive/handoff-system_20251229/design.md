# Design: HumanLayer-Inspired Handoff System

## Overview

Replace the current LEDGER.md/continuity system with a HumanLayer-inspired handoff system that is shareable (git-committed), standalone, and deeply integrated with Conductor workflows.

## Problem Statement

> Current continuity system (LEDGER.md) is gitignored, tightly coupled to Conductor, and lacks structured metadata—making cross-session, cross-agent, and team handoffs unreliable.

### Pain Points Addressed

- ❌ Context loss between sessions (incomplete handoffs)
- ❌ Can't share with teammates (gitignored)
- ❌ Too coupled to Conductor workflow
- ❌ Missing structured metadata (commit, branch, etc.)

### Target Users

- Solo developers (different sessions)
- AI agents (cross-agent handoffs)
- Human teammates (code review, collaboration)

## Design Decisions

### 1. Directory Structure

```
conductor/
├── handoffs/
│   ├── general/                          ← No-track handoffs
│   │   ├── index.md
│   │   └── YYYY-MM-DD_HH-MM-SS-mmm_general_<trigger>.md
│   └── <track-id>/
│       ├── index.md                      ← Auto-generated log
│       ├── YYYY-MM-DD_HH-MM-SS-mmm_<track>_<trigger>.md
│       └── archive/                      ← After /conductor-finish
│           └── (moved files)
├── tracks/<track-id>/
│   └── metadata.json                     ← last_activity, validation state
└── (product.md, tech-stack.md, workflow.md)
```

**Key decisions:**
- Handoffs at `conductor/handoffs/` level (decoupled from track lifecycle)
- `general/` for non-track work
- `archive/` per track for cleanup on finish
- All handoffs committed to git (shareable)

### 2. File Strategy: Hybrid (Index + Separate Files)

**Individual files** for each handoff:
- Clean git diffs
- Easy to delete/archive individual entries
- Unique via millisecond timestamps + collision suffix

**index.md** for consolidated view:
- Auto-generated, append-only
- Quick overview of all handoffs
- Links to individual files
- Sort on read for display

### 3. Commands

| Command | Description |
|---------|-------------|
| `/create_handoff` | Create handoff (manual or auto-triggered) |
| `/resume_handoff` | Find and load handoff with smart discovery |
| `/conductor-handoff` | Alias for both (subcommand style) |

### 4. Triggers (6 Types)

| Trigger | Integration Point | Frequency |
|---------|-------------------|-----------|
| `design-end` | After `/conductor-newtrack` | 1x per track |
| `epic-start` | Before each CI epic in `/conductor-implement` | N per track |
| `epic-end` | After each CI epic closes | N per track |
| `pre-finish` | Start of `/conductor-finish` | 1x per track |
| `manual` | User runs `/create_handoff` | On-demand |
| `idle` | Message-triggered after 30min gap | On-demand |

**Configurable in `conductor/workflow.md`:**
```yaml
handoff:
  quiet: false
  idle_threshold_minutes: 30
  auto_triggers:
    - design-end      # Always on
    - epic-start      # Can disable
    - epic-end        # Can disable
    - pre-finish      # Always on
```

### 5. Template (4 Sections)

```markdown
---
timestamp: 2025-12-29T10:00:00.123+07:00
trigger: design-end | epic-start | epic-end | pre-finish | manual | idle
track_id: auth-system | general
bead_id: E1-user-login                    # if epic trigger
git_commit: abc123f
git_branch: feat/auth-system
author: agent | human
validation_snapshot:
  gates_passed: [design, spec, plan-structure]
  current_gate: plan-execution
---

# Handoff: <track-id> | <trigger>

## Context
{What you were working on, current state, active decisions}

## Changes
{Files modified with line references}
- `path/to/file.ts:10-45` - Added login handler

## Learnings
{Patterns discovered, gotchas, important context}

## Next Steps
{Immediate actions for resuming agent}
1. [ ] First task
2. [ ] Second task
```

### 6. Index.md Format

```markdown
---
track_id: auth-system
created: 2025-12-29T10:00:00+07:00
last_updated: 2025-12-29T18:30:00+07:00
---

# Handoff Log: auth-system

| Timestamp | Trigger | Bead | Summary | File |
|-----------|---------|------|---------|------|
| 10:00:00.123 | design-end | - | RS256 decision | [→](./2025-12-29_10-00-00-123_auth-system_design-end.md) |
| 11:30:00.456 | epic-start | E1 | Starting login | [→](./2025-12-29_11-30-00-456_auth-system_E1_epic-start.md) |
```

### 7. Smart Discovery (`/resume_handoff`)

```
/resume_handoff
├── Only 1 active track? → Auto-resume most recent
├── Multiple tracks? → List recent per track, user picks
└── Explicit path given? → Use that directly
```

**Validation on resume:**
- Check git branch matches
- Check files mentioned still exist
- Flag drift if present
- Warn if handoff is stale (>7 days)

### 8. Idle Detection

**Location:** `maestro-core` skill (universal, works outside Conductor)

**Mechanism:** Session marker file `conductor/.last_activity`
- Touch on every significant action
- Check mtime on next user message
- If gap > 30 minutes → prompt:
  ```
  It's been a while. Create handoff first? [Y/n/skip]
  ```

### 9. Secrets Scanning

**Layered approach:**
1. Hardcoded patterns (default):
   ```regex
   sk-[a-zA-Z0-9]{20,}          # OpenAI
   ghp_[a-zA-Z0-9]{36}          # GitHub PAT
   AKIA[0-9A-Z]{16}             # AWS Access Key
   -----BEGIN.*PRIVATE KEY-----  # Private keys
   ```
2. Configurable patterns in `conductor/workflow.md`
3. Use `gitleaks` if available in PATH

**Behavior:** WARN with pattern found, ask `[P]roceed / [A]bort`

### 10. Parallel Agent Safety

**Filename:** Millisecond timestamps + collision suffix
```
YYYY-MM-DD_HH-MM-SS-mmm_<track>_<trigger>.md
If exists: ..._<trigger>-1.md, ..._<trigger>-2.md
```

**Index:** Atomic append (>>), sort on read

### 11. Archive on Finish

**In `/conductor-finish`:**
1. Find `handoffs/<track-id>/` directory
2. Move all `*.md` files (except `index.md`) to `archive/`
3. Update `index.md`: mark entries as archived
4. Keep `index.md` in place (historical reference)

### 12. Validation Integration

**Validation state moves to `metadata.json`:**
```json
{
  "validation": {
    "gates_passed": ["design", "spec", "plan-structure"],
    "current_gate": "plan-execution",
    "retries": 0,
    "last_failure": null
  }
}
```

**Handoff captures snapshot** in frontmatter for audit trail.

### 13. Continuity Deprecation

**Local stub skill** overrides marketplace plugin:
```yaml
# skills/continuity/SKILL.md
---
name: continuity
description: DEPRECATED - use handoff system
---

This skill is deprecated. Use `/create_handoff` and `/resume_handoff` instead.
```

## Comparison with HumanLayer

| Aspect | HumanLayer | Our Design |
|--------|------------|------------|
| Commands | `/create_handoff`, `/resume_handoff` | Same ✅ |
| Structure | `thoughts/shared/handoffs/ENG-XXXX/` | `conductor/handoffs/<track>/` |
| Sections | 7 | 4 (leaner, beads handles tasks) |
| Git | Committed | Committed ✅ |
| Triggers | Manual only | 6 auto-triggers (deeper integration) |
| Index | None | `index.md` per track (log view) |
| Parallel safety | None | Millisecond timestamps |
| Validation | None | Snapshot in frontmatter |

## Implementation Scope

### CREATE (8 items)

| File | Purpose |
|------|---------|
| `skills/conductor/references/handoff/create.md` | `/create_handoff` workflow |
| `skills/conductor/references/handoff/resume.md` | `/resume_handoff` workflow |
| `skills/conductor/references/handoff/template.md` | 4-section template |
| `skills/conductor/references/handoff/triggers.md` | 6 trigger definitions |
| `skills/conductor/references/handoff/idle-detection.md` | Gap detection logic |
| `skills/continuity/SKILL.md` | Deprecation stub |
| `conductor/handoffs/general/index.md` | Initial general index |
| `docs/handoff-system.md` | User guide |

### DELETE (2 directories)

| Path | Reason |
|------|--------|
| `conductor/sessions/` | Replaced by `handoffs/` |
| `skills/conductor/references/ledger/` | No longer used (6 files) |

### MODIFY (28 files)

**High impact (19 files):**
- `skills/conductor/SKILL.md`
- `skills/maestro-core/SKILL.md`
- `skills/maestro-core/references/hierarchy.md`
- `skills/design/SKILL.md`
- `skills/beads/references/WORKFLOWS.md`
- `skills/conductor/references/beads-integration.md`
- `skills/conductor/references/beads-facade.md`
- `skills/conductor/references/finish-workflow.md`
- `skills/conductor/references/validation/lifecycle.md`
- `skills/conductor/references/validation/quality/judge-prompt.md`
- `skills/conductor/references/validation/shared/*.md` (5 files)
- `skills/conductor/references/tdd/cycle.md`
- `skills/conductor/references/conductor/beads-session.md`
- `skills/conductor/references/conductor/preflight-beads.md`
- `skills/conductor/references/conductor/tdd-checkpoints-beads.md`
- `skills/conductor/references/coordination/patterns/session-lifecycle.md`
- `skills/conductor/references/workflows/setup.md`
- `skills/conductor/references/workflows/implement.md`
- `skills/conductor/references/workflows/newtrack.md`

**Documentation (9 files):**
- `AGENTS.md`
- `conductor/AGENTS.md`
- `conductor/CODEMAPS/overview.md`
- `conductor/CODEMAPS/skills.md`
- `SETUP_GUIDE.md`
- `TUTORIAL.md`
- `docs/PIPELINE_ARCHITECTURE.md`
- `docs/GLOBAL_CONFIG.md`
- `README.md`

**Scripts:**
- `scripts/test-hooks.sh`

## Error Handling

| Scenario | Handling |
|----------|----------|
| No conductor/ dir | `/conductor-setup` creates `handoffs/general/` |
| Handoff file write fails | Rollback: don't update index, show error |
| Index append fails | Auto-repair: scan dir, rebuild on next resume |
| Secrets detected | WARN, ask `[P]roceed / [A]bort` |
| Git not available | Use "unknown" for commit/branch |
| Handoff not found | List available, ask user to pick |
| Stale handoff (>7 days) | WARN: "This handoff is old. Continue?" |
| Branch mismatch | WARN: "Handoff was on X, now on Y. Continue?" |
| Malformed frontmatter | Infer from filename → skip with warning |
| Parallel write collision | Millisecond timestamp + suffix |

## Success Criteria

| # | Criterion | Verification |
|---|-----------|--------------|
| 1 | `/create_handoff` creates file in correct location | Manual test |
| 2 | `/resume_handoff` finds and loads latest handoff | Manual test |
| 3 | Index.md updated on each handoff | Check file after create |
| 4 | Secrets scan warns on patterns | Test with `sk-test123` |
| 5 | Archive on `/conductor-finish` | Check files moved |
| 6 | Idle detection prompts after 30min gap | Manual test |
| 7 | Old `sessions/` dir deleted | `ls conductor/` |
| 8 | Triggers work at all 6 integration points | End-to-end test |

## Risks & Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Breaking existing sessions/ users | Low (unused) | Document in CHANGELOG |
| Marketplace continuity conflicts | Medium | Local stub overrides |
| Idle detection false positives | Low | Configurable threshold |
| Index corruption | Low | Auto-repair on resume |

## References

- [HumanLayer create_handoff.md](https://github.com/humanlayer/humanlayer/blob/main/.claude/commands/create_handoff.md)
- [HumanLayer resume_handoff.md](https://github.com/humanlayer/humanlayer/blob/main/.claude/commands/resume_handoff.md)
