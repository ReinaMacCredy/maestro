# Skill Integration — Technical Specification

## Overview

Consolidate 15 scattered skills into 6 tightly-integrated skills by merging 9 skills into conductor.

## Requirements

### Functional Requirements

#### FR-1: Skill Consolidation
- **FR-1.1:** Reduce skill count from 15 to 6
- **FR-1.2:** Merge 9 skills into conductor/references/
- **FR-1.3:** Delete 9 skill directories after merge
- **FR-1.4:** Move using-superpowers discipline rules to AGENTS.md

#### FR-2: Conductor Structure
- **FR-2.1:** conductor/SKILL.md must be overview only (~100 lines)
- **FR-2.2:** Create 9 reference subdirectories:
  - planning/, execution/, prompts/, coordination/
  - tdd/, verification/, doc-sync/, ledger/, finish/
- **FR-2.3:** All detailed content lives in references/

#### FR-3: LEDGER System
- **FR-3.1:** Replace LEDGER.md (YAML) with LEDGER.log (append-only)
- **FR-3.2:** Log format: `TIMESTAMP | EVENT | DATA`
- **FR-3.3:** Max 1000 entries, auto-rotate to .log.1
- **FR-3.4:** Recovery: parse log to find completed issues

#### FR-4: Reference Updates
- **FR-4.1:** Update writing-skills to reference conductor/references/tdd/
- **FR-4.2:** Ensure no broken references after migration

### Non-Functional Requirements

#### NFR-1: Compatibility
- Existing beads CLI must work unchanged
- Existing design skill must work unchanged

#### NFR-2: Git History
- Use `git mv` for file moves to preserve history
- Single PR with ordered commits for atomic merge

## File Mappings

### Source → Destination

| Source Skill | Source Files | Destination |
|--------------|--------------|-------------|
| subagent-driven-development | implementer-prompt.md | conductor/references/prompts/ |
| | spec-reviewer-prompt.md | conductor/references/prompts/ |
| | code-quality-reviewer-prompt.md | conductor/references/prompts/ |
| dispatching-parallel-agents | references/agent-coordination/* | conductor/references/coordination/ |
| continuity | references/ledger-format.md | conductor/references/ledger/ |
| | references/handoff-format.md | conductor/references/ledger/ |
| | references/amp-setup.md | conductor/references/ledger/ |
| doc-sync | references/* | conductor/references/doc-sync/ |
| test-driven-development | SKILL.md (extract) | conductor/references/tdd/cycle.md |
| verification-before-completion | SKILL.md (extract) | conductor/references/verification/gate.md |
| finishing-a-development-branch | SKILL.md (extract) | conductor/references/finish/branch-options.md |
| using-superpowers | SKILL.md (discipline) | AGENTS.md (append) |

### Skills to Delete

```
skills/create-plan/
skills/dispatching-parallel-agents/
skills/subagent-driven-development/
skills/test-driven-development/
skills/verification-before-completion/
skills/doc-sync/
skills/continuity/
skills/finishing-a-development-branch/
skills/using-superpowers/
```

## LEDGER.log Specification

### Format

```
# LEDGER.log - Append-only session log
# Format: ISO_TIMESTAMP | EVENT_TYPE | DATA
# Max 1000 entries, rotates to .log.1

2025-12-28T10:00:00Z | SESSION_START | track:skill-integration
2025-12-28T10:01:00Z | CLAIMED | issue:bd-42
2025-12-28T10:02:00Z | TDD_PHASE | RED | issue:bd-42
2025-12-28T10:05:00Z | TDD_PHASE | GREEN | issue:bd-42
2025-12-28T10:07:00Z | TDD_PHASE | REFACTOR | issue:bd-42
2025-12-28T10:10:00Z | PRE_VERIFY | PASS | issue:bd-42
2025-12-28T10:11:00Z | COMPLETED | issue:bd-42
2025-12-28T10:15:00Z | TRACK_COMPLETE | track:skill-integration
```

### Event Types

| Event | Data | Description |
|-------|------|-------------|
| SESSION_START | track:id | New session begins |
| CLAIMED | issue:id | Task claimed |
| TDD_PHASE | RED/GREEN/REFACTOR, issue:id | TDD state change |
| PRE_VERIFY | PASS/FAIL, issue:id | Pre-verification result |
| COMPLETED | issue:id | Task completed |
| RESERVED | files:[list] | Parallel mode file reservation |
| CONFLICT | files:[list] | Merge conflict detected |
| TRACK_COMPLETE | track:id | Track finished |

### Rotation Logic

```
if (lineCount(LEDGER.log) > 1000):
    mv LEDGER.log.1 LEDGER.log.2  # if exists
    mv LEDGER.log LEDGER.log.1
    touch LEDGER.log
```

### Recovery Logic

```
# On session resume:
completed = grep "COMPLETED" LEDGER.log | extract issue:id
for issue in epic.issues:
    if issue.id in completed:
        skip
    else:
        claim and work
```

## Acceptance Criteria

| ID | Criteria | Verification |
|----|----------|--------------|
| AC-1 | `ls skills/ \| wc -l` returns 6 | Command output |
| AC-2 | `ls skills/conductor/references/` shows 9 dirs | Command output |
| AC-3 | LEDGER.log write/read/parse works | Manual test |
| AC-4 | Log rotation triggers at 1001 entries | Automated test |
| AC-5 | Session recovery skips completed issues | Manual test |
| AC-6 | writing-skills references work | Grep verification |
| AC-7 | No broken skill references | Full grep scan |
| AC-8 | AGENTS.md contains discipline rules | Content check |

## Dependencies

- Git (for `git mv`)
- Existing beads CLI
- Existing conductor structure

## Out of Scope

- Rewriting beads CLI
- Changing design skill workflow
- Multi-repo support
- New conductor commands
- UI/dashboard
