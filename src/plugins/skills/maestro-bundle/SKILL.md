---
name: maestro-bundle
description: Route work into the right maestro tier and drive the SPEC/NOTES/VERIFY bundle lifecycle - open, resume, close, recall.
---
<!-- maestro-skill-version: dev -->

# maestro-bundle

Routing brain for maestro method work. Decide the tier first, then follow the
matching skill; this skill owns the bundle lifecycle itself.

## Tier rule

Work directly with `maestro work add|start|done` (no bundle) when the change is
one session, one branch, and its acceptance fits in a sentence.

Open a bundle when ANY trigger holds:
- the work spans multiple sessions or must survive a context reset
- multiple branches, worktrees, or agents touch the same scope
- the scope is high risk (schema change, wide refactor, irreversible step)
- a previous fix attempt for the same problem failed

```
maestro bundle open <id> --work <workId>   # scaffold SPEC/NOTES/VERIFY, link work
```

## The trio contract

- `SPEC.md` is a pure contract: problem, solution, scope, anti-goals.
  Mid-flight decisions are NOT written into SPEC; record them with
  `maestro decision draft "<text>" --rationale "<why>" --work <id>` and link
  the decision ids from SPEC.
- `NOTES.md` is a pure handoff: current state, next action, base commit.
  It also names Authority transferred and retained, Failed approaches, and Do
  not repeat. Overwrite it; never append. History lives in `maestro trace` and
  decisions.
- `VERIFY.md` is scenarios + results; each scenario points at a work item's
  acceptance or claim instead of restating it.

## Verbs

```
maestro bundle open <id> [--work <id>]  # scaffold + active row
maestro bundle close <id>               # snapshot trio into the store, archive
maestro bundle list                     # states: active | archived only
maestro bundle show <id>                # trio + linked work + decisions
maestro bundle save <dir>               # ingest a foreign trio dir as archived
maestro search "<term>"                 # recall: hits labeled (bundle, ...)
```

## Resume protocol

On resume, read the active bundle's NOTES.md first, then `maestro bundle show`
for linked work and decisions. Never trust conversational memory over the
bundle; the files and the store are the spec.

## Compact or hand off

Hand off instead of compacting when:

- the owner changes
- a dependency becomes its own branch
- the role changes
- the context is full of false starts

Compact only when ownership, scope, and role stay stable and the history still
helps the same writer continue.

Use break-before-make when the writer on a moving scope changes: release the
lease and overwrite NOTES.md before the new session starts. The handoff packet
must preserve the base, Current State, Next Action, Authority transferred and
retained, Failed approaches, and Do not repeat.

## Hand-off

Design lane -> use the `maestro-design` skill. Implementation -> `maestro-work`.
Verification and close -> `maestro-verify`.
