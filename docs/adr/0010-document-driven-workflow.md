# Document-driven workflow: spec markdown is the source of truth

Both light and heavy mode start by writing a spec markdown file. There is no `maestro task new "<title>"` shortcut and no interactive intake verb. The agent (or human) authors the spec; maestro ingests it via `maestro task from-spec <path>` (light) or `maestro plan from-spec <path>` (heavy), creating the task or plan entity from the spec content.

Specs live at `.maestro/specs/<slug>.md` with YAML frontmatter carrying `acceptance_criteria`, `non_goals`, `risk_class`, and `mode` (light|heavy). Body is freeform markdown for context and design rationale. AGENTS.md explicitly points agents at `.maestro/specs/` because maestro state is not under `docs/` by default.

Rationale: file-shaped intent aligns with "what Codex can't see doesn't exist": every task is anchored to a versioned, grep-able artifact. No magic in workflow entry; the agent always writes the spec first.

Rejected: separate light/heavy entry verbs with inline title (less file-anchored); single unified entry with hidden classification (chaos at a different layer); always-heavy (light path ceremony tax).
