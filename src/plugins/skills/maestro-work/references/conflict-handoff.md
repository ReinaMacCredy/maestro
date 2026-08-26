# Conflict handoff

Use this recipe when sessions overlap, a shared file is contested, or a
worktree branch needs a deliberate merge owner.

## Detect and contain

- Read `maestro status`, relevant dispatches and handbacks, `git status --short --branch`, and
  `git worktree list` before editing.
- Use `herdr pane list` to find the lane pane. Send immediate coordination with
  `herdr pane send-text <pane-id> "<coordination note>"`.
- Name the contested paths and stop overlapping writes until ownership is
  explicit.
- Keep the ownership request and scope in the `maestro dispatch` record. Do not
  hide the durable boundary only in pane text.

## Handoff packet

File the result with `maestro handback file <dispatch-id>`, including the work
id, source branch, verified HEAD, target branch, owned paths, checks run, dirty
state, remaining blocker, and exact next command. Preserve unrelated changes
and state whether the branch can fast-forward cleanly.

## Merge-back

One owner reconciles and merges with normal git commands. If conflict intent
is unclear, stop and ask the author instead of choosing semantics. Re-run the
affected checks after resolution, report the landed commit, then resolve the
recorded blocker only when the shared branch contains the result.
