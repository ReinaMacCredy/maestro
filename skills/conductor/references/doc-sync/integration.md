# Conductor Integration

How doc-sync integrates with the Conductor workflow.

## Overview

Doc-sync integrates at two points:
1. **Auto-trigger** - Phase 7 in `/conductor-finish`
2. **Manual trigger** - `/doc-sync` command

### Data Sources

Doc-sync now extracts knowledge from **two sources**:
1. **Code Changes** - Git diff, file renames, function changes
2. **Thread History** - Amp threads that touched track files (NEW)

Both sources are merged by Oracle before applying updates.

---

## Phase 7 in /conductor-finish

### Position in Workflow

```
/conductor-finish phases:
  0. Pre-Flight Validation
  1. Thread Compaction
  2. Beads Compaction
  3. Knowledge Merge
  4. Context Refresh
  5. Archive
  6. CODEMAPS Regeneration
  7. Doc-Sync              ← NEW
```

### Trigger Conditions

Phase 7 runs when ALL of:
- Phase 6 (CODEMAPS) completed successfully
- Project has `.md` files with code references
- `--skip-doc-sync` flag NOT passed

### Skip Conditions

Skip Phase 7 if ANY of:
- `--skip-doc-sync` flag passed
- No markdown files found in project
- No code changes detected in track
- Track is documentation-only (no code files changed)

### Integration Code

```
function conductorFinishPhase7():
  # Check skip conditions
  if flags.skipDocSync:
    log("⏭️  Skipping doc-sync (--skip-doc-sync)")
    return
    
  log("📄 Phase 7: Doc-Sync")
  
  # === STEP 1: Code Scanner ===
  log("   1/4 Scanning code changes...")
  docs = scanForDocs()
  changes = detectChanges(track.beads, track.gitRange)
  
  # === STEP 2: Thread Extraction (NEW) ===
  log("   2/4 Extracting thread knowledge...")
  threads = find_thread(
    after: track.started_at,
    file: track.files
  )
  
  if threads.length > 0:
    # Parallel extraction with Task agents
    topics = extractTopicsParallel(threads)
    # Oracle synthesizes
    topics = oracleSynthesize(topics)
  else:
    topics = []
  
  # === STEP 3: Oracle Reconcile ===
  log("   3/4 Reconciling sources...")
  updates = oracleReconcile(
    code_changes: changes,
    thread_topics: topics,
    current_docs: docs
  )
  
  # === STEP 4: Apply Updates ===
  log("   4/4 Applying updates...")
  results = applyUpdates(updates)
  
  showSummary(results)
  log("✅ Phase 7: Doc-Sync complete")
```

See also:
- [extraction.md](extraction.md) - Thread extraction pipeline details
- [reconcile.md](reconcile.md) - Oracle reconciliation logic
- [prompts.md](prompts.md) - Prompt templates
- [mapping.md](mapping.md) - Doc target mapping rules

### State Tracking

Add to `finish_state.json`:

```json
{
  "phases": {
    "doc_sync": {
      "status": "complete",
      "started_at": "2025-12-27T10:00:00Z",
      "completed_at": "2025-12-27T10:00:05Z",
      "results": {
        "files_scanned": 8,
        "minor_updates": 6,
        "major_updates": 1
      }
    }
  }
}
```

---

## Manual /doc-sync Command

### Command Syntax

```
/doc-sync [options]

Options:
  --dry-run    Show proposed changes without applying
  --force      Apply all changes without prompts
  --verbose    Show detailed output
  --json       Output results as JSON
```

### Command Implementation

```
function docSyncCommand(options):
  # 1. Load skill
  loadSkill('doc-sync')
  
  # 2. Scan for docs
  log("🔍 Scanning for documentation...")
  docs = scanForDocs()
  log(f"   Found {docs.length} docs with code references")
  
  # 3. Detect changes
  log("📊 Detecting code changes...")
  changes = detectChanges()
  log(f"   Found {changes.length} changes")
  
  # 4. Check dry-run
  if options.dryRun:
    showDryRunPreview(docs, changes)
    log("\nRun without --dry-run to apply changes.")
    return
    
  # 5. Apply updates
  if options.force:
    results = applyAllUpdates(docs, changes)
  else:
    results = applyUpdatesWithPrompts(docs, changes)
    
  # 6. Show results
  if options.json:
    print(JSON.stringify(results))
  else:
    showSummary(results)
```

### Context Requirements

When `/doc-sync` runs, it needs:
- Access to git (for `git diff`)
- Access to beads (for closed issues context)
- Current track info (if in a track)

If NOT in a track:
```
⚠️  No active track found.
    Doc-sync will use git diff from last 10 commits.
    For better accuracy, run within a Conductor track.
```

---

## Flag Reference

### /conductor-finish flags

| Flag | Effect on Doc-Sync |
|------|-------------------|
| `--skip-doc-sync` | Skip Phase 7 entirely |
| `--force` | Auto-apply all changes in Phase 7 |

### /doc-sync flags

| Flag | Effect |
|------|--------|
| `--dry-run` | Preview only, no changes |
| `--force` | Apply all without prompts |
| `--verbose` | Detailed logging |
| `--json` | JSON output format |

---

## Error Handling

### Phase 7 Errors

If doc-sync fails during `/conductor-finish`:

```
⚠️  Phase 7 (Doc-Sync) encountered errors:
    - README.md: Could not parse code block at line 45
    
Continuing to Archive phase...
(Doc-sync errors are non-blocking)
```

Doc-sync errors are **non-blocking** - they don't stop the finish workflow.

### Manual Command Errors

If `/doc-sync` fails:

```
❌ Doc-Sync failed:
    Error: Git repository not found
    
Ensure you're in a git repository and try again.
```

### Recovery

All doc-sync changes are applied via standard file edits:
- Undo with `git checkout -- <file>`
- Or use editor undo

---

## Integration with Other Skills

### With Beads

Doc-sync reads from `.fb-progress.json` to get:
- Closed issues in current track
- Feature names and descriptions
- File associations

### With CODEMAPS

Doc-sync runs AFTER CODEMAPS regeneration:
- CODEMAPS updates architecture docs
- Doc-sync updates user-facing docs (README, tutorials)

No conflicts - different targets.

### With Continuity

Doc-sync results are included in session handoff:

```markdown
## Doc-Sync Results

- Updated README.md (3 path changes)
- Added Authentication section
- Skipped API examples update (user choice)
```

---

## Examples

### Example 1: Auto-trigger in /conductor-finish

```
$ /conductor-finish auth-feature_20251227

Phase 0: Pre-Flight Validation ✅
Phase 1: Thread Compaction ✅
Phase 2: Beads Compaction ✅
Phase 3: Knowledge Merge ✅
Phase 4: Context Refresh ✅
Phase 5: Archive ✅
Phase 6: CODEMAPS Regeneration ✅
Phase 7: Doc-Sync
  📄 Scanning docs... 5 found
  📊 Detecting changes... 3 found
  ✅ Auto-updated: 2 path changes
  ❓ New feature detected: Add section to README.md? [Y/n] Y
  ✅ Added "Authentication" section
Phase 7: Doc-Sync ✅

Track archived successfully.
```

### Example 2: Manual /doc-sync --dry-run

```
$ /doc-sync --dry-run

📄 Doc-Sync Preview
═══════════════════

Would update: README.md
  Line 15: src/utils.ts → src/helpers.ts
  Line 42: initApp() → initialize()

Would prompt: Add "Authentication" section?

Run without --dry-run to apply.
```

---

*See [SKILL.md](../../SKILL.md) for full workflow.*
