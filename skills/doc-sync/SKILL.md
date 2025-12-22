---
name: doc-sync
version: "1.0.0"
description: "Sync AGENTS.md files by extracting knowledge from completed work threads. Use after closing an epic or manually with doc-sync command."
---

# Doc Sync

Automatically sync AGENTS.md files by extracting knowledge from Amp threads linked in closed beads issues.

## Trigger Phrases

- `doc-sync`
- `/doc-sync`
- `sync docs`
- `update agents.md from threads`

## Prerequisites

**Thread URLs must be saved in beads comments** (via `/conductor-implement`):
- When claiming: `bd comment <id> "THREAD: <url>"`
- When completing: `bd comment <id> "IN_PROGRESS: ... NEXT: ... THREAD: <url>"`

> **Note:** Comments are used instead of notes for multi-agent concurrency safety (append-only operations).

## Workflow

### Phase 1: Get Closed Issues

```bash
bd list --status closed --json
```

Filter to relevant issues:
- If epic specified: Get child issues of that epic
- If no epic: Get issues closed in last session/timeframe

### Phase 2: Extract Thread URLs

For each closed issue:
1. Get comments: `bd comments <issue-id> --json`
2. Parse comments for `THREAD: https://ampcode.com/threads/T-xxx` (or localhost URLs)
3. Collect unique thread IDs

**Fallback** if no threads in comments:
1. Check notes field (legacy format)
2. Use `find_thread file:<changed-files-from-notes>`

### Phase 3: Read Threads (Parallel)

For each thread ID, use read_thread to extract:
- API changes or new patterns introduced
- Gotchas or edge cases discovered
- Key decisions and rationale
- Commands or conventions established

```
read_thread(
  threadID: "T-xxx",
  goal: "Extract: API changes, patterns, gotchas, decisions, commands"
)
```

### Phase 4: Identify Target AGENTS.md

For each issue with extracted findings:
1. Get changed files from notes
2. Walk up directory tree from each file to find nearest AGENTS.md
3. Group findings by target AGENTS.md file

**Priority:**
- Module-level AGENTS.md (e.g., `skills/beads/AGENTS.md`)
- Project root AGENTS.md
- Skip if no AGENTS.md found (ask user if should create)

### Phase 5: Edit AGENTS.md Files

For each target AGENTS.md:
1. Read current content
2. Identify appropriate sections to update:
   - Commands → add to Build/Test Commands section
   - Patterns → add to Code Style section
   - Architecture changes → update Architecture section
3. Merge new info without duplicating existing content
4. Edit file with updates

### Phase 6: User Review

1. Show git diff of all changes:
   ```bash
   git diff -- "**/AGENTS.md"
   ```
2. Wait for user confirmation
3. If approved → commit changes and proceed to Phase 7
4. If rejected → abort workflow

### Phase 7: Auto-Cleanup

After successful commit, automatically maintain beads database.

> **Note:** Cleanup runs after Phases 1-6 complete, so deleted issues' knowledge is already preserved in AGENTS.md files. No context is lost.

**1. Compact remaining issues**

Generate AI summaries for closed issues that lack them:

```bash
bd compact --analyze --json
```

For each candidate returned:
```bash
bd compact --apply --id <id> --summary "<generated-summary>"
```

Generate summary from issue content (title, description, notes, thread findings).

**2. Cleanup if over threshold**

Check closed issue count:
```bash
bd count --status closed --json
```

If count > 150, remove excess oldest issues:
```bash
excess=$((closed_count - 150))
bd cleanup --older-than 0 --limit "$excess" --force
```

**3. Sync changes**

Commit beads state:
```bash
bd sync
```

**Report:** `"Compacted X issues. Cleaned up Y oldest (threshold: 150)."`

| Setting | Value | Rationale |
|---------|-------|-----------|
| Max closed | 150 | ~3-6 months history |
| Cleanup target | Oldest first | Preserve recent work |
| Open issues | Never touched | Active work protected |

**Recovery:** Compacted issues retain summaries. Full content via `bd restore <id>` from git history.

## Output Format

```
DOC-SYNC: <epic-or-scope>

THREADS FOUND: <count>
- T-xxx: <brief description>
- T-yyy: <brief description>

EXTRACTED:
- [commands] <new command discovered>
- [pattern] <new pattern established>
- [decision] <key decision made>

TARGETS:
- skills/beads/AGENTS.md → +2 commands, +1 pattern
- AGENTS.md → +1 architecture note

[Shows git diff]

REVIEW: Confirm changes? (y/n)
```

## Edge Cases

| Case | Handling |
|------|----------|
| No threads in comments | Check notes (legacy), then use `find_thread file:<files>` |
| AGENTS.md doesn't exist | Ask user if should create new file |
| Thread not accessible | Log warning, continue with remaining threads |
| Conflicting info between threads | Prefer most recent thread (by timestamp) |
| No changes to make | Report "No new documentation needed" |

## Examples

### Manual sync after epic

```
User: doc-sync for the auth epic
Agent: [Finds closed issues under auth epic]
       [Extracts 3 thread URLs from notes]
       [Reads threads, finds 2 new patterns and 1 command]
       [Updates skills/auth/AGENTS.md]
       [Shows diff for review]
```

### Auto-triggered after bd close

When closing an epic, agent automatically:
1. Detects epic has closed child issues with thread URLs
2. Triggers doc-sync workflow
3. Presents diff for user review before commit
