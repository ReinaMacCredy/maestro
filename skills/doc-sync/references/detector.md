# Change Detector

Logic for detecting code changes from git diff and beads context.

## Overview

The Change Detector combines two data sources:
1. **Git Diff** - What files changed in the codebase
2. **Beads Context** - What features/fixes were completed

Together, these provide accurate change detection for doc-sync.

---

## Git Diff Parsing

### Commands

```bash
# Get file status changes (A=added, D=deleted, M=modified, R=renamed)
git diff --name-status HEAD~N

# Get file status since specific commit
git diff --name-status <commit-sha>

# Get detailed diff with context
git diff --unified=3 HEAD~N -- "*.ts" "*.js" "*.py"

# Get renamed files with similarity index
git diff --name-status -M HEAD~N
```

### Parsing Output

**Name-status output format:**
```
A       src/new-file.ts           # Added
D       src/old-file.ts           # Deleted  
M       src/modified.ts           # Modified
R100    src/old.ts    src/new.ts  # Renamed (100% similar)
R085    src/a.ts      src/b.ts    # Renamed with changes (85% similar)
```

**Parsing logic:**
```
For each line:
  status = first character(s)
  if status == 'R':
    old_path = second column
    new_path = third column
    similarity = number after R
  else:
    path = second column
    
  Add to changes list with status and path(s)
```

### Change Types from Git

| Status | Meaning | Doc Impact |
|--------|---------|------------|
| `A` | File added | Check if new feature needs docs |
| `D` | File deleted | Check if docs reference deleted file |
| `M` | File modified | Check for API/signature changes |
| `R` | File renamed | Update all path references |

### Detecting Function Signature Changes

For modified files, parse diff content:

```bash
git diff HEAD~N -- src/api.ts | grep -E "^[-+].*function|^[-+].*export|^[-+].*class"
```

**Pattern matching:**
```regex
# Function signature changes
^[-+]\s*(export\s+)?(async\s+)?function\s+(\w+)\s*\(([^)]*)\)

# Class changes
^[-+]\s*(export\s+)?class\s+(\w+)

# Method changes
^[-+]\s*(public|private|protected)?\s*(async\s+)?(\w+)\s*\(([^)]*)\)
```

---

## Beads Context Extraction

### Commands

```bash
# Get closed issues in JSON format
bd list --status=closed --json

# Get specific issue details
bd show <issue-id> --json
```

### Extracting Change Information

**From closed beads:**

```json
{
  "id": "my-project-abc123",
  "title": "Add user authentication",
  "description": "Implement JWT-based auth...",
  "status": "closed",
  "issue_type": "feature",
  "labels": ["auth", "api"]
}
```

**Extract:**
- `title` → Feature/fix name for doc sections
- `description` → Details about what changed
- `issue_type` → `feature` = likely needs new docs, `bug` = maybe update existing
- `labels` → Keywords to search in docs

### Filtering Relevant Beads

Only consider beads closed in current track:

```bash
# Get beads from .fb-progress.json
jq '.issues[]' conductor/tracks/<track-id>/.fb-progress.json
```

### Merging Git + Beads Data

```json
{
  "changes": [
    {
      "type": "file_renamed",
      "old_path": "src/utils.ts",
      "new_path": "src/helpers.ts",
      "source": "git",
      "impact": "minor"
    },
    {
      "type": "feature_added",
      "name": "User Authentication",
      "files": ["src/auth/index.ts", "src/auth/jwt.ts"],
      "source": "beads",
      "bead_id": "my-project-abc123",
      "impact": "major"
    }
  ]
}
```

---

## Impact Classification

### Classification Rules

| Change | Impact | Reason |
|--------|--------|--------|
| File path changed | **Minor** | Simple find-replace in docs |
| Function renamed (same signature) | **Minor** | Simple find-replace |
| File deleted | **Major** | May need to remove doc sections |
| New feature added | **Major** | May need new doc sections |
| API signature changed | **Major** | Examples may be invalid |
| Breaking change | **Major** | Docs need prominent update |

### Classification Algorithm

```
function classifyImpact(change):
  # Minor changes (auto-update)
  if change.type == "file_renamed":
    return "minor"
  if change.type == "function_renamed" AND signature_unchanged:
    return "minor"
  if change.type == "file_modified" AND only_internal_changes:
    return "minor"
    
  # Major changes (require review)
  if change.type == "file_deleted":
    return "major"
  if change.type == "feature_added":
    return "major"
  if change.type == "api_changed":
    return "major"
  if change.source == "beads" AND issue_type == "feature":
    return "major"
    
  # Default to minor if uncertain
  return "minor"
```

### Impact Score Calculation

For complex changes, calculate a score:

| Factor | Score |
|--------|-------|
| File renamed | +1 |
| File deleted | +3 |
| New feature (from beads) | +4 |
| API signature changed | +3 |
| Multiple files affected | +1 per file (max +5) |
| Breaking change label | +5 |

**Thresholds:**
- Score 1-2: Minor (auto-update)
- Score 3-5: Minor with summary
- Score 6+: Major (prompt user)

### Examples

**Example 1: Minor Change**
```
Change: src/utils.ts → src/helpers.ts
Score: 1 (file renamed)
Impact: Minor
Action: Auto-update all references
```

**Example 2: Major Change**
```
Change: New "authentication" feature added
Files: src/auth/index.ts, src/auth/jwt.ts, src/auth/middleware.ts
Bead: "Add JWT authentication" (feature)
Score: 4 (new feature) + 3 (multiple files) = 7
Impact: Major
Action: Prompt "Add authentication section to README.md?"
```

**Example 3: API Change**
```
Change: function signature changed
  Before: createUser(name: string)
  After: createUser(name: string, options?: UserOptions)
Score: 3 (API changed)
Impact: Major
Action: Prompt "Update createUser examples in docs/api.md?"
```

---

## Output Format

### Change Manifest

```json
{
  "detected_at": "2025-12-27T10:00:00Z",
  "git_range": "HEAD~5..HEAD",
  "track_id": "feature-auth_20251227",
  "changes": [
    {
      "id": "change-001",
      "type": "file_renamed",
      "source": "git",
      "old_path": "src/utils.ts",
      "new_path": "src/helpers.ts",
      "impact": "minor",
      "score": 1,
      "affected_docs": ["README.md", "docs/api.md"]
    },
    {
      "id": "change-002",
      "type": "feature_added",
      "source": "beads",
      "bead_id": "my-project-abc123",
      "name": "User Authentication",
      "description": "JWT-based authentication system",
      "files": ["src/auth/index.ts", "src/auth/jwt.ts"],
      "impact": "major",
      "score": 7,
      "suggested_docs": ["README.md"],
      "suggested_section": "## Authentication"
    }
  ],
  "summary": {
    "total_changes": 2,
    "minor_changes": 1,
    "major_changes": 1,
    "affected_docs": 2
  }
}
```

---

*See [SKILL.md](../SKILL.md) for full workflow.*
