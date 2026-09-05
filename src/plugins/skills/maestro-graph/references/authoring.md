# Authoring a graph

One markdown file `<name>.md`: YAML frontmatter for the structure, one
`## <node>` section per agent and human node holding its prompt. Everything
is data; conditions and placeholders are evaluated by a fixed rule set,
never as code (anti-goal A2).

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
  verify: {kind: agent, profile: refuter, schema: {type: object, required: [refuted, reason], properties: {refuted: {type: boolean}, reason: {type: string}}}}
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

## gather

## synthesize

Questions, answers and refutations:

{gather}

Write the synthesis.
```

(`gather` needs no section; only agent and human nodes carry a prompt.)

## Frontmatter keys

- `name` (required, lowercase id), `description`.
- `input`: `name: {required, default, description}`. Values arrive as
  strings from `key=value` on `graph run`; a name never collides with a
  node id or a reserved root.
- `limits`: `nodes` (agent nodes issued over the run, a later round counts
  again), `loops` (loop-back firings), `fanout` (agent and human nodes in
  flight at once, bound team nodes included).
  Shipped defaults 40, 3, 12; `--limit k=v` overrides per run.
- `verdict`: a dotted state path whose value becomes the run's verdict.
  Default: the result of the single sink node, or an object of every sink.
- `nodes`, `edges`: below.

## Node kinds

| kind | keys | maestro does | you do |
|---|---|---|---|
| `agent` | `profile` (required), `schema`, `writes` | renders the prompt, issues it | spawn `maestro-<profile>` with the prompt, return the result |
| `human` | none | issues the prompt | ask the user, return the answer |
| `function` | `command` (required) | runs `sh -c command` in the repo; stdout that parses as JSON becomes the result, else the text | nothing |
| `router` | none | evaluates the `when:` on its outgoing edges; targets not selected are skipped | nothing |
| `join` | `collect`, `key`, `window` | waits for every producer, collects to `{items, total}` | nothing |
| `foreach` | `over` (required), `key` | creates one instance of every downstream node per item of the list at `over` | results per instance (`node@key`) |

- `profile:` must resolve through repo, room, shipped profiles; `graph run`
  fails `GRAPH_INVALID` naming node and profile before the run exists.
- `schema` is a JSON-schema subset: `type`, `properties`, `required`,
  `items`, `enum`. Declare one whenever a later node reads fields.
- `writes: true` marks a node that edits the working tree (fix loops): it is
  issued alone, nothing else issues while it runs, only the run's holder may
  issue it, and the run commits nothing; name the files in `--files`.
- A function node's placeholders are shell-quoted; write the command as if
  each value were one argument. Repo graphs run function nodes only after
  `maestro graph trust <name>`; room and shipped graphs run them freely.

## Edges

`{from, to}` connects two node ids. Two extra keys:

- `when:` on an edge leaving a `router`: a condition (below). A router with
  no `when` edge is invalid. Edges without `when` from a router are the
  fallback, taken only when no condition matched.
- `max_rounds: N` marks a loop-back edge: when `from` finishes, the target
  and every node between target and `from` re-run with `round + 1`, until
  the target has run `N` rounds; then the run proceeds along the other
  edges. Optional `when:` makes the loop conditional. `limits.loops` still
  caps firings.

Fan-out is several edges from one node; a pipeline is a chain. Neither is
a primitive and neither is a barrier: a node runs when its own inputs are
done, so item A's stage 2 starts while item B's stage 1 is still out.

## foreach and join

- `foreach` reads a list at `over` (a dotted path over state). Every node
  reachable from it along forward edges, up to the first `join`, exists
  once per item. Inside, `{item}` is the element, `{index}` its position,
  `{instance}` its key (`key: <field>` picks a field of the item;
  default the index).
- `join` collects the results of all its producers (every instance of a
  scoped producer). `collect: <path>` picks a list inside each result;
  each item gets `producer` (node id) and, for instances, `instance`.
  `key: [file, line]` dedups: the first item is kept, later matches add
  their producer to its `provenance`; `window: N` treats numeric key
  fields within N as equal (line ranges). The join is the only barrier and
  never calls a model (Hub d82); semantic merging is an agent node after it.

## Placeholders and conditions

- `{path}` in a prompt or command reads the run state: an input, a node
  result (`{classify.summary}`), `{item}`, `{index}`, `{instance}`,
  `{round}` (the run's current round), `{run}`. Objects and lists render as
  JSON. A placeholder whose
  root is none of these fails `GRAPH_INVALID`. Literal JSON braces in a
  prompt are safe: only `{identifier.path}` is a placeholder.
- A condition is one of: a path string (truthy: non-empty list, non-zero,
  true), `{path, eq: v}`, `{path, ne: v}`, `{path, gt|gte|lt|lte: n}`,
  `{all: [...]}`, `{any: [...]}`, `{not: c}`. A string with an operator,
  a call or `${}` is refused as data.
- `subsystems.length` works: a list's `length` is readable through a path.

## Patterns

- **Adversarial verify** (review-gate): reviewers fan out, `join` dedups by
  `[file, line]`, `foreach` over `items` opens one `refuter` per finding,
  a second `join` gathers verdicts, a `synthesizer` writes the verdict.
- **Loop until dry**: `review` then `fix` (`writes: true`) with a loop-back
  edge `fix` to `review` and `max_rounds: 3`, a `router` after `review`
  sending an empty findings list to the exit edge (`fix-loop`). A branch the
  router skipped in one round is decided again when the loop re-runs.
- **Judge panel**: three agent nodes with different profiles on the same
  prompt, one `join` with no key, one `synthesizer` that must cite the
  majority.
- **Gate before cost**: a `router` on a cheap `classifier` result skips the
  expensive lenses when the diff does not touch them (review-gate does this
  for regression, contracts and security).

## Gotchas

- Every agent and human node needs a `## <node>` section; a section with an
  unknown name is ignored.
- A node reached from two foreach nodes is unsupported; place a join
  between fan-outs.
- A join whose producers were all skipped by a router is skipped too, and
  so is everything after it; give the router a fallback edge when the
  downstream must always run.
- Results are text: a sub-agent that answers with prose around its JSON is
  fine, one that answers with two JSON objects gets the first.
- `graph run --file` freezes the file's text in the run; editing the file
  changes later runs only (and drops a repo grant).
- The state placeholder `{gather}` renders the whole join result as JSON;
  large fan-outs make long prompts. Point at `{gather.items}` when the
  total is noise.
