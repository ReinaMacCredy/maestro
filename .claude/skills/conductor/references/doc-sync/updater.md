# Doc Updater

Strategies for updating documentation based on detected changes.

## Overview

The Doc Updater applies changes to markdown files based on the change manifest from the detector. It handles:

1. **Minor updates** - Automatic find-replace operations
2. **Major updates** - User prompts for confirmation

---

## Minor Update Strategies

Minor updates are applied automatically without user confirmation.

### 1. Path Replacement

When a file is renamed, update all references:

```
Change: src/utils.ts → src/helpers.ts

Algorithm:
1. Find all occurrences of "src/utils.ts" in affected docs
2. Replace with "src/helpers.ts"
3. Preserve surrounding context (backticks, quotes, etc.)

Patterns to match:
- `src/utils.ts` → `src/helpers.ts`
- "src/utils.ts" → "src/helpers.ts"
- (src/utils.ts) → (src/helpers.ts)
- [link](src/utils.ts) → [link](src/helpers.ts)
```

**Edge cases:**
- Preserve line anchors: `src/utils.ts#L42` → `src/helpers.ts#L42`
- Handle with/without leading `./`: `./src/utils.ts` → `./src/helpers.ts`

### 2. Function/Class Name Replacement

When a function or class is renamed:

```
Change: initApp() → initialize()

Algorithm:
1. Find backtick-wrapped occurrences: `initApp` or `initApp()`
2. Replace with new name: `initialize` or `initialize()`
3. Also check code blocks for examples

Patterns:
- `initApp()` → `initialize()`
- `initApp` → `initialize`
- In code blocks: initApp( → initialize(
```

**Safety checks:**
- Only replace exact matches (word boundaries)
- Don't replace partial matches (e.g., `initAppConfig` should not become `initializeConfig`)

### 3. Import Statement Updates

When imports change in code examples:

```
Change: import from './utils' → import from './helpers'

In code blocks:
- import { foo } from './utils' → import { foo } from './helpers'
- const x = require('./utils') → const x = require('./helpers')
```

### 4. Batch Replacement

For efficiency, batch all minor updates per file:

```
file: README.md
replacements:
  - { old: "src/utils.ts", new: "src/helpers.ts", count: 3 }
  - { old: "initApp", new: "initialize", count: 2 }
  
Apply all replacements in single file write
```

---

## Major Update Prompts

Major updates require user confirmation before applying.

### Prompt Templates

#### New Feature Added

```
┌─────────────────────────────────────────────────────────┐
│ 📝 New Feature Detected                                  │
├─────────────────────────────────────────────────────────┤
│ Feature: User Authentication                             │
│ Source: Bead my-project-abc123                          │
│ Files: src/auth/index.ts, src/auth/jwt.ts               │
│                                                          │
│ Suggested: Add section to README.md                      │
│                                                          │
│ Options:                                                 │
│   [Y] Yes, add section                                   │
│   [N] No, skip                                           │
│   [C] Custom location                                    │
└─────────────────────────────────────────────────────────┘
```

#### Feature Removed

```
┌─────────────────────────────────────────────────────────┐
│ 🗑️  Feature Removed Detected                             │
├─────────────────────────────────────────────────────────┤
│ Deleted: src/legacy/old-feature.ts                       │
│                                                          │
│ Found references in:                                     │
│   - README.md (lines 45-52)                              │
│   - docs/api.md (lines 120-135)                          │
│                                                          │
│ Options:                                                 │
│   [Y] Yes, remove references                             │
│   [N] No, keep (may be outdated)                         │
│   [R] Review each reference                              │
└─────────────────────────────────────────────────────────┘
```

#### API Signature Changed

```
┌─────────────────────────────────────────────────────────┐
│ ⚠️  API Signature Changed                                │
├─────────────────────────────────────────────────────────┤
│ Function: createUser                                     │
│                                                          │
│ Before: createUser(name: string)                         │
│ After:  createUser(name: string, options?: UserOptions)  │
│                                                          │
│ Found examples in:                                       │
│   - docs/api.md (line 78)                                │
│   - README.md (line 156)                                 │
│                                                          │
│ Options:                                                 │
│   [Y] Yes, update examples                               │
│   [N] No, skip                                           │
│   [R] Review each example                                │
└─────────────────────────────────────────────────────────┘
```

### User Response Handling

```
function handleResponse(response, change):
  switch response:
    case 'Y', 'y', 'yes':
      applyChange(change)
      return { applied: true }
      
    case 'N', 'n', 'no':
      skipChange(change)
      return { applied: false, reason: 'user_skipped' }
      
    case 'C', 'c', 'custom':
      location = promptForLocation()
      applyChangeAt(change, location)
      return { applied: true, custom: true }
      
    case 'R', 'r', 'review':
      for each reference in change.references:
        showReference(reference)
        subResponse = prompt("Update this reference?")
        if subResponse == 'Y':
          applyReferenceUpdate(reference)
      return { applied: 'partial' }
```

### Section Templates

When adding new sections, use templates:

```markdown
## {Feature Name}

{Brief description from bead}

### Usage

```{language}
// Example code here
```

### Configuration

{If applicable}

See [{related file}]({file path}) for implementation details.
```

---

## Output Format

### Summary Display

After running doc-sync, display a summary:

```
📄 Doc-Sync Results
══════════════════════════════════════════════════════════

Scanned: 8 markdown files
Changes detected: 5

✅ Auto-Updated (Minor):
┌──────────────────┬────────────────────────────┬───────┐
│ File             │ Change                     │ Count │
├──────────────────┼────────────────────────────┼───────┤
│ README.md        │ src/utils.ts → helpers.ts  │ 3     │
│ README.md        │ initApp → initialize       │ 2     │
│ docs/api.md      │ src/utils.ts → helpers.ts  │ 1     │
└──────────────────┴────────────────────────────┴───────┘

⚠️  Reviewed (Major):
┌──────────────────┬────────────────────────────┬────────┐
│ File             │ Change                     │ Action │
├──────────────────┼────────────────────────────┼────────┤
│ README.md        │ Add "Authentication" section│ Added  │
│ docs/api.md      │ Update createUser examples │ Skipped│
└──────────────────┴────────────────────────────┴────────┘

📊 Summary:
   Files modified: 3
   Auto-updates: 6
   User-reviewed: 2 (1 applied, 1 skipped)
```

### Diff Preview (--dry-run)

When running with `--dry-run`, show proposed changes:

```
📄 Doc-Sync Preview (--dry-run)
══════════════════════════════════════════════════════════

Would update: README.md

@@ -15,7 +15,7 @@
 ## Getting Started
 
-See `src/utils.ts` for helper functions.
+See `src/helpers.ts` for helper functions.
 
 ## Usage

@@ -42,7 +42,7 @@
 To initialize the app:
 
-Call `initApp()` with your configuration.
+Call `initialize()` with your configuration.

──────────────────────────────────────────────────────────
Would update: docs/api.md
...

Run without --dry-run to apply changes.
```

### JSON Output

For programmatic use:

```json
{
  "executed_at": "2025-12-27T10:00:00Z",
  "mode": "normal",
  "results": {
    "files_scanned": 8,
    "files_modified": 3,
    "changes": [
      {
        "file": "README.md",
        "type": "minor",
        "updates": [
          {
            "old": "src/utils.ts",
            "new": "src/helpers.ts",
            "occurrences": 3,
            "lines": [15, 42, 78]
          }
        ],
        "applied": true
      },
      {
        "file": "README.md",
        "type": "major",
        "description": "Add Authentication section",
        "applied": true,
        "user_response": "Y"
      }
    ]
  },
  "summary": {
    "minor_applied": 6,
    "major_applied": 1,
    "major_skipped": 1
  }
}
```

### Error Handling

```
❌ Doc-Sync Errors
══════════════════════════════════════════════════════════

File: docs/legacy.md
Error: File not found (may have been deleted)
Action: Skipped

File: README.md
Error: Conflict - multiple possible replacements
  Line 45: "utils" could match src/utils.ts or lib/utils.js
Action: Skipped (resolve manually)

──────────────────────────────────────────────────────────
Completed with 2 errors. Review above and re-run if needed.
```

---

## Force Mode (--force)

When running with `--force`:

1. Apply all minor updates (same as normal)
2. Apply all major updates without prompting
3. Use default templates for new sections
4. Log all actions for review

```
📄 Doc-Sync (--force mode)
══════════════════════════════════════════════════════════

All changes applied automatically.

Minor updates: 6
Major updates: 2 (auto-applied)

Review changes with: git diff
Undo with: git checkout -- <files>
```

---

*See [SKILL.md](../../SKILL.md) for full workflow.*
