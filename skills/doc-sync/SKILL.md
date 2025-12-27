---
name: doc-sync
description: Auto-sync documentation with code changes. Use when completing tracks via /conductor-finish or manually with /doc-sync command. Detects outdated docs and updates file paths, function names, and prompts for major changes.
version: 1.0.0
---

# Doc-Sync Skill

Automatically synchronize documentation with code changes in any project using Conductor workflow.

## When To Use

Trigger on:

- After `/conductor-finish` completes (auto, as Phase 7)
- User runs `/doc-sync` command
- User asks to "sync docs", "update documentation", or "check if docs are outdated"

## Triggers

| Trigger | When |
|---------|------|
| Auto | After `/conductor-finish` Phase 6 (CODEMAPS) |
| Manual | `/doc-sync [--dry-run] [--force]` |

## Flags

| Flag | Description |
|------|-------------|
| `--dry-run` | Show proposed changes without applying |
| `--force` | Auto-apply all changes (skip review prompts) |

## Workflow

```
┌─────────────────────────────────────────────────────────┐
│                     doc-sync workflow                    │
├─────────────────────────────────────────────────────────┤
│                                                          │
│  1. SCAN          2. DETECT         3. UPDATE           │
│  ┌──────────┐    ┌──────────┐     ┌──────────┐         │
│  │ Find .md │───▶│ Git diff │────▶│ Minor:   │         │
│  │ with code│    │ + Beads  │     │ Auto-edit│         │
│  │ refs     │    │ context  │     │          │         │
│  └──────────┘    └──────────┘     │ Major:   │         │
│                                   │ Review   │         │
│                                   └──────────┘         │
└─────────────────────────────────────────────────────────┘
```

### Phase 1: Scan

Find all `.md` files with code references:

1. Scan project for markdown files
2. Extract code references:
   - File paths (`src/`, `lib/`, etc.)
   - Import statements
   - Function/class names in backticks
   - Code blocks with language tags
3. Build dependency map: `doc → [referenced code files]`

See [references/scanner.md](references/scanner.md) for patterns.

### Phase 2: Detect

Identify code changes from two sources:

**Git Diff:**
- Files added (A), deleted (D), modified (M), renamed (R)
- Function signature changes

**Beads Context:**
- Closed issues in current track
- Change summaries from beads

Classify impact:
- **Minor**: File path changes, renames
- **Major**: New features, removed features, API changes

See [references/detector.md](references/detector.md) for classification.

### Phase 3: Update

Apply updates based on impact:

| Impact | Action |
|--------|--------|
| Minor | Auto-update (paths, names) |
| Major | Prompt user for confirmation |

**Minor changes (auto-apply):**
- `src/old.ts` → `src/new.ts` in all docs
- `oldFunction` → `newFunction` in examples

**Major changes (prompt):**
- "New feature X detected. Add section to README.md?"
- "Feature Y removed. Remove from docs?"
- "API signature changed. Update examples?"

See [references/updater.md](references/updater.md) for strategies.

## Integration

### With `/conductor-finish`

Doc-sync runs as Phase 7 in the finish workflow:

```
Phase 6 (CODEMAPS) → Phase 7 (Doc-Sync) → Archive
```

Skip conditions:
- No `.md` files with code references found
- No code changes detected
- `--skip-doc-sync` flag passed

### Manual Usage

```bash
/doc-sync              # Run with prompts for major changes
/doc-sync --dry-run    # Preview changes only
/doc-sync --force      # Apply all changes without prompts
```

See [references/integration.md](references/integration.md) for details.

## Output

After running, displays summary:

```
📄 Doc-Sync Results
──────────────────────
Files scanned: 5
Changes detected: 3

✅ Auto-updated:
  - README.md: 2 path updates
  - docs/api.md: 1 function rename

⚠️ Needs review:
  - TUTORIAL.md: New feature section? [Y/n]
```

## Examples

### Example 1: File Renamed

```
Code change: src/utils.ts → src/helpers.ts

README.md before:
  See `src/utils.ts` for helper functions.

README.md after (auto-updated):
  See `src/helpers.ts` for helper functions.
```

### Example 2: New Feature

```
Beads: Closed issue "Add authentication"

Prompt:
  New feature "authentication" detected.
  Add section to README.md? [Y/n]
```

## References

- [scanner.md](references/scanner.md) - Document scanning logic
- [detector.md](references/detector.md) - Change detection logic
- [updater.md](references/updater.md) - Update strategies
- [integration.md](references/integration.md) - Conductor integration
