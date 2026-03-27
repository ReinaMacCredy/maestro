## Discovery
The current feature needs a cross-agent handoff to Claude, but Maestro's handoff flow only works after a valid plan exists, the plan has been approved, and the derived tasks have been synced. This repository's current active feature is `handoff-pipeline-test`, and `maestro status --json` showed that the feature was still in `planning` with no plan file and no tasks. The requested work is therefore to convert the provided handoff outline into a valid Maestro plan so the CLI can export a Claude-ready handoff artifact safely and deterministically.

### 1. Write plan into Maestro state
Use the repository root as the working directory and write this handoff-focused plan for the active feature so Maestro has a valid plan file with numbered task headings.

### 2. Approve the plan
Approve the written plan for `handoff-pipeline-test` so the workflow can advance beyond planning and allow downstream task generation.

### 3. Sync tasks from the plan
Run task sync so the numbered plan sections become executable Maestro tasks, which is a required prerequisite for `handoff-plan`.

### 4. Generate the Claude handoff
Create a cross-agent handoff targeted at `claude` for the `handoff-pipeline-test` feature and capture the resulting handoff artifact path and summary.

### 5. Verify Claude pickup
Verify that `maestro handoff-pickup --feature handoff-pipeline-test --json` succeeds and returns the quickstart instructions Claude should follow.
