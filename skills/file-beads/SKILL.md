---
name: file-beads
version: "2.0.0"
description: File detailed Beads epics and issues from a plan
argument-hint: <plan-description-or-context>
---

# File Beads Epics and Issues from Plan

Convert a plan into Beads epics and issues using **parallel subagents** for speed, with checkpointing for resume capability.

## Phase 0: Initialize & Check for Resume

### 0.1 Determine Track Context

Review the plan context: `$ARGUMENTS`

If no plan provided, check:
- Recent `/conductor-design` output in current context
- `conductor/tracks/` for design.md, spec.md, and plan.md files

**Extract track ID** from plan path (e.g., `conductor/tracks/auth_20251223/plan.md` → `auth_20251223`).

### 0.2 Check for Existing Progress

Look for `.fb-progress.json` in track directory:

```bash
cat conductor/tracks/<track_id>/.fb-progress.json 2>/dev/null
```

**If progress file exists:**
1. Read `status` field
2. **If `status: "complete"`:** Announce "Beads already filed for this track. Use `--force` to re-file." HALT.
3. **If `status: "in_progress"` or `"failed"`:** 
   - Read `resumeFrom` field to identify checkpoint
   - Read `epics` array to get already-created epics
   - Announce: "Resuming from checkpoint: <resumeFrom>"
   - Skip to appropriate phase
4. **If `status: "pending"`:**
   - Fresh start, proceed to Phase 1

### 0.3 Validate Required State Files

**MANDATORY:** Verify all prerequisite state files exist before proceeding.

```bash
TRACK_PATH="conductor/tracks/<track_id>"

# Check required files
MISSING=""
[ ! -f "$TRACK_PATH/metadata.json" ] && MISSING="$MISSING metadata.json"
[ ! -f "$TRACK_PATH/.track-progress.json" ] && MISSING="$MISSING .track-progress.json"
[ ! -f "$TRACK_PATH/.fb-progress.json" ] && MISSING="$MISSING .fb-progress.json"
[ ! -f "$TRACK_PATH/plan.md" ] && MISSING="$MISSING plan.md"

if [ -n "$MISSING" ]; then
  echo "❌ Cannot file beads. Missing:$MISSING"
  echo ""
  echo "Fix: Run /conductor-newtrack <track_id> first"
  exit 1
fi

# Validate JSON files
for FILE in metadata.json .track-progress.json .fb-progress.json; do
  if ! jq empty "$TRACK_PATH/$FILE" 2>/dev/null; then
    echo "❌ Corrupted: $FILE"
    exit 1
  fi
done

# Auto-fix trackId mismatch
DIR_NAME=$(basename "$TRACK_PATH")
for FILE in .fb-progress.json; do
  CURRENT_ID=$(jq -r '.trackId // empty' "$TRACK_PATH/$FILE")
  if [ -n "$CURRENT_ID" ] && [ "$CURRENT_ID" != "$DIR_NAME" ]; then
    echo "ℹ️ Auto-fixing trackId in $FILE: $CURRENT_ID → $DIR_NAME"
    jq --arg id "$DIR_NAME" '.trackId = $id' "$TRACK_PATH/$FILE" > tmp && mv tmp "$TRACK_PATH/$FILE"
  fi
done

echo "✓ State files validated"
```

**If any file missing or corrupted:** HALT. Do NOT auto-create.

### 0.4 Update Progress to In-Progress

Update `.fb-progress.json` to mark start:

```bash
jq --arg time "<current-timestamp>" \
   --arg thread "<current-thread-id>" \
   '.status = "in_progress" | .startedAt = $time | .threadId = $thread | .resumeFrom = "phase1"' \
   "conductor/tracks/<track_id>/.fb-progress.json" > tmp && mv tmp "conductor/tracks/<track_id>/.fb-progress.json"
```

## Phase 1: Analyze Plan

**Identify for each epic:**

| Field | Description |
|-------|-------------|
| Epic title | Clear workstream name |
| Child tasks | Individual issues under this epic |
| Intra-epic deps | Dependencies within the epic |
| Cross-epic hints | Tasks that depend on other epics (by name, not ID) |
| Priority | 0-4 scale |

**Update checkpoint:** Set `resumeFrom: "phase2"`.

## Phase 2: Create Epics First (Sequential)

Create all epics FIRST to get stable IDs before parallelizing child issues.

**For each epic:**

```bash
bd create "Epic: Authentication" -t epic -p 1 --json
# Returns: {"id": "bd-1", ...}
```

**After EACH epic creation, update progress file:**

```json
{
  "trackId": "<track_id>",
  "status": "in_progress",
  "epics": [
    {
      "id": "bd-1",
      "title": "Epic: Authentication",
      "status": "created",
      "createdAt": "<timestamp>",
      "reviewed": false
    },
    {
      "id": "bd-2",
      "title": "Epic: Database Layer",
      "status": "created",
      "createdAt": "<timestamp>",
      "reviewed": false
    }
  ],
  "resumeFrom": "phase2_epic_3",
  ...
}
```

This ensures resume after interruption won't create duplicate epics.

**Update checkpoint:** Set `resumeFrom: "phase3"`.

## Phase 3: Parallel Dispatch for Child Issues (BATCHED)

Dispatch subagents in **batches of 5** to avoid rate limits.

### 3.1 Batch Calculation

```
If 12 epics:
  Batch 1: epics 1-5 (parallel)
  Batch 2: epics 6-10 (parallel)
  Batch 3: epics 11-12 (parallel)
```

### 3.2 Subagent Prompt Template

```markdown
Fill Epic: "<EPIC_TITLE>" (ID: <EPIC_ID>)

## Your Task
Create all child issues for this epic. The epic already exists.

## Epic Context
<PASTE_EPIC_SECTION_FROM_PLAN>

## Steps

1. For each task, create an issue with parent dependency:
   ```bash
   bd create "<task title>" -t <type> -p <priority> --deps <EPIC_ID> --json
   ```
   
   Include in each issue:
   - Clear action-oriented title
   - Acceptance criteria
   - Technical notes if relevant

2. Link intra-epic dependencies:
   ```bash
   bd dep add <child> <blocker> --type blocks --json
   ```

## Return Format

**CRITICAL: Validate your JSON before returning.**

Check that your response:
- Contains ONLY the JSON object (no markdown, no explanation)
- Has all required fields: epicId, epicTitle, issues, crossEpicDeps
- Each issue has: id, title, deps (array)
- crossEpicDeps items have: issueId, needsLinkTo

Return ONLY this JSON:
```json
{
  "epicId": "<EPIC_ID>",
  "epicTitle": "<title>",
  "issues": [
    {"id": "<issue-id>", "title": "...", "deps": ["<dep-id>"]}
  ],
  "crossEpicDeps": [
    {"issueId": "<issue-id>", "needsLinkTo": "<epic or task name>"}
  ]
}
```
```

### 3.3 Batch Dispatch

> **IMPORTANT:** You MUST actually invoke the Task tool. Do not just describe or write about dispatching — execute it.

**For each batch:**

```
// Dispatch batch (max 5 at once)
Task(description: "Fill Epic: Authentication (bd-1)", prompt: <above template>)
Task(description: "Fill Epic: Database Layer (bd-2)", prompt: <above template>)
Task(description: "Fill Epic: API Endpoints (bd-3)", prompt: <above template>)
// ... up to 5 parallel
```

**Wait for batch to complete.**

### 3.4 Handle Subagent Results

For each subagent result:

1. **Parse JSON response**
2. **If parse fails:**
   - **Retry once** with hint: "Your previous response was not valid JSON. Error: <parse-error>. Please return ONLY the JSON object."
   - **If retry fails:** Log warning, handle this epic in main agent context (fallback)
3. **If parse succeeds:**
   - Update progress file with issues
   - Record cross-epic dependencies for Phase 4

**Update progress file after each batch:**

```json
{
  "status": "in_progress",
  "resumeFrom": "phase3_batch_2",
  "epics": [...],
  "issues": ["bd-4", "bd-5", "bd-6", ...],
  "lastBatchCompleted": 1
}
```

### 3.5 Proceed to Next Batch

Repeat 3.3-3.4 until all batches complete.

**Update checkpoint:** Set `resumeFrom: "phase4"`.

## Phase 4: Collect & Link Cross-Epic Dependencies

When ALL subagents return:

1. Parse JSON results from each subagent
2. Build ID lookup table:
   ```
   "Authentication" → bd-1
   "Setup user table" → bd-5
   "Create auth middleware" → bd-8
   ```

3. Resolve cross-epic dependencies:
   ```bash
   bd dep add <from> <to> --type blocks --json
   ```

4. **Detect cross-track dependencies:**
   - If a task references another conductor track (e.g., "depends on api_20251223")
   - Record in `crossTrackDeps` array:
     ```json
     {"from": "bd-3", "to": "api_20251223:bd-7"}
     ```
   - Attempt to update both tracks' progress files

**Update checkpoint:** Set `resumeFrom: "phase5"`.

## Phase 5: Verify & Summarize

Run verification:

```bash
bd list --json
bd ready --json
```

Check:
- All epics have child issues
- No dependency cycles
- Some issues are ready (unblocked)

### Update Final Progress File

```json
{
  "trackId": "<track_id>",
  "status": "complete",
  "startedAt": "<start-timestamp>",
  "completedAt": "<end-timestamp>",
  "threadId": "<thread-id>",
  "lastVerified": "<timestamp>",
  
  "epics": [
    {
      "id": "bd-1",
      "title": "Epic: Authentication",
      "status": "complete",
      "createdAt": "<timestamp>",
      "reviewed": false
    }
  ],
  
  "issues": ["bd-4", "bd-5", "bd-6", ...],
  
  "crossTrackDeps": [
    {"from": "bd-3", "to": "api_20251223:bd-7"}
  ],
  
  "resumeFrom": null,
  "lastError": null
}
```

### Summary Format

Present to user:

```
## Filed Beads Summary

**Epics created:** 3
**Issues created:** 12

| Epic | Issues | Ready |
|------|--------|-------|
| Authentication | 4 | 2 |
| Database Layer | 5 | 1 |
| API Endpoints | 3 | 0 (blocked) |

**Start with:** bd-5 (Setup user table), bd-8 (Init auth config)

**Cross-epic deps linked:** 2
```

### After Completion

After parallel agents finish filing beads:

1. Summarize what was created (epic ID, issue count)
2. Say: "Beads filed. Say `rb` to review and refine."

## Priority Guide

| Priority | Use For |
|----------|---------|
| 0 | Critical path blockers, security |
| 1 | Core functionality, high value |
| 2 | Standard work (default) |
| 3 | Nice-to-haves, polish |
| 4 | Backlog, future |

## Error Recovery

| Error | Recovery |
|-------|----------|
| Subagent returns invalid JSON | Retry once with hint, then fallback to main agent |
| Batch interrupted | Resume from `resumeFrom` field in progress file |
| Epic creation fails | Log error, continue with remaining epics |
| Dependency cycle detected | Log warning, skip that dependency |
| Cross-track reference fails | Log in progress file, suggest manual linking |

## Why This Approach?

- **Epics first (sequential)** — Prevents ID collisions; epic IDs are stable before parallelization
- **Child issues (parallel, batched)** — Each subagent works on a different epic, no conflicts, rate-limit safe
- **Checkpointing** — Resume after any interruption without duplicate work
- **JSON validation** — Catches subagent formatting issues early
- **Context hygiene** — `bd create` output stays in subagent contexts
- **Reliable linking** — Cross-epic dependencies resolve correctly since all epic IDs known upfront
