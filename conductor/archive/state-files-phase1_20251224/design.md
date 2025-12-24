# Design: Conductor Track Validation System

## Problem Statement

Tracks can be left in inconsistent state when state files (metadata.json, .track-progress.json, .fb-progress.json) are not created. This happens when:

1. Workflow is interrupted mid-execution
2. Agent skips steps or uses `bd create` directly
3. Manual track creation bypasses workflow

**Root cause discovered:** In thread `T-019b4c0b-e9ae-7650-b2f8-0845df48b214`, the agent:

- Created spec.md and plan.md
- Used `bd create` directly instead of file-beads subagent
- Never created metadata.json, .track-progress.json, .fb-progress.json

## Solution

Two-layer validation architecture:

1. **Prevention:** Move state file creation to Phase 1.3 of `/conductor-newtrack`
2. **Recovery:** Centralized validation system for pre-flight checks and auto-repair

---

## Architecture

```
workflows/
├── validate.md                      # High-level overview + checklist
└── schemas/
    ├── metadata.schema.json         # EXISTS
    ├── implement_state.schema.json  # EXISTS
    ├── track_progress.schema.json   # CREATE
    └── fb_progress.schema.json      # CREATE

skills/conductor/references/validation/
├── README.md                        # Index for subsystems
├── quality/                         # AI output scoring (reserved)
│   ├── README.md
│   ├── judge-prompt.md
│   └── rubrics.md
└── track/                           # Track integrity validation
    ├── README.md                    # Quick reference
    ├── checks.md                    # Inline-able validation logic
    ├── snippets.md                  # Bash templates
    └── recovery.md                  # Troubleshooting guide

commands/conductor/
└── validate.toml                    # User-facing validation (references both)
```

### Responsibility Split

| Location                                  | Purpose                  | Used By             |
| ----------------------------------------- | ------------------------ | ------------------- |
| `workflows/validate.md`                   | Human-readable checklist | Documentation       |
| `workflows/schemas/*.json`                | JSON schema validation   | JSON parsers        |
| `references/validation/track/checks.md`   | Inline-able logic        | Commands as Phase 0 |
| `references/validation/track/recovery.md` | Troubleshooting          | Humans              |
| `commands/conductor/validate.toml`        | Full project scan        | User invocation     |

---

## Validation Modes

| Mode         | Flag         | Behavior                                   |
| ------------ | ------------ | ------------------------------------------ |
| **Default**  | (none)       | Auto-repair what's safe, HALT on unfixable |
| **Diagnose** | `--diagnose` | Report only, never modify, never HALT      |

---

## Commands Using Validation

| Command                | When Validation Runs                      |
| ---------------------- | ----------------------------------------- |
| `/conductor-implement` | Before starting any task                  |
| `/conductor-finish`    | Before archiving                          |
| `/conductor-status`    | Reports health (+ beads orphan detection) |
| `file-beads` (fb)      | Before filing beads                       |
| `review-beads` (rb)    | Before reviewing beads                    |

---

## track_id Validation

**Source of Truth:** Directory name

| File                   | Field          | Action on Mismatch |
| ---------------------- | -------------- | ------------------ |
| `metadata.json`        | `track_id`     | Auto-fix           |
| `.track-progress.json` | `trackId`      | Auto-fix           |
| `.fb-progress.json`    | `trackId`      | Auto-fix           |
| `design.md`            | header content | Warn + ask         |
| `spec.md`              | header content | Warn + ask         |
| `plan.md`              | header content | Warn + ask         |

---

## File Existence Matrix

| design.md | spec.md | plan.md | State Files | Action                     |
| :-------: | :-----: | :-----: | :---------: | -------------------------- |
|     ✗     |    ✗    |    ✗    |      ✗      | SKIP + warn (empty)        |
|     ✓     |    ✗    |    ✗    |      ✗      | PASS (design-only)         |
|     ✓     |    ✓    |    ✓    |      ✗      | Auto-create state files    |
|     ✗     |    ✓    |    ✓    |      ✗      | Auto-create state files    |
|    \*     |    ✓    |    ✗    |     \*      | HALT (spec XOR plan)       |
|    \*     |    ✗    |    ✓    |     \*      | HALT (spec XOR plan)       |
|    \*     |    ✓    |    ✓    |   Partial   | Create missing state files |
|    \*     |    ✓    |    ✓    |  Complete   | Validate + pass            |

---

## Auto-Create State Files Logic

**Trigger:** spec.md + plan.md exist, state files missing

**Pre-checks:**

1. Both files have content (size > 0)
2. Both files are < 30 days old
3. No track_id mismatch in content headers

| Pre-checks | Action                                     |
| ---------- | ------------------------------------------ |
| All pass   | Auto-create, log to repairs                |
| Any fail   | Warn with options, require explicit choice |

---

## Auto-Repair Actions

| Issue                                              | Action                     | Logged? |
| -------------------------------------------------- | -------------------------- | ------- |
| track_id mismatch in state files                   | Auto-fix to directory name | ✓       |
| Missing state files (spec+plan exist, checks pass) | Auto-create                | ✓       |
| Missing fields in valid JSON                       | Add defaults               | ✓       |
| Extra unknown fields in JSON                       | Keep (forward compat)      | ✗       |

---

## HALT Conditions (Never Auto-Repair)

| Issue                           | Why                        |
| ------------------------------- | -------------------------- |
| Corrupted JSON                  | Risk of data loss          |
| spec.md XOR plan.md             | Ambiguous intent           |
| Empty spec.md or plan.md        | Nothing to build from      |
| Beads exist without state files | Can't reliably reconstruct |

---

## Staleness Detection

**For `.fb-progress.json.status = "in_progress"`:**

```
⚠️ Track has incomplete operation
   Started: 2025-12-24T10:30:00Z (X hours ago)

   Options:
   [1] Resume: fb <track_id>
   [2] Reset: fb <track_id> --force
   [3] Diagnose: validate --diagnose
```

**No auto-reset.** Always require explicit user action.

---

## Audit Trail

**Location:** `metadata.json.repairs[]` (last 10 entries)

```json
{
  "repairs": [
    {
      "at": "2025-12-24T10:30:00Z",
      "action": "track_id_mismatch",
      "field": "metadata.json.track_id",
      "from": "old_name",
      "to": "new_name",
      "by": "validate"
    },
    {
      "at": "2025-12-24T11:00:00Z",
      "action": "created_state_file",
      "file": ".track-progress.json",
      "by": "validate"
    }
  ]
}
```

---

## Atomic Writes

All state file updates use temp + rename pattern:

```bash
jq '...' file.json > file.json.tmp && mv file.json.tmp file.json
```

---

## Out of Scope

| Feature                           | Where It Belongs                      |
| --------------------------------- | ------------------------------------- |
| Beads orphan detection            | `/conductor-status`                   |
| Cross-track dependency validation | `/conductor-status --all`             |
| Lock files for concurrent access  | Not needed (atomic writes sufficient) |

---

## Implementation Plan

### Files to Create

| File                                                             | Purpose                              |
| ---------------------------------------------------------------- | ------------------------------------ |
| `workflows/schemas/track_progress.schema.json`                   | JSON schema for .track-progress.json |
| `workflows/schemas/fb_progress.schema.json`                      | JSON schema for .fb-progress.json    |
| `skills/conductor/references/validation/quality/README.md`       | Move existing                        |
| `skills/conductor/references/validation/quality/judge-prompt.md` | Move existing                        |
| `skills/conductor/references/validation/quality/rubrics.md`      | Move existing                        |
| `skills/conductor/references/validation/track/README.md`         | Quick reference                      |
| `skills/conductor/references/validation/track/checks.md`         | Inline-able logic                    |
| `skills/conductor/references/validation/track/snippets.md`       | Bash templates                       |
| `skills/conductor/references/validation/track/recovery.md`       | Troubleshooting                      |

### Files to Update

| File                                               | Change                                      |
| -------------------------------------------------- | ------------------------------------------- |
| `workflows/validate.md`                            | Add track_id validation, state file section |
| `skills/conductor/references/validation/README.md` | Add index for subsystems                    |
| `commands/conductor/validate.toml`                 | Add Phase 0 cross-ref to checks.md          |
| `commands/conductor/implement.toml`                | Add Phase 0 validation                      |
| `commands/conductor/finish.toml`                   | Add Phase 0 validation                      |
| `skills/file-beads/SKILL.md`                       | Reference checks.md (already done)          |
| `skills/review-beads/SKILL.md`                     | Reference checks.md                         |

---

## Acceptance Criteria

- [x] All 3 state file schemas exist in workflows/schemas/ (track_progress, fb_progress, implement_state already existed)
- [x] validate.md updated with track_id and state file validation
- [x] references/validation/track/ folder created with 4 files (README, checks, snippets, recovery)
- [x] quality/ subfolder created with moved files (README, judge-prompt, rubrics)
- [x] validate.toml references checks.md for per-track logic (Section 2.7)
- [x] implement.toml has Phase 0 validation
- [ ] finish.toml has Phase 0 validation (SKIPPED: file doesn't exist yet)
- [x] file-beads SKILL.md references checks.md (done in previous work)
- [x] review-beads SKILL.md references checks.md (Phase 0.1 added)
- [x] track_id mismatch is auto-fixed (logic in checks.md)
- [x] Missing state files auto-created when spec+plan exist (logic in snippets.md)
- [x] Corrupted JSON causes HALT (logic in checks.md Step 0.4)
- [x] spec.md XOR plan.md causes HALT (logic in checks.md Step 0.3)
- [x] Repairs logged to metadata.json.repairs[] (template in snippets.md)

---

## Design Session History

- **Thread:** T-019b4c5e-39ff-7275-91b6-14855ba2492e (initial design)
- **Thread:** T-019b4cca-03cc-736f-9d83-60a0d36d29af (validation system ds)
- **Date:** 2025-12-24
- **Methodology:** Double Diamond with Party Mode reviews
- **Agents consulted:** Winston (Architect), Murat (QA), Victor (Strategist), Mary (Analyst), Amelia (Developer), Paige (Docs), Dr. Quinn (Solver)
