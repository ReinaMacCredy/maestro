# Conductor Validation Workflow

This document defines the core validation logic for the Conductor framework.

## Validation Checklist

### 1. Core Files

| File | Required | Description |
|------|----------|-------------|
| `conductor/product.md` | ✓ | Product vision and goals |
| `conductor/tech-stack.md` | ✓ | Technology choices |
| `conductor/workflow.md` | ✓ | Development workflow |
| `conductor/tracks.md` | ✓ | Master track list |
| `conductor/product-guidelines.md` | Optional | Brand/style guidelines |
| `conductor/setup_state.json` | Optional | Setup resume state |

### 2. Track Structure

Each track in `conductor/tracks/<track_id>/` must contain:

| File | Required | Validation |
|------|----------|------------|
| `metadata.json` | ✓ | Valid JSON with: track_id, type, status, created_at |
| `spec.md` | ✓ | Requirements specification |
| `plan.md` | ✓ | Phased task list |
| `implement_state.json` | Optional | Valid JSON if present |

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
|-----------|---------------|-------|
| `[ ]` | `new` | ✓ |
| `[~]` | `in_progress` | ✓ |
| `[x]` | `completed` | ✓ |

### 6. Plan Structure

Valid `plan.md` must have:
- At least one phase heading (`## Phase N:`)
- At least one task per phase (`- [ ] Task description`)
- All tasks marked `[x]` if track is completed

## Auto-Fixable Issues

| Issue | Fix Action |
|-------|------------|
| Missing `created_at` in metadata | Add current timestamp |
| Status mismatch | Update metadata to match tracks.md |
| Orphan track directory | Prompt to add to tracks.md or delete |
| Missing `updated_at` | Add current timestamp |

## Non-Fixable Issues

These require manual intervention:
- Missing core files (run `/conductor-setup`)
- Missing required track files (spec.md, plan.md)
- Invalid JSON structure
- Empty plan (no phases/tasks)
