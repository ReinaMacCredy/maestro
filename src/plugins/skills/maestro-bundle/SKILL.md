---
name: maestro-bundle
description: Route work into the right maestro tier and drive the SPEC/NOTES/VERIFY bundle lifecycle - open, resume, close, recall.
review-date: 2026-11-28
---
<!-- maestro-skill-version: dev -->

# maestro-bundle

Use [WORKFLOW.md](~/maestro/WORKFLOW.md) for method rules and routing.
This skill owns record creation, resume, and handoff procedures.

## Tier rule

Apply [Tiers](~/maestro/WORKFLOW.md#tiers) after bounded reconnaissance.
For tracked work use `maestro work add|start|done`. When Full is warranted,
open the bundle in the store whose checkout will change, even when design
runs in the Hub. List Hub decisions as `hub:<id>`; `maestro bundle show`
renders them. Note the bundle on the Hub map and the map on its work item.

```
maestro bundle open <id> --work <workId>   # scaffold SPEC/NOTES/VERIFY, link work
```

## The trio contract

Follow [Bundle contract](~/maestro/WORKFLOW.md#bundle-contract-tier-full).
Use `maestro handoff <bundle-id>` to render NOTES.md from work, decisions,
handbacks, failures, and the latest checkpoint. Fill only placeholders the
store cannot prove, including original authorization and retained gates.
Use `maestro bundle show <id>` to read the contract and linked decisions.
Never create root `SPEC-*`, `NOTES-*`, or `VERIFY-*` files.

## Verbs

```
maestro bundle open <id> [--work <id>]  # scaffold + active row
maestro bundle close <id>               # snapshot trio into the store, archive
maestro bundle pause <id> [--reason]    # active stays open, out of the way
maestro bundle resume <id>              # paused back to active
maestro bundle list                     # states: active | paused | archived
maestro bundle show <id>                # trio + linked work + decisions
maestro bundle save <dir>               # ingest a foreign trio dir as archived
maestro search "<term>"                 # recall: hits labeled (bundle, ...)
```

## Authorization boundaries

Follow [Authorization boundaries](~/maestro/WORKFLOW.md#authorization-boundaries).
Put the original user instruction or retrievable reference on the work item
so the successor can verify the grant, rather than relying on an agent's summary.

## Routing

Use [Routing](~/maestro/WORKFLOW.md#routing); a skill transition is internal,
not another user approval gate.

## Resume protocol

Follow [Resume](~/maestro/WORKFLOW.md#resume). For Light, read the work and
checkpoint without opening a bundle. For Full, locate and read the existing
bundle, then reconcile it with the checkout before continuing.

## Concurrency and git

Follow [Concurrency and git](~/maestro/WORKFLOW.md#concurrency-and-git).
Record the work owner and task-owned paths in the handoff, not just the branch.

## Compact or hand off

Hand off instead of compacting when:

- the owner changes
- a dependency becomes its own branch
- the role changes
- the context is full of false starts

Compact only when ownership, scope, and role stay stable and the history still
helps the same writer continue. Compaction gives no warning turn in either
harness, so the checkpoint must already exist: keep a `checkpoint:` work note
(state / next / avoid, latest wins) on each held item, rewritten at every
meaningful change of state or next action; the SessionStart brief prints it back after
the compaction. Nothing summarized is trusted over it.

Use break-before-make when the writer on a moving scope changes: release the
lease and refresh the handoff before the new session starts. The
handoff packet must preserve the base, Current State, Next Action, Authority
transferred and retained, Failed approaches, and Do not repeat.

## Hand-off

Run `maestro handoff <bundle-id>` to seed untouched NOTES.md sections when a bundle exists.
Then decide which of three cases this is; the destination differs:

1. **Continuation with a bundle.** A future session, in any tool, continues
   this bundle in this workspace. The handoff IS the rendered NOTES.md
   covering every section the trio contract names, with a `Driver:` line
   naming the tool expected to resume. The bundle stays active; the next
   session's resume protocol must find it.
2. **Transfer.** The work leaves this workspace, to a person, another repo,
   or an agent that will not resume the bundle. Write the standalone document
   below, then close the bundle per `maestro-verify`'s close order, citing
   the handoff target.
3. **No bundle.** For tracked work, save the continuation packet as a
   `checkpoint:` work note with original authorization and retained gates.
   If an untracked quickfix needs continuation, create a Light work item.
   For a transfer outside this workspace, provide a standalone document in
   the OS temporary directory, report its path, and transfer it to the recipient;
   a local path alone is not accessible from another machine.

The standalone document (cases 2 and 3) names the suggested skills for the
next agent, references specs, decisions, commits, and diffs by path or id
instead of duplicating them, and redacts secrets and personal data.

Design lane -> use the `maestro-design` skill. Implementation -> `maestro-work`.
Verification and close -> `maestro-verify`.
