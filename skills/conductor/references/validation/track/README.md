# Track Validation Reference

Pre-flight validation for Conductor tracks. Ensures track integrity before operations.

## Validation Modes

| Mode | Flag | Behavior |
|------|------|----------|
| **Default** | (none) | Auto-repair + HALT on unfixable |
| **Diagnose** | `--diagnose` | Report only, no modifications |

## Files

| File | Purpose | Inline By |
|------|---------|-----------|
| `checks.md` | Core validation logic | Commands (Phase 0) |
| `snippets.md` | State file templates | Auto-create operations |
| `recovery.md` | Troubleshooting guide | Manual reference |

## Quick Start

### For Commands

Add Phase 0 validation by inlining `checks.md`:

```markdown
## Phase 0: Track Validation

Inline `skills/conductor/references/validation/track/checks.md` and execute.
```

### For Skills

Reference validation in pre-flight:

```markdown
## Phase 0.3: Track Validation

Before processing, validate track integrity per `checks.md`.
```

## Validation Summary

1. **Resolve track path** - Find track in `conductor/tracks/` or `conductor/archive/`
2. **Check directory** - Validate not empty
3. **File existence matrix** - Validate file combinations (see checks.md)
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
