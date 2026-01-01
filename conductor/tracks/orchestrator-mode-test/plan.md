# Plan: Orchestrator Mode Test

## Epic

Validate orchestrator Light mode with parallel workers

## Tasks

### 1. Worker Execution

#### 1.1 Count files in skills/
- Worker BlueStar counts files in skills/ directory
- Returns structured result with count
- **Files:** (read-only)

#### 1.2 Count files in conductor/
- Worker GreenMountain counts files in conductor/ directory  
- Returns structured result with count
- **Files:** (read-only)

### 2. Verification

#### 2.1 Verify results
- Check both workers returned SUCCEEDED status
- Verify beads are closed
- Display aggregated summary
- **Files:** (verification only)

## Track Assignments

| Track | Agent | Tasks | File Scope | Depends On |
|-------|-------|-------|------------|------------|
| 1 | BlueStar | 1.1 | skills/** | - |
| 2 | GreenMountain | 1.2 | conductor/** | - |
| 3 | (main) | 2.1 | - | 1, 2 |

## Orchestration Config

```yaml
mode: auto  # Will select LIGHT (no cross-deps)
max_workers: 2
heartbeat_required: false
```

## Verification

```bash
# Check beads status
bd list --json | jq '.[] | select(.status == "closed")'

# Verify epic closed
bd show <epic-id>
```
