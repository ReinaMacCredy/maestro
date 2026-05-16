# Task to PR is strictly 1:1

Every task produces exactly one PR. The task lifecycle's `shipped` state means the PR is merged. `ready` means the PR is open with green CI and a PASS verdict. `verifying` means post-edit pre-PR checks are running. Verdicts continue to bind to `(task_id, PR, tree_sha)`.

If a unit of work needs multiple PRs (prep + main; refactor + behavior change), it must be promoted to an exec-plan with N child tasks, each producing one PR. The exec-plan owns the multi-PR coordination; the task stays one-PR-shaped.

This matches the article's "PRs are short-lived" stance and gives the state machine a clean anchor (PR state) for the auto-exit transitions in ADR-0004.

Rejected: 1:N (state machine becomes set-based, harder to reason about); loose coupling (weakens the verdict-as-merge-anchor invariant); configurable per task (every option is another mode the agent reasons about).
