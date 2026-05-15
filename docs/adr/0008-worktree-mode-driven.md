# Worktree binding is mode-driven

Heavy mode (exec-plan with N child tasks) auto-creates a worktree per child task at `task claim`, because exec-plans imply parallel/concurrent work where the article's per-worktree isolation pays off. Light mode (standalone task) runs in the current checkout by default; an opt-in `--worktree` flag enables isolation when the user explicitly wants it.

Rationale: maestro's typical solo Claude Code session does not need worktree-per-task ceremony; spinning up a new branch directory for every small-spec fix would be friction without benefit. Parallel scenarios (exec-plan with multiple agents working concurrently) are the case where isolation matters.

Rejected: always-worktree (too heavy for solo light-mode); worktree-only-via-explicit-verb (agents forget; article isolation benefits rarely materialize); one-worktree-per-plan-shared (defeats isolation purpose).
