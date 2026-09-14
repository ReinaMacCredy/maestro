# Design

Use this recipe when a material choice blocks the next slice. Apply
[WORKFLOW.md](~/maestro/WORKFLOW.md) for readiness, decision ownership,
authorization, and tier rules. This recipe is a procedure, not another policy.

## Working method

- Read the current `maestro work show`, linked decisions, notes, and relevant source.
- Present one unresolved fork at a time with a concrete recommendation.
- Record durable choices with `maestro decision draft` and `maestro decision lock`;
  keep reversible implementation details in the work instead. Supersede an
  old decision instead of rewriting its history.
- Keep acceptance, non-goals, and authority visible on the work item.
- Finish with the next decision, an explicit implementation gate, or a named
  blocker.

## Loop anatomy

### Perceive

Read the current work, decisions, handbacks, and source evidence. Identify one
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

Return the next blocking choice, a scoped design result, or a concrete blocker.
When the next slice is ready and the original request already authorizes its
implementation, continue with `maestro recipe show work` without another
approval round. A design-only request stops before production edits.
