---
name: maestro-graph
description: Drive a pre-known multi-agent path as a maestro graph - run it by name or from a file you just wrote, pull each agent node with graph next, spawn it as a sub-agent under its maestro-<profile> definition, hand the result back with graph result, repeat until the verdict. Author a new graph from the reference when no preset fits.
review-date: 2026-11-28
---
<!-- maestro-skill-version: dev -->

# maestro-graph

Use when a task is a pre-known path with several agent steps: a review gate,
a research sweep, a judge panel, a fix loop. The path is one markdown graph
file; maestro holds the run, executes the deterministic nodes itself and
hands you only the agent and human nodes to spawn. maestro never starts a
model (Hub d78), so the loop below is yours on every harness. Design,
diagnosis and the SLP seat protocol stay outside graphs.

## Pick or write the graph

- `maestro graph list` shows every graph across the repo
  (`<repo>/.maestro/graphs`), the room (`~/maestro/graphs`) and the shipped
  set; a nearer file shadows a farther one by name. `maestro graph show
  <name>` prints one.
- A shipped preset fits: run it by name. `review-gate` takes `range=<git
  range>` and `tier=light|full`.
- Nothing fits: write a graph for the task from
  [references/authoring.md](references/authoring.md) and run it with
  `--file <path>`. Keep a good one by copying the file into a graphs
  directory (Hub d100; there is no save verb).

## The pull loop (executor subagent)

```text
maestro graph run <name>|--file <path> [key=value ...] [--limit k=v] --json
loop:
  envelope = the JSON just returned (or maestro graph next <run> --json)
  if envelope.done: stop; the verdict, LIMIT stop or failed node is in it
  for each node in envelope.nodes (all at once, they are independent):
    kind human  -> stop and ask the user the prompt; feed the answer back
    kind agent  -> spawn a sub-agent with the node's profile and prompt
  for each returned sub-agent:
    maestro graph result <run> <ref> --file <path>|--text "<result>"
  maestro graph next <run> --json
```

- `run` returns the first envelope, so the first `next` is implicit.
- Every node in `nodes` is ready now; spawn them in parallel. Nodes that
  depend on one of them appear on a later `next`, as soon as their own
  inputs are in: only a `join` waits for a whole fan-out (Hub d82).
- `ref` is the node id, or `node@key` for one instance of a foreach.
- A node with a `schema` must return JSON of that shape. Write the
  sub-agent's answer to a file and pass `--file`; maestro extracts the
  first JSON block from prose or a fence. `PARSE_FAILED` with `retry:
  true` means re-ask that sub-agent once for JSON matching the schema
  carried in the error; the second failure marks the node failed and the
  run ends with `failed`.
- `stopped: "LIMIT"` ends the run at a structural limit (`nodes`, `loops`,
  `fanout`, Hub d84) with `partial` state; rerun with `--limit <k>=<N>`
  when the cap, not the graph, was wrong.
- `GRAPH_UNTRUSTED` on a repo graph's function node: review the file the
  error names, then run the `maestro graph trust` command it gives and
  `graph next` again. Home and shipped graphs never ask.
- `maestro trace <run>` is the journal: every node transition and round.

## Spawning an agent node

The profile is a definition `maestro install` rendered for both harnesses
(Hub d83): `~/.claude/agents/maestro-<profile>.md` and
`~/.codex/agents/maestro-<profile>.toml`.

- Claude Code: the `Agent` tool with `subagent_type: "maestro-<profile>"`,
  `model: "opus"`, and the node's `prompt` verbatim as the task.
- Codex: `spawn_agent` with agent type `maestro-<profile>` and the node's
  `prompt` verbatim.

Send the prompt as maestro rendered it; it already carries the run state
the graph author placed in it. Add only what the harness needs to return
the answer (for example, "write your JSON answer to <path>"). Never merge
two nodes into one spawn and never run a function node's command yourself;
maestro already did.

## Executor team

`graph run` reports `executor` in every envelope (Hub d88): `subagent`
from a plain session, `claude -p`, `codex exec` or a desktop app; `team`
when the driver is a role pane of a running SLP team. Under `team` the
Lead drives and each agent node becomes one Peer work item; the binding
verb (`graph result --work`) lands with the second close of bundle
graph-engine. Until then run graphs from a plain session, or pass
`--executor subagent` from a role pane.

## Hand-off

The verdict is the run's evidence on its work item (`maestro work show
<run>`); quote it in the return that asked for the gate. A graph that
misbehaved is a finding for the handback, not a card.
