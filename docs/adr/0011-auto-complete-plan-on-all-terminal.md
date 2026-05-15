# Exec-plan auto-completes when all child tasks reach a terminal state

An exec-plan transitions from `in-progress` to `completed` automatically when every child task is in `shipped` or `abandoned`. The completion record captures the breakdown (e.g. "4 shipped, 1 abandoned"). No manual `plan complete` step; no aggregate verdict; abandoned tasks do not auto-cancel the plan.

This mirrors the task-level auto-exit pattern (ADR-0004): the harness reacts to the result of verbs the agent just called, not background polling. When the last child task ships or is abandoned, the verb that drove that transition also evaluates the parent plan and completes it if eligible.

Rejected: manual `plan complete` (ceremony, plans rot in-progress); aggregate verdict on plan (another verdict layer); strict all-shipped-or-cancel (one abandoned task voids whole plan, even if rest was valuable).
