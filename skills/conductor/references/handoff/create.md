# Create Handoff Workflow

Reference for the `/create_handoff` command.

## Command Aliases

- `/create_handoff` - Primary command
- `/conductor-handoff create` - Subcommand style

## Workflow

```
/create_handoff [trigger]
       │
       ▼
┌─────────────────────┐
│  1. Detect Context  │
└─────────────────────┘
       │
       ▼
┌─────────────────────┐
│  2. Gather Metadata │
└─────────────────────┘
       │
       ▼
┌─────────────────────┐
│  3. Scan for Secrets│
└─────────────────────┘
       │
       ▼
┌─────────────────────────────┐
│  4. Send to Agent Mail      │ ◀── PRIMARY (new)
│     (if available)          │
└─────────────────────────────┘
       │
       ▼
┌─────────────────────────────┐
│  5. Write Markdown File     │ ◀── SECONDARY (for git)
└─────────────────────────────┘
       │
       ▼
┌─────────────────────┐
│  6. Update Index    │
└─────────────────────┘
       │
       ▼
┌─────────────────────┐
│  7. Touch Activity  │
└─────────────────────┘
```

**Key change:** Agent Mail is now the **primary** storage for handoffs. Markdown files remain for git history and offline access.

## Step 1: Detect Context

```
IF bound_track in session state
  → track_id = bound_track
  → handoff_dir = conductor/handoffs/<track_id>/

ELSE IF active track detected (metadata.json with status != archived)
  → track_id = active track
  → handoff_dir = conductor/handoffs/<track_id>/

ELSE
  → track_id = "general"
  → handoff_dir = conductor/handoffs/general/
```

### Active Track Detection

Check `conductor/tracks/*/metadata.json`:
```json
{
  "status": "in_progress" | "new" | "planned"
}
```

If multiple active tracks, use most recently modified.

## Step 2: Gather Metadata

### Git Information

```bash
# Get current commit (7-char SHA)
git_commit=$(git rev-parse --short=7 HEAD 2>/dev/null || echo "unknown")

# Get current branch
git_branch=$(git branch --show-current 2>/dev/null || echo "unknown")
```

### Validation Snapshot

```bash
# Read from track's metadata.json
if [ -f "conductor/tracks/${track_id}/metadata.json" ]; then
  validation=$(jq '.validation' "conductor/tracks/${track_id}/metadata.json")
fi
```

### Timestamp

```javascript
// ISO 8601 with milliseconds
const timestamp = new Date().toISOString(); // 2025-12-29T10:00:00.123Z
```

### Bead ID (Epic Triggers Only)

For `epic-start` and `epic-end` triggers, include the bead ID:
```bash
bead_id=$(jq -r '.beads.epicId' "conductor/tracks/${track_id}/metadata.json")
```

## Step 3: Scan for Secrets

**IMPORTANT:** Run BEFORE writing handoff file.

### Hardcoded Patterns (Default)

```regex
# OpenAI API Keys
sk-[a-zA-Z0-9]{20,}

# GitHub PAT (classic and fine-grained)
ghp_[a-zA-Z0-9]{36}
github_pat_[a-zA-Z0-9]{22}_[a-zA-Z0-9]{59}

# AWS Access Key
AKIA[0-9A-Z]{16}

# AWS Secret Key (context-dependent)
(?i)aws.{0,20}secret.{0,20}['"][0-9a-zA-Z/+]{40}['"]

# Private Keys
-----BEGIN.*PRIVATE KEY-----

# Generic API key patterns
(?i)(api[_-]?key|apikey|secret[_-]?key|access[_-]?token)\s*[=:]\s*['"][a-zA-Z0-9_\-]{20,}['"]
```

### Configurable Patterns

Check `conductor/workflow.md` for additional patterns:

```yaml
handoff:
  secrets_patterns:
    - 'MY_CUSTOM_TOKEN_[A-Z0-9]{32}'
    - 'company_secret_[a-z0-9]{40}'
```

### Gitleaks Integration

```bash
# If gitleaks is available, use it for comprehensive scanning
if command -v gitleaks &>/dev/null; then
  gitleaks detect --no-git --source . --config .gitleaks.toml 2>/dev/null
fi
```

### On Secret Detection

```
⚠️ Potential secret detected in handoff content:
   Pattern: sk-******* (OpenAI API Key)
   Location: Context section, line 12

[P]roceed anyway  [A]bort
```

- **[P]roceed**: Write handoff with warning comment
- **[A]bort**: Cancel handoff creation

## Step 4: Send to Agent Mail

**Primary storage** - send handoff to Agent Mail first for FTS5 search and cross-session context.

### Check Agent Mail Availability

```python
try:
    # Verify MCP server is available
    health_check(reason="handoff creation")
    agent_mail_available = True
except:
    agent_mail_available = False
    # Log warning: "⚠️ Agent Mail unavailable - using markdown-only"
```

### Build Message

Use the schema from [agent-mail-format.md](agent-mail-format.md):

```python
message = {
    "project_key": absolute_workspace_path,  # e.g., "/Users/alice/project"
    "sender_name": agent_name,               # e.g., "BlueLake"
    "to": ["Human"],                         # or orchestrator name
    "subject": f"[HANDOFF:{trigger}] {track_id} - {brief_context}",
    "body_md": format_body(context, changes, learnings, next_steps),
    "thread_id": f"handoff-{track_id}",      # or "handoff-general"
    "importance": "high" if trigger in AUTO_TRIGGERS else "normal"
}
```

### Auto-Triggers (high importance)

```python
AUTO_TRIGGERS = ["design-end", "epic-start", "epic-end", "pre-finish"]
```

### Send Message

```python
if agent_mail_available:
    result = send_message(**message)
    # Store message ID for reference
    agent_mail_message_id = result["deliveries"][0]["payload"]["id"]
```

### Fallback Behavior

If Agent Mail unavailable:

```text
⚠️ Agent Mail unavailable - handoff stored in markdown only
```

Continue to Step 5 regardless of Agent Mail availability.

## Step 5: Write Markdown File

**Secondary storage** - write markdown for git history and offline access.

### Filename Generation

```
YYYY-MM-DD_HH-MM-SS-mmm_<track>_<trigger>.md
```

Example: `2025-12-29_10-00-00-123_auth-system_design-end.md`

### Collision Handling

```bash
filename="${base_filename}.md"
counter=1
while [ -f "${handoff_dir}/${filename}" ]; do
  filename="${base_filename}-${counter}.md"
  counter=$((counter + 1))
done
```

### Template Population

Use [template.md](template.md) and populate:
1. All frontmatter fields
2. Gather context from current session
3. List changes made in session
4. Extract learnings from conversation
5. Define next steps

### Content Generation

For each section, the agent should:

**Context:**
- Summarize current work state
- List active decisions
- Note any blockers or open questions

**Changes:**
- Scan git diff for modified files
- Include line ranges where relevant
- Brief description of each change

**Learnings:**
- Extract patterns discovered
- Note gotchas encountered
- Capture important context

**Next Steps:**
- List immediate actions needed
- Use checkbox format `[ ]`
- Order by priority

## Step 6: Update Index

### Append Entry

```bash
# Atomic append to index.md
echo "| ${timestamp_short} | ${trigger} | ${bead_id:-"-"} | ${summary} | [→](./${filename}) |" >> "${handoff_dir}/index.md"
```

### Index Format

```markdown
| Timestamp | Trigger | Bead | Summary | File |
|-----------|---------|------|---------|------|
| 10:00:00.123 | design-end | - | RS256 decision | [→](./2025-12-29_10-00-00-123_auth-system_design-end.md) |
```

### Index Repair

If index.md is corrupted or missing, rebuild:

```bash
# Scan directory, rebuild from filenames
for file in "${handoff_dir}"/*.md; do
  [ "$file" = "${handoff_dir}/index.md" ] && continue
  # Parse filename: YYYY-MM-DD_HH-MM-SS-mmm_track_trigger.md
  # Extract metadata and append to index
done
```

## Step 7: Touch Activity Marker

```bash
# Update last activity timestamp
touch conductor/.last_activity
```

This enables idle detection (see [idle-detection.md](idle-detection.md)).

## Error Handling

| Scenario | Action |
|----------|--------|
| No `conductor/` directory | Create `conductor/handoffs/general/` first |
| Git not available | Use "unknown" for commit/branch |
| Agent Mail unavailable | Log warning, continue with markdown-only |
| Index write fails | Log warning, handoff file still created |
| Secrets detected | Prompt user: [P]roceed / [A]bort |
| Handoff dir doesn't exist | Create it with index.md |
| Disk full | HALT with error message |

## Example Session

```
User: /create_handoff

Agent: Creating handoff...

📍 Context: Track auth-system_20251229 (in_progress)
📍 Branch: feat/auth-system @ abc123f

📤 Sending to Agent Mail... ✅
📝 Writing: conductor/handoffs/auth-system_20251229/2025-12-29_10-00-00-123_auth-system_manual.md

✅ Handoff created successfully.
   → Agent Mail: thread handoff-auth-system_20251229
   → Markdown: 2025-12-29_10-00-00-123_auth-system_manual.md
📝 Index updated.
```

### Agent Mail Unavailable

```
User: /create_handoff

Agent: Creating handoff...

📍 Context: Track auth-system_20251229 (in_progress)
📍 Branch: feat/auth-system @ abc123f

⚠️ Agent Mail unavailable - using markdown-only
📝 Writing: conductor/handoffs/auth-system_20251229/2025-12-29_10-00-00-123_auth-system_manual.md

✅ Handoff created (markdown only).
📝 Index updated.
```

## Integration Points

| Integration | When | Automatic |
|-------------|------|-----------|
| `/conductor-newtrack` | After Phase 7 | ✅ (design-end) |
| `/conductor-implement` | Before/after epic | ✅ (epic-start/end) |
| `/conductor-finish` | Phase 0 | ✅ (pre-finish) |
| User request | Anytime | `/create_handoff` |
| Idle detection | 30min gap | ✅ (idle) |

See [triggers.md](triggers.md) for trigger details.
