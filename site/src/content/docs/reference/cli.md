---
title: CLI reference
description: Every top-level verb reported by the installed Maestro shim at commit 483f6d12.
---

This reference was generated from the installed `maestro` shim at
commit `483f6d126bed7928ce3a2adbeb61ad3a6f189a11`, released as 0.110.0.

```sh
maestro help
maestro <verb> --help
```

`maestro help` and `maestro help --help` print the top-level inventory.
`maestro help <verb>` prints one verb's help.

## Coordination and observation

### `attention`

`maestro attention` scans current store state without mutating work. Flags:
`--stale <minutes>` (default 30), `--decision-stale <hours>` (default 24),
`--dispatch-stale <hours>` (default 2), and `--json`.

### `brief`

`maestro brief` summarizes registered repository work without changing project
stores. It has no flags.

### `dispatch`

- `accept <id>` accepts a dispatch without taking the work write lease.
- `cancel <id> --reason <value>` records why an open dispatch was abandoned;
  only the session that opened it may cancel it.
- `confirm <id> --session <value>` confirms a claimed dispatch as its opener;
  a targeted dispatch already held by that session confirms as a no-op.
- `list [work-id] [--json]` lists contracts, optionally for one work item.
- `open <work-id>` requires `--objective`, `--owned-scope`, `--excluded-scope`,
  `--mutation`, `--stop-condition`, `--lane`, `--evidence-required`, and
  `--pane`; `--target-session` is optional. `--council-members <n>` declares
  a council of `n` seats on a new anchor, and `--council-anchor <id>` adds a
  seat to a declared council; the two are mutually exclusive. One handback per
  dispatch is a store constraint.
- `show <id>` reads one stored contract.
- `unseal <work-id> --reason <value>` opens a sealed council early and records why.

### `handback`

- `file <dispatch-id>` requires `--status`, `--claim`, `--proof`,
  `--assumptions`, `--residual-risks`, and `--incidental-findings`.
- `list <dispatch-or-work-id> [--json]` lists handbacks for one dispatch or
  work item.
- `show <id>` reads the handback for a handback or dispatch id.

### `prompt`

`maestro prompt list [--session <value>] [--json]` lists the 20 most recent
recorded user prompts, optionally for one session.

### `ready`

`maestro ready [--json]` lists ready work and gated items with their blockers.

### `room`

`maestro room forget <path>` removes one repository from the room registry
without uninstalling it. `maestro room mark` is internal: the installer runs it
inside the room store to record that the store is the Supervisor's.

### `status`

`maestro status [--live] [--json]` shows sessions, live peers, and held work.

## Work, decisions, and bundles

### `work`

- `add <title>` accepts repeatable `--blocked-by`, plus `--acceptance`,
  `--atomic-reason`, `--kind`, and `--parent`. Kinds are `feature`, `task`,
  `bug`, `chore`, `implement`, `idea`, and `research`; default is `task`.
- `start <id> [--atomic-reason <value>]` takes the live session lease.
- `show <id> [--json]` reads blockers, children, notes, evidence, and lease.
- `list [--json]` lists tracked work.
- `note <id> <text>` appends a durable note.
- `done <id>` accepts repeatable `--claim` and `--proof`, plus opaque
  `--evidence`, and completes through enabled policy gates.
- `cancel <id> --reason <value>` permanently cancels open or held work and its
  open or active descendants, each with the reason prefixed `parent <id>
  cancelled:`.
- `release <id>` releases the current session's lease without completion.
- `reclaim <id> --reason <value>` takes an existing lease with a recorded reason.

### `decision`

- `draft <text-or-id> [replacement]` creates or edits a draft; flags are
  `--rationale`, `--work`, `--parent`, and `--supersedes`.
- `lock <id>` locks a draft against further edits.
- `show <id>` reads one decision and its links.
- `list [--json]` lists current decision states.

### `bundle`

- `open <id> [--work <value>]` scaffolds SPEC/NOTES/VERIFY; `--work` repeats.
- `close <id>` snapshots the trio and archives the bundle.
- `show <id> [--json]` composes trio text, linked work, and decisions.
- `list [--json]` lists active and archived bundles.
- `save <directory>` ingests a foreign trio directly as archived.

### `handoff`

`maestro handoff <bundle-id> [--json]` seeds untouched `NOTES.md` sections from
store and Git evidence.

### `trace`

`maestro trace <id>` reconstructs one work item's event history.

## Methods and extensions

### `recipe`

- `list [--json]` lists shipped workflow recipes.
- `show <name>` prints one recipe.

### `plugin`

- `list [--json]` lists built-in, global, and repository plugins.
- `add <url>` clones and enables a plugin from Git.
- `new <name>` scaffolds a repository-local plugin.
- `enable <name>` and `disable <name>` change installed plugin state.
- `remove <name>` removes a managed plugin and its files.

### `mcp`

`maestro mcp serve` starts the foreground stdio server with the
`maestro_find` and `maestro_run` meta-tools.

## Runtime and harness

### `install`

`maestro install` installs the runtime and current repository hook wiring. It
has no flags. Run inside the Supervisor room it refuses with `INSTALL_IN_ROOM`;
the room is maintained by installing from a repository checkout.

### `update`

`maestro update` fast-forwards the recorded source and resynchronizes the
runtime. It has no flags.

### `uninstall`

`maestro uninstall` removes Maestro-managed wiring from the current repository.
It has no flags.

### `doctor`

`maestro doctor` diagnoses the machine runtime and repository wiring read-only.
It has no flags. In the Supervisor room it applies the room contract (room
files, registry, hooks, deny list, store) instead of the repository wiring
checks.

### `version`

`maestro version` prints the package version and installed or source commit.

### `hook`

`maestro hook record` records a harness event and prints the dynamic brief.
Flags are `--event <value>` and `--harness <value>`.

## Search and legacy data

### `search`

`maestro search <query> [--json]` searches work, decisions, notes, event
history, bundles, and imported Rust records.

### `import`

`maestro import rust --path <value> [--promote]` imports one legacy Rust store
read-only and optionally promotes cards into native work and decisions.

### `legacy`

`maestro legacy show <id> [--file <value>]` reads an imported legacy card and
its files, or only one named file.

## Help

### `help`

`maestro help` and `maestro help --help` show the top-level verb inventory.
`maestro help <verb>` prints the same per-verb description that registered
verbs expose through `--help`.
