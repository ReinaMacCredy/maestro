# Plan-to-Task Conversion Examples

Mapping realistic plan shapes into `maestro task plan --file -` batches.

## Example 1: Linear phased plan

Given a markdown plan with 4 phases, each depending on the previous:

```markdown
### Phase 1: Scaffold feature X (feature dir, index.ts, services.ts)
### Phase 2: Implement the core use-case behind the port
### Phase 3: Unit and integration tests
### Phase 4: Open PR with release notes
```

Task batch:

```json
{
  "batchId": "plan-feature-x",
  "tasks": [
    { "name": "scaffold", "title": "Scaffold feature X", "description": "Create src/features/x/ with index.ts and services.ts.", "type": "feature", "priority": 1 },
    { "name": "impl",     "title": "Implement core use-case", "description": "Build the use-case behind the port; cover happy path first.", "blockedBy": ["scaffold"] },
    { "name": "tests",    "title": "Add unit + integration tests", "blockedBy": ["impl"] },
    { "name": "ship",     "title": "Open PR with release notes", "type": "chore", "blockedBy": ["tests"] }
  ]
}
```

Start immediately: `--start scaffold`.

## Example 2: Parallel fan-out

Two implementation tasks can run in parallel after scaffolding:

```json
{
  "batchId": "plan-parallel-x",
  "tasks": [
    { "name": "scaffold",   "title": "Scaffold", "priority": 1 },
    { "name": "impl-api",   "title": "Implement API", "blockedBy": ["scaffold"] },
    { "name": "impl-ui",    "title": "Implement UI", "blockedBy": ["scaffold"] },
    { "name": "tests",      "title": "End-to-end tests", "blockedBy": ["impl-api", "impl-ui"] }
  ]
}
```

The blocker graph naturally serializes `tests` after both implementation tasks while leaving `impl-api` and `impl-ui` independent.

## Example 3: Parent/child hierarchy

Use `parent` to group subtasks under an epic:

```json
{
  "tasks": [
    { "name": "epic",   "title": "Auth migration",        "type": "epic", "priority": 0 },
    { "name": "db",     "title": "Database schema",       "parent": "epic" },
    { "name": "api",    "title": "API layer",             "parent": "epic", "blockedBy": ["db"] },
    { "name": "ui",     "title": "Login/logout UI",       "parent": "epic", "blockedBy": ["api"] },
    { "name": "rollout","title": "Rollout + feature flag","parent": "epic", "blockedBy": ["ui"] }
  ]
}
```

`parent` takes either a batch-local `name` or an existing `tsk-XXXXXX` id.

## Example 4: Appending to an existing task

`blockedBy` and `parent` accept real `tsk-*` ids alongside batch-local names. To append a new phase after an existing task:

```json
{
  "tasks": [
    { "name": "followup", "title": "Rollback plan", "blockedBy": ["tsk-a9df6e"] }
  ]
}
```

## Idempotent retry

Reuse the same `batchId` to replay a batch receipt instead of creating duplicates. Maestro stores the receipt under `.maestro/tasks/batches/<batchId>.json` and returns it on duplicate submission.

## Schema

`maestro task plan --schema` prints the full JSON Schema. Key fields per task:
- `name` (string, batch-local symbolic reference; cannot match `tsk-[0-9a-f]{6}`)
- `title` (required, non-empty)
- `description` (string)
- `type` (`task|bug|feature|epic|chore`)
- `priority` (integer, 0-4)
- `labels` (string array)
- `parent` (batch-local `name` or real `tsk-*` id)
- `blockedBy` (array of batch-local names or real `tsk-*` ids)

## Validation

The whole batch is atomic. One invalid task rejects the whole batch with a helpful error. Nothing is written to disk unless every task passes validation.
