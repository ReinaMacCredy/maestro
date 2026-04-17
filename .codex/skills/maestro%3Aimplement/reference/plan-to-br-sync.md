# Plan-to-BR Sync Protocol

Protocol for converting a track's `plan.md` into `br` issues during `/maestro:new-track`.

## Prerequisites

- `.beads/` directory exists (bootstrapped by setup or new-track Step 4.5)
- `plan.md` has been written and approved
- `metadata.json` exists with track metadata

## Sync Steps

### 1. Parse plan.md

Extract the plan structure:
- **Phases**: Lines matching `## Phase {N}: {title}`
- **Tasks**: Lines matching `### Task {N}.{M}: {title}` or `- [ ] **{title}**`
- **Sub-tasks**: Indented `- [ ]` items under a task (these become issue description, not separate br issues)
- **Acceptance criteria**: Content under `#### Acceptance Criteria` or similar headings

### 2. Create Epic

Create a parent epic representing the entire track:

```bash
br create --title "Track: {track_description}" \
  --labels "type:{track_type}" \
  --description "Maestro track: {track_id}" \
  --json
```

Capture the returned `id` as `{epic_id}`.

### 3. Create Issues per Task

For each task in the plan, create a br issue:

```bash
br create --title "P{N}T{M}: {task_title}" \
  --parent {epic_id} \
  --labels "phase:{N}-{kebab_phase_title},type:{track_type}" \
  --description "{sub_tasks_as_markdown}\n\n## Acceptance Criteria\n{criteria}" \
  --json
```

Build a mapping as you go:
```json
{
  "P1T1": "{issue_id_1}",
  "P1T2": "{issue_id_2}",
  "P2T1": "{issue_id_3}"
}
```

### 4. Set Dependencies

For sequential tasks within the same phase, add dependencies:

```bash
br dep add {task_M+1_id} {task_M_id}
```

This means task M+1 depends on (is blocked by) task M.

Cross-phase dependencies: the first task of phase N+1 depends on the last task of phase N:

```bash
br dep add {first_task_phase_N+1} {last_task_phase_N}
```

### 5. Validate

Check for circular dependencies:

```bash
br dep cycles --json
```

If cycles detected: report and ask user to resolve before continuing.

### 6. Store in metadata.json

Add two fields to the existing `metadata.json`:

```json
{
  "beads_epic_id": "{epic_id}",
  "beads_issue_map": {
    "P1T1": "{issue_id_1}",
    "P1T2": "{issue_id_2}",
    "P2T1": "{issue_id_3}"
  }
}
```

### 7. Sync and Stage

```bash
br sync --flush-only
git add .beads/
```

The `.beads/` directory is included in the new-track commit (Step 13 of SKILL.md).

## Error Handling

- If `br create` fails: report the error and fall back to plan.md-only mode (do NOT set `beads_epic_id`)
- If `br dep cycles` finds cycles: warn user, attempt to remove the cycle-causing dep, re-validate
- If any step fails after epic creation: clean up by closing the incomplete epic and removing `beads_epic_id` from metadata

## Label Conventions

| Label | Format | Example |
|-------|--------|---------|
| Phase | `phase:{N}-{kebab-title}` | `phase:1-authentication-setup` |
| Type | `type:{track_type}` | `type:feature` |
