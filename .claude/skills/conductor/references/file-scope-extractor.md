# File Scope Extraction

Extracts file paths from task descriptions in plan.md to enable smart file reservations and conflict detection.

## Purpose

When orchestrating parallel workers, each task may target specific files. Extracting file scopes allows:
- **Pre-flight conflict detection** - Identify overlapping file edits before spawning workers
- **Automatic file reservations** - Reserve files for workers based on their task scope
- **Smarter routing** - Group related tasks to minimize coordination overhead

## Extraction Algorithm

```
FOR each task in plan.md:
  1. Extract explicit file declarations (File: ...)
  2. Extract backtick-wrapped paths (`path/to/file.ext`)
  3. Extract directory references (ending with /)
  4. Extract glob patterns (containing * or **)
  5. Normalize paths (resolve . and .., deduplicate)
  6. Store in metadata.json under beads.fileScopes
```

## Regex Patterns

| Pattern Type | Regex | Example Match |
|--------------|-------|---------------|
| Explicit file | `File:\s*(.+?)(?:\n\|$)` | `File: src/main.ts` |
| Backtick path | `` `([^`]+\.[a-z]+)` `` | `` `lib/utils.py` `` |
| Backtick dir | `` `([^`]+/)` `` | `` `schemas/` `` |
| Glob pattern | `` `(\*\*?/[^`]+)` `` | `` `**/*.md` `` |
| Inline path ref | `\b([\w-]+/[\w./-]+\.[a-z]{2,4})\b` | `conductor/plan.md` |

## Edge Cases

| Case | Handling |
|------|----------|
| Relative paths (`./file.md`) | Normalize to project-relative path |
| Parent refs (`../lib/`) | Resolve relative to task context, reject if escapes project |
| Glob patterns (`**/*.ts`) | Store as-is for reservation matching |
| Directory refs (`schemas/`) | Expand to `schemas/**` for reservation |
| URLs (http://...) | Ignore - not file paths |
| Import statements | Extract path from import if local (`.` or `..` prefix) |
| No explicit paths | Infer from task title if possible (e.g., "Update README" → `README.md`) |

## Examples

### Task with explicit paths
```markdown
### 1.1 Update schema
- Modify `schemas/metadata.schema.json`
- Add new field
- File: schemas/validation.ts
```
**Extracted:** `["schemas/metadata.schema.json", "schemas/validation.ts"]`

### Task with directory reference
```markdown
### 2.1 Refactor utils
- Reorganize `lib/utils/`
```
**Extracted:** `["lib/utils/**"]`

### Task with glob
```markdown
### 3.1 Update all configs
- Modify `**/*.config.js`
```
**Extracted:** `["**/*.config.js"]`

### Task with inferred path
```markdown
### 4.1 Add CONTRIBUTING guide
```
**Inferred:** `["CONTRIBUTING.md"]`

## Output Format

File scopes are stored in `metadata.json` under `beads.fileScopes`:

```json
{
  "beads": {
    "fileScopes": {
      "1.1": ["schemas/metadata.schema.json", "schemas/validation.ts"],
      "2.1": ["lib/utils/**"],
      "3.1": ["**/*.config.js"]
    }
  }
}
```

## Integration Points

- **`fb` (file-beads)** - Populates fileScopes during bead filing
- **Orchestrator preflight** - Reads fileScopes to detect conflicts
- **Agent Mail reservations** - Uses fileScopes for `file_reservation_paths`
