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
  range>` and `tier=light|full`; `fix-loop` takes `scope=<what to fix>` and
  `check=<command that must pass>` and drives a writing fixer for at most
  three rounds; `council` takes `brief=<neutral brief>` and
  `tier=lens|debate|debate-with-proof|high-risk` and runs the maestro-council
  protocol with you (the Lead) answering the draft and verdict nodes.
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
    kind agent  -> spawn a sub-agent with the node's profile and brief
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
  true` means re-ask that sub-agent for JSON matching the schema carried
  in the error (two retries); the third failure marks the node failed and
  the run ends with `failed`.
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
  `model: "opus"`, and the node's `brief` verbatim as the task.
- Codex: `spawn_agent` with agent type `maestro-<profile>` and the node's
  `brief` verbatim.

Send the `brief`, never the bare `prompt`: the brief is the prompt plus, when
a schema is declared, one sentence naming the required keys and the schema as
a JSON block (Hub d838 and its successor), so the agent answers in the declared
shape instead of its harness habit. It already
carries the run state the graph author placed in it. Add only what the harness needs to return
the answer (for example, "write your JSON answer to <path>"). Never merge
two nodes into one spawn and never run a function node's command yourself;
maestro already did.

## Executor team

`graph run` reports `executor` in every envelope (Hub d88): `subagent`
from a plain session, `claude -p`, `codex exec` or a desktop app; `team`
when the driver is a role pane of a running SLP team. Under `team` the
Lead is the driver and each agent node is one Peer work item (Hub d89);
the Lead is never a node and maestro still spawns nothing.

```text
Team Supervisor: maestro work add "run graph <name> <key=value ...>" \
                   --acceptance "the run's verdict"
Lead:            maestro work take <item>
                 maestro graph run <name> [key=value ...] --json
loop:
  envelope = the JSON just returned (or maestro graph next <run> --json)
  if envelope.done: maestro work return <item> "<verdict JSON>"; stop
  for each node in envelope.nodes without a work field:
    kind human -> answer it yourself: maestro graph result <run> <ref> --text "<answer>"
    kind agent -> maestro work add "<node.brief>" --to peer-<node.profile> \
                    --acceptance "one JSON object matching the schema in the brief" --json
                  maestro graph result <run> <ref> --work <new item id>
  for each node with a retry field (its item's body failed the schema):
    open a fresh item with node.brief and rebind exactly as above; two
    retries, the third failure fails the node
  for each node with a work field whose workState is RETURNED:
    read it (maestro status <item>), then maestro work accept <item>
    (or maestro work note <item> "<gap>" --rework for one retake)
  maestro graph next <run> --json
Team Supervisor: maestro work accept <item>
```

- One pane per profile: `--to peer-<profile>` opens the Peer lazily on the
  first item and queues later nodes of that profile on the same pane.
- A bound node stays in `nodes` with `work` and `workState` until its item
  is DONE; `next` then parses the item's returned body like any result
  (schema and all) and issues what depended on it. A body that fails the
  schema unbinds the node and lists it with `retry: {error, schema, work}`,
  twice at most (d838 and its successor); a cancelled item fails the node.
- Bound nodes count toward `limits.fanout`; keep the fan-out under the
  number of Peers you are willing to open.
- Prompts to a Peer must open with a lowercase plain sentence; a brief that
  opens "You are ..." is swallowed as a slash command. The node prompts in
  the shipped presets already do.
- The graph runtime writes no SLP state: every `work add`, `accept` and
  `return` above is yours (A7).

## Hand-off

The verdict is the run's evidence on its work item (`maestro work show
<run>`); quote it in the return that asked for the gate. A graph that
misbehaved is a finding for the handback, not a card.
