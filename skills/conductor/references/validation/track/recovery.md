# Track Validation Recovery Guide

Troubleshooting guide for common track integrity issues.

## Quick Fixes

| Issue | Fix |
|-------|-----|
| Missing metadata.json | Run `/conductor-validate <track_id>` to auto-create |
| Missing .track-progress.json | Run `/conductor-validate <track_id>` to auto-create |
| Missing .fb-progress.json | Run `/conductor-validate <track_id>` to auto-create |
| track_id mismatch in state files | Auto-fixed by validation |
| track_id mismatch in content files | Manual edit required |
| Corrupted JSON | Restore from git or recreate |
| Stale in_progress status | Use diagnose mode, then reset or resume |

## Recovery Scenarios

### Scenario 1: Copied Track

**Symptoms:**
- track_id in files doesn't match directory name
- Usually happens after `cp -r` or manual copy

**Recovery:**
```bash
# Option A: Let validation auto-fix state files
/conductor-validate <new_track_id>

# Option B: Rename directory to match content
mv conductor/tracks/<new_name> conductor/tracks/<original_name>
```

### Scenario 2: Interrupted Workflow

**Symptoms:**
- `.fb-progress.json.status = "in_progress"`
- Beads partially filed
- Thread that started it no longer active

**Recovery:**
```bash
# 1. Diagnose current state
/conductor-validate <track_id> --diagnose

# 2. Check what was filed
bd list --labels "track:<track_id>"

# 3. Choose action:
#    Resume - continue from checkpoint
#    Reset  - clear progress and start fresh
```

**Reset command:**
```bash
jq '.status = "pending" | .resumeFrom = "phase1" | .startedAt = null | .threadId = null' \
  "$TRACK_DIR/.fb-progress.json" > "$TRACK_DIR/.fb-progress.json.tmp"
mv "$TRACK_DIR/.fb-progress.json.tmp" "$TRACK_DIR/.fb-progress.json"
```

### Scenario 3: Manual Track Creation

**Symptoms:**
- spec.md and plan.md exist
- No state files (metadata.json, .track-progress.json, .fb-progress.json)

**Recovery:**
```bash
# Auto-create state files (if pre-checks pass)
/conductor-validate <track_id>

# If auto-create fails (files > 30 days old), manual creation:
# See snippets.md for templates
```

### Scenario 4: Orphan Beads

**Symptoms:**
- Beads reference track that no longer exists
- `.fb-progress.json` lists beads but they're deleted

**Recovery:**
```bash
# 1. Find orphan beads
bd list --labels "track:<track_id>"

# 2. Either:
#    - Re-link to new track
#    - Close as obsolete
bd close <bead_id> --reason "Orphaned from deleted track"
```

### Scenario 5: spec.md XOR plan.md

**Symptoms:**
- HALT: spec.md exists without plan.md (or vice versa)
- Invalid partial state

**Recovery:**
```bash
# Option A: Create the missing file
# spec.md exists → create plan.md from spec
# plan.md exists → create spec.md (unusual)

# Option B: Remove the orphan file
rm "$TRACK_DIR/spec.md"  # or plan.md

# Option C: Move to design-only state
# Keep only design.md, remove spec/plan
```

## Diagnose Mode

Full diagnostic report without making changes:

```bash
/conductor-validate <track_id> --diagnose
```

**Output includes:**
- File listing with sizes and dates
- State file contents (pretty-printed)
- All detected issues with severity
- Suggested fixes for each issue

## Audit Trail

All auto-repairs are logged to `metadata.json.repairs[]`:

```bash
# View repair history
jq '.repairs' conductor/tracks/<track_id>/metadata.json
```

**Example output:**
```json
[
  {
    "timestamp": "2024-12-24T10:30:00Z",
    "action": "auto-fix",
    "field": "track_id",
    "from": "old-track_20241223",
    "to": "new-track_20241224",
    "by": "T-019b4cec-3f99-70ad-a0c6-617e57d5c0ad"
  },
  {
    "timestamp": "2024-12-24T10:25:00Z",
    "action": "auto-create",
    "field": "file",
    "from": null,
    "to": ".fb-progress.json",
    "by": "validation"
  }
]
```

## Non-Recoverable Issues

These require manual intervention:

| Issue | Why Not Auto-Fixable | Manual Action |
|-------|---------------------|---------------|
| Corrupted JSON | Can't parse to fix | Restore from git or recreate |
| Content file track_id mismatch | May indicate copy error | Edit headers manually |
| Files > 30 days old | Risk of stale data | Review and create manually |
| Missing spec+plan together | Fundamental design issue | Create via /conductor-newtrack |

## Prevention

Best practices to avoid track integrity issues:

1. **Always use commands** - Use `/conductor-newtrack`, not manual file creation
2. **Don't copy tracks** - Create new tracks; reference old ones if needed
3. **Complete workflows** - If interrupted, resume or explicitly reset
4. **Commit state files** - Include `.track-progress.json` and `.fb-progress.json` in git
5. **Run validation** - Before `/conductor-implement` or `/conductor-finish`
