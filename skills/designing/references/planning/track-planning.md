# Track Planning Reference

> Multi-agent track planning for parallel execution.

## Overview

Track planning enables parallel work distribution across multiple agents. This reference covers:
- Analyzing beads for parallel track candidates
- File scope assignment rules
- Agent name generation
- Plan.md track assignments schema
- Execution plan validation

---

## 1. Parallel Track Analysis with `bv --robot-plan`

Use `bv --robot-plan` to analyze beads and identify parallel track candidates:

```bash
bv --robot-plan
```

### Output Format

```json
{
  "tracks": [
    {
      "id": "A",
      "beads": [".1", ".2"],
      "file_scope": ["src/api/"],
      "wave": 1,
      "dependencies": []
    },
    {
      "id": "B",
      "beads": [".3", ".4"],
      "file_scope": ["lib/utils/"],
      "wave": 1,
      "dependencies": []
    }
  ],
  "conflicts": [],
  "warnings": []
}
```

### Grouping Heuristics

| Heuristic | Description |
|-----------|-------------|
| **File proximity** | Beads touching same directory grouped together |
| **Dependency chains** | Sequential dependencies stay in same track |
| **Independent paths** | Unrelated work split to separate tracks |
| **Wave ordering** | Dependencies determine wave assignment |

### Example Analysis

```bash
# Show track candidates with file scopes
bv --robot-plan | jq '.tracks[] | {id, beads, file_scope}'

# Check for conflicts
bv --robot-plan | jq '.conflicts'
```

---

## 2. File Scope Assignment Rules

### Core Principles

1. **No Overlap Between Tracks**: Each file/directory belongs to exactly one track
2. **Exclusive Ownership**: Track owns all files in its scope
3. **Directory-Level Preferred**: Assign at directory level, not individual files

### Assignment Rules

| Rule | Description | Example |
|------|-------------|---------|
| **Directory ownership** | Track owns entire directory tree | `src/api/` → Track A owns all files under api |
| **No cross-track edits** | Worker only touches assigned files | Track B cannot edit `src/api/users.py` |
| **Shared read-only** | Config files readable by all, editable by none | `config.yaml` read access for all |

### Validation: Check for Conflicts

Before assigning tracks, validate no overlaps exist:

```bash
# Check for file scope conflicts
bv --robot-plan | jq '.conflicts'
```

**Conflict types:**

| Type | Description | Resolution |
|------|-------------|------------|
| `file_overlap` | Same file in multiple tracks | Merge tracks or sequence |
| `directory_overlap` | Nested directory ownership | Use more specific scopes |
| `implicit_dependency` | File A imports from file B in different track | Add explicit dependency |

### Scope Definition Patterns

```markdown
# Good: Clear boundaries
Track A: src/api/
Track B: src/models/
Track C: lib/

# Bad: Overlapping scopes
Track A: src/
Track B: src/api/  # Overlaps with A!
```

---

## 3. Agent Name Generation

### Adjective+Noun Pattern

Agent names follow the **AdjectiveNoun** pattern (PascalCase):

| Valid ✅ | Invalid ❌ | Why Invalid |
|----------|-----------|-------------|
| `BlueLake` | `BackendMigrator` | Descriptive, not random |
| `GreenCastle` | `APIHandler` | Describes function |
| `RedStone` | `Worker1` | Not adjective+noun |
| `PurpleBear` | `blue_lake` | Wrong casing |
| `SilverMoon` | `Agent-A` | Not adjective+noun |

### Generation Requirements

1. **Random selection**: Names are randomly generated, not chosen for meaning
2. **Unique per project**: No duplicate names within a project
3. **Case-sensitive**: Must be PascalCase (`BlueLake`, not `bluelake`)
4. **No descriptive names**: Name should NOT describe the agent's task

### Automatic Generation

When registering agents, omit the name for auto-generation:

```bash
# Recommended: Auto-generate name
register_agent --program amp --model claude-sonnet

# Not recommended: Manual naming
register_agent --program amp --model claude-sonnet --name BlueLake
```

### Name Validation Regex

```regex
^[A-Z][a-z]+[A-Z][a-z]+$
```

Examples that match:
- `BlueLake` ✅
- `GreenCastle` ✅
- `RedStone` ✅

Examples that don't match:
- `bluelake` ❌ (lowercase start)
- `BLUELAKE` ❌ (all caps)
- `Blue_Lake` ❌ (underscore)
- `BackendWorker` ❌ (descriptive)

---

## 4. plan.md Track Assignments Table Schema

### Table Format

```markdown
## Track Assignments

| Track | Agent | Beads | Files | Wave |
|-------|-------|-------|-------|------|
| A | BlueLake | .1, .2 | src/api/ | 1 |
| B | GreenCastle | .3 | lib/utils/ | 1 |
| C | RedStone | .4, .5, .6 | src/models/ | 2 |
```

### Column Definitions

| Column | Type | Description |
|--------|------|-------------|
| **Track** | string | Single letter or short identifier (A, B, C...) |
| **Agent** | string | Adjective+Noun agent name |
| **Beads** | string | Comma-separated bead suffixes (.1, .2, .3) |
| **Files** | string | Directory or file scope owned by this track |
| **Wave** | integer | Execution order (1 = first, 2 = after wave 1 completes) |

### Wave Dependencies

```
Wave 1: All tracks can run in parallel
Wave 2: Runs after ALL Wave 1 tracks complete
Wave 3: Runs after ALL Wave 2 tracks complete
```

### Example with Dependencies

```markdown
## Track Assignments

| Track | Agent | Beads | Files | Wave | Depends On |
|-------|-------|-------|-------|------|------------|
| A | BlueLake | .1 | src/schemas/ | 1 | - |
| B | GreenCastle | .2, .3 | src/api/ | 2 | A |
| C | RedStone | .4 | src/cli/ | 2 | A |
| D | PurpleBear | .5 | tests/ | 3 | B, C |
```

---

## 5. Execution Plan Validation Checklist

Before executing parallel tracks, validate the plan:

### Mandatory Checks

```markdown
## Validation Checklist

- [ ] **All beads assigned**: Every bead appears in exactly one track
- [ ] **No file scope overlaps**: No directory/file appears in multiple tracks
- [ ] **Dependencies respect wave order**: Lower wave completes before higher wave starts
- [ ] **Agent names are valid**: All names match adjective+noun pattern
- [ ] **No orphan beads**: No beads left unassigned
```

### Validation Commands

```bash
# Check all beads assigned
bv --robot-plan | jq '.orphan_beads'

# Check file scope overlaps
bv --robot-plan | jq '.conflicts | select(.type == "file_overlap")'

# Validate wave ordering
bv --robot-plan | jq '.wave_violations'
```

### Common Validation Failures

| Failure | Cause | Fix |
|---------|-------|-----|
| `orphan_bead` | Bead not assigned to any track | Add to appropriate track |
| `file_overlap` | Same file in multiple tracks | Merge tracks or sequence |
| `wave_violation` | Dependency in same/later wave | Reorder waves |
| `invalid_agent_name` | Name doesn't match pattern | Use auto-generated name |

### Pre-Execution Validation Script

```bash
#!/bin/bash
# validate-plan.sh

echo "Validating track plan..."

# Check for conflicts
conflicts=$(bv --robot-plan | jq '.conflicts | length')
if [ "$conflicts" -gt 0 ]; then
  echo "❌ Found $conflicts conflicts"
  exit 1
fi

# Check for orphan beads
orphans=$(bv --robot-plan | jq '.orphan_beads | length')
if [ "$orphans" -gt 0 ]; then
  echo "❌ Found $orphans orphan beads"
  exit 1
fi

echo "✅ Plan validated successfully"
```

---

## Quick Reference

### Command Summary

| Command | Purpose |
|---------|---------|
| `bv --robot-plan` | Analyze beads for parallel tracks |
| `bv --robot-plan \| jq '.tracks'` | List proposed tracks |
| `bv --robot-plan \| jq '.conflicts'` | Check for file conflicts |

### Schema Summary

```
Track Assignments Table:
| Track | Agent | Beads | Files | Wave |
- Track: Single letter identifier
- Agent: AdjectiveNoun format (auto-generated)
- Beads: Comma-separated bead IDs
- Files: Directory scope (exclusive)
- Wave: Execution order (1, 2, 3...)
```

### Validation Summary

1. All beads assigned to tracks ✓
2. No file scope overlaps ✓
3. Dependencies respect wave order ✓
4. Agent names are valid adjective+noun ✓
