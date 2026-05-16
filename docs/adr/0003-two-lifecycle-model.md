# Two-lifecycle model: task and exec-plan

Maestro adopts two distinct state machines:

- **Task lifecycle**: `draft -> claimed -> doing <-> verifying <-> blocked -> ready -> shipped`, plus `abandoned` from any state. A task is one PR-shaped unit of work.
- **Exec-plan lifecycle**: `intake -> specified -> planned -> in-progress -> completed`, plus `cancelled` from any state. An exec-plan owns N tasks.

Light mode creates a task directly with no exec-plan. Heavy mode runs an exec-plan that owns N tasks, each running the task lifecycle internally. Mode is chosen by the agent at task creation.

Rejected: (B) single unified lifecycle with skip-fields for light mode (too heavy for small fixes); (C) minimal 3-state TODO/DOING/DONE (not auditable enough); (D) event-sourced state (harder to debug).
