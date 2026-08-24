# Design

Use this recipe when the request, acceptance, authority, or implementation
boundary is unsettled. Design is human-guided whenever a choice changes what
will be built. Do not begin implementation until the relevant decisions are
locked and the user has approved the resulting scope.

## Working method

- Read the current `maestro work show`, linked decisions, notes, and relevant source.
- Present one unresolved fork at a time with a concrete recommendation.
- Record each choice with `maestro decision draft` and `maestro decision lock`; supersede an
  old decision instead of rewriting its history.
- Keep acceptance, non-goals, and authority visible on the work item.
- Finish with the next decision, an explicit implementation gate, or a named
  blocker.

## Loop anatomy

### Perceive

Read the current work, decisions, messages, and source evidence. Identify one
unsettled fork or contradiction. Stop if the requested authority is unclear.

### Choose

Select the smallest decision that makes later work materially safer. Explain
the options in plain language and state which one you recommend.

### Act

Record the selected direction through `maestro decision draft`, `maestro decision lock`, or a
scoped `maestro work note`. Do not edit code during a design-only engagement.

### Observe

Read the work and decision state back. Check that acceptance covers the chosen
behavior, non-goals remain intact, and no locked decisions conflict.

### Learn

Record only durable corrections or reusable constraints. Tie the lesson to a
decision or work note; do not leave it only in chat.

### Continue

Return exactly one of: the next design fork, a request for explicit build
approval, or a concrete blocker. Once the contract is settled and approved,
use `maestro recipe show work` for implementation.
