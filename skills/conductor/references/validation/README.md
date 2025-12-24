# Validation Reference Files

Two validation subsystems for Conductor tracks.

## Subsystems

| Folder | Purpose | Used By |
|--------|---------|---------|
| `track/` | Track integrity validation (state files, track_id, file existence) | `/conductor-validate`, `/conductor-implement`, `fb`, `rb` |
| `quality/` | Compaction quality evaluation (reserved for future LLM scoring) | Reserved |

> **Note:** `/conductor-finish` uses inline Phase 0 validation, not track/checks.md.

## Quick Reference

### Track Validation (Pre-flight checks)

```
track/
├── README.md      # Quick reference
├── checks.md      # Validation logic (inline by commands)
├── snippets.md    # Bash code templates
└── recovery.md    # Troubleshooting guide
```

Key files for inline reference:
- **checks.md** - Core validation logic, inline in command Phase 0
- **snippets.md** - State file templates for auto-creation

### Quality Validation (Reserved)

```
quality/
├── README.md        # Status and potential future use
├── judge-prompt.md  # LLM judge prompt template
└── rubrics.md       # 6-dimension scoring rubric
```

Not currently used. Reserved for automated quality scoring of compaction summaries.

## Usage Pattern

Commands that perform track operations should include Phase 0 validation:

```markdown
## Phase 0: Track Validation

Inline `skills/conductor/references/validation/track/checks.md` and execute.
```

See [track/checks.md](track/checks.md) for the full validation logic.
