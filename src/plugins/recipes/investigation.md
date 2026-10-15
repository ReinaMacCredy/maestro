# Investigation

Use this recipe for a read-only question such as how does X work, why was Y
built this way, are we sure about Z, or should we do X or Y. It produces a cited
explanation or a recommendation, not a code change. Ported from pstack's
investigation playbook (Lauren Tan, MIT; notice in THIRD_PARTY.md).

You own the answer. Plan, route, write.

## Steps

1. Route through `maestro recipe show how`. For motivation questions, also
   route through `maestro recipe show why`. The research rules live in `how`.
2. Keep the throughput checkpoint to one plan-list item,
   `throughput checkpoint: n/a, read-only investigation`.
3. Produce the `how`-shaped output (Overview / Key Concepts / How It Works /
   Where Things Live / Gotchas), or a recommendation with a tradeoffs table if
   the request is a decision between alternatives.
4. Land the findings as the how recipe's "Where findings land" section
   says (`maestro recipe show how`).
5. Apply `maestro recipe show unslop` to the reply.

No PR, no babysit, no `architect` unless the investigation precedes a code
change. If it does, hand back to the owner and re-route to
`maestro recipe show bug-fix` or `maestro recipe show feature`.

## Reply

The investigation output, and where each finding landed. For "are we sure?"
answers, include your real judgment with reasons. Push back if the premise is
wrong.
