# RESUME Mode - Handoff Workflow

9-step process to restore session context.

## Step 1: Parse Input

```python
if argument.endswith('.md'):
    mode = "explicit"
    handoff_path = argument
elif argument matches track_pattern:  # e.g., "auth-system_20251229"
    mode = "track"
    track_id = argument
else:
    mode = "discover"
```

## Step 2: Agent Mail Lookup (Primary)

```python
if agent_mail_available:
    thread_id = f"handoff-{track_id}" if track_id else None
    
    summary = summarize_thread(
        project_key=WORKSPACE_PATH,
        thread_id=thread_id,
        include_examples=True,
        llm_mode=True
    )
    
    if summary:
        context_source = "agent_mail"
        handoff_context = summary
```

## Step 3: File Discovery (Fallback)

**Mode: discover**
1. Scan `conductor/handoffs/*/`
2. Find most recent handoff per track
3. Sort by timestamp
4. Single track → auto-select
5. Multiple → present list

**Mode: track**
```bash
handoff_dir="conductor/handoffs/${track_id}/"
# Find most recent *.md (excluding index.md)
```

## Step 4: Load Handoff Content

Parse YAML frontmatter + 4 sections:
- Context
- Changes
- Learnings
- Next Steps

## Step 5: Beads Context

```bash
if bd available; then
    epic_id=$(jq -r '.beads.epicId' "conductor/tracks/${track_id}/metadata.json")
    
    bd show "$epic_id"
    
    ready_tasks=$(bd ready --json | jq -r '.[] | select(.parent == "'$epic_id'") | .title')
    completed=$(bd list --parent=$epic_id --status=closed --json | jq 'length')
    total=$(bd list --parent=$epic_id --json | jq 'length')
    progress=$((completed * 100 / total))
    
    echo "📊 Progress: ${progress}% (${completed}/${total} tasks)"
    echo "🎯 Ready: ${ready_tasks}"
fi
```

## Step 6: Validate State

**7-Day Freshness Check:**
```bash
handoff_age_days=$(( ($(date +%s) - $(stat -f%m "$handoff_path")) / 86400 ))

if [ "$handoff_age_days" -gt 7 ]; then
    echo "⚠️ Stale handoff (>7 days) - skipping"
    echo "   Recommend: Start fresh with /handoff create"
    return
fi
```

**Git validation:**
```bash
current_branch=$(git branch --show-current)
handoff_branch=$(grep 'git_branch:' handoff.md | cut -d' ' -f2)

if [ "$current_branch" != "$handoff_branch" ]; then
    echo "⚠️ Branch mismatch: was ${handoff_branch}, now ${current_branch}"
fi
```

## Step 7: Present Analysis

```
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
📋 Resuming: auth-system_20251229 | manual
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

## Step 8: Create TodoWrite Items

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

## Step 9: Update Status

```bash
jq '.handoff.status = "active"' \
   "conductor/tracks/${track_id}/metadata.json" > tmp.$$ && mv tmp.$$ metadata.json

touch conductor/.last_activity
```

## Output

```
✅ Resumed from handoff.

📋 Track: auth-system_20251229
📊 Progress: 45% (5/12 tasks)
🎯 Next: E2-login-endpoint

Loaded 3 tasks to TODO list.
```
