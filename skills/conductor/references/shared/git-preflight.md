# Git Preflight

Reference for git branch checking before track operations.

## Purpose

Prevents accidental work on `main`/`master` by:
1. Detecting when user is on the main branch
2. Checking for uncommitted changes (dirty state)
3. Prompting to create a feature branch

## Trigger Points

| Command | When Preflight Runs |
|---------|---------------------|
| `/conductor-newtrack` | Phase 0.5 (after setup validation) |
| `/conductor-implement` | Phase 0 (before track selection) |

## Decision Tree

```
                    ┌─────────────────┐
                    │ Get current     │
                    │ branch name     │
                    └────────┬────────┘
                             │
                    ┌────────▼────────┐
                    │ On main/master? │
                    └────────┬────────┘
                             │
              ┌──────────────┴──────────────┐
              │                              │
        ┌─────▼─────┐                  ┌─────▼─────┐
        │    YES    │                  │    NO     │
        └─────┬─────┘                  └─────┬─────┘
              │                              │
     ┌────────▼────────┐              ┌──────▼──────┐
     │ Check git       │              │ Proceed     │
     │ status          │              │ silently    │
     └────────┬────────┘              └─────────────┘
              │
    ┌─────────┴─────────┐
    │                    │
┌───▼───┐          ┌────▼────┐
│ DIRTY │          │  CLEAN  │
└───┬───┘          └────┬────┘
    │                   │
┌───▼───────────┐  ┌────▼────────────────┐
│ HALT:         │  │ Prompt:             │
│ "Uncommitted  │  │ "Create branch      │
│ changes on    │  │ feat/{id}? [Y/n]"   │
│ {branch}"     │  └─────────────────────┘
└───────────────┘
```

## Bash Implementation

```bash
#!/bin/bash
# Git Preflight Check
# Usage: source this or inline in command execution

TRACK_ID="${1:-unknown}"

# 0.5.1 Get current branch
BRANCH=$(git branch --show-current 2>/dev/null)
if [ -z "$BRANCH" ]; then
    echo "⚠️ Not in a git repository or detached HEAD"
    exit 0  # Allow proceeding - not a blocker
fi

# 0.5.2 Check if on main/master
if [ "$BRANCH" = "main" ] || [ "$BRANCH" = "master" ]; then
    
    # 0.5.3 Check for uncommitted changes
    if [ -n "$(git status --porcelain)" ]; then
        echo "⚠️ Uncommitted changes on $BRANCH."
        echo "   Commit or stash changes before creating a track."
        exit 1
    fi
    
    # 0.5.4 Generate branch name
    NEW_BRANCH="feat/${TRACK_ID}"
    
    # 0.5.5 Check if branch already exists
    if git show-ref --verify --quiet "refs/heads/$NEW_BRANCH"; then
        # Try with -v2 suffix
        NEW_BRANCH="feat/${TRACK_ID}-v2"
        
        # If still exists, try -v3, etc.
        SUFFIX=2
        MAX_RETRIES=10
        while git show-ref --verify --quiet "refs/heads/$NEW_BRANCH"; do
            SUFFIX=$((SUFFIX + 1))
            NEW_BRANCH="feat/${TRACK_ID}-v${SUFFIX}"
            if [ "$SUFFIX" -gt "$MAX_RETRIES" ]; then
                echo "Error: Too many existing branches for ${TRACK_ID}" >&2
                exit 1
            fi
        done
        
        echo "ℹ️ Branch feat/${TRACK_ID} exists. Using ${NEW_BRANCH}"
    fi
    
    # 0.5.6 Prompt for branch creation
    echo ""
    echo "On $BRANCH. Create branch '$NEW_BRANCH'? [Y/n]"
    read -r response
    
    if [ "$response" != "n" ] && [ "$response" != "N" ]; then
        # Fetch latest from origin (ignore errors for offline work)
        git fetch origin 2>/dev/null || true
        
        # Create and checkout new branch
        git checkout -b "$NEW_BRANCH"
        
        if [ $? -eq 0 ]; then
            echo "✓ Created and switched to branch: $NEW_BRANCH"
            # Output for capturing by caller
            echo "GIT_PREFLIGHT_BRANCH=$NEW_BRANCH"
        else
            echo "✗ Failed to create branch"
            exit 1
        fi
    else
        echo "ℹ️ Staying on $BRANCH"
        echo "GIT_PREFLIGHT_BRANCH="
    fi
else
    # Already on feature branch - proceed silently
    echo "GIT_PREFLIGHT_BRANCH=$BRANCH"
fi
```

## Integration with metadata.json

When a branch is created via preflight, store it in the workflow object:

```json
{
  "workflow": {
    "state": "TRACKED",
    "branch": "feat/ux-automation_20251227",
    "history": [...]
  }
}
```

## Update metadata.json Snippet

```bash
# After successful branch creation
if [ -n "$NEW_BRANCH" ]; then
    # Update metadata.json with branch name
    jq --arg branch "$NEW_BRANCH" \
       '.workflow.branch = $branch' \
       "conductor/tracks/${TRACK_ID}/metadata.json" > tmp.$$ && \
       mv tmp.$$ "conductor/tracks/${TRACK_ID}/metadata.json"
fi
```

## Error Scenarios

| Scenario | Behavior | Exit Code |
|----------|----------|-----------|
| Not a git repo | WARN, continue | 0 |
| Detached HEAD | WARN, continue | 0 |
| Dirty on main/master | HALT with message | 1 |
| Branch exists | Auto-suffix (-v2, -v3) | 0 |
| User declines | Continue on main | 0 |
| Checkout fails | HALT with error | 1 |
| Offline (fetch fails) | WARN, create anyway | 0 |

## Test Matrix

| Branch | Git Status | User Choice | Expected Result |
|--------|------------|-------------|-----------------|
| `main` | clean | Y | Create `feat/{id}`, checkout |
| `main` | clean | n | Stay on main, proceed |
| `main` | dirty | — | HALT: "Uncommitted changes" |
| `master` | clean | Y | Create `feat/{id}`, checkout |
| `master` | dirty | — | HALT: "Uncommitted changes" |
| `feat/X` | clean | — | Proceed silently |
| `feat/X` | dirty | — | Proceed (user's responsibility) |
| `feat/{id}` exists | Y | — | Create `feat/{id}-v2` |
| Not git repo | — | — | WARN, proceed |

## Inline in newTrack.toml

The git preflight should be executed as **Phase 0.5** in the newTrack workflow:

```
Phase 0: Parse flags
Phase 0.5: Git preflight ← INSERT HERE
Phase 1: Setup check
Phase 1.3: State files initialization
...
```

## Agent Integration

For AI agents, the preflight should:

1. **Check branch silently** - don't prompt interactively
2. **Auto-accept branch creation** - assume Y for [Y/n] prompts
3. **Store branch in metadata** - for audit trail
4. **Report in handoff** - include branch name in completion message

```
# Agent-friendly version
if on main/master AND clean:
    auto-create feat/{track_id} branch
    store in metadata.workflow.branch
```
