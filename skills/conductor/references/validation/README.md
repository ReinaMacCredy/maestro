# Validation Reference Files

Unified validation subsystems for Conductor tracks, covering track integrity, quality evaluation, beads state, and validation gates.

## Directory Structure

```
validation/
├── beads-checks.md          # Beads state validation
├── quality-judge-prompt.md  # LLM judge prompt template
├── quality-rubrics.md       # 6-dimension scoring rubric
├── validate-completion.md   # Completion validation gate
├── validate-design.md       # Design validation gate
├── validate-plan-execution.md  # Plan execution gate
├── validate-plan-structure.md  # Plan structure gate
├── validate-spec.md         # Spec validation gate
├── track-checks.md          # Track integrity checks
├── track-recovery.md        # Troubleshooting guide
├── track-snippets.md        # State file templates
├── lifecycle.md             # Validation lifecycle
└── README.md                # This file
```

## Subsystems

| Category | Files | Purpose | Used By |
|----------|-------|---------|---------|
| **Track** | `track-*.md` | Track integrity (state files, track_id, file existence) | `/conductor-validate`, `/conductor-implement`, `fb`, `rb` |
| **Quality** | `quality-*.md` | Compaction quality evaluation (reserved for future) | Reserved |
| **Beads** | `beads-*.md` | Beads state validation | `/conductor-implement`, `rb` |
| **Gates** | `validate-*.md` | Validation gates for workflow stages | Various workflows |
| **Lifecycle** | `lifecycle.md` | Validation sequencing and timing | All workflows |

## File Reference

### Track Validation

| File | Purpose | Inline By |
|------|---------|-----------|
| `track-checks.md` | Core validation logic | Commands (Phase 0) |
| `track-snippets.md` | State file templates | Auto-create operations |
| `track-recovery.md` | Troubleshooting guide | Manual reference |

### Quality Validation (Reserved)

| File | Purpose |
|------|---------|
| `quality-judge-prompt.md` | Prompt template for LLM-based quality evaluation |
| `quality-rubrics.md` | 6-dimension scoring rubric (Accuracy, Context Awareness, etc.) |

> **Status:** Not currently used by `/conductor-finish`. Reserved for potential future use in automated quality scoring.

### Beads Validation

| File | Purpose |
|------|---------|
| `beads-checks.md` | Beads state validation, sync checks |

### Validation Gates

| File | Purpose |
|------|---------|
| `validate-design.md` | Validates design.md before spec creation |
| `validate-spec.md` | Validates spec.md before plan creation |
| `validate-plan-structure.md` | Validates plan.md structure |
| `validate-plan-execution.md` | Validates plan execution readiness |
| `validate-completion.md` | Validates track completion |

### Lifecycle

| File | Purpose |
|------|---------|
| `lifecycle.md` | When to run each validation, sequencing |

## Validation Modes

| Mode | Flag | Behavior |
|------|------|----------|
| **Default** | (none) | Auto-repair + HALT on unfixable |
| **Diagnose** | `--diagnose` | Report only, no modifications |

## Usage Pattern

Commands that perform track operations should include Phase 0 validation:

```markdown
## Phase 0: Track Validation

Inline `skills/conductor/references/validation/track-checks.md` and execute.
```

### For Skills

Reference validation in pre-flight:

```markdown
## Phase 0.3: Track Validation

Before processing, validate track integrity per `track-checks.md`.
```

## Track Validation Summary

1. **Resolve track path** - Find track in `conductor/tracks/` or `conductor/archive/`
2. **Check directory** - Validate not empty
3. **File existence matrix** - Validate file combinations (see track-checks.md)
4. **Validate JSON** - Parse all JSON files, HALT on corruption
5. **Auto-create state files** - If spec+plan exist but state files missing
6. **Auto-fix track_id** - Directory name is source of truth
7. **Staleness detection** - Warn on stale in_progress state

## Source of Truth

**Directory name is the canonical track_id.**

Auto-fix mismatches in:
- `metadata.json.track_id`
- `.track-progress.json.trackId`
- `.fb-progress.json.trackId`

Warn (don't auto-fix) mismatches in:
- `design.md`, `spec.md`, `plan.md` headers

## Potential Future Use (Quality)

The quality rubrics could be used to:

1. Score AI-generated summaries in Phase 2
2. Evaluate LEARNINGS.md extraction quality
3. Validate conductor/AGENTS.md merge quality

## History

- Quality rubrics moved from `skills/conductor/references/commands/compact/` during `/conductor-finish` integration (2025-12)
- Flattened from 4 subdirectories (`beads/`, `quality/`, `shared/`, `track/`) to single directory (2026-01)
