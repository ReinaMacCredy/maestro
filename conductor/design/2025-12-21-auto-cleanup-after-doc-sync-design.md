# Auto-Cleanup After Doc-Sync

## Overview

Integrate automatic bead cleanup into doc-sync workflow. After doc-sync extracts knowledge from closed issues into AGENTS.md, compact and cleanup old beads to keep database bounded.

## Design

### Trigger

Runs automatically after doc-sync completes successfully.

### Flow

```
doc-sync completes
    ↓
1. COMPACT all synced issues
   bd compact --analyze --json
   For each candidate:
       Generate AI summary from issue content
       bd compact --apply --id <id> --summary <summary>
   Report: "Compacted {count} issues"
    ↓
2. CLEANUP (if closed > 150)
   bd count --status closed --json → closed_count
   If closed_count > 150:
       excess = closed_count - 150
       bd cleanup --older-than 0 --limit <excess> --force
       Report: "Cleaned up {excess} oldest issues (threshold: 150)"
    ↓
3. SYNC
   bd sync
```

### Why This Order

1. **Compact first** - Ensures every issue gets a summary before deletion
2. **Cleanup second** - Removes oldest issues (now have summaries in git)
3. **High volume safe** - Works when plans create many issues (oldest may be 3-4 days old)

### Thresholds

| Setting | Value | Rationale |
|---------|-------|-----------|
| Max closed issues | 150 | ~3-6 months history |
| Cleanup target | Oldest first | Preserve recent work |
| Open issues | Unlimited | Never touch active work |

### Recovery

- Compacted issues retain summaries in beads
- Full content recoverable via `bd restore <id>` (from git history)
- Knowledge preserved in AGENTS.md via doc-sync

## Changes Required

### File: skills/doc-sync/SKILL.md

Add new section after "Step 6: Show diff for review":

```markdown
## Step 7: Auto-Cleanup

After successful sync:

1. **Compact synced issues**
   ```bash
   bd compact --analyze --json
   ```
   For each candidate, generate summary and apply:
   ```bash
   bd compact --apply --id <id> --summary <generated-summary>
   ```

2. **Cleanup if over threshold**
   ```bash
   bd count --status closed --json
   ```
   If count > 150:
   ```bash
   bd cleanup --older-than 0 --limit <excess> --force
   ```

3. **Sync changes**
   ```bash
   bd sync
   ```

Report: "Compacted X issues. Cleaned up Y oldest (threshold: 150)."
```

## Acceptance Criteria

- [ ] doc-sync skill updated with Step 7: Auto-Cleanup
- [ ] Compact runs on all synced issues
- [ ] Cleanup triggers only when closed > 150
- [ ] Oldest issues deleted first
- [ ] Open issues never affected
- [ ] Final `bd sync` commits changes
