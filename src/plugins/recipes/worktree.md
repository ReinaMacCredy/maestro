# Worktree

Use this recipe to isolate concurrent changes or a bounded delivery lane. Git
owns branches and worktrees; Maestro records the work, decisions, and notes
that make the lane understandable.

## Create the lane

1. Read `git status --short --branch`, `git worktree list`, `maestro status`, and the
   target `maestro work show <id>`.
2. Choose a unique branch and path from the intended base commit.
3. Create it with normal git commands, then confirm branch, HEAD, and path.
4. Use `herdr pane list` to identify the lane pane, then record its value, the
   owned paths, and the stop condition in a `maestro work note` when another
   session owns the bounded work.
5. Use `herdr pane send-text <pane-id> "<coordination note>"` for immediate
   coordination; keep durable scope and outcomes in work notes.

## Commands that work here

```
git worktree add -b <branch> .maestro/worktree/<slug> <base>
ln -s "$(git rev-parse --show-toplevel)/node_modules" .maestro/worktree/<slug>/node_modules   # bun/npm repos only
# ... edit, test, commit on <branch> ...
git -C "$(git rev-parse --show-toplevel)" merge --no-ff <branch>
git worktree remove --force .maestro/worktree/<slug>   # --force: the node_modules link makes the tree look dirty
git branch -d <branch>
```

## Work and verify

- Keep edits inside the declared path boundary and commit one logical change
  at a time.
- Reconcile the branch against its target before handoff.
- Run the focused and full checks required by the work item.
- Record the verified HEAD and any residual blocker in a work note or handback.

## Return and clean up

Return maestro-work's handback packet with the branch, commit, target,
verification commands, dirty state, and exact next git command. The receiving
owner performs merge-back. Remove the worktree and branch only after the landed
commit is verified and no live owner or conflict remains.

## Prune worktrees and branches

Pruning deletes local state with no code review to catch a slip, so these
gates are the review. They come from pstack's worktree-cleanup playbook
(Lauren Tan, MIT; notice in THIRD_PARTY.md).

1. Read the candidates from `git worktree list`, never from hand-typed paths,
   because a worktree can live outside `.maestro/worktree/`. For each one,
   note its size, age, branch, uncommitted work, PR state, and whether the
   branch is merged into the base its PRs target, not only `origin/HEAD`.
   When the goal is disk space, record `df -h /` first.
2. A classification is advice, not permission. Cross-check every candidate
   against what is live: `maestro status` for live sessions and held work,
   and the owner's word on which sessions are in use. A live
   session can own sibling worktrees whose names appear nowhere else. When the
   owner's set and your classification disagree, the owner's set wins.
3. Pause on irreversible loss. For a worktree with tracked uncommitted edits,
   show the owner its diff and wait for a decision, because a clean worktree is
   recoverable from its branch and uncommitted work is not. Untracked
   throwaway files may go, but name them. A clean, merged worktree that
   nothing uses proceeds without asking.
4. Prune the confirmed set with `git worktree remove --force <path>` per path.
   If the directory survives on ignored build artifacts, delete it, then run
   `git worktree prune`. Delete a branch only with `git branch -d`, which
   refuses an unmerged branch. An unmerged branch is irreversible loss and
   goes to the owner like uncommitted work. Confirm by listing again, and with
   `df -h /` when space was the goal.

Report the worktrees pruned, a one-line reason for each one held back (in use
by which session, or uncommitted work), and the space reclaimed when space was
the goal.
