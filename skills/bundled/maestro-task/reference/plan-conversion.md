# Plan-to-Task Conversion Examples

Mapping realistic plan shapes into `maestro task plan --file -` batches.

## Mandatory slug at plan conversion

Every top-level entry (no `parent`) carries a `slug` like `<verb>/<kebab>`
(`implement | fix | chore | spike | epic`). Either provide it explicitly or
omit it and let the title derive one. Step entries (with `parent`) MUST NOT
carry a slug. Slug enforcement rules (PC1-PC9):

- PC1: invalid slug shape rejects the batch with a clear error.
- PC3 / PC7: missing slug on a top-level entry auto-derives from the title.
- PC4: two batch entries that derive to the same slug reject the batch.
- PC5 / PC9: a batch slug that collides with an on-disk slug rejects the batch.
- PC6: an entry with both `slug` and `parent` rejects the batch.
- PC8: any entry's slug becomes a valid in-batch reference for `parent` /
  `blockedBy` (alongside `name` and real `tsk-<id>`).

Auto-derive collision suffixes: `-2..-9`. After 9, you must pass an explicit
slug.

## Example 1: Linear phased plan with explicit + derived slugs

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
    { "name": "scaffold", "title": "Scaffold feature X", "type": "feature", "priority": 1, "slug": "implement/feature-x" },
    { "name": "impl",     "title": "Implement core use-case",  "parent": "scaffold" },
    { "name": "tests",    "title": "Add unit + integration tests", "parent": "scaffold", "blockedBy": ["impl"] },
    { "name": "ship",     "title": "Open PR with release notes",   "parent": "scaffold", "type": "chore", "blockedBy": ["tests"] }
  ]
}
```

Start immediately: `--start scaffold`.

## Example 2: Parallel tracks (each top-level entry needs its own slug)

```json
{
  "batchId": "plan-parallel-x",
  "tasks": [
    { "name": "api",    "title": "API server",         "type": "feature", "slug": "implement/api-server" },
    { "name": "ui",     "title": "Web UI",             "type": "feature", "slug": "implement/web-ui" },
    { "name": "tests",  "title": "End-to-end tests",   "type": "feature", "slug": "implement/e2e-tests", "blockedBy": ["api", "ui"] }
  ]
}
```

`blockedBy` accepts batch-local `name` (above), the entry's `slug` (e.g.
`"blockedBy": ["implement/api-server"]`), and real `tsk-<id>` ids
interchangeably.

## Example 3: Parent/child hierarchy

Use `parent` to group subtasks under a track:

```json
{
  "tasks": [
    { "name": "epic",   "title": "Auth migration",        "type": "epic", "priority": 0, "slug": "epic/auth-migration" },
    { "name": "db",     "title": "Database schema",       "parent": "epic" },
    { "name": "api",    "title": "API layer",             "parent": "epic", "blockedBy": ["db"] },
    { "name": "ui",     "title": "Login/logout UI",       "parent": "epic", "blockedBy": ["api"] },
    { "name": "rollout","title": "Rollout + feature flag","parent": "epic", "blockedBy": ["ui"] }
  ]
}
```

`parent` takes a batch-local `name`, another entry's `slug`, or an existing
`tsk-XXXXXX` id.

## Example 4: Auto-derive when a slug is omitted

```json
{
  "tasks": [
    { "name": "fix-race", "title": "Fix race in writer", "type": "bug" },
    { "name": "follow", "title": "Follow up doc note", "type": "chore" }
  ]
}
```

Slugs derive to `fix/fix-race-in-writer` and `chore/follow-up-doc-note`
(verb chosen from `type`). When two derives collide, suffixes `-2..-9` are
appended; if all 9 collide, the batch is rejected with a clear error.

## Example 5: Appending to an existing task

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
- `parent` (batch-local `name`, another entry's `slug`, or real `tsk-*` id)
- `slug` (required for top-level entries; `<verb>/<kebab>`; forbidden when `parent` is set)
- `blockedBy` (array of batch-local names, slugs, or real `tsk-*` ids)

## Validation

The whole batch is atomic. One invalid task rejects the whole batch with a helpful error. Nothing is written to disk unless every task passes validation.
