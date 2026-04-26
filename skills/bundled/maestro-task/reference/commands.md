# `maestro task` Command Surface

Full list of verbs grouped by purpose.

## Planning and creation

```bash
maestro task plan --file <path|-> [--start <name>]   # atomic batch create
maestro task plan --schema                            # print JSON Schema

maestro task create "Title" \
  [--description "..."] \
  [--type task|bug|feature|epic|chore] \
  [--priority 0-4] \
  [--labels a,b] \
  [--parent <id-or-slug>] \
  [--slug <verb>/<kebab>] \
  [--blocked-by <id1,id2>] \
  [--status pending|in_progress]
```

`--slug` is optional on a top-level create (auto-derived from the title when
omitted). Forbidden when `--parent` is set. `--parent` accepts either a
`tsk-<id>` or a track slug.

## Discovery

```bash
maestro task next --json                              # claim next ready task
maestro task ready --json --compact --limit 5         # list of ready tasks (each item carries `slug` if it has one)
maestro task mine                                     # tasks owned by current session
maestro task show <id-or-slug>                        # full task + continuation state (accepts slug)
maestro task list [--tracks]                          # list tasks; --tracks prints just the track headers
maestro task status [--all] [--track <slug>] [--json] # screenshot-style grouped view
maestro task stuck [--older-than 4h]                  # in_progress with no recent activity
maestro task similar <id>                             # past tasks with keyword overlap
```

`task status` glyph palette: `o` active, `!` blocked, `·` pending, `v`
completed (only with `--all`). Color is auto-detected via `NO_COLOR` and
TTY; pipe-safe by default.

## Ownership

```bash
maestro task claim <id> [--session <id>] [--stale-after <duration>] \
  [--contract-required] [--no-contract]
maestro task unclaim <id>
maestro task release-owned <sessionId>               # release all tasks owned by a dead session
maestro task heartbeat <id>                          # bump lastActivityAt so the claim does not age out
```

## State and continuation

```bash
maestro task update <id-or-slug> --status in_progress  # auto-claims if unowned
maestro task update <id-or-slug> --current-state "..."
maestro task update <id-or-slug> --next-action "..."
maestro task update <id-or-slug> --add-decision "..."
maestro task update <id-or-slug> --remove-decision "..."

# Slug lifecycle
maestro task update <id> --slug implement/<kebab>          # rename or set on a track
maestro task update <id> --parent "" --slug implement/foo  # promote step -> track (slug required)
maestro task update <id> --parent <other> --drop-slug      # demote track -> step (slug cleared)

maestro task update <id-or-slug> --status completed \
  --reason "<one-line outcome>" \
  [--summary "<receipt summary>"] \
  [--surprise "<gotcha>"] \
  [--verified-by <name>] \
  [--strict]

maestro task reopen <id>
```

Silent mode (for scripts):
```bash
maestro task update <id> ... --silent
MAESTRO_TASK_SILENT=1 maestro task update <id> ...
```

## Blockers

```bash
maestro task block <blockerId> <blockedId...>         # blockerId must finish before blockedId is ready
maestro task unblock <blockerId> <blockedId...>
```

Rules:
- A task cannot move to `in_progress` or `completed` while unresolved blockers remain.
- Completion `--reason` is persisted verbatim. Keep it short, factual, and free of secrets.

## Contracts

See `./contracts.md` for the full contract surface.

## Maintenance

```bash
maestro task delete <id> [--session <id>] [--force]
maestro task prune [--keep N] [--candidates-only|--continuations-only] [--all] [--dry-run] [--json]
```

Default prune keeps newest 500 per kind. `--all` purges local candidates and completed continuations entirely.

## State files

- `.maestro/tasks/tasks.jsonl`: task rows.
- `.maestro/tasks/continuations/active/<id>.json`: live continuation state.
- `.maestro/tasks/continuations/completed/<id>.json`: completed continuations (local, gitignored).
- `.maestro/tasks/local-history/<id>.jsonl`: per-task history.
- `.maestro/tasks/NOW.md`: auto-refreshed short view of in-progress/ready/stuck tasks.
- `.maestro/tasks/batches/<batchId>.json`: plan batch receipts.
- `.maestro/tasks/candidates/`: local-only candidate hints (gitignored).
