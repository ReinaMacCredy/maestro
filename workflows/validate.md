# Conductor Validation Workflow

This document defines the core validation logic for the Conductor framework.

## Validation Checklist

### 1. Core Files

| File                              | Required | Description              |
| --------------------------------- | -------- | ------------------------ |
| `conductor/product.md`            | ✓        | Product vision and goals |
| `conductor/tech-stack.md`         | ✓        | Technology choices       |
| `conductor/workflow.md`           | ✓        | Development workflow     |
| `conductor/tracks.md`             | ✓        | Master track list        |
| `conductor/product-guidelines.md` | Optional | Brand/style guidelines   |
| `conductor/setup_state.json`      | Optional | Setup resume state       |

### 2. Track Structure

Each track in `conductor/tracks/<track_id>/` must contain:

| File                   | Required | Validation                                          |
| ---------------------- | -------- | --------------------------------------------------- |
| `metadata.json`        | ✓        | Valid JSON with: track_id, type, status, created_at |
| `spec.md`              | ✓        | Requirements specification                          |
| `plan.md`              | ✓        | Phased task list                                    |
| `implement_state.json` | Optional | Valid JSON if present                               |

### 3. Status Markers

Valid status markers:

- `[ ]` - New/pending
- `[~]` - In progress
- `[x]` - Completed
- `[!]` - Blocked/error (tasks only)

### 4. Metadata Schema

```json
{
  "track_id": "string (required)",
  "type": "feature | bugfix (required)",
  "status": "new | in_progress | completed (required)",
  "created_at": "ISO 8601 datetime (required)",
  "updated_at": "ISO 8601 datetime (optional)",
  "description": "string (optional)"
}
```

### 5. Status Mapping

| tracks.md | metadata.json | Valid |
| --------- | ------------- | ----- |
| `[ ]`     | `new`         | ✓     |
| `[~]`     | `in_progress` | ✓     |
| `[x]`     | `completed`   | ✓     |

### 6. Plan Structure

Valid `plan.md` must have:

- At least one phase heading (`## Phase N:`)
- At least one task per phase (`- [ ] Task description`)
- All tasks marked `[x]` if track is completed

## Auto-Fixable Issues

| Issue                            | Fix Action                           |
| -------------------------------- | ------------------------------------ |
| Missing `created_at` in metadata | Add current timestamp                |
| Status mismatch                  | Update metadata to match tracks.md   |
| Orphan track directory           | Prompt to add to tracks.md or delete |
| Missing `updated_at`             | Add current timestamp                |

## Non-Fixable Issues

These require manual intervention:

- Missing core files (run `/conductor-setup`)
- Missing required track files (spec.md, plan.md)
- Invalid JSON structure
- Empty plan (no phases/tasks)

---

## track_id Validation

The **directory name is the canonical source of truth** for track_id.

### Auto-Fix Behavior

State files with mismatched track_id are automatically corrected:

| File                   | Field      | Auto-Fix   |
| ---------------------- | ---------- | ---------- |
| `metadata.json`        | `track_id` | ✓ Auto-fix |
| `.track-progress.json` | `trackId`  | ✓ Auto-fix |
| `.fb-progress.json`    | `trackId`  | ✓ Auto-fix |

### Warn-Only Behavior

Content files with mismatched track_id in headers generate warnings but are NOT auto-fixed:

| File        | Location                   | Action       |
| ----------- | -------------------------- | ------------ |
| `design.md` | Header `# ... Track: <id>` | ⚠️ Warn only |
| `spec.md`   | Header `# ... Track: <id>` | ⚠️ Warn only |
| `plan.md`   | Header `# ... Track: <id>` | ⚠️ Warn only |

Content file mismatches often indicate a copied track that needs manual review.

### Validation Logic

```bash
# Directory name = source of truth
TRACK_ID=$(basename "$TRACK_DIR")

# Check each state file
for file in metadata.json .track-progress.json .fb-progress.json; do
  CURRENT=$(jq -r '.track_id // .trackId' "$TRACK_DIR/$file")
  if [[ "$CURRENT" != "$TRACK_ID" ]]; then
    # Auto-fix: update to match directory name
    # Log repair to metadata.json.repairs[]
  fi
done
```

---

## State File Validation

### State File Types

| File                   | Purpose                                   | Required     |
| ---------------------- | ----------------------------------------- | ------------ |
| `metadata.json`        | Track metadata (type, status, created_at) | ✓ Required   |
| `.track-progress.json` | Spec/plan generation progress             | Auto-created |
| `.fb-progress.json`    | Beads filing progress                     | Auto-created |
| `implement_state.json` | Implementation progress                   | Optional     |

### File Existence Matrix

| design.md | spec.md | plan.md | state files | Action                        |
| --------- | ------- | ------- | ----------- | ----------------------------- |
| ✗         | ✗       | ✗       | ✗           | SKIP + warn (empty directory) |
| ✓         | ✗       | ✗       | ✗           | PASS (design-only state)      |
| ✗         | ✓       | ✗       | any         | HALT (spec without plan)      |
| ✗         | ✗       | ✓       | any         | HALT (plan without spec)      |
| any       | ✓       | ✓       | ✗           | Auto-create state files       |
| any       | ✓       | ✓       | ✓           | Validate and PASS             |

### Auto-Create Conditions

When spec.md + plan.md exist but state files are missing, auto-create if:

1. **Content check**: Both files have content (size > 0)
2. **Age check**: Both files < 30 days old
3. **ID check**: No track_id mismatch in content file headers

If any pre-check fails: HALT with explanation.

### HALT Conditions

| Condition                            | Reason                    |
| ------------------------------------ | ------------------------- |
| Corrupted JSON                       | Cannot parse state files  |
| spec.md XOR plan.md                  | Invalid partial state     |
| Files > 30 days with missing state   | Risk of stale data        |
| track_id mismatch in content headers | May indicate copied track |

---

## Auto-Repair Actions

### Repairable Issues

| Issue                            | Repair Action                  |
| -------------------------------- | ------------------------------ |
| Missing state files              | Auto-create with defaults      |
| track_id mismatch in state files | Update to match directory name |
| Missing `created_at`             | Add current timestamp          |
| Missing `updated_at`             | Add current timestamp          |

### Non-Repairable Issues

| Issue                        | Manual Action Required                |
| ---------------------------- | ------------------------------------- |
| Corrupted JSON               | Restore from git or recreate          |
| spec.md XOR plan.md          | Create missing file or remove orphan  |
| Files > 30 days old          | Manual review and state file creation |
| track_id mismatch in content | Edit headers manually                 |

### Audit Trail

All repairs are logged to `metadata.json.repairs[]`:

```json
{
  "repairs": [
    {
      "timestamp": "2025-12-24T10:30:00Z",
      "action": "auto-fix",
      "field": "track_id",
      "from": "old-track_20251223",
      "to": "new-track_20251224",
      "by": "T-019b4cec-3f99-70ad-a0c6-617e57d5c0ad"
    }
  ]
}
```

- Maximum 10 entries retained (oldest trimmed)
- `by` field contains thread ID or "validation" for automated repairs

### Validation Modes

| Mode         | Flag         | Behavior                        |
| ------------ | ------------ | ------------------------------- |
| **Default**  | (none)       | Auto-repair + HALT on unfixable |
| **Diagnose** | `--diagnose` | Report only, no modifications   |

For detailed validation logic, see `skills/conductor/references/validation/track/checks.md`.
