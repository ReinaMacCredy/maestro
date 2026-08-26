# Worktree

Use this recipe to isolate concurrent changes or a bounded delivery lane. Git
owns branches and worktrees; Maestro records the work, decisions, notes,
dispatches, and handbacks that make the lane understandable.

## Create the lane

1. Read `git status --short --branch`, `git worktree list`, `maestro status`, and the
   target `maestro work show <id>`.
2. Choose a unique branch and path from the intended base commit.
3. Create it with normal git commands, then confirm branch, HEAD, and path.
4. Use `herdr pane list` to identify the lane pane, then record its value, the
   owned paths, and the stop condition with `maestro dispatch open --pane <pane-id>`
   when another session owns the bounded work.
5. Use `herdr pane send-text <pane-id> "<coordination note>"` for immediate
   coordination; keep durable scope and outcomes in the dispatch and handback.

## Work and verify

- Keep edits inside the declared path boundary and commit one logical change
  at a time.
- Reconcile the branch against its target before handoff.
- Run the focused and full checks required by the work item.
- Record the verified HEAD and any residual blocker in a work note or handback.

## Return and clean up

File `maestro handback file <dispatch-id>` with the branch, commit, target,
verification commands, dirty state, and exact next git command. The receiving
owner performs merge-back. Remove the worktree and branch only after the landed
commit is verified and no live owner or conflict remains.
