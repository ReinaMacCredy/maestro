---
title: CLI reference
description: The SLP v2 role toolbelt plus Maestro's separate development and administrative commands.
---

The live binary is the final source of flag details:

```sh
maestro help
maestro <verb> --help
```

SLP roles have exactly nine public operations. Other commands on this page are
development or administrative Maestro surfaces and are not extra SLP tools.

## SLP v2 operations

### `team start`

```sh
maestro team start <project> "<objective>" \
  [--supervisor-model <model>] \
  [--lead-model <model>] \
  [--peer-model <model>] \
  [--json]
```

Hub Supervisor authority. It runs from `~/maestro`, pins the canonical
Workspace Pack into the project, opens Team Supervisor and Lead, and creates
initial `OPEN` work for Lead. Repeating an identical running start verifies
and restores required roles without duplicates.

### `team stop`

```sh
maestro team stop <team-id>
```

Team Supervisor authority for normal stop; Hub Supervisor authority for an
emergency stop. Normal stop requires every work item to be `DONE`. The
Team Supervisor delegates its own final close to a transient foreground
non-agent Hub pane; this is internal, not a tenth operation. State changes to
`STOPPED` only after every team pane, the workspace, and raw transcript are
gone. A partial close remains `RUNNING`, and retry continues cleanup. The
generation snapshot and durable records remain.

```sh
cd ~/maestro
maestro team stop <team-id> --emergency
```

### `status`

```sh
maestro status
maestro status <work-id>
```

Read-only and role-scoped. Hub sees team generations and counts; team roles see
their team's actionable work; the work form includes notes, current return,
acceptance and linked decisions.

### `work add`

```sh
maestro work add "<objective>"
maestro work add "<objective>" --to <peer-name>
```

Team Supervisor creates work for Lead. Lead must name a Peer with `--to`;
Maestro reuses that Peer or opens it with the generation's pinned Peer model.
The new work state is `OPEN`.

### `work take`

```sh
maestro work take <work-id>
```

The assigned Lead or Peer moves `OPEN` work to `ACTIVE`. `RETURNED` work also
requires an unused reviewer grant for its current return revision.

### `work note`

```sh
maestro work note <work-id> "<material note>"
maestro work note <work-id> "<specific gap>" --rework
```

Appends context without changing state. `--rework` is restricted to the
reviewer responsible for that return. It grants the same assignee one retake
of the current return revision; an ordinary note never grants a retake.

### `work return`

```sh
maestro work return <work-id> "<result; proof; blocker; residual risk>"
```

The current owner moves `ACTIVE` work to `RETURNED`. One concise body carries
the bounded result and any blocker or residual risk.

### `work accept`

```sh
maestro work accept <work-id>
maestro work accept <work-id> --outcome cancelled
```

Moves `RETURNED` work to `DONE`. Lead accepts Peer work; Team Supervisor
accepts Lead work. A worker cannot accept its own return. Cancellation may
close `OPEN` or `RETURNED` work; `ACTIVE` work must return first.

### `decide`

```sh
maestro decide "<choice>" --why "<reason>" \
  [--work <work-id>] \
  [--replaces <decision-id>] \
  [--scope owner|cross-team]
```

Writes one immutable settled decision. Lead owns technical scope, Team
Supervisor owns team scope, and Hub Supervisor owns owner or cross-team scope.
Inside a team workspace, `--work <work-id>` links local work. At Hub, a unique
work id resolves directly; qualify an id shared by several teams as
`<team-id>:<work-id>`.

## Work states

```text
OPEN -> ACTIVE -> RETURNED -> DONE
```

`work note` changes no state. Rework is `work note --rework` by the correct
reviewer followed by one assignee retake of that exact return revision. A
blocker is written in the return body; there is no separate blocked state.

## Hard-cut mapping

These mappings explain removed SLP commands. They perform no alias or
compatibility action.

| Previous SLP command or layer | SLP v2 operation |
| --- | --- |
| `team open` | `team start` |
| `team status`, `team health`, `team await-ready` | `status` |
| `team bind` | removed; one start owns one project generation |
| `team review`, `team advise` | direct conversation, then `work note` or `decide` when material |
| `team reconcile` | repeat identical `team start`, or stop then start a changed generation |
| `dispatch open` | `work add` |
| `dispatch accept` and SLP `work start` | `work take` |
| `handback file` | `work return` |
| `handback review` and SLP `work done` | `work accept` |
| `decision draft` plus `decision lock` | `decide` |
| team `ready`, `attention`, review holds and health receipts | `status` plus direct supervision |

Old lifecycle data remains read-only legacy history and is not translated into
new work.

## Administrative observation

These commands remain available to the Hub or project maintainer, but are not
team role operations.

### `attention`

`maestro attention [--stale <minutes>] [--decision-stale <hours>] [--json]`
scans administrative store state at read time. It is not a background watcher
and does not replace SLP `status`.

### `brief`

`maestro brief` summarizes registered repositories without changing their
stores.

### `prompt`

`maestro prompt list [--session <value>] [--json]` lists recent recorded user
prompts.

### `ready`

`maestro ready [--json]` lists ready development work and its gates. It is not
an SLP team-readiness operation.

### `room`

`maestro room forget <path>` removes one repository from the Hub registry
without uninstalling it. `room mark` is installer-owned.

## Development workflow commands

Outside an active SLP team, the existing Maestro development workflow remains
available. Its work commands include `work start`, `work show`, `work list`,
`work done`, `work cancel`, `work release`, and `work reclaim`. These lease and
policy operations are not part of the SLP role toolbelt.

### `decision`

`decision draft`, `decision lock`, `decision show`, and `decision list` remain
for Maestro's design and bundle workflow. A running SLP role records its
settled choice with the one-step `decide` operation instead.

### `lesson`

- `file <what-happened>` requires `--target`, `--expected`, `--why`, and one or
  more `--evidence` values.
- `process <id> --commit <value> | --answer <value>` marks a lesson processed.
- `show <id>` and `list [--all] [--project <value>]` read lessons.
- `render` writes generated project views under `~/maestro/PROJECT/`.

See [Self-improvement](/guides/self-improvement/).

### `bundle`

- `open <id> [--work <value>]` scaffolds `SPEC.md`, `NOTES.md`, and `VERIFY.md`.
- `close <id>` snapshots and archives the bundle.
- `show`, `list`, and `save` read or ingest bundle state.

### `handoff` and `trace`

`maestro handoff <bundle-id>` composes a recovery packet.
`maestro trace <id>` reconstructs development work history.

## Methods and extensions

### `recipe`

`recipe list` lists shipped methods; `recipe show <name>` prints one.

### `plugin`

`plugin list`, `add`, `new`, `trust`, `untrust`, `enable`, `disable`, and
`remove` manage plugin lifecycle. Enabling a plugin never grants trust.

### `mcp`

`maestro mcp serve` starts the foreground stdio server.

## Runtime and harness

### `install`, `update`, and `uninstall`

- `maestro install` installs the runtime, wires the current repository and
  scaffolds the Hub room including `~/maestro/SLP.md`.
- `maestro update` fast-forwards the recorded source and resynchronizes the
  runtime.
- `maestro uninstall` removes managed repository wiring without deleting its
  data or the Hub room.

### `doctor`, `version`, and `hook`

- `maestro doctor` checks runtime and repository wiring read-only.
- `maestro version` prints the package and source identity.
- `maestro hook record --event <value> [--harness <value>]` records a harness
  event and prints the dynamic brief.

## Search and legacy data

- `maestro search <query> [--json]` searches native work, decisions, notes,
  bundles and imported legacy records.
- `maestro import rust --path <value> [--promote]` imports a preserved Rust
  store read-only.
- `maestro legacy show <id> [--file <value>]` reads imported legacy content.

## Help

`maestro help`, `maestro help <verb>`, and `maestro <verb> --help` print the
inventory registered by the running binary.
