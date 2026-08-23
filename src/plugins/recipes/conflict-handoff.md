# Conflict handoff

Use this recipe when sessions overlap, a shared file is contested, or a
worktree branch needs a deliberate merge owner.

## Detect and contain

- Read `status`, relevant messages, `git status --short --branch`, and
  `git worktree list` before editing.
- Name the contested paths and stop overlapping writes until ownership is
  explicit.
- Use `msg send <session> "..."` for the ownership request or handoff. Do not
  hide coordination only in chat.

## Handoff packet

Send the work id, source branch, verified HEAD, target branch, owned paths,
checks run, dirty state, remaining blocker, and exact next command. Preserve
unrelated changes and state whether the branch can fast-forward cleanly.

## Merge-back

One owner reconciles and merges with normal git commands. If conflict intent
is unclear, stop and ask the author instead of choosing semantics. Re-run the
affected checks after resolution, report the landed commit, then clear the
coordination message or note only when the shared branch contains the result.
