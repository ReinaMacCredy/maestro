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
4. Land the findings where they will be used.
   - A fact is evidence, not automatically a decision. When it supports a
     durable choice, record that choice with
     `maestro decision draft "<choice>" --rationale "<why, with source link>" --work <id>`,
     then `maestro decision lock <id>`.
   - Working evidence for an open work item goes to
     `maestro work note <id> "<finding + link>"`, and in a bundle also to the
     NOTES.md Current State.
   - A baseline the work must preserve becomes a VERIFY.md scenario in the
     bundle, or the work item's acceptance when there is no bundle.
   - A long-form artifact (a report, an inventory, a subagent answer, a
     capture, a demo) goes in the folder `maestro topic path <slug>` prints,
     never in `docs/`. Link it from the work note or NOTES.md.
   - With no work item and no bundle, the findings go in the reply. Open a
     work item only if the question feeds work that will actually happen. A
     finding alone never opens a bundle.
5. Apply `maestro recipe show unslop` to the reply.

No PR, no babysit, no `architect` unless the investigation precedes a code
change. If it does, hand back to the owner and re-route to
`maestro recipe show bug-fix` or `maestro recipe show feature`.

## Reply

The investigation output, and where each finding landed. For "are we sure?"
answers, include your real judgment with reasons. Push back if the premise is
wrong.
