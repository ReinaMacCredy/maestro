---
title: CLI reference
description: Every Maestro verb, its subverbs and flags, and the SLP plugin's team operations.
---

The live binary is the final source of flag details:

```sh
maestro help
maestro help <verb>
maestro <verb> --help
```

`maestro help` lists every verb the running binary registers. A trailing `*`
marks a verb that runs under `MAESTRO_READ_ONLY=1`; on a root verb it means at
least one subverb does. Every verb accepts `--json`, which prints one compact
JSON success envelope; a failure prints the JSON error envelope on stderr.

## Work

### `work`

```sh
maestro work add "<title>" [--start] [--kind <kind>] [--parent <id>] [--blocked-by <id>]... [--acceptance "<condition>"]
maestro work start <id>
maestro work note <id> "<text>" [--file <path>]
maestro work done <id> --check "<command>"
maestro work done <id> --evidence "<text>"
maestro work done <id> --claim "<claim>" --proof "<proof>" [--candidate <commit>]
maestro work show <id> [--notes <n>|all]
maestro work list [--all] [--parent <id>]
maestro work cancel <id> --reason "<why>"
maestro work block <id> --by <work-id>
maestro work unblock <id> --by <work-id>
maestro work release <id> [--force]
maestro work reclaim <id> --reason "<why>"
```

- `add` creates an open work item (a card). `--start` also claims it for the
  calling session, as `work start` does. `--kind` is one of `feature`, `task`
  (the default), `bug`, `chore`, `implement`, `idea`, or `research`.
- `start` claims the item's lease for the calling session.
- `note` appends a note without changing state; `--file` reads the body from a
  file.
- `done` completes the item, taking an unheld lease. `--check` runs the
  command in the repository root and records its exit code, duration, commit
  and whether the tree was clean (`proof kind: check`); a failing check is
  recorded and the item still closes. `--evidence` records text
  (`proof kind: text`). `--claim` and `--proof` are repeatable and paired.
  With `policy-proof` on, one of the three forms is required.
- `show` prints one item with its gates, blockers, evidence, children, and the
  last five notes.
- `list` prints open and active items with holder (`held by <session>
  (live|dead)`), blockers and gates, then one line counting done and
  cancelled items. `--all` lists every item; `--parent <id>` lists one item's
  children in every state. JSON: `{ works: [...], counts: { open, active,
  done, cancelled } }`, each active item with `holder: { id, harness, live,
  lastSeen }`.
- `cancel` closes open or held work and its open or active descendants with a
  recorded reason.
- `block` and `unblock` add or remove a blocker after creation under the same
  checks as `work add --blocked-by`.
- `release` gives up the calling session's lease; `--force` drops another
  session's lease even while it is alive, and the event records who forced
  it. `reclaim` takes an existing lease with a recorded reason.

`work take`, `work return`, `work accept`, and the `--to`, `--fresh`,
`--model`, `--blocked`, `--owner`, and `--rework` flags belong to the SLP
plugin; see [SLP plugin](#slp-plugin).

### `decision`

```sh
maestro decision add "<text>" [--rationale "<why>"] [--work <id>] [--supersedes <id>] [--parent <id>] [--dissent "<view>"]
maestro decision draft "<text>" [--needs-owner] [--rationale "<why>"] [--work <id>] [--supersedes <id>] [--parent <id>] [--dissent "<view>"]
maestro decision draft <id> "<replacement text>"
maestro decision lock <id> [--dissent "<view>"]
maestro decision withdraw <id> [--reason "<why>"]
maestro decision show <id> [--wide]
maestro decision list [--all] [--work <id>]
```

- `add` records a decision and locks it in one command.
- `draft` creates or edits a draft that waits on the owner; `--needs-owner`
  makes `maestro attention` report it. `lock` locks it; `withdraw` drops a
  draft that will not be locked.
- `--supersedes` replaces a locked decision. Supersession takes effect when the
  replacement is locked.
- `list` prints the 20 most recently updated decisions, newest first, with the
  total. `--all` lists every decision in id order; `--work` lists the ones
  linked to one item.

### `bundle` and `handoff`

```sh
maestro bundle open <id> [--work <work-id>]...
maestro bundle check <id> [--final]
maestro bundle close <id>
maestro bundle show <id>
maestro bundle list [--all]
maestro handoff <bundle-id>
```

- `open` scaffolds `SPEC.md`, `NOTES.md`, and `VERIFY.md` under
  `.maestro/bundle/<id>` and links the named work items.
- `check` fails only when a SPEC unit has no VERIFY row; `--final` also fails
  when a row has no result.
- A bundle archives itself when `work done` closes the last open item linked
  to it; VERIFY rows with no result are reported, not blocking. `close`
  snapshots the three files into the store and archives the bundle by hand.
- `show` prints the three files, linked work and decisions.
- `list` prints active bundles, most recently updated first, then a count of
  archived ones; `--all` lists those too.
- `handoff` fills untouched sections of a bundle's `NOTES.md` from the store
  and Git.

### `lesson`

```sh
maestro lesson file "<what happened>" --target "<doctrine>" --expected "<what should have happened>" --why "<why>" --evidence <id> [--project <name>]
maestro lesson list [--all] [--project <name>]
maestro lesson show <id>
maestro lesson process <id> --commit <commit>
maestro lesson process <id> --answer "<why no edit>"
```

A lesson records a correction to a recipe, rule, template, skill or Workspace
Pack. `file` requires `--target`, `--expected`, `--why`, and at least one
`--evidence` work or decision id. `process` marks a lesson answered by the
commit that carries the edit, or by the reason no edit was made. Nothing
deletes a lesson.

### `search`

```sh
maestro search "<query>" [--limit <n>] [--local]
```

Searches work, decisions, notes, terms, bundles, memory facts, recorded
prompts, event history and imported legacy records in the project store, then
the Hub room at `~/maestro`; Hub hits carry `store: hub` (`[hub]` in text).
`--limit` bounds the combined list, project hits first. A Hub room that exists
but cannot be read fails the search with `HUB_UNAVAILABLE`; `--local` skips
the Hub and searches this store only.

### `term`

```sh
maestro term list
maestro term show <name>
```

Reads the glossary kept in the store. `maestro search` finds terms next to the
work and decisions that use them.

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

### `topic`

```sh
maestro topic path <slug>
maestro topic list
```

`path` creates `.maestro/topic/<slug>` and prints its absolute path, the place
for files an agent makes while discussing or researching a topic (demos,
reports, notes, captures). The folder is outside Git. `list` lists the topics
and their files.

## Observation

### `attention`

`maestro attention` scans the store when called and prints three kinds of
finding: `HUMAN_DECISION_REQUIRED` (a draft marked `--needs-owner` on
unfinished work), `LESSONS_PENDING`, and `REPEATED_FAILURE`. It changes no
work and runs no watcher.

### `brief`

`maestro brief` summarizes registered repositories without changing their
stores. `maestro brief --session` prints this session's SessionStart brief, the
text the hooks and `maestro mcp serve` deliver.

### `legacy`

`maestro legacy show <id> [--file <path>]` reads a card imported from the
Rust line, with its files.

## Methods and plugins

### `recipe`

`recipe list` lists shipped recipes; `recipe show <name>` prints one.

### `plugin`

`plugin list` lists the built-in plugins and their state. `plugin enable
<name>` and `plugin disable <name>` turn one on or off for this repository and
let it set up or undo its effects.

### `mcp`

`maestro mcp serve` starts a foreground stdio MCP server that delivers the
session brief as its initialize instructions. It offers no tools.

## Runtime and harness

### `install`, `update`, and `uninstall`

- `maestro install` installs the runtime, wires the current repository, and
  creates or refreshes the Hub room.
- `maestro install --seat-token` also reads one `claude setup-token` token
  from stdin and stores it at `~/.maestro/claude/oauth-token` (0600) for every
  Claude seat; run it as `claude setup-token | maestro install --seat-token`.
- `maestro update` fast-forwards the recorded source and resynchronizes the
  runtime.
- `maestro uninstall` removes managed repository wiring without deleting its
  data or the Hub room.

### `room`

`maestro room forget <path>` removes one repository from the Hub registry
without uninstalling it. `room mark` is run by the installer only.

### `doctor`, `version`, and `hook`

- `maestro doctor` checks runtime and repository wiring read-only.
- `maestro version` prints the package and source identity.
- `maestro hook record --event <name> [--harness <name>] [--source <why>]`
  records a harness event and prints the brief.

## SLP plugin

The team verbs belong to the built-in plugin `slp`, which is off by default.
Enable it with `maestro plugin enable slp` in the Hub room and in the project;
`maestro team start` enables it in its target project. While it is off, these
verbs are refused with a line naming the enable command. See
[SLP setup and storage](/getting-started/slp-setup/).

A team's seats have exactly nine public operations. Flags configure one
operation; they are not separate tools.

```text
maestro team start
maestro team stop
maestro status [work-id]
maestro work add
maestro work take
maestro work note
maestro work return
maestro work accept
maestro decide
```

### `team start`

```sh
maestro team start <project> "<objective>" [--lead-model <model>] [--peer-model <model>]
```

Hub Supervisor authority; it runs from `~/maestro`. It pins the Workspace Pack
`~/maestro/SLP.md` and both seat files into the project generation, opens the
Lead from its seat file and waits for it to acknowledge, creates initial
`OPEN` work for the Lead, and opens the generation's runtime pane through
Maestro's Herdr plugin. Repeating an identical running start verifies and
restores missing panes without duplicates. Every Herdr call goes over the
socket at `HERDR_SOCKET_PATH`; an unreachable socket fails with
`HERDR_UNAVAILABLE`, and a generation whose runtime pane did not open fails
with `RUNTIME_PANE_FAILED` naming `maestro install`.

`--lead-model` and `--peer-model` launch that seat with the named model
(`claude --model` or `codex --model` on its launch line) and are recorded on
the team row. The seat file's own model is the default; the seat file and its
pinned digest are unchanged. An empty value is `INVALID_OPTION`, and the name
is passed through unvalidated for the harness to judge. A seat whose render is
missing fails with `SEAT_NOT_INSTALLED`, and a Claude seat with no seat token
fails with `SEAT_TOKEN_MISSING`, both before any pane opens.

### `team stop`

```sh
cd ~/maestro
maestro team stop <team-id> --reason "<closing report>"
maestro team stop <team-id> --emergency --reason "<why this generation is abandoned>"
```

Hub Supervisor authority. Normal stop requires every work item to be `DONE`.
It closes the Peers, the Lead, the runtime pane, and the workspace, and
records `STOPPED` only once every pane and the workspace are gone; a partial
close stays `RUNNING` and a retry continues cleanup. `--reason` is the closing
report the Hub's `maestro status` prints as
`<team> g<n> STOPPED (supervisor): <reason>`.

Emergency stop keeps each unfinished item's four-state value and records
immutable abandonment actor, reason, generation, and time. Later generations
neither inherit nor change those records. Without `--reason`, Maestro records
a generic Hub emergency reason.

### `status`

```sh
maestro status [--all]
maestro status <work-id> [--json]
maestro status <decision-id>
```

Read-only and scoped to the caller. At the Hub it lists team generations with
their runtime pane state, missing panes, abandoned work counts, and each
escalation waiting on the Hub as `blocked for the Hub: <team-id>:<id>
<note>`. In a team pane it lists the team's unfinished items (a Peer sees only
its own) with `*` on those waiting on the caller and collapses `DONE` into a
count that `--all` expands. The work form prints state, revision, objective,
the latest entry, linked decisions, and a `next:` line naming what the caller
may run; its text shows one line of the objective and `--json` prints it in
full. A decision id prints that decision's choice, why and scope.

### `work add` in a team

```sh
maestro work add "<objective>"
maestro work add "<objective>" --to <peer-name>
maestro work add "<objective>" --to <peer-name> --fresh
maestro work add "<objective>" --to <peer-name> --model <model>
```

Without `--to`, the Lead opens an item for itself, `OPEN` with no pane opened
and no line pushed. With `--to`, Maestro reuses the named Peer or opens it from
the generation's pinned Peer seat file, then wakes it with `[from
<role>][<id> OPEN] <objective>; read: maestro status <id>` whether it was just
opened or already acknowledged. A Claude Peer is opened with
`CLAUDE_CONFIG_DIR=~/.maestro/claude/peer`. Inside a team, `--acceptance` and
`--blocked-by` are refused with `INVALID_OPTION`; write them into the
objective, which never changes.

`--fresh` resets a reused Peer pane to a fresh harness context first (Claude
Code `/clear`, Codex `/new`), proven by a new harness session and the same
READY check a new pane passes. It is refused with `PEER_ACTIVE` while that
Peer holds ACTIVE work, and with `PEER_RESET_FAILED`, nothing added, when the
reset does not take. `--model <model>` launches that one Peer with the named
model over the generation's `--peer-model`. It is `INVALID_OPTION` without
`--to`, with an empty value, or on a Peer pane that is already running: drop
it to reuse that pane, or pick another Peer name.

### `work take`

```sh
maestro work take <work-id>
```

The assigned Lead or Peer moves `OPEN` work to `ACTIVE`. `RETURNED` work also
requires an unused reviewer grant for its current return revision.

### `work note` in a team

```sh
maestro work note <work-id> "<material note>"
maestro work note <work-id> "<specific gap>" --rework
maestro work note <work-id> "<what I need>" --blocked
maestro work note <work-id> "owner asked: <what>" --owner
```

A note adds context without changing state. `--rework` is restricted to the
reviewer of that return; it grants the same assignee one retake of the current
return revision. `--blocked` flags the note and pushes a `BLOCKED` line to the
seat above the caller. `--owner` records an owner instruction typed into the
caller's pane: it sets no blocked flag, pushes an `OWNER` line to the seat
above, and is refused together with `--blocked` or `--rework` as
`INVALID_OPTION`. The Hub may record a plain note on the Lead's item, which
reaches the Lead's pane as `[from hub-supervisor][<id> NOTE]`; `--blocked` and
`--owner` stay refused from the Hub. No form can change the objective.

### `work return`

```sh
maestro work return <work-id> "<result; proof; blocker; residual risk>"
```

The current owner moves `ACTIVE` work to `RETURNED`. One concise body carries
the result and any blocker or residual risk.

### `work accept`

```sh
maestro work accept <work-id>
maestro work accept <work-id> --outcome cancelled
```

Moves `RETURNED` work to `DONE`. The Lead accepts Peer work; the Hub
Supervisor accepts Lead work. Nobody accepts their own return. Cancellation may
close `OPEN` or `RETURNED` work; `ACTIVE` work must return first.

### `decide`

```sh
maestro decide "<choice>" --why "<reason>" \
  [--work <work-id>] \
  [--replaces <decision-id>] \
  [--scope owner|cross-team]
```

Writes one immutable settled decision. The Lead owns technical scope, and the
Hub Supervisor owns owner or cross-team scope. `--work <work-id>` links the
item and wakes the seat holding it with a `DECIDED` line. At the Hub, a unique
work id resolves directly; qualify an id shared by several teams as
`<team-id>:<work-id>`.

### Work states

```text
OPEN -> ACTIVE -> RETURNED -> DONE
```

`work note` changes no state. Rework is `work note --rework` by the reviewer
followed by one assignee retake of that exact return revision. A blocker is
written in the return body, or flagged on a note with `work note --blocked`
while the holder keeps the item; there is no separate blocked state.

### `slp runtime`, `slp restore`, and `slp status`

`maestro install` renders `~/maestro/herdr-plugin.toml` and links the Hub
room as the Herdr plugin `maestro`; `maestro uninstall` removes the link and
the file. These subverbs are not SLP operations. Herdr launches the first two
from that manifest: `slp runtime` is the `runtime` pane entrypoint
(`team start` opens it with `MAESTRO_SLP_TEAM` and `MAESTRO_SLP_GENERATION` in
its environment; it holds the generation's event subscription and records
stalls, idle wakes and pane loss as the actor `runtime`), and `slp restore` is
the startup hook that reopens the runtime pane of every RUNNING generation
whose seat panes survived a Herdr restart. `maestro slp status`, from a team
pane, prints whether the runtime is running, the Hub pane `team start`
recorded, the wakes it still holds for working seats, and the wakes it dropped
as `unreachable` because no target could be resolved.
