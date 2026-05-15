# Five-skill agent-facing bundle for v2

Maestro v2 ships five bundled skills the agent reaches for:

- **maestro-setup**: one-time per-project init and `--migrate-v2`.
- **maestro-design**: structured spec authoring. Q&A for acceptance criteria, non-goals, risk class, mode selection.
- **maestro-plan**: heavy-mode workflow. `from-spec` → `decompose` → orchestrate N child tasks.
- **maestro-task**: light-mode workflow. `from-spec` → `claim` → ralph loop → handoff.
- **maestro-verify**: verify subroutine, called from task/plan. Handles loop-until-PASS/HUMAN/BLOCK; routes FAIL through observability before retry.

Observability (`see`) is baked into `maestro-verify` on FAIL rather than shipping as a standalone skill. Handoff is baked into `maestro-task` at session boundaries. Today's 10-skill bundle (brainstorm, classify, handoff, intake, mission, plan, qa, setup, task, verify) collapses into these five; the rest become documentation or are absorbed.

Rejected: 7-skill bundle (more surface to maintain); 3-skill collapse (`maestro-work` becomes a god-skill); keep-10-with-renames (defeats the small-stable-context principle).
