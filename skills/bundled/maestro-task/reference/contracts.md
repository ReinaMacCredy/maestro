# Task Contracts

Lightweight agreements about what a task will change before it is worked on. Use for non-trivial tasks where scope drift is a risk.

## Lifecycle

Pipe the YAML on stdin (auto-detected when stdin is not a TTY) or pass `--from <path|name|->`:

```bash
cat <<'YAML' | maestro task contract new <id>
intent: >
  One to three sentences on what this task changes and why.
scope:
  filesExpected:
    - src/**
  filesForbidden: []
doneWhen:
  - text: Describe the observable signal that proves the task is done.
    kind: manual
YAML

maestro task contract lock <id>
```

Load a project-local template:

```bash
maestro task contract new <id> --from default
```

This reads `.maestro/tasks/contract-templates/<name>.md`. Contract drafts live under `.maestro/tasks/contracts/`. Locked contracts are recorded in the contract index.

## Required fields

- `intent`: 1-3 sentences on what the task will change and why.
- `scope.filesExpected`: globs the task expects to touch.
- `scope.filesForbidden`: globs the task commits not to touch.
- `doneWhen`: bullets that describe the observable signal of completion.

## Reusable templates

Project-local draft templates live under `.maestro/tasks/contract-templates/`. Load one with:

```bash
maestro task contract new <id> --from default
```

This loads `.maestro/tasks/contract-templates/default.md`.

## Verbs

```bash
maestro task contract new <id> [--from <path|name|->] [--editor <cmd>]
maestro task contract edit <id> [--from <path|name|->] [--editor <cmd>]
maestro task contract show <id>
maestro task contract verdict <id>
maestro task contract list
maestro task contract discard <id>
maestro task contract lock <id>
maestro task contract reopen <id>
maestro task contract amend <id> --reason "..." [--from <path|name|->] [--editor <cmd>]
```

`new`, `edit`, and `amend` accept either a `--from` source (file path, template name, or `-` for stdin) or an `--editor <cmd>`. When neither is passed and stdin is piped, the YAML is read from stdin automatically. An interactive TTY falls back to `$EDITOR`.

Criteria:
```bash
maestro task contract criteria mark <id> <criterionId> --met
maestro task contract criteria add <id> "..."
maestro task contract criteria remove <id> <criterionId>
```

## Session

When the owning task is claimed outside the current shell, pass `--session <id>` on `new/edit/lock/discard/amend/criteria` commands.

## Verdict at completion

At completion, the declared scope is diffed against actual changes.

- Default: out-of-scope files are signal, not failure. The verdict is stored with the task.
- `--strict` on `task update --status completed`: blocks completion on a broken contract verdict.
- `contracts.overlapPolicy: annotate`: allow overlapping active contracts and record the overlap in the verdict.

## Reopen and amend

- Reopening a completed task reactivates its contract, clears the stored verdict, and preserves amendment history.
- Previously amended contracts reopen as amended.
- Amend a locked contract with a recorded reason when scope must change mid-work.

## Claim policies

```bash
maestro task claim <id> --contract-required       # force the contract-reminder note
maestro task claim <id> --no-contract             # suppress the reminder for one claim
```

Use `--no-contract` only when the policy requires a contract but this task intentionally has none.

## Stale reclaim

By default, stale reclaim inherits active contract ownership. Set `contracts.staleReclaimContractPolicy: block` in `.maestro/config.yaml` to refuse reclaim when a contract is active.

## Handoff interaction

Handoff pickup transfers active contract ownership with the linked task.

## Deletion

Deleting a task removes its linked contract file and appends a `task_deleted` discard record to the contract index.
