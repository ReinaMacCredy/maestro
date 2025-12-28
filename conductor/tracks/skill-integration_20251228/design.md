# Skill Integration — Design Document

## Problem Statement

> **15 skills operate in isolation**, lacking clear integration points, causing confusion about when to use which skill and losing context between workflow phases.

## Solution

Merge 9 skills into **conductor** as a unified workflow, reducing from 15 to 6 skills.

## Final Architecture

```
skills/
├── beads/                    # KEEP - Issue tracking
├── conductor/                # UNIFIED (9 skills merged)
│   ├── SKILL.md              # Overview (~100 lines)
│   └── references/
│       ├── planning/         # ds, newtrack
│       ├── execution/        # implement, modes
│       ├── prompts/          # ← subagent-driven-development
│       ├── coordination/     # ← dispatching-parallel-agents
│       ├── tdd/              # ← test-driven-development
│       ├── verification/     # ← verification-before-completion
│       ├── doc-sync/         # ← doc-sync
│       ├── ledger/           # ← continuity
│       └── finish/           # ← finishing-a-development-branch
├── design/                   # KEEP - Double Diamond
├── sharing-skills/           # KEEP
├── using-git-worktrees/      # KEEP
└── writing-skills/           # KEEP (update refs)

AGENTS.md                     # ← using-superpowers discipline rules
```

## Workflow Diagram

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    CONDUCTOR - Unified Workflow                          │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  PLANNING                                                                │
│  ┌──────┐    ┌─────────┐    ┌──────────────────┐    ┌─────┐            │
│  │  ds  │───▶│ design  │───▶│ /conductor-      │───▶│ fb  │            │
│  │      │    │  .md    │    │   newtrack       │    │     │            │
│  └──────┘    └─────────┘    └──────────────────┘    └─────┘            │
│      │                              │                    │              │
│      ▼                              ▼                    ▼              │
│  ledger load                  spec + plan            Epic + Issues      │
│                                                                          │
│  EXECUTION (/conductor-implement)                                        │
│  ┌────────────────────────────────────────────────────────────────┐     │
│  │  Resume Check → Auto-Dispatch → Mode Selection                  │     │
│  │       │                              │                          │     │
│  │       ▼                              ▼                          │     │
│  │  ┌─────────────┐              ┌──────────────┐                 │     │
│  │  │ SINGLE MODE │              │ PARALLEL MODE│                 │     │
│  │  │             │              │              │                 │     │
│  │  │ Claim       │              │ Reserve      │                 │     │
│  │  │   ▼         │              │   ▼          │                 │     │
│  │  │ RED-GREEN-  │              │ Dispatch N   │                 │     │
│  │  │ REFACTOR TDD│              │   ▼          │                 │     │
│  │  │   ▼         │              │ RED-GREEN-   │                 │     │
│  │  │ 2-Stage Rev │              │ REFACTOR TDD │                 │     │
│  │  │   ▼         │              │   ▼          │                 │     │
│  │  │ Pre-verify  │              │ 3-Way Merge  │                 │     │
│  │  │   ▼         │              │   ▼          │                 │     │
│  │  │ Close       │              │ Close x N    │                 │     │
│  │  │   ▼         │              │   ▼          │                 │     │
│  │  │ log         │              │ log          │                 │     │
│  │  └─────────────┘              └──────────────┘                 │     │
│  │                        │                                        │     │
│  │                        ▼                                        │     │
│  │                   More issues? ──Yes──▶ Loop                   │     │
│  └────────────────────────────────────────────────────────────────┘     │
│                               │ No                                       │
│                               ▼                                          │
│  FINISH (/conductor-finish)                                              │
│  ┌────────────────────────────────────────────────────────────────┐     │
│  │  Verification Gate (MUST PASS)                                  │     │
│  │         │                                                       │     │
│  │         ▼                                                       │     │
│  │     CODEMAPS                                                    │     │
│  │         │                                                       │     │
│  │         ▼                                                       │     │
│  │     Doc-Sync                                                    │     │
│  │         │                                                       │     │
│  │         ▼                                                       │     │
│  │     Compact beads                                               │     │
│  │         │                                                       │     │
│  │         ▼                                                       │     │
│  │     Branch Options: [Merge] [PR] [Cleanup]                      │     │
│  │         │                                                       │     │
│  │         ▼                                                       │     │
│  │     Archive track                                               │     │
│  │         │                                                       │     │
│  │         ▼                                                       │     │
│  │     TRACK_COMPLETE                                              │     │
│  └────────────────────────────────────────────────────────────────┘     │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

## LEDGER.log Format

Append-only log with max 1000 entries, auto-rotate.

```
# LEDGER.log - Append-only session log
2025-12-28T10:00:00Z | SESSION_START | track:skill-integration
2025-12-28T10:01:00Z | CLAIMED | issue:bd-42
2025-12-28T10:02:00Z | TDD_PHASE | RED | issue:bd-42
2025-12-28T10:05:00Z | TDD_PHASE | GREEN | issue:bd-42
2025-12-28T10:07:00Z | TDD_PHASE | REFACTOR | issue:bd-42
2025-12-28T10:10:00Z | PRE_VERIFY | PASS | issue:bd-42
2025-12-28T10:11:00Z | COMPLETED | issue:bd-42
```

**Recovery:** Read log → filter COMPLETED → skip completed issues on resume.

**Rotation:** When entries > 1000 → archive to `.log.1`

## Skills Transformation

| Before (15) | After (6) | Action |
|-------------|-----------|--------|
| beads | beads | KEEP |
| conductor | conductor | ENHANCED |
| continuity | → conductor/ledger/ | MERGE |
| create-plan | (removed) | DELETE |
| design | design | KEEP |
| dispatching-parallel-agents | → conductor/coordination/ | MERGE |
| doc-sync | → conductor/doc-sync/ | MERGE |
| finishing-a-development-branch | → conductor/finish/ | MERGE |
| sharing-skills | sharing-skills | KEEP |
| subagent-driven-development | → conductor/prompts/ | MERGE |
| test-driven-development | → conductor/tdd/ | MERGE |
| using-git-worktrees | using-git-worktrees | KEEP |
| using-superpowers | → AGENTS.md | MOVE |
| verification-before-completion | → conductor/verification/ | MERGE |
| writing-skills | writing-skills | KEEP (update refs) |

## Implementation Approach

**Big Bang** - Single PR with ordered commits:

1. `chore: scaffold conductor/references/` - Create directories
2. `refactor: move prompts from subagent-dev` - 3 files
3. `refactor: move coordination from dispatching` - 5 files
4. `refactor: move TDD content` - Extract from SKILL.md
5. `refactor: move verification content` - Extract from SKILL.md
6. `refactor: move doc-sync content` - 4 files
7. `refactor: move continuity to ledger/` - 3 files
8. `refactor: move finishing-branch to finish/` - Extract
9. `feat: LEDGER.log format + rotation` - New files
10. `refactor: conductor SKILL.md overview` - Slim down
11. `fix: update writing-skills references` - Update paths
12. `refactor: move discipline to AGENTS.md` - Append
13. `chore: delete 9 merged skills` - Remove directories
14. `test: smoke test full workflow` - Verify

## Acceptance Criteria

| # | Criteria | Test |
|---|----------|------|
| 1 | Skill count = 6 | `ls skills/ \| wc -l` = 6 |
| 2 | Conductor refs = 9 dirs | `ls skills/conductor/references/` |
| 3 | LEDGER.log works | Write → Read → Parse |
| 4 | Log rotation | 1001 entries → .log.1 created |
| 5 | TDD enforced | Cannot code without failing test |
| 6 | Recovery works | Kill → Resume → Skips completed |
| 7 | Branch options | Merge/PR/Clean functional |
| 8 | Old skills deleted | 9 directories removed |
| 9 | Refs updated | writing-skills points to conductor |

## Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| Broken refs after move | Grep all refs before delete |
| LEDGER.log parse errors | Simple format, unit tests |
| Missing prompts | Verify file counts match |
| Git history loss | Use `git mv` for moves |

## Party Mode Feedback (Applied)

| Agent | Feedback | Applied |
|-------|----------|---------|
| Winston | LEDGER race condition | → Append-only log |
| Winston | TDD phase per agent | → Agent-specific logs |
| Bob | Dispatch auto-detect | → Analyze file deps |
| Bob | Conflict resolution | → 3-way merge agent |
| Murat | Pre-verification | → After each issue |
| Mary | Recovery flow | → Resume from completed[] |

## Grounding Verified

- 15 skills exist
- conductor/references/ exists (merge into)
- All source files located
- LEDGER.md empty (fresh start OK)
- AGENTS.md exists

---

**Design Status:** APPROVED

**Next:** Run `/conductor-newtrack skill-integration` to generate spec + plan + beads.
