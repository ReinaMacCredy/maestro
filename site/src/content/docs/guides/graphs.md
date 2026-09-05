---
title: Graphs
description: Run a pre-known multi-agent path as one markdown file that both harnesses drive identically, from a plain session or from a running team.
---

A graph is one markdown file that describes a multi-agent path you already
know: a review gate, a fix loop, a council, a research sweep. Maestro holds
the run, executes the deterministic nodes itself, and hands your agent only
the agent and human nodes to spawn. Maestro never starts a model, so the same
file runs under Claude Code and Codex. Design, diagnosis and the SLP seat
protocol stay outside graphs.

You ask for one the way you ask for any method:

```text
run the review-gate graph over main..HEAD at tier light and report the verdict
```

The agent loads the `maestro-graph` skill and drives the pull loop below.

## The graph file

```markdown
---
name: research-sweep
description: fan out one researcher per question, verify, synthesize
input:
  topic: {required: true, description: "the question under study"}
  depth: {default: 2}
limits: {nodes: 40, loops: 3, fanout: 12}
verdict: synthesize
nodes:
  questions: {kind: agent, profile: classifier, schema: {type: object, required: [questions], properties: {questions: {type: array, items: {type: string}}}}}
  each: {kind: foreach, over: questions.questions}
  research: {kind: agent, profile: reviewer-correctness}
  verify: {kind: agent, profile: refuter}
  gather: {kind: join}
  synthesize: {kind: agent, profile: synthesizer}
edges:
  - {from: questions, to: each}
  - {from: each, to: research}
  - {from: research, to: verify}
  - {from: verify, to: gather}
  - {from: gather, to: synthesize}
---

## questions

List the {depth} most decision-relevant sub-questions about {topic} as JSON.

## research

Answer this question with sources: {item}

## verify

Refute this answer to "{item}": {research}

## synthesize

Questions, answers and refutations: {gather.items}. Write the synthesis.
```

- `input` names the values `graph run` takes as `key=value`, each with
  `required`, `default` and `description`.
- `nodes` are `agent` (a `profile`, an optional `schema`, `writes: true` for
  a node that edits the working tree), `human` (a question for you),
  `function` (a shell command whose JSON stdout becomes its result), `router`
  (picks outgoing edges by `when:`), `join` (the only barrier; collects and
  dedups) and `foreach` (one instance of the downstream nodes per item).
- `edges` connect nodes; `when:` on a router edge is a data condition, never
  code, and `max_rounds: N` marks a loop-back edge.
- `limits` caps `nodes` (agent nodes issued over the run), `loops` (loop-back
  firings) and `fanout` (agent and human nodes in flight); the shipped
  defaults are 40, 3 and 12.
- One `## <node>` section per agent or human node holds its prompt;
  `{placeholders}` read the run state: an input, a node result
  (`{classify.summary}`), `{item}`, `{index}`, `{instance}`, `{round}`.

The full key reference and the patterns (adversarial verify, loop until dry,
judge panel, gate before cost) are in the `maestro-graph` skill's authoring
reference, installed under `~/maestro/skills/maestro-graph/references/`.

## Where graphs live and `graph trust`

`maestro graph list` reads three locations, nearer shadowing farther by
name: `<repo>/.maestro/graphs/`, `~/maestro/graphs/`, then the shipped set.
`maestro graph show <name>` prints the nearest file; `maestro graph run
--file <path>` runs a file you just wrote without installing it (`-` reads
stdin). There is no save verb: keep a good graph by copying the file into one
of the two directories.

A repo graph's function nodes run shell commands from a file the repository
controls, so they wait for your grant: the first `graph next` that reaches
one fails with `GRAPH_UNTRUSTED` naming the file and the command to run.
Review the file, then:

```sh
maestro graph trust <name>          # or: maestro graph trust --file <path>
maestro graph next <run>
```

The grant is keyed to the file's current bytes; an edit drops it. Room and
shipped graphs never ask.

## Running a graph

```sh
maestro graph run review-gate range=main..HEAD tier=light --json
maestro graph next <run> --json
maestro graph result <run> <node>[@instance] --file <path>|--text "<result>" [--files a,b]
maestro trace <run>
```

`graph run` creates one work item of kind `graph`, held by the driving
session, and returns the first envelope; the nodes live in their own table and
never appear in `ready` or count toward the card budget. Each envelope lists
every node whose inputs are ready now, each with its `brief`: the prompt plus,
when the node declares a schema, one sentence naming the required keys and the
schema as a JSON block. `next` executes every ready function, router, join and
foreach node itself and returns only agent and human nodes. The run ends with
`{done: true, verdict}`, with `{done: true, failed}` after a failed node, or at
a structural limit with `{done: true, stopped: "LIMIT", limit, used, partial}`;
rerun with `--limit nodes|loops|fanout=N` when the cap, not the graph, was
wrong. `maestro trace <run>` is the journal of every node transition and round.

### Node profiles

An agent node names a profile, and the profile is the sub-agent definition
`maestro install` rendered for both harnesses:
`~/.claude/agents/maestro-<profile>.md` and
`~/.codex/agents/maestro-<profile>.toml`. Nine node profiles ship:
`classifier`, the five `reviewer-simplify|correctness|regression|contracts|security`
lenses, `refuter`, `fixer` and `synthesizer`. A profile file in
`<repo>/.maestro/profiles/` or `~/maestro/profiles/` shadows the shipped one
by name; see [Seat profiles](/getting-started/slp-setup/#seat-profiles).

### Retries

A node with a schema must return JSON of that shape. Maestro extracts the
first JSON block from prose or a fence; a miss fails `graph result` with
`PARSE_FAILED` carrying the schema, and `next` lists the node again with
`retry`. Each node gets two retries; the third miss marks the node failed and
the run ends with `failed`. Because the brief already leads with the required
keys, a retry is normally a re-ask of the same sub-agent with the same brief.

## Two executors

Every envelope carries `executor`, chosen by where the run was started.

**`subagent`** is the default from a plain session, `claude -p`, `codex exec`
or a desktop app. The agent driving the run spawns each agent node as a
sub-agent of type `maestro-<profile>` with the node's `brief` verbatim, asks
you each human node, and records every answer with `graph result`:

```text
graph run <name> [key=value ...] --json
loop:
  envelope = the JSON just returned (or graph next <run> --json)
  if envelope.done: stop; the verdict, LIMIT stop or failed node is in it
  spawn every node in envelope.nodes at once (they are independent)
  graph result <run> <ref> --file <path> for each returned sub-agent
  graph next <run> --json
```

**`team`** applies when the driver is the Lead pane of a running SLP team.
The Lead is never a node; each agent node is one Peer work item and Maestro
still spawns nothing. From the owner's seat the whole thing is one ask to the
Team Supervisor:

```text
have the Lead run the council graph on this brief at tier debate and accept the verdict
```

The Lead opens each agent node as work for a Peer whose name is the node's
profile and binds the node to that item:

```sh
maestro work add "<node.brief>" --to peer-<profile> --acceptance "one JSON object matching the schema in the brief" --json
maestro graph result <run> <node> --work <work-id>
```

`--to peer-<profile>` opens the Peer on the first item and sends later nodes
of that profile to the same pane; every `work add` wakes it with
`[from lead][<id> OPEN]`. A bound node stays in `nodes` with `work` and
`workState` through OPEN, ACTIVE and RETURNED; once the Lead accepts the item,
`next` parses the returned body like any result and issues what depended on
it. A body that fails the schema unbinds the node and lists it with `retry`
so the Lead opens a fresh item, twice at most; a cancelled item fails the
node. Bound nodes count toward `limits.fanout`, so keep the fan-out under the
number of Peers you are willing to open. A `writes: true` node is issued
alone, nothing else issues while it runs, and only the run's holder may issue
it (`LEASE_HELD` names another live holder); the run commits nothing.

## The three shipped presets

| Graph | Inputs | Shape |
| --- | --- | --- |
| `review-gate` | `range=<git range>`, `tier=light\|full` | diffstat, classifier, router to the lenses the diff touches, join deduped by file and line, one refuter per finding, synthesizer verdict |
| `fix-loop` | `scope=<what to fix>`, `check=<command>` (default `bun test`) | run the check, two review lenses, a writing `fixer` in a loop of at most three rounds, a human `confirm` or `escalate` |
| `council` | `brief=<neutral brief>`, `tier=lens\|debate\|debate-with-proof\|high-risk` | the maestro-council protocol: seats by tier, sealed join, a decision model, premise verifier on unanimity or bounded verifiers, one cross-examination round, the Lead's draft and verdict as human nodes, auditor by tier |

Ask for them in plain words:

```text
run the fix-loop graph with scope "tests/store.test.ts fails on the WAL case" and check "bun test tests/store.test.ts"
```

The review gate is the graph `WORKFLOW.md` names for the review step (Light
after green before commit, Full after verify). The council preset is how the
`maestro-council` skill's run section executes; see
[Recipes, skills, and plugins](/guides/recipes-skills-plugins/).
