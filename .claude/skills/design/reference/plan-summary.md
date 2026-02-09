# Plan Summary Display Template

When the plan is ready (leviathan PASS, or quick mode, or max loops reached):

1. Extract the plan content from Prometheus's PLAN READY message (everything after the first line `PLAN READY`). The full plan markdown is inline in the message body — do NOT try to read it from a file path.
2. Parse the plan content and display a structured summary:

   **Parse these sections from the plan markdown:**
   - **Title**: First line starting with `# ` (single `#`)
   - **Objective**: Content after `## Objective` heading (take first sentence only)
   - **Scope In**: Count bullet points under `**In**:` in `## Scope`
   - **Scope Out**: Count bullet points under `**Out**:` in `## Scope`
   - **Tasks**: Count lines matching `- [ ] Task N:` pattern. For each, extract `**Agent**:` value. Group by agent type.
   - **Key Decisions**: From `## Notes` section, extract first 3 numbered or bulleted items (take the bold title only, e.g., "Two tasks, sequential dependency")

   **Display this summary to the user:**

   ```
   ---
   ## Plan Summary

   **{Plan Title}**

   **Objective**: {first sentence of Objective section}

   **Scope**: {N} items in | {M} items out

   **Tasks**: {total} total — {breakdown by agent, e.g., "2 spark, 1 kraken"}

   **Dependency Chain**:

   Parse the dependency graph from each task's `**Dependencies**:` field and display as a blockedBy list:

   For each task, show what blocks it:
   > T1: {short title} `[agent]`
   > T2: {short title} `[agent]`
   > T3: {short title} `[agent]` — blocked by T1, T2
   > T4: {short title} `[agent]` — blocked by T1, T2
   > T5: {short title} `[agent]` — blocked by T3, T4

   Tasks with no dependencies have no suffix. Tasks with dependencies show `— blocked by T{N}, T{M}`.

   **Execution Phases**:

   Parse the dependency graph and group tasks into sequential phases:
   - **Phase 1**: Tasks with no dependencies (can all run in parallel)
   - **Phase 2**: Tasks whose dependencies are all in Phase 1
   - **Phase N**: Tasks whose dependencies are all satisfied by prior phases
   - Tasks in the same phase run in parallel

   Display each phase on its own line with task number, short title, and agent:

   > **Phase 1** — T1: {short title} `[spark]`, T2: {short title} `[kraken]`
   > **Phase 2** — T3: {short title} `[spark]`
   > **Phase 3** — T4: {short title} `[kraken]`, T5: {short title} `[spark]`

   If all tasks are independent (single phase), show:
   > **Phase 1** — T1: {title} `[agent]`, T2: {title} `[agent]`, ... *(all parallel)*

   **Key Decisions**:
   - {decision 1}
   - {decision 2}
   - {decision 3}

   **Dependency Flow**:

   Generate an ASCII dependency flowchart. Parse dependencies from each task's `**Dependencies**:` field and render:

   - **Row 0**: Tasks with no dependencies (side-by-side if multiple)
   - **Subsequent rows**: Tasks whose dependencies are all in prior rows (side-by-side = parallel)
   - Connect rows with `│` and `▼` arrows
   - Use `┌───┐ └───┘` box-drawing for task boxes
   - Branch with `┌───┴───┐`, merge with `└───┬───┘`

   Example (T1 no deps, T2+T3 depend on T1, T4 depends on T2+T3):
   ```
   ┌──────────────────────────────────┐
   │ T1: Set up scaffolding   [kraken]│
   └──────────────────────────────────┘
               │
       ┌───────┴───────┐
       ▼               ▼
   ┌────────────────┐  ┌────────────────┐
   │ T2: Auth [kraken]│  │ T3: Config [spark]│
   └────────────────┘  └────────────────┘
       │               │
       └───────┬───────┘
               ▼
   ┌──────────────────────────────────┐
   │ T4: Integration tests   [kraken]│
   └──────────────────────────────────┘
   ```

   If all tasks are independent, show them side-by-side with: `All tasks run in parallel — no dependencies.`

   ---
   ```

3. If leviathan had remaining concerns after max loops, note them for the user
4. Ask the user to approve, reject, or request revisions:

```
AskUserQuestion(
  questions: [{
    question: "Prometheus has drafted the plan. How would you like to proceed?",
    header: "Plan Review",
    options: [
      { label: "Approve", description: "Accept the plan and save it" },
      { label: "Revise", description: "Send feedback to Prometheus for changes" },
      { label: "Cancel", description: "Discard the plan and clean up" }
    ],
    multiSelect: false
  }]
)
```

**On Approve** → Continue to Step 9.

**On Revise** → Send feedback to Prometheus:
```
SendMessage(
  type: "message",
  recipient: "prometheus",
  summary: "User requests revision",
  content: "REVISE\n{user's feedback}"
)
```
Then wait for the next `PLAN READY` message from Prometheus and repeat Step 8.

**On Cancel** → Skip to Step 11 (Cleanup) without saving.
