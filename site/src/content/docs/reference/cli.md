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
maestro team start <project> "<objective>" [--peer-profile <name>] [--json]
```

Hub Supervisor authority. It runs from `~/maestro`, pins the canonical
Workspace Pack and the profiles it names into the project generation, opens
the Team Supervisor and Lead as native harness profiles
(`claude --agent maestro-<name>` or `codex --profile maestro-<name>`, rendered
by `maestro install`), sends each a one-line prompt carrying team, generation,
instance and the ready challenge, creates initial `OPEN` work for Lead, and
opens the generation's runtime pane through Maestro's Herdr plugin (Hub d96).
Repeating an identical running start verifies and restores required roles and
the runtime pane without duplicates. Every Herdr call goes over the socket at
`HERDR_SOCKET_PATH`; an unreachable socket fails with `HERDR_UNAVAILABLE`, a
generation whose runtime pane did not open fails with `RUNTIME_PANE_FAILED`
naming `maestro install`. `--peer-profile <name>` overrides the pack's `peer`
profile for this generation and is recorded on the team row; the Team
Supervisor and Lead change through a shadowing file in `~/maestro/profiles/`.
The retired `--lead-model`, `--peer-model` and `--supervisor-model` flags are
refused with `RETIRED_FLAG` naming the replacement; a missing render fails with
`PROFILE_NOT_INSTALLED` before any pane opens.

### `team stop`

```sh
maestro team stop <team-id> --reason "<closing report>"
```

Team Supervisor authority for normal stop; Hub Supervisor authority for an
emergency stop. The optional `--reason` is the Team Supervisor's closing
report: it is stored on the Hub's STOP record, printed by Hub `status` as
`<team> g<n> STOPPED (supervisor): <reason>`, and pushed to the Hub agent named
`supervisor` when one exists. Normal stop requires every work item to be
`DONE`. The
Team Supervisor delegates its own final close to a transient foreground
non-agent Hub pane; this is internal, not a tenth operation. State changes to
`STOPPED` only after every team pane, the workspace, and raw transcript are
gone. A partial close remains `RUNNING`, and retry continues cleanup. The
generation snapshot and durable records remain.

```sh
cd ~/maestro
maestro team stop <team-id> --emergency --reason "<why this generation is abandoned>"
```

Emergency stop keeps each unfinished item's existing four-state value and
records immutable abandonment actor, reason, generation, and time metadata.
Later generations neither inherit nor mutate those records. If `--reason` is
omitted, Maestro records a generic Hub emergency reason.

### `status`

```sh
maestro status
maestro status <work-id>
```

Hub status includes `abandonedWorkCount` per generation and `runtimePane: on|off`.
Team work readback includes abandonment fields when present.

Read-only and role-scoped. Hub sees team generations and counts; team roles see
their team's non-DONE items (a Peer sees only its own) with `*` marking the ones
waiting on the caller, DONE collapsed into a count that `--all` expands, and the
generation's decision ids; the work form prints state, from -> to, revision,
objective, the latest entry, linked decisions, and a `next:` line naming what
the caller may run. `--json` output is unchanged.

Outside a running team, bare status shows live sessions plus dead sessions that
still hold work or an open dispatch; each line carries the age of the session's
last hook event. `--all` lists every recorded session; `--live` lists only live
sessions. A SessionStart hook prunes dead sessions older than 30 days that hold
no work and appear in no dispatch.

### `work add`

```sh
maestro work add "<objective>"
maestro work add "<objective>" --to <peer-name>
maestro work add "<objective>" --to <peer-name> --profile <name>
```

Team Supervisor creates work for Lead. Lead must name a Peer with `--to`;
Maestro reuses that Peer or opens it through a rendered profile: `--profile
<name>` names it, a `--to peer-<name>` whose `<name>` is a profile composes
shared contract + Peer mandate + that body (`maestro-peer-<name>`), and
otherwise the generation's `peer` profile applies. A Peer that already runs
another profile is refused with `PEER_PROFILE_MISMATCH`; a missing render
fails with `PROFILE_NOT_INSTALLED` and nothing is rendered on demand. The new
work state is `OPEN`.

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
maestro work note <work-id> "<what I need>" --blocked
```

Appends context without changing state. `--rework` is restricted to the
reviewer responsible for that return. It grants the same assignee one retake
of the current return revision; an ordinary note never grants a retake.
`--blocked` flags the note and pushes a `BLOCKED` line to the seat above the
caller; it is the team's attention mechanism until the team runtime records
stalls itself (Hub d97, d98). The retired `--stall` is refused for every pane
with `STALL_RETIRED`. No form can change the work objective or acceptance
contract. Changed scope is new work.

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
blocker is written in the return body, or flagged on a note with
`work note --blocked` while the holder keeps the item; there is no separate
blocked state.

## Hard-cut mapping

These mappings explain the commands a running SLP team redirects. They
perform no alias or compatibility action, and they retire nothing outside a
team: `dispatch` and `handback` stay the lane contracts of the development
workflow (see below).

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
stores. `maestro brief --session` prints this session's SessionStart brief, the
text the hooks and the MCP instructions deliver.

### `prompt`

`maestro prompt list [--session <value>] [--json]` lists recent recorded user
prompts.

### `ready`

`maestro ready [--all] [--json]` lists ready development work and its policy
gates. Work blocked by open work is counted, not listed, unless `--all`; the
JSON `gated` array always carries it with origin `work-blockers`. It is not
an SLP team-readiness operation.

### `room`

`maestro room forget <path>` removes one repository from the Hub registry
without uninstalling it. `room mark` is installer-owned.

## Development workflow commands

Outside an active SLP team, the existing Maestro development workflow remains
available. Its work commands include `work start`, `work show`, `work list`,
`work done`, `work cancel`, `work release`, `work reclaim`, and
`work block|unblock <id> --by <work-id>`, which add or remove a blocker after
creation under the same checks as `work add --blocked-by`. These lease and
policy operations are not part of the SLP role toolbelt.

### `dispatch` and `handback`

Outside a running SLP team, `dispatch open <work-id>` stores a lane contract
(objective, owned and excluded scope, mutation boundary, stop condition,
evidence required, optional blind council); `dispatch accept`, `confirm`,
`cancel`, `show`, and `list` move and read it; `handback file`, `list`,
`review`, and `show` carry the shape-checked return packet. Inside a team both
verbs redirect to `work add`, `work take`, `work return`, and `work accept`.
Neither verb is legacy.

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
- `import <dir> [--dry-run]` copies a waymark or spec-workflow tree into the
  store and `.maestro/bundle/`; a failed import removes the directories it
  copied.

### `term`

- `add <name> <definition> [--work <id>]` records or redefines one glossary
  term (a single word, no spaces). Names shaped like a generated term id
  (`t1`, `t12`) are reserved, so a name never shadows another term's id.
- `show <name-or-id>` and `list` read terms; `maestro search` finds them.

### `memory`

Global memory lives in the Hub store at `~/maestro`. Every memory verb works
from any cwd: `list` and `show` read the Hub store directly, and `ingest`,
`retract` and `render` run through the Hub's own CLI when the current store is
not the Hub (`--from` and `--out` paths resolve against your cwd). A missing Hub
store fails with `HUB_UNAVAILABLE`. The SessionStart brief counts buffer facts
the next ingest would promote.

- `ingest [--dry-run] [--from <dir>]` promotes buffer facts (Claude
  auto-memory, Codex memories) through supersession, dedup and evidence gates.
  A fact whose slug is shaped like a generated fact id (`m1`, `m12`) is refused,
  so a slug never shadows another fact's id.
- `list [--all]` and `show <id-or-slug>` read facts; `--all` includes
  superseded and retracted ones.
- `retract <id-or-slug> --reason <why>` retires a fact so the buffers can never
  promote it again.
- `render [--check] [--force] [--out <path>]` writes `~/maestro/MEMORY.md`
  from the store and refuses to overwrite a hand edit.

### `handoff` and `trace`

`maestro handoff <bundle-id>` composes a recovery packet.
`maestro trace <id>` reconstructs development work history.

## Methods and extensions

### `recipe`

`recipe list` lists shipped methods; `recipe show <name>` prints one.

### `graph`

A graph is one markdown file (YAML frontmatter for nodes, edges, inputs and
limits; one `## <node>` section per agent or human prompt) that both harnesses
drive identically through a pull loop; maestro never spawns a model.

- `graph list` shows every graph across `<repo>/.maestro/graphs`,
  `~/maestro/graphs` and the shipped set, with origin and shadowing; `graph
  show <name>` prints the nearest file.
- `graph run <name>|--file <path> [key=value ...] [--limit nodes|loops|fanout=N]
  [--executor subagent|team]` starts a run as one work item of kind `graph`
  held by the driving session (nodes live in `graph_nodes`, never in `ready`
  or the card budget) and returns the first envelope; `--file -` reads stdin.
- `graph next <run>` executes every ready function, router, join and foreach
  node and returns `{run, graph, executor, round, state, done, nodes}` listing
  only the agent and human nodes whose inputs are ready; `{done: true,
  verdict}` at the end, `{done: true, stopped: "LIMIT", limit, used, partial}`
  at a structural limit, `{done: true, failed}` after a failed node.
- `graph result <run> <node>[@instance] --file <path>|--text <result>
  [--files a,b]` records a node's result; a declared schema validates JSON
  extracted from the text and `PARSE_FAILED` carries the schema for one retry.
  Under the team executor the Lead binds a node instead with `graph result
  <run> <node> --work <slp-work-id>`; `next` lists the node with `work` and
  `workState` until the item is DONE and then takes its returned body as the
  result. A `writes: true` node is issued alone and only to the run's holder
  (`LEASE_HELD` names another holder); the run itself commits nothing.
  A finished run refuses further results.
- `graph trust <name>|--file <path>` records a plugin-trust grant for a repo
  graph's current file so its function nodes may run; room and shipped graphs
  never need one. `maestro trace <run>` is the run's journal.

Shipped presets: `review-gate` (`range=<git range>`, `tier=light|full`),
`fix-loop` (`scope=<what to fix>`, `check=<command>`, a writing fixer in a
loop of at most three rounds with a human close) and `council`
(`brief=<neutral brief>`, `tier=lens|debate|debate-with-proof|high-risk`, the
maestro-council protocol with the Lead's draft and verdict as human nodes).
Limits: `nodes` counts issued agent nodes, `loops` counts loop-back firings,
`fanout` counts agent and human nodes in flight (bound nodes included). The
`maestro-graph` skill carries both executor loops and the authoring reference.

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

### `slp runtime`, `slp restore`, `slp event`, and `slp status`

`maestro install` renders `~/maestro/herdr-plugin.toml` and links the Hub
room as the Herdr plugin `maestro` (its hooks run from the room, d833);
`maestro uninstall` removes the link and the file.
Herdr launches the first three from that manifest and they are not SLP
operations: `slp runtime` is the `runtime` pane entrypoint (`team start`
opens it with `MAESTRO_SLP_TEAM` and `MAESTRO_SLP_GENERATION` in its env;
it holds the generation's event subscription and records stalls, idle wakes
and pane loss as the actor `runtime`); `slp restore` is the startup hook that
reopens the runtime pane of every RUNNING generation whose role panes survived
a Herdr restart and notes a generation whose panes are all gone as lost;
`slp event` is the `pane.exited` and `pane.closed` hook that records a role
pane loss when no runtime is subscribed. `maestro slp status [--json]`, from a
team pane, prints whether the runtime is running and the wakes it still holds
for working seats.

### `doctor`, `version`, and `hook`

- `maestro doctor` checks runtime and repository wiring read-only.
- `maestro version` prints the package and source identity.
- `maestro hook record --event <value> [--harness <value>]` records a harness
  event and prints the dynamic brief.

## Search and legacy data

- `maestro search <query> [--limit <n>] [--local] [--json]` searches native work,
  decisions, notes, terms, bundles, memory facts and imported legacy records in
  the project store, then the Hub room at `~/maestro`; Hub hits carry
  `store: hub` (`[hub]` in text). `--limit` bounds the combined list, project
  hits first. A Hub room that exists but cannot be read fails the search with
  `HUB_UNAVAILABLE`; `--local` skips the Hub and searches this store only.
- `maestro import rust --path <value> [--promote]` imports a preserved Rust
  store read-only.
- `maestro legacy show <id> [--file <value>]` reads imported legacy content.

## Help

`maestro help`, `maestro help <verb>`, and `maestro <verb> --help` print the
inventory registered by the running binary. A trailing `*` marks a verb that
runs under `MAESTRO_READ_ONLY=1`; on a root verb it means at least one subverb
does.
