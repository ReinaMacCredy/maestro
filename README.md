# maestro

Maestro is a local record of work for people and coding agents. It keeps work
items, decisions, sessions, and proof in a SQLite store under `.maestro/` in
each repository's shared Git root. It is written in TypeScript, runs on Bun,
and needs no background service.

Documentation: [maestro.maccredyreina.me](https://maestro.maccredyreina.me/)

## Three layers

- The **kernel** owns the store, the event log, sessions, command routing, and
  plugin loading. It holds mechanism only, not workflow rules.
- **Plugins** add verbs and optional gates. A repository turns gates on or off
  in `.maestro/config`.
- **Recipes and skills** are working methods written in Markdown. Read one with
  `maestro recipe show <name>`; nothing is copied into your repository.

## Install, update, and remove

Install from source with one command. It needs `git` and [Bun](https://bun.sh):

```sh
curl -fsSL https://raw.githubusercontent.com/ReinaMacCredy/maestro/main/scripts/install.sh | sh
```

The script clones the repository into `~/.maestro/source` and runs the
installer from there. `MAESTRO_SOURCE_DIR` changes the directory and
`MAESTRO_REF` changes the branch (default `main`). From your own checkout, run
the installer directly:

```sh
bun bin/maestro.ts install
maestro version
```

Install copies the runtime to `~/.maestro/runtime`, writes the shim at
`~/.local/bin/maestro`, records the source checkout in `~/.maestro/source.json`,
and wires the current repository. It also creates the Hub room at `~/maestro`:
a folder that holds the shared method `~/maestro/WORKFLOW.md`, the list of
registered repositories, and seven skills under `~/maestro/skills`
(`maestro-bundle`, `maestro-design`, `maestro-work`, `maestro-verify`,
`maestro-council`, `maestro-coach`, and `maestro-questionnaire`). The skills
are linked for Claude and Codex without overwriting skills Maestro does not
manage.

- `maestro update` fetches the recorded source, accepts only a fast-forward,
  and resyncs the runtime. It refuses a dirty, diverged, missing, or
  unreachable source and changes nothing in that case.
- `maestro uninstall` removes Maestro's hooks, settings keys, and managed
  blocks from the current repository. It keeps repository data, the runtime,
  the shim, and the Hub room.
- `maestro doctor` checks the shim, runtime, source, repository wiring,
  permissions, and store access without changing them. When a repair is
  mechanical, it names the command to run.

## Track one piece of work

A work item (a card) is one bounded task with an id such as `w1`. This is the
whole loop:

```sh
maestro work add "Fix the CSV parser" --start
maestro work done w1 --check "bun test tests/csv.test.ts"
maestro work list
```

- `work add "<title>" --start` creates the card and claims it for your session
  in one command. Without `--start` the card is created open, with no holder.
- `work done <id> --check "<command>"` runs the command in the repository root
  and records its exit code, duration, commit, and whether the tree was clean.
  A failing check is recorded and the card still closes. `--evidence "<text>"`
  records plain text instead and is shown as `proof kind: text`.
- `work list` prints open and active cards with their holder (`held by
  <session> (live|dead)`), blockers, and gates, then one line that counts done
  and cancelled cards. `--all` lists every card; `--parent <id>` lists one
  card's children.

A decision records a choice and why it was made:

```sh
maestro decision add "Keep LF line endings" --rationale "one format for every fixture" --work w1
```

`decision add` records and locks the choice in one command. When a choice
must wait for the owner, write it with `decision draft --needs-owner` and lock
it later with `decision lock`.

Large or risky work gets a bundle: a folder with `SPEC.md`, `NOTES.md`, and
`VERIFY.md` under `.maestro/bundle/<id>`. Open it with
`maestro bundle open <id> --work <work-id>`. When `work done` closes the last
open card linked to the bundle, the bundle archives itself and reports any
VERIFY row that has no result. `bundle close <id>` archives one by hand, and
`bundle check <id>` fails when a SPEC unit has no VERIFY row.

Claims about a change name the layer they were proven at: `source`,
`artifact`, `installed`, `live`, or `journey`. A claim stops at the last layer
that was actually checked.

A failed command writes a JSON error envelope to stderr and exits nonzero.
Empty required arguments are refused.

## Verb tour

- `maestro work add|start|note|done|show|list|cancel` runs the work lifecycle;
  `maestro work block|unblock|release|reclaim` manage blockers and leases.
- `maestro decision add|draft|lock|withdraw|show|list` records choices.
- `maestro bundle open|close|list|show|check` runs design bundles;
  `maestro handoff` seeds a bundle's NOTES.md from the store and Git.
- `maestro attention` lists what needs a person in this repository, and
  `maestro brief` summarizes every registered repository.
- `maestro search` finds work, decisions, notes, terms, bundles, and memory
  facts here and in the Hub room.
- `maestro lesson file|list|show|process` keeps the ledger of corrections.
- `maestro term list|show` reads the glossary, and
  `maestro memory ingest|list|show|retract|render` runs the global memory in
  the Hub room.
- `maestro topic path|list` keeps files an agent makes while researching in
  `.maestro/topic`, outside Git.
- `maestro recipe list|show` serves methods, and
  `maestro plugin list|enable|disable` turns built-in plugins on or off.
- `maestro legacy show` reads cards imported from the Rust line.
- `maestro install`, `maestro update`, `maestro uninstall`, `maestro doctor`,
  and `maestro version` manage the installed runtime.

Run `maestro help` for every verb and `maestro <verb> --help` for its flags.

## Attention and brief

`maestro attention` reads the store when you call it and reports three
things: a draft decision that waits on the owner, lessons that are still
pending, and work that keeps failing. It records nothing and runs no daemon.

`maestro brief` opens each repository listed in `~/maestro/registry` without
changing it and reports only what needs attention. When nothing does, it says
so in one line.

## Read-only mode

Set `MAESTRO_READ_ONLY=1` to inspect a store without changing it. Read
commands such as `work list`, `search`, and `recipe show` still run. Commands
that write fail with `READ_ONLY`, and no session, lease, or liveness update is
saved. `maestro help` marks each verb that runs in this mode with `*`.

## Harness integration

`maestro install` writes hooks for Claude Code and Codex and merges only
Maestro's own entries. On `SessionStart` and `UserPromptSubmit` the hook
records the session and prints a short brief: how to add and close tracked
work, the work this session holds, and where the playbooks are. Install adds
nothing to `CLAUDE.md` or `AGENTS.md`. `maestro mcp serve` delivers the
same brief through MCP and offers no tools.

## Recipes, skills, and plugins

`maestro recipe list` names every shipped recipe. Most are playbooks: the
steps for one kind of task, such as `bug-fix`, `feature`, or `review`.
`~/maestro/WORKFLOW.md` says which playbook fits which task.

`maestro plugin list` shows the built-in plugins. Two policy gates are on by
default: `policy-proof` (`work done` needs a check, evidence, or a claim with
proof) and `policy-breakdown` (a card whose write-like children are still
open cannot be started or finished). `policy-card-budget` and `policy-auto-pr` are off until you enable
them.

## Teams (SLP)

SLP is Maestro's team layer. It runs several agents on one project, each in
its own pane of [Herdr](https://herdr.dev), a terminal workspace for agents,
and records every hand-off in the store. It is a built-in plugin named `slp` and is off by default. Turn it on
with `maestro plugin enable slp` in the Hub room and in the project.
`maestro team start` also turns it on in the project it starts a team for.

A team has one shape. Your Hub agent in `~/maestro` starts the team and
reviews the Lead. The Lead does the work itself or hands bounded pieces to
Peers, and reviews what each Peer returns. Each of these roles is a seat: one
agent session with its own mandate. The contract every seat follows is
`~/maestro/SLP.md`, which `team start` pins into the project. `--lead-model`
and `--peer-model` choose the model each seat launches with. The team uses
nine operations:

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

SLP is a cooperative-agent protocol, not a shell security sandbox. It checks
its own operations, but it does not block native commands or direct Herdr
calls. Read [SLP setup and storage](https://maestro.maccredyreina.me/getting-started/slp-setup/)
before starting a team.

## Rust-era data

The last Rust stores are kept under `legacy/rust/`; see
[`legacy/rust/README.md`](legacy/rust/README.md). A store that holds imported
Rust-era cards keeps them readable through `maestro search` and
`maestro legacy show <id>`.

## Contributing, security, and license

Contributions follow the toolchain, layout, and checks in
[`CONTRIBUTING.md`](CONTRIBUTING.md). Report a suspected vulnerability through
the private channel in [`SECURITY.md`](SECURITY.md), never a public issue.
Maestro is released under the MIT license; see [`LICENSE`](LICENSE).
