# Status Workflow

## Purpose
Display a comprehensive progress overview of all tracks and tasks in the project.

## Prerequisites
- Conductor environment initialized
- Required files exist:
  - `conductor/tech-stack.md`
  - `conductor/workflow.md`
  - `conductor/product.md`
  - `conductor/tracks.md`

## State Management
This workflow is read-only and does not modify any state.

## Workflow Steps

### Phase 1: Setup Verification

1. **Check Tracks File**
   - Verify `conductor/tracks.md` exists
   - Verify file is not empty
   - If missing/empty: Halt with setup instructions

2. **Check Required Files**
   - Verify core conductor files exist
   - If missing: Halt with setup instructions

### Phase 2: Data Collection

1. **Read Tracks File**
   - Parse `conductor/tracks.md`
   - Extract all track entries

2. **Read Individual Plans**
   - List directories in `conductor/tracks/`
   - For each track, read `plan.md`

### Phase 3: Parse and Analyze

1. **Parse Track Statuses**
   - `[ ]` = New/Pending
   - `[~]` = In Progress
   - `[x]` = Completed

2. **Parse Task Statuses**
   - Identify phases (markdown headings)
   - Identify tasks (checkbox items)
   - Count by status

3. **Calculate Metrics**
   - Total phases
   - Total tasks
   - Tasks completed/in-progress/pending
   - Completion percentage

### Phase 4: Generate Report

1. **Report Structure**
   ```
   ┌─────────────────────────────────────────┐
   │           PROJECT STATUS REPORT         │
   ├─────────────────────────────────────────┤
   │ Current Date/Time: YYYY-MM-DD HH:MM:SS  │
   │ Project Status: [On Track|Behind|Blocked]│
   ├─────────────────────────────────────────┤
   │ CURRENT WORK                            │
   │ • Phase: <current phase name>           │
   │ • Task: <current task in progress>      │
   │ • Next: <next pending task>             │
   ├─────────────────────────────────────────┤
   │ BLOCKERS                                │
   │ • <any items marked as blocked>         │
   ├─────────────────────────────────────────┤
   │ PROGRESS                                │
   │ • Tracks: X/Y completed                 │
   │ • Phases: X total                       │
   │ • Tasks: X/Y (Z% complete)              │
   └─────────────────────────────────────────┘
   ```

2. **Status Determination**
   - **On Track**: Work in progress, no blockers
   - **Behind Schedule**: Stale in-progress items (optional heuristic)
   - **Blocked**: Explicit blockers or errors noted

### Phase 5: Present Report

1. **Output Format**
   - Clear, readable format
   - Use tables or structured text
   - Highlight current focus areas

2. **Optional Details**
   - List all tracks with status
   - Show phase breakdown per track

## Report Fields

| Field | Description |
|-------|-------------|
| Current Date/Time | Timestamp of report |
| Project Status | High-level health indicator |
| Current Phase | Active phase name |
| Current Task | Task marked `[~]` |
| Next Action | First `[ ]` task |
| Blockers | Any noted blockers |
| Phases (total) | Count of all phases |
| Tasks (total) | Count of all tasks |
| Progress | completed/total (percentage) |

## Error Handling

| Error | Action |
|-------|--------|
| `tracks.md` missing | Halt, direct to `/conductor:setup` |
| `tracks.md` empty | Halt, direct to `/conductor:setup` |
| `plan.md` missing for track | Note in report, continue |
| Parse error | Report warning, continue with available data |

## Output

Status report is presented to user. No files are modified.

## Example Output

```
📊 PROJECT STATUS REPORT
========================
Generated: 2024-01-15 14:30:00

Overall Status: ✅ On Track

CURRENT FOCUS
─────────────
Track: user_auth_20240115
Phase: Phase 2 - Backend Implementation
Task:  [~] Implement JWT token generation
Next:  [ ] Add refresh token logic

BLOCKERS
────────
None identified

PROGRESS SUMMARY
────────────────
Tracks:  1/3 completed (33%)
Phases:  8 total
Tasks:   12/25 completed (48%)

TRACK BREAKDOWN
───────────────
[x] Track: project_setup_20240110
[~] Track: user_auth_20240115
[ ] Track: dashboard_ui_20240120
```
