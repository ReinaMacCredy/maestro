---
name: maestro-task
description: Use at the start of any multi-step work in any project that uses maestro, and throughout task execution. Decompose plans into `maestro task` entries before starting (including converting /maestro-plan or /planner output into a task batch), claim one task at a time, keep continuation state fresh, and close with a receipt. Agent auto-invokes without user action whenever a `.maestro/` directory is present in the current working tree or an ancestor.
---

# Maestro Task

You are working in a maestro-initialized project. Decompose multi-step work into maestro tasks before starting. Keep continuation state fresh while working. Close with a receipt.

---

## When to activate

Auto-activate when:
1. The user asks for a multi-step implementation.
2. You just received output from `/maestro-plan`, `/planner`, or a markdown checklist. **Convert it to a task batch before executing.**
3. The user names a task id (`tsk-abc123`) or says "resume this task" / "work on X".
4. Starting a fresh session in a maestro project (`.maestro/` exists in cwd or ancestor).

Do not activate for one-liner edits or read-only questions.

## Hard rules

1. **One task `in_progress` per session** unless the user explicitly passes `--force`.
2. **Every completion carries `--reason`.** The receipt is shared context for future sessions.
3. **Update continuation state on meaningful change only.** Current state shifted, next action changed, a decision was made, blockers appeared. Not on every trivial edit.
4. **Blockers block transitions.** A task cannot move to `in_progress` or `completed` with unresolved blockers.
5. **Handoff is for cross-session transfer only.** Same-session resume uses continuation state, not handoffs.
6. **Mandatory slug at plan conversion.** Every top-level "track" carries a slug like `implement/<kebab>` (verbs: `implement | fix | chore | spike | epic`). Pass `slug` explicitly on top-level entries or omit it and let the title derive one. Step entries (those with `parent`) must NOT carry a slug. The whole batch is rejected on slug shape errors, on-disk collisions, or `slug` + `parent` together.

## Converting a plan into a task batch

When you have a plan (from `/maestro-plan`, `/planner`, or a markdown checklist):

1. Read the plan. Identify phases or steps as candidate tasks.
2. Map each step to a task JSON object. Use `description` for detail, `type` for kind (`task|bug|feature|epic|chore`), `priority` (0-4, 0 highest) for ordering hints, `blockedBy` for sequencing.
3. Submit atomically via `maestro task plan --file -`:

```bash
cat <<'JSON' | maestro task plan --file - --start scaffold
{
  "batchId": "plan-<short-slug>",
  "tasks": [
    {
      "name": "scaffold",
      "title": "Scaffold feature X",
      "description": "Create the feature directory, wire index.ts, add skeleton services.ts.",
      "type": "feature",
      "priority": 1,
      "slug": "implement/feature-x"
    },
    {
      "name": "impl",
      "title": "Implement the core use-case",
      "description": "Build the use-case behind the port; cover happy path first.",
      "parent": "scaffold"
    },
    {
      "name": "tests",
      "title": "Add unit + integration tests",
      "parent": "scaffold",
      "blockedBy": ["impl"]
    },
    {
      "name": "ship",
      "title": "Open PR",
      "type": "chore",
      "parent": "scaffold",
      "blockedBy": ["tests"]
    }
  ]
}
JSON
```

`slug` is REQUIRED on every top-level entry. Either pass it explicitly (preferred when the kebab matters) or omit it and let `task plan` derive one from the title (`Title text` → `<verb>/title-text`). Step entries (`parent` set) must NOT carry a slug — they address by `tsk-<id>`.

`--start <name>` claims the named task and flips it to `in_progress` in the same command. `batchId` makes retries idempotent (receipt persists under `.maestro/tasks/batches/`). Any validation error rejects the whole batch.

Report to the user: tasks created, which one is `in_progress`, the task id of each.

**Why plan-first:** atomic create, referential integrity (`blockedBy` / `parent` resolved in one pass), idempotent retry, no reactive one-at-a-time drift.

Full schema: `maestro task plan --schema`. Longer examples: `./reference/plan-conversion.md`.

## Single-task shortcut

For a one-task job (no plan):
```bash
maestro task create "Title" --description "..." --type feature --priority 1 --status in_progress
```

## Claim and start

Existing task:
```bash
maestro task next --json           # claim the next ready task
maestro task claim <id>            # claim a specific task
maestro task update <id> --status in_progress
```

## Optional: lock a contract for non-trivial work

Pipe the YAML on stdin (the CLI auto-detects piped input) or pass `--from <path|name|->`:

```bash
cat <<'YAML' | maestro task contract new <id>
intent: >
  One to three sentences on what this task changes and why.
scope:
  filesExpected:
    - src/features/foo/**
  filesForbidden: []
doneWhen:
  - text: Describe the observable signal that proves the task is done.
    kind: manual
YAML

maestro task contract lock <id>
```

Load a project-local template: `maestro task contract new <id> --from default` (reads `.maestro/tasks/contract-templates/default.md`).

Contract amend/reopen/criteria verbs and verdict semantics live in `./reference/contracts.md`.

## Stay in scope; amend on genuine discovery

Your task has a Contract that locks the files you may touch. Do not modify files outside `allowed_files`. Do not touch any path in `forbidden_paths`.

If you discover during implementation that a path you did not anticipate must change (a generated file regenerates differently, an unmocked dependency is reachable, a test helper needs a tweak), call:

```bash
maestro contract amend --task <id> --add-path <new-path> --reason "<why>"
```

This writes a new contract version and an Evidence row of kind `contract-amendment`. If the amendment is rejected (budget exhausted, or the path is in `forbidden_amendment_paths`), an Evidence row of kind `contract-amendment-blocked` is written instead. Do not retry the same amendment — surface it in your handoff or stop and ask.

## While working, keep resume state fresh

Only on meaningful change:
```bash
maestro task update <id> --current-state "..." --next-action "..."
maestro task update <id> --add-decision "keep API stable"
maestro task update <id> --remove-decision "old constraint"
```

Long-running work, heartbeat so the claim does not age out:
```bash
maestro task heartbeat <id>
```

Blockers:
```bash
maestro task block <blockerId> <blockedId...>
maestro task unblock <blockerId> <blockedId...>
```

## Complete with a receipt

```bash
maestro task update <id> --status completed \
  --reason "<one-line outcome>" \
  [--summary "<receipt summary>"] \
  [--surprise "<gotcha>"] \
  [--verified-by <name>] \
  [--strict]
```

`--reason` is persisted verbatim. Short, factual, no secrets. `--strict` blocks completion on a broken contract verdict.

## Before claiming the task complete

Run this loop before marking any task done:

1. `maestro task verify --task <id>` — Trust Verifier (scope, lockfile,
   generated, sensitive-paths, commit-metadata, secrets). Address every
   error finding before proceeding.

2. `maestro task proof --task <id>` — confirm criterion coverage. Every
   Spec acceptance criterion must have at least one Evidence row.

3. `maestro verdict request --task <id>` — produces a Verdict.

4. Branch on the verdict's exit code:
   - **0 PASS** — claim the task done.
   - **1 FAIL** — fix the cited findings, then loop back to step 1.
   - **2 HUMAN** — run `maestro handoff create` and stop. A human must
     approve before the task can complete.
   - **3 BLOCK** — stop. The task is blocked (typically cost-budget
     exhaustion). Surface the BLOCK reason to the user; do not retry
     without their guidance.

If retries are accumulating, run `maestro task budget --task <id>` to
see the current cost-budget consumption. Once retries reach the
contract's `maxRetries`, the next `verdict request` will return BLOCK.

See the `maestro-verify` skill for the canonical verification protocol
(witness levels, ProofMap, plan-check, AI Reviewer protocol,
threat-model production).

## Evidence

After each verification command (test, build, typecheck, lint), record
the result so future sessions can see what was actually run:

```bash
maestro evidence record --task <id> --kind command \
  --command "bun test" --exit 0
```

For manual checks that maestro can't witness, use `--kind manual-note`:

```bash
maestro evidence record --task <id> --kind manual-note \
  --note "Verified UI on staging at 1280x800"
```

Recorded evidence appears in `maestro task show` and the Mission Control task detail pane.

## Discovery

```bash
maestro status --json
maestro task ready --json --compact --limit 5
maestro task show <id-or-slug>
maestro task mine
maestro task stuck --older-than 4h
maestro task similar <id>
```

`task show` and `task update` accept either `tsk-<id>` or a track slug like `implement/foo`. `task list --tracks` prints just the track headers (slugs + slugless legacy ids), one per line.

## Status view

```bash
maestro task status                       # all open tracks
maestro task status --all                 # include completed (with `v` glyph)
maestro task status --no-compact          # unsectioned grouped detail view
maestro task status --track implement/foo # restrict to one track
maestro task status --json                # structured projection
```

Status glyphs: `o` active (in_progress), `!` blocked, `·` pending, `v`
completed (only with `--all`).

Default render shape: a hybrid operator board. The header reports open, active,
ready, blocked, and blocked-track counts. Simple one-task tracks render as
compact rows under `ACTIVE`, `READY`, or `BLOCKED`. Multi-step tracks expand
only when dependency structure matters: blocked steps or ready steps that unlock
downstream work. If a ready task unlocks blocked downstream work, a one-line
`next:` hint appears under the header.

Default examples:

```text
tasks: 12 open | 3 active | 7 ready | 2 blocked | 1 blocked track

ACTIVE
  o implement/template-prompt-fixes  Remove contradictory close-issue instruction from implement-prompt.md

DEPENDENCY TRACKS

implement/init-template-e2e-tests
  ! Add AgentInvoker seam, test support module, and blank template e2e test
      blocked by implement/template-prompt-fixes
  · Add e2e test for simple-loop init template

READY
  · implement/template-prompt-fixes  Replace hardcoded 'main' in review-prompt.md with {{SOURCE_BRANCH}}
```

`--no-compact` renders the unsectioned grouped detail view: solo tracks (no step
children) render on a single line (`  o slug  title  in-progress`) with no
blank line between consecutive solo tracks. Tracks with step children render
multi-line (slug header, indented bullet list, status text under blocked /
in-progress steps) so step structure stays readable.

Blocked rows render `blocked by <slug-or-id>` inline, while blocked steps inside
dependency tracks render the blocker on the next line. If a blocker has
completed it's marked `(done)` as a hint that the wait is over.

## Slug backfill (legacy slugless top-level tasks)

Existing top-level tasks without a slug render with their bare `tsk-<id>` as
the header. Bulk-backfill the whole queue (derives a slug from each title +
type, applies after preview):

```bash
maestro task backfill-slugs                       # dry-run / planning
maestro task backfill-slugs --apply               # write the slugs
maestro task backfill-slugs --apply --limit 10
maestro task backfill-slugs --rederive --apply    # refresh existing auto-derived slugs
```

Derivation drops stop-words, hex shas, and digit-only tokens, caps at four
significant words, and only cuts at word boundaries (no `...beads-ru` mid-word
truncation).

Backfill is display-only metadata: it bypasses the completion + ownership
locks so it works on completed and currently-claimed tasks. By default it
refuses to overwrite an existing slug; `--rederive` opts in to overwriting.

To set or rename one slug at a time:

```bash
maestro task update tsk-<id> --slug implement/<kebab>
```

Slug uniqueness is enforced across all top-level tasks. Slugs are not
preserved when a track is demoted to a step (`task update <id> --parent
<other>`); the CLI requires `--drop-slug` to acknowledge that.

## Recovery

Stale or dead session:
```bash
maestro task claim <id> --stale-after 4h
maestro task release-owned <deadSessionId>
maestro task unclaim <id>
```

Local artifact pruning:
```bash
maestro task prune --dry-run
maestro task prune [--keep N] [--candidates-only|--continuations-only] [--all]
```

Deeper recovery patterns live in `./reference/recovery.md`. The full command surface lives in `./reference/commands.md`.

## Reference

- `./reference/plan-conversion.md`: longer examples mapping markdown plans to task batches
- `./reference/commands.md`: full CLI surface for `maestro task *`
- `./reference/contracts.md`: contract criteria, amend/reopen, verdicts, strict mode
- `./reference/recovery.md`: stuck / stale / release-owned flows
