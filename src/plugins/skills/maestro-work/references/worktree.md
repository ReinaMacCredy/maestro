# Worktree

Use this recipe to isolate concurrent changes or a bounded delivery lane. Git
owns branches and worktrees; Maestro records the work, decisions, notes, and
messages that make the lane understandable.

## Create the lane

1. Read `git status --short --branch`, `git worktree list`, `maestro status`, and the
   target `maestro work show <id>`.
2. Choose a unique branch and path from the intended base commit.
3. Create it with normal git commands, then confirm branch, HEAD, and path.
4. Send the lane, owned paths, and merge target to peers with `maestro msg send` when
   another session may overlap.

## Work and verify

- Keep edits inside the declared path boundary and commit one logical change
  at a time.
- Reconcile the branch against its target before handoff.
- Run the focused and full checks required by the work item.
- Record the verified HEAD and any residual blocker in a work note or message.

## Return and clean up

Hand back the branch, commit, target, verification commands, dirty state, and
exact next git command. The receiving owner performs merge-back. Remove the
worktree and branch only after the landed commit is verified and no live owner
or conflict remains.
