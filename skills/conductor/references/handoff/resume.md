# Resume Handoff Workflow

Reference for the `/resume_handoff` command.

## Command Aliases

- `/resume_handoff` - Primary command
- `/resume_handoff <path>` - Resume specific handoff file
- `/resume_handoff <track-id>` - Resume latest from track
- `/conductor-handoff resume` - Subcommand style

## Workflow

```
/resume_handoff [path|track|none]
       │
       ▼
┌─────────────────────┐
│  1. Parse Input     │
└─────────────────────┘
       │
       ▼
┌─────────────────────┐
│  2. Smart Discovery │
└─────────────────────┘
       │
       ▼
┌─────────────────────┐
│  3. Load Handoff    │
└─────────────────────┘
       │
       ▼
┌─────────────────────┐
│  4. Validate State  │
└─────────────────────┘
       │
       ▼
┌─────────────────────┐
│  5. Present Analysis│
└─────────────────────┘
       │
       ▼
┌─────────────────────┐
│  6. Create Todos    │
└─────────────────────┘
```

## Step 1: Parse Input

```
IF argument is file path
  → mode = "explicit"
  → handoff_path = argument

ELSE IF argument matches track pattern (e.g., "auth-system_20251229")
  → mode = "track"
  → track_id = argument

ELSE (no argument)
  → mode = "discover"
```

### Track Pattern

```regex
^[a-z0-9-]+_\d{8}$
```

Examples: `auth-system_20251229`, `fix-bug_20251215`

## Step 2: Smart Discovery

### Mode: Explicit

```
Use provided path directly.
Validate file exists.
```

### Mode: Track

```
handoff_dir = conductor/handoffs/<track_id>/
Find most recent handoff:
  - List all *.md except index.md
  - Sort by filename (timestamp in name)
  - Select newest
```

### Mode: Discover

```
1. Scan conductor/handoffs/*/
2. For each directory:
   - Find most recent handoff
   - Extract timestamp from filename
3. Sort all by timestamp

IF only 1 active track
  → Auto-select most recent handoff
  → Show: "Auto-resuming from <track>..."

ELSE IF multiple tracks
  → Present list:
    
    Recent handoffs:
    
    1. auth-system_20251229 (2h ago) - epic-end: E2 login
    2. payments_20251228 (1d ago) - manual: refactor
    3. general (3d ago) - manual: cleanup
    
    Select [1-3] or path:

ELSE (no handoffs found)
  → Show: "No handoffs found. Use /create_handoff first."
```

## Step 3: Load Handoff

### Parse File

```markdown
1. Read entire file
2. Parse YAML frontmatter
3. Extract 4 sections:
   - Context
   - Changes
   - Learnings
   - Next Steps
```

### Frontmatter Extraction

```yaml
timestamp: 2025-12-29T10:00:00.123+07:00
trigger: epic-end
track_id: auth-system_20251229
bead_id: E1-jwt-core
git_commit: abc123f
git_branch: feat/auth-system
author: agent
validation_snapshot:
  gates_passed: [design, spec, plan-structure]
  current_gate: plan-execution
```

### Malformed Frontmatter Handling

If YAML parsing fails:

```
1. Infer from filename:
   YYYY-MM-DD_HH-MM-SS-mmm_<track>_<trigger>.md
   
2. Extract:
   - timestamp from date/time parts
   - track_id from track part
   - trigger from trigger part

3. Log warning:
   ⚠️ Malformed frontmatter in handoff. Using filename metadata.
```

## Step 4: Validate State

### Git Branch Check

```bash
current_branch=$(git branch --show-current 2>/dev/null)
handoff_branch=$(yq '.git_branch' handoff.md)

if [ "$current_branch" != "$handoff_branch" ]; then
  # Branch mismatch warning
fi
```

**Branch Mismatch Warning:**

```
⚠️ Branch mismatch detected:
   Handoff: feat/auth-system
   Current: main
   
Continue anyway? [Y/n]
```

### Stale Handoff Check

```bash
handoff_timestamp=$(yq '.timestamp' handoff.md)
now=$(date +%s)
handoff_epoch=$(date -d "$handoff_timestamp" +%s)
age_days=$(( (now - handoff_epoch) / 86400 ))

if [ $age_days -gt 7 ]; then
  # Stale warning
fi
```

**Stale Handoff Warning:**

```
⚠️ This handoff is 12 days old.
   Context may be outdated.
   
Continue anyway? [Y/n]
```

### File Existence Check

```bash
# Extract file paths from Changes section
files=$(grep -oE '`[^`]+`' changes_section | tr -d '`' | cut -d: -f1)

for file in $files; do
  if [ ! -f "$file" ]; then
    missing+=("$file")
  fi
done

if [ ${#missing[@]} -gt 0 ]; then
  # Files missing warning
fi
```

**Files Missing Warning:**

```
⚠️ Some files from handoff no longer exist:
   - src/auth/old-handler.ts (deleted)
   - src/utils/helper.ts (deleted)
   
Continue anyway? [Y/n]
```

### Drift Detection

Combine all warnings into drift summary:

```
📊 Drift Analysis:
   ✅ Branch: feat/auth-system (matches)
   ⚠️ Age: 3 days (acceptable)
   ⚠️ Files: 1 deleted, 2 modified
   
Proceed with resume? [Y/n]
```

## Step 5: Present Analysis

### Display Format

```
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
📋 Resuming: auth-system_20251229 | epic-end
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

🕐 Created: 2025-12-29 15:30:00 (2 hours ago)
🔀 Branch: feat/auth-system @ def456a
🎯 Trigger: epic-end (E1-jwt-core)

## Context

Completed E1: Core JWT module.
- All tests passing (12 new tests)
- Coverage: 94%
- Bead status: completed

## Changes

- src/auth/jwt.ts:1-120 - JWT token generation
- src/auth/keys.ts:1-45 - Key pair management  
- tests/auth/jwt.test.ts:1-200 - Unit tests

## Learnings

- jsonwebtoken needs explicit algorithm
- Key rotation requires graceful fallback
- Redis TTL should match token lifetime

## Next Steps

1. [ ] Start E2: Login endpoint
2. [ ] Wire JWT module to login handler
3. [ ] Add integration tests

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
```

## Step 6: Create Todos

### Extract from Next Steps

```markdown
## Next Steps

1. [ ] Start E2: Login endpoint
2. [ ] Wire JWT module to login handler
3. [ ] Add integration tests
```

### Convert to TodoWrite

```json
[
  {"id": "resume-1", "content": "Start E2: Login endpoint", "status": "todo"},
  {"id": "resume-2", "content": "Wire JWT module to login handler", "status": "todo"},
  {"id": "resume-3", "content": "Add integration tests", "status": "todo"}
]
```

### Touch Activity Marker

```bash
touch conductor/.last_activity
```

## Index Auto-Repair

If index.md is corrupted or missing:

### Detection

```bash
# Check if index.md exists and is valid
if [ ! -f "${handoff_dir}/index.md" ]; then
  repair_needed=true
elif ! grep -q "^| Timestamp |" "${handoff_dir}/index.md"; then
  repair_needed=true
fi
```

### Repair Process

```bash
# 1. Scan directory for handoff files
files=$(ls -1 "${handoff_dir}"/*.md 2>/dev/null | grep -v index.md)

# 2. Create new index header
cat > "${handoff_dir}/index.md" << 'EOF'
---
track_id: {{TRACK_ID}}
created: {{NOW}}
last_updated: {{NOW}}
---

# Handoff Log: {{TRACK_ID}}

| Timestamp | Trigger | Bead | Summary | File |
|-----------|---------|------|---------|------|
EOF

# 3. For each handoff file, extract metadata and append
for file in $files; do
  # Parse filename: YYYY-MM-DD_HH-MM-SS-mmm_track_trigger.md
  filename=$(basename "$file")
  timestamp=$(echo "$filename" | cut -d_ -f1,2 | tr '_' ' ')
  trigger=$(echo "$filename" | rev | cut -d_ -f1 | rev | sed 's/.md$//')
  
  # Try to get bead from frontmatter, fallback to "-"
  bead=$(yq '.bead_id // "-"' "$file" 2>/dev/null || echo "-")
  
  # Generate summary from first line of Context section
  summary=$(sed -n '/^## Context/,/^##/p' "$file" | head -3 | tail -1 | cut -c1-40)
  
  echo "| ${timestamp} | ${trigger} | ${bead} | ${summary} | [→](./${filename}) |" >> "${handoff_dir}/index.md"
done

# 4. Log repair
echo "⚠️ Index repaired: ${handoff_dir}/index.md"
```

## Error Handling

| Scenario | Action |
|----------|--------|
| No handoffs found | Show message, suggest `/create_handoff` |
| File not found | Error: "Handoff file not found: <path>" |
| Malformed YAML | Warn, infer from filename |
| Branch mismatch | Warn, ask to continue |
| Stale handoff (>7d) | Warn, ask to continue |
| Files deleted | Warn, list missing files |
| Index corrupted | Auto-repair, log warning |
| No conductor/ dir | Error: "Run /conductor-setup first" |

## Example Sessions

### Auto-Resume (Single Track)

```
User: /resume_handoff

Agent: Auto-resuming from auth-system_20251229...

📋 Resuming: auth-system_20251229 | epic-end
🕐 Created: 2 hours ago
🔀 Branch: feat/auth-system @ def456a

[... full analysis ...]

✅ Loaded 3 tasks from Next Steps.
```

### Multi-Track Selection

```
User: /resume_handoff

Agent: Multiple tracks found. Select one:

   1. auth-system_20251229 (2h ago) - epic-end: E2 login
   2. payments_20251228 (1d ago) - manual: refactor
   3. general (3d ago) - idle: session end

Select [1-3] or enter path:

User: 1

Agent: 📋 Resuming: auth-system_20251229 | epic-end
[... full analysis ...]
```

### Explicit Path

```
User: /resume_handoff conductor/handoffs/auth-system_20251229/2025-12-29_10-00-00-123_auth-system_design-end.md

Agent: 📋 Resuming: auth-system_20251229 | design-end
[... full analysis ...]
```

## Integration with Beads

After resume, if track has active beads:

```bash
# Check for in-progress beads
bd ready --json | jq -r '.[] | select(.status == "in_progress")'
```

If found:

```
📍 Active bead detected: E2-login-endpoint (in_progress)
   Resume this task? [Y/n]
```

## Configuration

In `conductor/workflow.md`:

```yaml
handoff:
  resume:
    stale_threshold_days: 7      # Warn if older
    auto_select_single: true     # Auto-resume if 1 track
    show_drift_analysis: true    # Show file/branch drift
```
