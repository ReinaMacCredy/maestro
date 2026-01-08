# CREATE Mode - Handoff Workflow

9-step process to save session context.

## Step 1: Detect Context

```python
if bound_track in session_state:
    track_id = bound_track
elif active_track_exists():  # metadata.json.status != "archived"
    track_id = active_track
else:
    track_id = "general"

handoff_dir = f"conductor/handoffs/{track_id}/"
```

## Step 2: Gather Metadata

```bash
git_commit=$(git rev-parse --short=7 HEAD 2>/dev/null || echo "unknown")
git_branch=$(git branch --show-current 2>/dev/null || echo "unknown")
timestamp=$(date -u +"%Y-%m-%dT%H:%M:%S.%3NZ")
```

## Step 3: Scan for Secrets

```regex
sk-[a-zA-Z0-9]{20,}                           # OpenAI
ghp_[a-zA-Z0-9]{36}                           # GitHub PAT
AKIA[0-9A-Z]{16}                              # AWS
-----BEGIN.*PRIVATE KEY-----                  # Private Keys
```

On detection: `⚠️ Potential secret detected. [P]roceed / [A]bort`

## Step 4: Send to Agent Mail

```python
send_message(
    project_key=WORKSPACE_PATH,
    sender_name=AGENT_NAME,
    to=[AGENT_NAME],  # Self-mail for resume
    subject=f"[HANDOFF:{trigger}] {track_id} - {context[:50]}",
    body_md=format_body(context, changes, learnings, next_steps),
    thread_id=f"handoff-{track_id}",
    importance="high" if trigger in ["design-end", "epic-end"] else "normal"
)
```

## Step 5: Beads Sync

```bash
if bd available; then
    epic_id=$(jq -r '.beads.epicId' "conductor/tracks/${track_id}/metadata.json")
    completed=$(bd list --parent=$epic_id --status=closed --json | jq 'length')
    total=$(bd list --parent=$epic_id --json | jq 'length')
    progress=$((completed * 100 / total))
    
    bd update "$epic_id" --notes "HANDOFF: ${progress}% complete. NEXT: ${next_task}"
    bd sync
fi
```

## Step 6: Write Markdown File

Filename: `YYYY-MM-DD_HH-MM-SS-mmm_<track>_<trigger>.md`

See [template.md](template.md) for format.

## Step 7: Update metadata.json

```bash
jq '.handoff.status = "handed_off" |
    .handoff.section_count += 1 |
    .handoff.last_handoff = $ts' \
   "conductor/tracks/${track_id}/metadata.json" > tmp.$$ && mv tmp.$$ metadata.json
```

## Step 8: Update Index

```bash
echo "| ${timestamp} | ${trigger} | ${bead_id:-"-"} | ${summary} | [→](./${filename}) |" \
  >> "${handoff_dir}/index.md"
```

## Step 9: Touch Activity Marker

```bash
touch conductor/.last_activity
```

## Output

```
✅ Handoff saved.

📍 Track: auth-system_20251229
📊 Progress: 45% (5/12 tasks)
📝 File: 2026-01-04_10-00-00_auth-system_manual.md

To resume: /handoff resume
```
