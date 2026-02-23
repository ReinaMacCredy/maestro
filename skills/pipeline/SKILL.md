---
name: pipeline
description: "Runs sequential agent chains with context passed between stages. Use when work should flow through ordered specialist steps."
argument-hint: "<preset> | agent1 -> agent2 'task'"
allowed-tools: Read, Write, Edit, Grep, Glob, Bash, Task, TeamCreate, TeamDelete, SendMessage, TaskCreate, TaskList, TaskUpdate, TaskGet
---

# Pipeline Mode

> Sequential agent chains where each stage's output feeds the next.

## Pipeline Request

`$ARGUMENTS`

---

## Arguments

| Argument | Description |
|----------|-------------|
| `<preset>` | Built-in pipeline: `review`, `implement`, `debug` |
| `agent1 -> agent2 'task'` | Custom pipeline with explicit agent chain |
| `agent:model -> agent:model` | Custom pipeline with model override per stage |

## Built-in Presets

| Preset | Chain | Use Case |
|--------|-------|----------|
| `review` | explore -> leviathan -> kraken | Research, review, then fix |
| `implement` | explore -> kraken | Research then implement |
| `debug` | explore -> build-fixer | Research then fix build errors |

## Workflow

### Step 1: Parse Pipeline

Parse `$ARGUMENTS` to determine the pipeline stages.

**If preset name**: Map to the built-in chain from the presets table above.

**If custom chain**: Parse `agent1 -> agent2 -> agent3 "task description"` format.
- Each stage is separated by `->`
- Optional model override with `:` suffix (e.g., `explore:haiku`)
- The task description is the quoted string at the end, or all remaining text after the last agent

**Default model per agent**:
- explore: haiku
- oracle: sonnet
- leviathan: sonnet
- kraken: sonnet
- spark: sonnet
- build-fixer: sonnet
- critic: sonnet

### Step 2: Create Team

```
TeamCreate("pipeline-team")
```

### Step 3: Initialize Pipeline State

Create pipeline state file:

```
Write(".maestro/handoff/pipeline-{timestamp}.json", {
  "type": "pipeline",
  "stages": [
    { "agent": "agent1", "model": "model1", "status": "pending", "output": null },
    { "agent": "agent2", "model": "model2", "status": "pending", "output": null }
  ],
  "task": "the task description",
  "started_at": "ISO timestamp",
  "current_stage": 0
})
```

### Step 4: Execute Stages Sequentially

For each stage in order:

1. **Build context** from all previous stages:
   ```
   ## Pipeline Context

   ### Stage 1: explore (completed)
   {output from stage 1}

   ### Stage 2: leviathan (completed)
   {output from stage 2}

   ### Current Stage: kraken
   {original task description}
   ```

2. **Create a task** for the current stage agent:
   ```
   TaskCreate({
     subject: "Pipeline stage {N}: {agent} - {task}",
     description: "## Pipeline Stage {N}\n\n{context from previous stages}\n\n## Task\n{task description}"
   })
   ```

3. **Spawn the agent** as a teammate with the appropriate model:
   ```
   Task(agent: "{agent}", model: "{model}", prompt: "Execute your assigned task. Read the task description for full context including output from previous pipeline stages.")
   ```

4. **Wait for completion**: Monitor via `TaskGet` until the task status is `completed`.

5. **Capture output**: Read files modified or created by the agent. Update the pipeline state file with the stage output.

6. **Update state**: Mark current stage as completed, advance `current_stage`.

### Step 5: Finalize

After all stages complete:

1. **Summary**: Report what each stage produced
2. **Cleanup**: Call `TeamDelete(reason: "Pipeline complete")`. If it fails, fall back to: `rm -rf ~/.claude/teams/pipeline-{id} ~/.claude/tasks/pipeline-{id}`
3. **Archive state**: The pipeline state file remains in `.maestro/handoff/` for session recovery

## Error Handling

- If a stage fails (agent reports error or task stays in_progress too long), log the error in the pipeline state and stop the pipeline
- Report which stage failed and the error context
- Do NOT proceed to subsequent stages after a failure
- The user can inspect the pipeline state file and re-run from the failed stage

## Constraints

- Maximum 5 stages per pipeline
- Each stage runs one agent (no parallel stages -- use /work for parallel execution)
- Pipeline state is persisted for crash recovery
- Read-only agents (explore, oracle, leviathan) should be early in the chain
- Write agents (kraken, spark, build-fixer) should be late in the chain
