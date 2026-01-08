# Revert Workflow

## Purpose
Git-aware assistant for reverting logical units of work (Tracks, Phases, Tasks) tracked by Conductor.

## Prerequisites
- Conductor environment initialized
- `conductor/tracks.md` exists and is not empty
- Git repository with commit history
- Work to revert has associated commits

## State Management
This workflow does not use persistent state. Operations are atomic with user confirmation at each step.

## Workflow Steps

### Phase 1: Target Selection

1. **Check User Input**
   - If target provided as argument: go to Direct Confirmation (Path A)
   - If no target: go to Guided Selection (Path B)

2. **Path A: Direct Confirmation**
   - Find referenced track/phase/task in `tracks.md` or `plan.md`
   - Ask confirmation: "You asked to revert [type]: '[description]'. Correct?"
   - Options: A) Yes, B) No
   - If No: ask clarifying questions
   - Establish `target_intent`

3. **Path B: Guided Selection**
   - Scan `conductor/tracks.md` and all `conductor/tracks/*/plan.md`
   - **Priority 1**: Find all `[~]` (in-progress) items
   - **Priority 2**: If no in-progress, find 5 most recent `[x]` items
   - Present hierarchical menu grouped by track:
     ```
     Track: track_20251208_user_profile
       1) [Phase] Implement Backend API
       2) [Task] Update user model
     
     3) A different Track, Task, or Phase
     ```
   - Process selection or engage dialogue for clarification

### Phase 2: Git Reconciliation

1. **Find Implementation Commits**
   - Extract SHA(s) recorded in `plan.md` for target items
   - Verify each SHA exists in git history

2. **Handle Rewritten History (Ghost Commits)**
   - If SHA not found: announce
   - Search git log for similar commit message
   - Ask user to confirm replacement
   - If not confirmed: halt

3. **Find Plan-Update Commits**
   - For each implementation commit
   - Search git log for subsequent commit that modified `plan.md`
   - Associate with implementation commit

4. **Track Creation Commit** (Track revert only)
   - Search `git log -- conductor/tracks.md`
   - Find commit that introduced the track heading
   - Add to revert list

5. **Compile Final List**
   - All implementation commits
   - All plan-update commits
   - Track creation commit (if reverting track)
   - Identify merge commits (warn user)
   - Check for cherry-pick duplicates

### Phase 3: Execution Plan

1. **Present Summary**
   ```
   REVERT PLAN
   ───────────
   Target: Revert [Task/Phase/Track] '[Description]'
   
   Commits to Revert (in order):
   1. <sha_plan_commit>  - 'conductor(plan): Mark task complete'
   2. <sha_code_commit>  - 'feat: Add user profile'
   
   Action: Execute `git revert` in reverse chronological order
   
   ⚠️ Warnings:
   - [Any merge commits or complexities]
   ```

2. **Final Confirmation**
   - "Do you want to proceed? (yes/no)"
   - Options: A) Yes, B) No
   - If No: ask for correct plan clarification

### Phase 4: Execution

1. **Execute Reverts**
   - Run `git revert --no-edit <sha>` for each commit
   - Start from most recent, work backward
   - Verify each command succeeds

2. **Handle Conflicts**
   - If merge conflict occurs: halt
   - Provide manual resolution instructions:
     ```
     Merge conflict detected. To resolve:
     1. Run: git status (see conflicted files)
     2. Edit conflicted files
     3. Run: git add <files>
     4. Run: git revert --continue
     ```

3. **Verify Plan State**
   - Read relevant `plan.md` after reverts
   - Confirm reverted items show correct status
   - If incorrect: edit to fix, commit correction

4. **Announce Completion**
   - Report success
   - Show final state of reverted items
   - Confirm plan is synchronized

## Revert Scope Reference

| Target | What Gets Reverted |
|--------|-------------------|
| Task | Task commit + plan update commit |
| Phase | All task commits in phase + plan updates + checkpoint commit |
| Track | All phase commits + track creation commit + tracks.md update |

## Error Handling

| Error | Action |
|-------|--------|
| `tracks.md` missing/empty | Halt, direct to `/conductor:setup` |
| No revert candidates | Announce, halt |
| SHA not in git history | Announce "ghost commit", search for replacement |
| User rejects replacement | Halt |
| Merge conflict | Halt, provide resolution steps |
| Revert command fails | Halt, report error |

## Git Commands Used

```bash
# Find commits
git log --oneline
git log -- <file>
git log -1 --format="%H"

# Verify commit exists
git cat-file -t <sha>

# Execute revert
git revert --no-edit <sha>

# Handle conflicts
git status
git revert --continue
git revert --abort
```

## User Confirmation Points

1. **Target Selection**: Confirm correct item identified
2. **Ghost Commit Resolution**: Confirm replacement commit
3. **Final Execution Plan**: Go/No-go before any reverts
4. **Conflict Resolution**: Manual intervention if needed

## Output

- Git reverts applied
- `plan.md` status updated
- `tracks.md` updated (if reverting track)
- No orphaned state
