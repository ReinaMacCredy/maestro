---
title: Work, decisions, and evidence
description: Track one work entity, settle choices durably, and finish with layered proof.
---

## Work items

Maestro uses one work entity for features, tasks, bugs, chores, implementation,
ideas, and research. Work can form a parent tree, depend on other items, carry a
live lease, and end with claims and proofs.

Parentless write-like work needs either a child breakdown or an explicit atomic
reason:

```sh
maestro work add "<title>" --kind task --atomic-reason "<why this is one bounded unit>" --acceptance "<observable result>"
```

Use `maestro work show <id>` to read its blockers, children, notes, lease, and
evidence. Use `maestro ready` to see which work can start.

## Decisions

Draft the settled choice with its rationale and rejected alternative, then lock
it as a separate transition:

```sh
maestro decision draft "<choice>" --rationale "<why, including the rejected alternative>" --work <work-id>
maestro decision lock <decision-id>
```

To replace a locked decision, draft a new one with `--supersedes
<decision-id>` and lock the replacement. Supersession takes effect at lock,
not while the replacement remains a draft. History is never rewritten.

## Bundles

Decide the tier from the request before any recon. A quickfix, a diff that
fits in one sentence and hits no bundle trigger, is done directly with inline
verification and no record; if it grows past one sentence, stop and add a work
item. Direct work with a work item is appropriate for one session, one branch,
and acceptance that fits in a sentence. Open a bundle when work spans sessions
or branches, shares a moving scope, carries high risk, or repeats a failed fix:

```sh
maestro bundle open <bundle-id> --work <work-id>
```

The active bundle contains `SPEC.md` for the contract, `NOTES.md` for the
handoff, and `VERIFY.md` for scenarios and results. Close it only after the
verification table passes:

```sh
maestro bundle close <bundle-id>
```

## Claims and proofs

Complete held work with an observable claim paired to evidence that could
falsify it:

```sh
maestro work done <work-id> --claim "test: <behavior>" --proof "source: <falsifier>"
```

Evidence layers are `source`, `artifact`, `installed`, `live`, and `journey`.
Claim only as far as the last proven link and name untested links explicitly.

## Default policy gates

- `policy-breakdown` requires parentless write-like work to have a child
  breakdown or `--atomic-reason`; open write-like children block their parent.
- `policy-dispatch` blocks completion or cancellation while a dispatch lacks a
  handback and blocks work start while a sealed council is open.
- `policy-proof` requires opaque `--evidence` or paired `--claim` and `--proof`
  on completion.

The TDD, QA, research, witness, and lifecycle policy plugins ship disabled and
can be enabled per repository.
