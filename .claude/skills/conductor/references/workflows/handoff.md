# Unified Handoff Workflow

Reference for `/conductor-handoff` command - session continuity with Beads sync and progress tracking.

## Command Syntax

```
/conductor-handoff              # Auto-detect mode (default)
/conductor-handoff create       # Force CREATE mode
/conductor-handoff resume       # Force RESUME mode
/conductor-handoff resume <path|track>
```

**Aliases:**
- `/create_handoff` → `/conductor-handoff create`
- `/resume_handoff` → `/conductor-handoff resume`

---

## Auto-Detect Logic

```
IF session_first_message AND recent_handoff_exists(<7d)
  → RESUME mode
ELSE
  → CREATE mode
```

---

## CREATE Mode (9 Steps)

### Step 1: Detect Context

```
IF bound_track in session state
  → track_id = bound_track
  → handoff_dir = conductor/handoffs/<track_id>/

ELSE IF active track detected (metadata.json.status != archived)
  → track_id = active track
  → handoff_dir = conductor/handoffs/<track_id>/

ELSE
  → track_id = "general"
  → handoff_dir = conductor/handoffs/general/
```

### Step 1a: Parallel Worker Check ⭐ NEW

Before creating handoff, check for running parallel workers:

```python
parallel_state_path = "conductor/tracks/<track_id>/parallel_state.json"

if file_exists(parallel_state_path):
    state = read_json(parallel_state_path)
    active_workers = [w for w in state["workers"] if w["status"] == "running"]
    
    if active_workers:
        prompt_user(f"""
⚠️ Parallel workers running: {[w['name'] for w in active_workers]}

[A] Wait for completion
[B] Handoff anyway (include worker state)
[C] Cancel
""")
        
        if choice == "A":
            wait_for_workers()
        elif choice == "B":
            include_worker_state = True
        else:
            return  # Cancel handoff
```

### Step 2: Gather Metadata

```bash
# Git information
git_commit=$(git rev-parse --short=7 HEAD 2>/dev/null || echo "unknown")
git_branch=$(git branch --show-current 2>/dev/null || echo "unknown")

# Timestamp with milliseconds
timestamp=$(date -u +"%Y-%m-%dT%H:%M:%S.%3NZ")

# Bead ID (for epic triggers)
if [ "$trigger" = "epic-start" ] || [ "$trigger" = "epic-end" ]; then
  bead_id=$(jq -r '.beads.epicId' "conductor/tracks/${track_id}/metadata.json")
fi

# Validation snapshot
validation=$(jq '.validation' "conductor/tracks/${track_id}/metadata.json" 2>/dev/null)
```

### Step 3: Scan for Secrets

**Hardcoded patterns:**
```regex
sk-[a-zA-Z0-9]{20,}                           # OpenAI
ghp_[a-zA-Z0-9]{36}                           # GitHub PAT classic
github_pat_[a-zA-Z0-9]{22}_[a-zA-Z0-9]{59}   # GitHub PAT fine-grained
AKIA[0-9A-Z]{16}                              # AWS Access Key
-----BEGIN.*PRIVATE KEY-----                  # Private Keys
```

**On detection:**
```
⚠️ Potential secret detected in handoff content:
   Pattern: sk-******* (OpenAI API Key)
   
[P]roceed anyway  [A]bort
```

### Step 4: Send to Agent Mail

**Primary storage** - send to Agent Mail first for FTS5 search.

```python
if agent_mail_available:
    message = {
        "project_key": absolute_workspace_path,
        "sender_name": agent_name,
        "to": ["Human"],
        "subject": f"[HANDOFF:{trigger}] {track_id} - {brief_context}",
        "body_md": format_handoff_body(context, changes, learnings, next_steps),
        "thread_id": f"handoff-{track_id}",
        "importance": "high" if trigger in AUTO_TRIGGERS else "normal"
    }
    result = send_message(**message)
```

**Auto-triggers (high importance):** `design-end`, `epic-start`, `epic-end`, `pre-finish`

**Fallback:** If Agent Mail unavailable, log warning and continue to markdown.

### Step 5: Beads Sync ⭐ NEW

Sync handoff context to Beads for compaction-proof resumability:

```bash
if bd_available; then
    epic_id=$(jq -r '.beads.epicId' "conductor/tracks/${track_id}/metadata.json")
    
    # Calculate progress
    completed=$(bd list --parent=$epic_id --status=closed --json | jq 'length')
    total=$(bd list --parent=$epic_id --json | jq 'length')
    progress=$((completed * 100 / total))
    
    # Structured notes
    notes="COMPLETED: Tasks 1-${completed} (${progress}% of track)
KEY DECISIONS: ${decisions}
IN PROGRESS: ${current_task}
NEXT: ${next_task}
BLOCKER: ${blocker:-none}
HANDOFF: Section ${section_count} saved at ${handoff_path}"
    
    bd update "$epic_id" --notes "$notes"
    bd sync
fi
```

**Fallback:** If `bd` unavailable, log warning and continue.

### Step 6: Write Markdown File

**Filename format:**
```
YYYY-MM-DD_HH-MM-SS-mmm_<track>_<trigger>.md
```

**Collision handling:** Append `-1`, `-2`, etc. if exists.

**Template:** See [template.md](../handoff/template.md)

### Step 7: Update metadata.json.handoff ⭐ NEW

```bash
# Calculate progress
completed=$(grep -c '\[x\]' "conductor/tracks/${track_id}/plan.md" || echo 0)
total=$(grep -c '\[ \]\|\[x\]\|\[~\]' "conductor/tracks/${track_id}/plan.md" || echo 1)
progress=$((completed * 100 / total))

# Update metadata.json
jq --arg ts "$timestamp" \
   --arg trigger "$trigger" \
   --arg bead_id "$bead_id" \
   --arg phase "$current_phase" \
   --argjson completed "$completed" \
   --argjson total "$total" \
   --arg file "$filename" \
   --argjson progress "$progress" \
   '.handoff.status = "handed_off" |
    .handoff.section_count += 1 |
    .handoff.progress_percent = $progress |
    .handoff.last_handoff = $ts |
    .handoff.history += [{
      "section": (.handoff.section_count),
      "timestamp": $ts,
      "trigger": $trigger,
      "bead_id": $bead_id,
      "phase_at_handoff": $phase,
      "tasks_completed": $completed,
      "tasks_total": $total,
      "file": $file
    }]' \
   "conductor/tracks/${track_id}/metadata.json" > tmp.$$ && mv tmp.$$ "conductor/tracks/${track_id}/metadata.json"
```

### Step 8: Update Index

```bash
echo "| ${timestamp_short} | ${trigger} | ${bead_id:-"-"} | ${summary} | [→](./${filename}) |" >> "${handoff_dir}/index.md"
```

### Step 9: Touch Activity Marker

```bash
touch conductor/.last_activity
```

---

## RESUME Mode (9 Steps)

### Step 1: Parse Input

```
IF argument is file path
  → mode = "explicit", handoff_path = argument

ELSE IF argument matches track pattern (e.g., "auth-system_20251229")
  → mode = "track", track_id = argument

ELSE (no argument)
  → mode = "discover"
```

### Step 2: Agent Mail Lookup (Primary)

```python
if agent_mail_available:
    thread_id = f"handoff-{track_id}" if mode == "track" else None
    
    if thread_id:
        summary = summarize_thread(
            project_key=absolute_workspace_path,
            thread_id=thread_id,
            include_examples=True,
            llm_mode=True
        )
        if summary:
            context_source = "agent_mail"
            handoff_context = summary
```

**Fallback:** If unavailable or no results, proceed to Step 3.

### Step 3: File Discovery (Fallback)

**Mode: discover**
```
1. Scan conductor/handoffs/*/
2. Find most recent handoff per track
3. Sort by timestamp
4. IF single track → auto-select
5. ELSE → present list for selection
```

**Mode: track**
```
handoff_dir = conductor/handoffs/<track_id>/
Find most recent *.md (excluding index.md)
```

### Step 4: Load Handoff Content

Parse YAML frontmatter + 4 sections (Context, Changes, Learnings, Next Steps).

### Step 5: Beads Context ⭐ NEW

Load current beads state for progress tracking:

```bash
if bd_available; then
    epic_id=$(jq -r '.beads.epicId' "conductor/tracks/${track_id}/metadata.json")
    
    # Show epic details
    bd show "$epic_id"
    
    # Get ready tasks
    ready_tasks=$(bd ready --json | jq -r '.[] | select(.parent == "'$epic_id'") | .title')
    
    # Calculate and display progress
    completed=$(bd list --parent=$epic_id --status=closed --json | jq 'length')
    total=$(bd list --parent=$epic_id --json | jq 'length')
    progress=$((completed * 100 / total))
    
    echo "📊 Progress: ${progress}% (${completed}/${total} tasks)"
    echo "🎯 Ready tasks: ${ready_tasks}"
fi
```

### Step 6: Validate State

**Git validation:**
```bash
current_branch=$(git branch --show-current)
handoff_branch=$(yq '.git_branch' handoff.md)

if [ "$current_branch" != "$handoff_branch" ]; then
    echo "⚠️ Branch mismatch: was ${handoff_branch}, now ${current_branch}"
fi
```

**Staleness check:** Warn if handoff > 7 days old.

### Step 7: Present Analysis

```
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
📋 Resuming: auth-system_20251229 | epic-end
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

🕐 Created: 2 hours ago
🔀 Branch: feat/auth-system @ def456a
📊 Progress: 45% (5/12 tasks)

## Context
[...]

## Next Steps
1. [ ] Start E2: Login endpoint
2. [ ] Wire JWT module to login handler
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
```

### Step 8: Create TodoWrite Items

```python
todos = []
for i, step in enumerate(next_steps):
    todos.append({
        "id": f"resume-{i+1}",
        "content": step,
        "status": "todo"
    })
todo_write(todos)
```

### Step 9: Update metadata.json.handoff.status ⭐ NEW

```bash
jq '.handoff.status = "active"' \
   "conductor/tracks/${track_id}/metadata.json" > tmp.$$ && mv tmp.$$ "conductor/tracks/${track_id}/metadata.json"

touch conductor/.last_activity
```

---

## 6 Triggers

| Trigger | When | Auto | Integration Point |
|---------|------|:----:|-------------------|
| `design-end` | After `/conductor-newtrack` | ✅ | newtrack.md Phase 7 |
| `epic-start` | Before each epic | ✅ | implement.md Phase 0.5 |
| `epic-end` | After epic closes | ✅ | implement.md Phase 3 |
| `pre-finish` | Start of `/conductor-finish` | ✅ | finish.toml Phase 0 |
| `manual` | User runs command | ❌ | Direct invocation |
| `idle` | After 30min gap | ✅ | Session start check |

---

## Idle Detection

**Mechanism:** Check `conductor/.last_activity` mtime on user message.

```bash
if check_idle 30; then
    echo "⏰ It's been 45 minutes since your last activity."
    echo "[Y] Create handoff  [n] Skip  [s] Skip session"
fi
```

**Configuration in workflow.md:**
```yaml
handoff:
  idle_threshold_minutes: 30
  idle_prompt_enabled: true
```

---

## Agent Mail Format

**Subject:** `[HANDOFF:<trigger>] <track_id> - <brief_context>`

**Thread ID:** `handoff-<track_id>` or `handoff-general`

**Body sections:** Context, Changes, Learnings, Next Steps

**Search examples:**
```python
search_messages(project_key, query="HANDOFF")
search_messages(project_key, query="HANDOFF:design-end AND auth-system")
```

---

## Error Handling

| Scenario | Action |
|----------|--------|
| Agent Mail unavailable | Log warning, markdown-only |
| `bd` unavailable | Log warning, skip Beads sync |
| No handoffs found | Suggest `/conductor-handoff create` |
| Parallel workers running | Prompt: Wait / Proceed / Cancel |
| Branch mismatch | Warn, ask to continue |
| Stale handoff (>7d) | Warn, ask to continue |
| Secrets detected | Prompt: Proceed / Abort |

---

## Example Session

### CREATE with Beads Sync

```
User: /conductor-handoff create

Agent: Creating handoff...

📍 Context: Track auth-system_20251229 (in_progress)
📍 Branch: feat/auth-system @ abc123f

📤 Sending to Agent Mail... ✅
🔄 Syncing to Beads... ✅
   Notes: "COMPLETED: 5/12 (45%). NEXT: E2 login"
📝 Writing markdown...

✅ Handoff created successfully.
   → Section: 3
   → Progress: 45%
   → File: 2026-01-02_10-00-00-123_auth-system_manual.md
```

### RESUME with Progress

```
User: /conductor-handoff resume

Agent: 📬 Loading context from Agent Mail...
       Thread: handoff-auth-system_20251229

🔍 Loading Beads context...
   Epic: my-workflow:3-lzks
   
📊 Progress: 45% (5/12 tasks)
🎯 Ready: E2-login-endpoint

📋 Resuming: auth-system_20251229 | manual
🕐 Last handoff: 2 hours ago

[... context display ...]

✅ Loaded 3 tasks from Next Steps.
```
