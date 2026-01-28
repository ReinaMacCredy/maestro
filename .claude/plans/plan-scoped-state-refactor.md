# Plan-Scoped State Refactor

## Problem Statement

When running concurrent Claude sessions (e.g., two `/atlas-plan` commands), both sessions read and write to the same global `.atlas/pipeline-state.json` file. This causes race conditions where one session overwrites another's state, corrupting workflow tracking.

**Screenshot evidence**: Session A updating `momus_iterations: 0 → 1` for `state-management-refactor` while Session B simultaneously overwrites with `momus_iterations: 0` and changes `plan_name` to `oracle-codex-integration`.

## Solution: Plan-Scoped State

Move pipeline state from a single global file to per-plan state files:

```
BEFORE: .atlas/pipeline-state.json (single file, all sessions conflict)

AFTER:  .atlas/notepads/{plan-id}/pipeline-state.json (isolated per plan)
```

This leverages the existing `.atlas/notepads/{plan-name}/` directory structure already used for per-plan wisdom (learnings.md, decisions.md, etc.).

## Scope

**In scope:**
- Plan-id normalization and detection
- Plan-scoped state helpers with per-plan locking
- Hook and state-machine refactors
- CLI/command updates
- Migration and recovery tooling
- Schema/validation updates
- Documentation and tests

**Out of scope:**
- Redesigning boulder.json architecture
- Changing planning/execution workflow beyond state-file routing

## Architecture

### State File Locations

| Purpose | Old Path | New Path |
|---------|----------|----------|
| Pipeline state | `.atlas/pipeline-state.json` | `.atlas/notepads/{plan-id}/pipeline-state.json` |
| Boulder state | `.atlas/boulder.json` | `.atlas/boulder.json` (unchanged - global orchestration state) |

### Plan ID Normalization

Plan names must be sanitized to safe directory/lock names:
- **Slugification**: Convert spaces to hyphens, lowercase, remove special chars
- **Hash fallback**: If name contains slashes or path traversal attempts, use SHA256 hash
- **Block path traversal**: Reject `..` or absolute paths
- **Store original**: Keep `plan_name` (original) and `plan_id` (safe slug) in state
- **Auto-suffix for duplicates**: If plan-id directory already exists, append timestamp (e.g., `my-plan-20260127-120000`)

### Design Decisions (User Confirmed)

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Duplicate plan names | **Auto-suffix new ID** | Append timestamp for uniqueness, prevents accidental state sharing |
| Legacy global state | **Fully deprecate** | Archive and remove after migration, no last-active pointer |
| Detection failure | **Fallback to legacy** | Safer migration path, hooks use global state when plan unresolved |

### Plan ID Initialization

**When plan_id is first created**: In `atlas-plan.md` command at plan creation time, BEFORE any hooks run.

**Sequence**:
1. User runs `/atlas-plan my feature`
2. `atlas-plan.md` calculates `plan_id = normalize_plan_id("my-feature")`
3. If `.atlas/notepads/${plan_id}/` exists, append timestamp: `plan_id = "my-feature-20260127-120000"`
4. Create state file at `.atlas/notepads/${plan_id}/pipeline-state.json` with both `plan_name` and `plan_id`
5. All subsequent hooks read `plan_id` from state file or detect from context

### State Resolution Order

When a hook needs to access pipeline state:

1. **Input JSON** - Parse `plan_name` field from hook input
2. **Tool result parsing** - Extract from `tool_result.file_path` (e.g., `.claude/plans/{name}.md`)
3. **SubagentStop output** - Extract `plan_file` or `draft_file` via `extract_json_robust`
4. **Boulder index** - Check `.atlas/boulder.json` for active planning fields
5. **Recent plan fallback** - Use `find` (not `ls`) to get most recent plan, handling spaces/dots/subdirs
6. **Legacy global state** - Fall back to `.atlas/pipeline-state.json` during migration

---

## TODOs

### 1. Define canonical plan-id and safe naming
**File**: `scripts/lib/hook-common.sh`

Add functions for plan ID normalization:

```bash
# Normalize plan name to safe directory/lock name
# Args: $1 = raw plan name
# Returns: sanitized plan-id (lowercase, hyphens, no special chars)
# MUST call with: set -o pipefail in calling context
normalize_plan_id() {
  local name="$1"

  # Empty input check
  if [[ -z "$name" ]]; then
    echo "unknown"
    return 0
  fi

  # Block path traversal attempts
  if [[ "$name" == *".."* || "$name" == /* ]]; then
    # Use hash for unsafe names
    local hash
    hash=$(echo "$name" | shasum -a 256 | cut -c1-12)
    echo "$hash"
    return 0
  fi

  # Slugify: lowercase, spaces to hyphens, remove non-alphanumeric except hyphens
  local result
  result=$(echo "$name" | tr '[:upper:]' '[:lower:]' | tr ' ' '-' | sed 's/[^a-z0-9-]//g' | sed 's/--*/-/g' | sed 's/^-//' | sed 's/-$//')

  # If slugification produces empty string (e.g., name was "!!!"), use hash
  if [[ -z "$result" ]]; then
    result=$(echo "$name" | shasum -a 256 | cut -c1-12)
  fi

  echo "$result"
}

# Generate unique plan-id with timestamp suffix if directory exists
# Args: $1 = raw plan name
# Returns: unique plan-id (may have timestamp suffix)
generate_unique_plan_id() {
  local plan_name="$1"
  local base_id
  base_id=$(normalize_plan_id "$plan_name")
  local plan_id="$base_id"
  local notepad_dir=".atlas/notepads/${plan_id}"

  # If directory exists, append timestamp for uniqueness
  if [[ -d "$notepad_dir" ]]; then
    plan_id="${base_id}-$(date +%Y%m%d-%H%M%S)"
  fi

  echo "$plan_id"
}

# Get lock name for plan-specific operations
plan_lock_name() {
  local plan_id="$1"
  echo "plan-${plan_id}.lock"
}
```

**Acceptance Criteria**:
- [ ] `normalize_plan_id` handles spaces, dots, special chars
- [ ] Path traversal (`..`, `/`) blocked with hash fallback
- [ ] Empty input returns "unknown", empty slugification result uses hash
- [ ] `generate_unique_plan_id` appends timestamp if directory exists
- [ ] `plan_lock_name` returns unique lock per plan
- [ ] Both `plan_name` (original) and `plan_id` (safe) stored in state

---

### 2. Implement robust plan detection utilities
**File**: `scripts/lib/hook-common.sh`

```bash
# Detect plan name from hook input JSON
# Args: $1 = raw hook input
detect_plan_from_input() {
  local input="$1"

  # 1. Try plan_name field directly
  local plan_name
  plan_name=$(echo "$input" | jq -r '.plan_name // empty' 2>/dev/null)
  if [[ -n "$plan_name" ]]; then
    echo "$plan_name"
    return 0
  fi

  # 2. Try tool_result.file_path for plan files
  local file_path
  file_path=$(echo "$input" | jq -r '.tool_result.file_path // empty' 2>/dev/null)
  if [[ "$file_path" =~ \.claude/plans/(.+)\.md$ ]]; then
    echo "${BASH_REMATCH[1]}"
    return 0
  fi

  # 3. Try SubagentStop output (plan_file or draft_file)
  local response
  response=$(echo "$input" | jq -r '.tool_result.content // .response // empty' 2>/dev/null)
  if [[ -n "$response" ]]; then
    local extracted
    extracted=$(python3 "$SCRIPT_DIR/lib/json-extract.py" --field plan_file <<< "$response" 2>/dev/null)
    if [[ "$extracted" =~ \.claude/plans/(.+)\.md$ ]]; then
      echo "${BASH_REMATCH[1]}"
      return 0
    fi
    extracted=$(python3 "$SCRIPT_DIR/lib/json-extract.py" --field draft_file <<< "$response" 2>/dev/null)
    if [[ "$extracted" =~ \.atlas/drafts/(.+)\.md$ ]]; then
      echo "${BASH_REMATCH[1]}"
      return 0
    fi
  fi

  return 1
}

# Get most recently modified plan name (pure find, no ls)
# macOS compatible (no -printf)
detect_recent_plan() {
  local recent
  # Use stat to get modification time, sort, get newest
  # macOS stat format: -f "%m %N" (epoch time, filename)
  recent=$(find .claude/plans -maxdepth 1 -name "*.md" -type f -exec stat -f "%m %N" {} \; 2>/dev/null | \
    sort -rn | head -1 | cut -d' ' -f2-)
  if [[ -n "$recent" ]]; then
    basename "$recent" .md
  fi
}

# Check boulder.json for active planning context
detect_plan_from_boulder() {
  if [[ -f ".atlas/boulder.json" ]]; then
    local active_plan
    active_plan=$(jq -r '.active_plan // .current_plan // empty' ".atlas/boulder.json" 2>/dev/null)
    if [[ -n "$active_plan" ]]; then
      echo "$active_plan"
      return 0
    fi
  fi
  return 1
}

# Get current plan name with full fallback chain
# Returns: plan name or empty string
get_current_plan() {
  local input="${1:-}"

  # 1. Try from input
  if [[ -n "$input" ]]; then
    local plan_name
    plan_name=$(detect_plan_from_input "$input")
    if [[ -n "$plan_name" ]]; then
      echo "$plan_name"
      return 0
    fi
  fi

  # 2. Try from boulder active plan
  local plan_name
  plan_name=$(detect_plan_from_boulder)
  if [[ -n "$plan_name" ]]; then
    echo "$plan_name"
    return 0
  fi

  # 3. Try from legacy global state (migration compatibility)
  if [[ -f ".atlas/pipeline-state.json" ]]; then
    plan_name=$(jq -r '.plan_name // empty' ".atlas/pipeline-state.json" 2>/dev/null)
    if [[ -n "$plan_name" ]]; then
      echo "$plan_name"
      return 0
    fi
  fi

  # 4. Try most recent plan file
  plan_name=$(detect_recent_plan)
  if [[ -n "$plan_name" ]]; then
    echo "$plan_name"
    return 0
  fi

  return 1
}
```

**Acceptance Criteria**:
- [ ] `detect_plan_from_input` parses input JSON, tool_result.file_path, SubagentStop output
- [ ] `detect_recent_plan` uses `find` instead of `ls` (handles spaces, dots, subdirs)
- [ ] `detect_plan_from_boulder` checks boulder.json active plan
- [ ] `get_current_plan` implements full fallback chain

---

### 3. Add plan-scoped state helpers with locking
**File**: `scripts/lib/hook-common.sh`

```bash
# Get pipeline state path for a specific plan
# Args: $1 = plan_name (raw, will be normalized)
get_plan_state_path() {
  local plan_name="$1"
  if [[ -z "$plan_name" ]]; then
    echo ".atlas/pipeline-state.json"  # Legacy fallback
    return 1
  fi
  local plan_id
  plan_id=$(normalize_plan_id "$plan_name")
  echo ".atlas/notepads/${plan_id}/pipeline-state.json"
}

# Ensure plan notepad directory and state file exist
# Args: $1 = plan_name (raw)
ensure_plan_state() {
  local plan_name="$1"
  local plan_id
  plan_id=$(normalize_plan_id "$plan_name")
  local state_path
  state_path=$(get_plan_state_path "$plan_name")
  local notepad_dir
  notepad_dir=$(dirname "$state_path")

  # Re-create notepad dir if deleted mid-workflow
  if [[ ! -d "$notepad_dir" ]]; then
    if ! mkdir -p "$notepad_dir" 2>/dev/null; then
      echo "[ERROR] Failed to create notepad dir: $notepad_dir" >&2
      return 1
    fi
  fi

  if [[ ! -f "$state_path" ]]; then
    cat > "$state_path" <<EOF
{
  "phase": "IDLE",
  "codex_choice": "standard",
  "plan_mode_active": false,
  "momus_iterations": 0,
  "plan_name": "$plan_name",
  "plan_id": "$plan_id"
}
EOF
  fi
  echo "$state_path"
}

# Get field from plan-specific state
# Args: $1 = plan_name, $2 = field, $3 = default
get_plan_field() {
  local plan_name="$1"
  local field="$2"
  local default="${3:-}"
  local state_path
  state_path=$(get_plan_state_path "$plan_name")

  if [[ ! -f "$state_path" ]]; then
    echo "$default"
    return
  fi

  local value
  value=$(jq -r --arg f "$field" '.[$f] // empty' "$state_path" 2>/dev/null)

  # Handle jq failure gracefully
  if [[ $? -ne 0 ]]; then
    echo "[WARN] jq failed reading $state_path" >&2
    echo "$default"
    return
  fi

  if [[ -z "$value" || "$value" == "null" ]]; then
    echo "$default"
  else
    echo "$value"
  fi
}

# Update plan-specific state with locking
# Args: $1 = plan_name, $2 = phase, $3... = key-value pairs
update_plan_state() {
  local plan_name="$1"
  local new_phase="$2"
  shift 2

  local plan_id
  plan_id=$(normalize_plan_id "$plan_name")
  local lock_name
  lock_name=$(plan_lock_name "$plan_id")
  local lock_acquired=false

  # Acquire plan-specific lock
  if acquire_lock "$lock_name"; then
    lock_acquired=true
  else
    echo "[WARN] Could not acquire lock $lock_name, proceeding anyway" >&2
  fi

  # Helper to release lock and return
  _cleanup_and_return() {
    local code="$1"
    [[ "$lock_acquired" == "true" ]] && release_lock "$lock_name"
    return "$code"
  }

  local state_path
  state_path=$(ensure_plan_state "$plan_name")
  if [[ $? -ne 0 ]]; then
    _cleanup_and_return 1
    return 1
  fi

  local tmp_file="${state_path}.tmp.$$"

  # Atomic write: only mv after successful jq
  if ! jq --arg phase "$new_phase" '.phase = $phase' "$state_path" > "$tmp_file" 2>/dev/null; then
    echo "[ERROR] jq failed updating phase in $state_path" >&2
    rm -f "$tmp_file"
    _cleanup_and_return 1
    return 1
  fi

  # Process additional key-value pairs
  while [[ $# -ge 2 ]]; do
    local key="$1"
    local val="$2"
    shift 2

    if [[ "$key" == "momus_iterations" ]]; then
      jq --argjson val "$val" '.momus_iterations = $val' "$tmp_file" > "${tmp_file}.2" 2>/dev/null
    elif [[ "$val" == "true" || "$val" == "false" ]]; then
      jq --argjson val "$val" --arg key "$key" '.[$key] = $val' "$tmp_file" > "${tmp_file}.2" 2>/dev/null
    else
      jq --arg key "$key" --arg val "$val" '.[$key] = $val' "$tmp_file" > "${tmp_file}.2" 2>/dev/null
    fi

    if [[ $? -ne 0 ]]; then
      echo "[ERROR] jq failed updating $key in $state_path" >&2
      rm -f "$tmp_file" "${tmp_file}.2"
      _cleanup_and_return 1
      return 1
    fi
    mv "${tmp_file}.2" "$tmp_file"
  done

  # Atomic mv - only if successful
  if ! mv "$tmp_file" "$state_path" 2>/dev/null; then
    echo "[ERROR] Failed to write $state_path (disk full?)" >&2
    rm -f "$tmp_file"
    _cleanup_and_return 1
    return 1
  fi

  _cleanup_and_return 0
}
```

**Acceptance Criteria**:
- [ ] `get_plan_state_path` returns plan-scoped path, falls back to global for empty plan
- [ ] `ensure_plan_state` creates notepad dir if missing, stores both `plan_name` and `plan_id`
- [ ] `get_plan_field` handles jq failures gracefully
- [ ] `update_plan_state` uses plan-specific locks, atomic tmp+mv, error handling for jq/mkdir/mv/disk-full
- [ ] Legacy global helpers remain for fallback

---

### 4. Refactor pipeline-state-machine.sh
**File**: `scripts/lib/pipeline-state-machine.sh`

Update `sm_update_pipeline_state` to accept plan context and use plan-scoped helpers.

```bash
# Updated signature
sm_update_pipeline_state() {
  local plan_name="$1"
  local new_phase="$2"
  shift 2

  if [[ -z "$plan_name" ]]; then
    # Fallback to legacy global state
    update_pipeline_state "$new_phase" "$@"
  else
    update_plan_state "$plan_name" "$new_phase" "$@"
  fi
}
```

**Also update**: `scripts/pipeline-transition.sh` to extract plan name from SubagentStop input:

```bash
# At start of pipeline-transition.sh, after reading input
input=$(cat)
plan_name=$(get_current_plan "$input")

# Pass plan_name to state machine transitions
# Example in handle_pipeline_transition function:
handle_pipeline_transition() {
  local agent_type="$1"
  local current_phase="$2"
  local status="$3"
  local response="$4"

  # ... existing logic ...

  # Use plan-scoped update instead of global
  sm_update_pipeline_state "$plan_name" "MOMUS_REVIEW" "plan_file" "$plan_file"
}
```

**Acceptance Criteria**:
- [ ] `sm_update_pipeline_state` accepts plan_name as first parameter
- [ ] Falls back to legacy function when plan_name empty
- [ ] `pipeline-transition.sh` extracts plan via `get_current_plan "$input"` at script start
- [ ] All `sm_update_pipeline_state` calls pass `$plan_name` as first argument

---

### 5. Update hook handlers
**Files**:
- `scripts/momus-loop-handler.sh`
- `scripts/plan-ready-handler.sh`
- `scripts/codex-completion-handler.sh`
- `scripts/generator-choice-handler.sh`
- `scripts/keyword-detector.sh`

Pattern for each hook:
```bash
# At start of script
input=$(cat)
plan_name=$(get_current_plan "$input")

# Replace global reads
# OLD: codex_choice=$(jq -r '.codex_choice' "$PIPELINE_STATE")
# NEW:
codex_choice=$(get_plan_field "$plan_name" "codex_choice" "standard")

# Replace global writes
# OLD: update_pipeline_state "COMPLETE" "momus_iterations" "0"
# NEW:
update_plan_state "$plan_name" "COMPLETE" "momus_iterations" "0"
```

**Special handling for CHOICE_POINT**: Ensure decisions update the correct plan's state.

**Fallback Behavior (Design Decision)**:
- If `get_current_plan` returns empty string (exit code 1), pass empty `plan_name` to helper functions
- `get_plan_state_path("")` returns `.atlas/pipeline-state.json` (legacy global path)
- This allows graceful degradation during migration when plan cannot be detected

**Acceptance Criteria**:
- [ ] All reads use `get_plan_field "$plan_name" ...`
- [ ] All writes use `update_plan_state "$plan_name" ...`
- [ ] Plan name detected at script start via `plan_name=$(get_current_plan "$input")`
- [ ] When `plan_name` is empty, helpers fall back to legacy global state automatically
- [ ] No explicit fallback logic needed in hook handlers (handled by helper functions)

---

### 6. Update ops/diagnostics scripts
**Files**:
- `scripts/diagnose-state.sh` - Show all plan states or specific plan
- `scripts/state-recover.sh` - Check/fix plan-scoped states
- `scripts/state-restore.sh` - Restore plan-scoped backups

Add `--plan` option:
```bash
# diagnose-state.sh
if [[ "$1" == "--plan" && -n "$2" ]]; then
  plan_id=$(normalize_plan_id "$2")
  cat ".atlas/notepads/${plan_id}/pipeline-state.json" | jq .
else
  # List all plan states
  for state in .atlas/notepads/*/pipeline-state.json; do
    echo "=== $(dirname "$state" | xargs basename) ==="
    cat "$state" | jq .
  done
fi
```

**Backup naming**: Include plan-id in backup filenames (e.g., `pipeline-state-{plan-id}.json.20260127_120000`)

**Acceptance Criteria**:
- [ ] `--plan <name>` option works on all ops scripts
- [ ] Handles missing notepad dirs gracefully
- [ ] Backup naming includes plan-id
- [ ] Legacy backup behavior preserved

---

### 7. Enhance CLI and commands
**Files**:
- `scripts/pipeline-state.sh` - Add `--plan` and `list` commands
- `.claude/commands/atlas-plan.md` - Write plan-scoped state

```bash
# pipeline-state.sh additions
case "$1" in
  list)
    # List all plan states
    for state in .atlas/notepads/*/pipeline-state.json; do
      if [[ -f "$state" ]]; then
        plan_id=$(dirname "$state" | xargs basename)
        phase=$(jq -r '.phase' "$state")
        echo "$plan_id: $phase"
      fi
    done
    ;;
  --plan)
    PLAN_NAME="$2"
    shift 2
    # Handle remaining commands with plan context
    ;;
esac
```

**atlas-plan.md update**:
```javascript
// Calculate plan_id from plan_name (after interview determines plan name)
// This MUST happen BEFORE creating state file

// 1. Normalize plan name to safe plan_id
const plan_id_result = Bash({
  command: `source scripts/lib/hook-common.sh && generate_unique_plan_id "${plan_name}"`
})
const plan_id = plan_id_result.trim()

// 2. Create plan-scoped state directory and file
Bash({ command: `mkdir -p .atlas/notepads/${plan_id}` })
Write({
  file_path: `.atlas/notepads/${plan_id}/pipeline-state.json`,
  content: JSON.stringify({
    "phase": "PLANNING",
    "codex_choice": "${user_choice}",
    "plan_mode_active": false,
    "momus_iterations": 0,
    "plan_name": "${plan_name}",
    "plan_id": "${plan_id}"
  }, null, 2)
})

// Note: generate_unique_plan_id handles duplicate names by appending timestamp
```

**Optional**: Maintain global `last_active_plan` pointer for hooks before plan resolution.

**Acceptance Criteria**:
- [ ] `pipeline-state.sh list` shows all plan states
- [ ] `--plan` flag works for get/set/transition/status/clear
- [ ] `atlas-plan.md` creates plan-scoped state with plan_id
- [ ] Without `--plan`, falls back to legacy global state

---

### 8. Migration and compatibility
**File**: New `scripts/migrate-pipeline-state.sh`

```bash
#!/bin/bash
# Migrate global pipeline-state.json to plan-scoped state
set -euo pipefail

source "$(dirname "$0")/lib/hook-common.sh"

GLOBAL_STATE=".atlas/pipeline-state.json"

if [[ ! -f "$GLOBAL_STATE" ]]; then
  echo "No global pipeline-state.json to migrate"
  exit 0
fi

# Extract plan info
plan_name=$(jq -r '.plan_name // empty' "$GLOBAL_STATE")
plan_file=$(jq -r '.plan_file // empty' "$GLOBAL_STATE")

# Derive plan name from plan_file if missing
if [[ -z "$plan_name" && -n "$plan_file" ]]; then
  plan_name=$(basename "$plan_file" .md)
fi

if [[ -z "$plan_name" ]]; then
  echo "[WARN] No plan_name in global state, using 'unknown'"
  plan_name="unknown"
fi

plan_id=$(normalize_plan_id "$plan_name")
target_dir=".atlas/notepads/${plan_id}"
target_file="${target_dir}/pipeline-state.json"

# Acquire lock for safe migration
lock_name=$(plan_lock_name "$plan_id")
if ! acquire_lock "$lock_name"; then
  echo "[ERROR] Could not acquire migration lock"
  exit 1
fi

# Idempotent: skip if already migrated
if [[ -f "$target_file" ]]; then
  echo "Already migrated to $target_file"
  release_lock "$lock_name"
  exit 0
fi

mkdir -p "$target_dir"

# Add plan_id to state during migration
jq --arg pid "$plan_id" '. + {plan_id: $pid}' "$GLOBAL_STATE" > "$target_file"

# Archive global state (with PID for uniqueness, no-clobber)
local backup_name="${GLOBAL_STATE}.migrated.$(date +%Y%m%d_%H%M%S).$$"
if ! mv -n "$GLOBAL_STATE" "$backup_name" 2>/dev/null; then
  echo "[WARN] Backup already exists or mv failed, state may already be migrated"
fi

echo "Migrated to $target_file (backup: $backup_name)"
release_lock "$lock_name"
```

**Also update**: `scripts/migrate-boulder-state.py` to recognize plan-scoped pipeline state.

**Migration Notes**:
- Migration should ideally run when no active sessions are using the plan
- Backup filename includes PID ($$) to prevent race condition between simultaneous migrations
- Uses `mv -n` (no-clobber) as additional safety

**Acceptance Criteria**:
- [ ] Migrates global state to plan-scoped path
- [ ] Derives plan_name from plan_file if missing
- [ ] Uses 'unknown' as fallback plan_id
- [ ] Idempotent (safe to run multiple times)
- [ ] Uses locking for concurrent safety
- [ ] Backup filename includes PID to prevent race conditions
- [ ] Uses `mv -n` for no-clobber safety
- [ ] Fully deprecates global state (archives, does not keep as pointer)

---

### 9. Update validation, schema, documentation, and tests
**Files**:
- `scripts/lib/state_validate.py` - Support plan-scoped validation
- `scripts/lib/state_schema.json` - Add `plan_id` field
- Docs: `docs/STATE_MANAGEMENT.md`, `docs/HOOK_ARCHITECTURE.md`, `hooks/AGENTS.md`, `docs/AGENTS.md`, `skills/AGENTS.md`, `scripts/AGENTS.md`, `skills/atlas/SKILL.md`, `skills/atlas/references/workflows/prometheus.md`
- Tests: `tests/unit/test_plan_scoped_state.py`, `tests/conftest.py`

**Schema update**:
```json
{
  "pipeline_state": {
    "properties": {
      "plan_id": {
        "type": "string",
        "description": "Normalized safe plan identifier (optional - calculated on-the-fly if missing)"
      },
      "plan_name": {
        "type": "string",
        "description": "Original plan name"
      }
    }
  }
}
```

**plan_id handling**:
- `plan_id` is **optional** in schema (not in `required` array)
- Code that reads state must handle missing `plan_id` by calling `normalize_plan_id(plan_name)` on-the-fly
- New states always include both `plan_name` and `plan_id`
- Existing states without `plan_id` are valid; `plan_id` calculated from `plan_name` when needed

**state_validate.py update**:
```python
# Support plan-scoped validation
def validate_pipeline_state(state_path: str = None, plan_name: str = None):
    """Validate pipeline state file.

    Args:
        state_path: Direct path to state file (e.g., .atlas/notepads/my-plan/pipeline-state.json)
        plan_name: Plan name to look up (e.g., "my-plan" -> .atlas/notepads/my-plan/pipeline-state.json)

    If neither provided, falls back to legacy .atlas/pipeline-state.json
    """
    if state_path:
        path = state_path
    elif plan_name:
        plan_id = normalize_plan_id(plan_name)  # Need Python version of this
        path = f".atlas/notepads/{plan_id}/pipeline-state.json"
    else:
        path = ".atlas/pipeline-state.json"

    # ... existing validation logic ...
```

**Test scenarios**:
```python
def test_plan_id_sanitization():
    """Test plan name with spaces, dots, special chars"""
    assert normalize_plan_id("My Feature Plan") == "my-feature-plan"
    assert normalize_plan_id("feature.v2.0") == "featurev20"
    # Path traversal blocked
    assert len(normalize_plan_id("../../../etc/passwd")) == 12  # Hash

def test_detection_from_complex_paths():
    """Test detection with spaces, dots, subdirs"""
    # ...

def test_fallback_when_plan_unknown():
    """Test graceful fallback to legacy global state"""
    # ...

def test_lock_behavior_same_plan_concurrency():
    """Test two updates to same plan state are serialized"""
    # ...

def test_isolation_concurrent_different_plans():
    """Test two different plans can update simultaneously without conflict"""
    # This is the main bug we're fixing
    # ...
```

**Acceptance Criteria**:
- [ ] `state_validate.py` accepts `--plan` or direct path
- [ ] Schema includes `plan_id` field
- [ ] All docs updated with new state location
- [ ] Tests cover: sanitization, detection, fallback, same-plan locking, cross-plan isolation

---

## Design Decisions (Resolved)

These were resolved with user input:

| Question | Decision | Implementation |
|----------|----------|----------------|
| **Duplicate plan names** | Auto-suffix new ID | `generate_unique_plan_id()` appends timestamp if directory exists |
| **Legacy global state** | Fully deprecate | Migration archives to `.migrated.*`, no last-active pointer |
| **Detection failure** | Fallback to legacy | Empty `plan_name` → helpers use `.atlas/pipeline-state.json` |

---

## Dependencies

```
[1] normalize_plan_id, plan_lock_name
 ↓
[2] detect_plan_from_input, get_current_plan
 ↓
[3] get_plan_state_path, ensure_plan_state, get_plan_field, update_plan_state
 ↓
[4] pipeline-state-machine.sh, pipeline-transition.sh
[5] Hook handlers (parallel after 3)
 ↓
[6] diagnose-state.sh, state-recover.sh, state-restore.sh
[7] pipeline-state.sh CLI, atlas-plan.md
 ↓
[8] Migration script
[9] Validation, schema, docs, tests
```

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Breaking existing sessions | Keep legacy global state functions, fallback chain |
| Plan name not available | Detection utilities with multi-source fallback chain |
| Special chars in plan names | Slugification with hash fallback for unsafe names |
| Same-plan concurrent access | Per-plan locking with `plan_lock_name` |
| jq/mkdir/mv failures | Error handling that logs but doesn't clobber state |
| Disk full | Atomic tmp+mv pattern, error detection before overwrite |
| Orphaned state files | State files in notepads/ cleaned with plan deletion |
| Migration concurrent sessions | Locking + archive instead of delete |

## Verification

After implementation:
1. Run two `/atlas-plan` commands concurrently with different plan names
2. Verify each session's state is isolated in `.atlas/notepads/{plan-id}/`
3. Verify `momus_iterations` updates don't conflict
4. Verify completion of one plan doesn't affect the other
5. Run migration script on existing global state
6. Verify `pipeline-state.sh list` shows all active plans
7. Verify fallback to legacy global state works during transition
