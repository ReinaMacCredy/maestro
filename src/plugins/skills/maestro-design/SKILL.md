---
name: maestro-design
description: Settle unknowns and lock decisions before implementation - recall past bundles, walk one fork at a time, record every settled choice with a rationale.
---
<!-- maestro-skill-version: dev -->

# maestro-design

Use when the request, acceptance, authority, or implementation boundary is
unsettled. Design is human-guided whenever a choice changes what will be
built. Do not begin implementation until the relevant decisions are locked and
the user has approved the resulting scope.

## Recall pass first

Before proposing anything, search the store for precedent:

```
maestro search "<topic keyword>"        # hits labeled work/decision/note/bundle
maestro bundle show <id>                # deep-read a bundle hit
maestro decision list                   # locked choices that bind this scope
```

A past bundle that settled the same fork is evidence; cite it instead of
re-deriving the argument.

## Working method

- Read the current `maestro work show`, linked decisions, notes, and source.
- Present ONE unresolved fork at a time with a concrete recommendation.
- Record each settled fork immediately:
  `maestro decision draft "<choice>" --rationale "<why, with the rejected alternative>" --work <id>`
  then `maestro decision lock <id>`. Supersede an old decision with
  `--supersedes`; never rewrite its history.
- Keep acceptance, non-goals, and authority visible on the work item.
- Do not edit code during a design-only engagement.

## Exit

Design ends with a SPEC (pure contract in the bundle) plus a red-test list:
one failing test per decided behavior at an accepted seam. If the contract
would need guessing to write a test, the fork is not settled - return to the
walk. Finish with the next fork, an explicit implementation gate, or a named
blocker.

For unattended/away-mode design constraints, read
[references/unattended.md](references/unattended.md).
