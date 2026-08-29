---
title: Lanes
description: Store a bounded Peer contract, deliver it through Herdr, and read the returned handback.
---

Coordination lanes are Herdr panes, never subprocess agents created by Maestro.
The Lead owns topology and delivery; Maestro owns the durable contract and
return packet.

## Lane lifecycle

```mermaid
flowchart LR
  Open["dispatch open"] --> Accept["dispatch accept"]
  Accept --> Handback["handback file"]
  Handback --> Review["Lead reviews claim and proof"]
  Review --> Close["Herdr pane close"]
```

The stored dispatch is the assignment. Acceptance binds its session to the
Peer role, the handback returns a claim, and only the Lead's review can accept
that claim before the pane closes.

## Lane types

- `scout` holds a no-write lease: discovery only, reports state.
- `decision` holds a no-write lease: investigates alternatives and recommends.
- `delivery` owns bounded writes inside the declared mutation scope.
- `challenge` holds a no-write lease: tries to break a premise or candidate and
  returns findings, not fixes.
- `shadow` holds a no-write lease beside the owner and returns comparison
  evidence that is never a candidate or a work write lease.

A no-write lease is enforced at the store, not at the filesystem: the lane
gate refuses `work start` to a session holding a no-write dispatch, and
nothing intercepts what the harness writes. The table says which boundary is
which. A soft-audited boundary is binding on the role and checkable after
the fact, not prevented.

| id | Boundary | Enforced by | Proof | Soft-audited |
|---|---|---|---|---|
| B1 | work write lease | `LEASE_HELD` on `work start`; the lane gate for no-write dispatch holders | test 470 | file writes by the harness |
| B2 | council seal | recorded council membership; `handback show`, `handback list`, attention and bundle handoff hide sealed returns | test 474 | the SQLite file and note files on disk stay readable |
| B3 | one handback per dispatch | `UNIQUE(handbacks.dispatch_id)` | test 476 | the truth of the claim |
| B4 | untargeted accept | opener confirmation before work or handback | test 423 | which pane the brief reached |
| B5 | dispatch cancel | the recorded opener | test 478 | the stated cancellation reason |
| B6 | Supervisor sub-agents | `PreToolUse` denies `Agent` and `Task` in the room's Claude settings | test 447 | Codex, and any Peer pane in a repository |
| B7 | Supervisor project writes | `MAESTRO_READ_ONLY=1` on the `hm` brief | test 214 | every other verb the room session runs |
| B8 | external effects (push, tag, publish, deploy, spend, delete) | nothing | soft-audited | the Human gate in the role contract |
| B9 | tool and call budgets | nothing | soft-audited | the assignment text |
| B10 | role identity | nothing | soft-audited | the pane name the opener set (d709) |
| B11 | team membership | Room binding plus fresh TeamRuntime generation inspection | lifecycle receipt | cwd and a pane label alone remain non-authoritative |
| B12 | Observer authority | one-use `team review raise` packet capability | review packet and receipt | whether the model reads outside the bounded packet remains a role obligation |
| B13 | deterministic team resources | generation-scoped TeamRuntime identities and duplicate rejection | health/open receipt | Herdr itself does not understand the Room ledger |
| B14 | a clean room workspace | TeamRuntime opens team resources in the generation workspace | open receipt | panes the owner opens manually in the room |
| B15 | the upward channel | nothing | soft-audited | `supervisor-<team>` is the only pane that prompts the room (d36) |
| B16 | a misrouted report | nothing | soft-audited | a supervisor bounces a `[from lead]` prompt from a Lead it does not own (d35) |
| B17 | the Observer sensor | TeamRuntime starts and inspects one foreground generation-scoped process | health receipt plus sensor-delivery effect | the semantics of model judgment after a packet |
| B18 | the external-effect gate | nothing | soft-audited | the room runs an external effect only after a locked room decision names the candidate and the evidence, and records the command and its output (d37, room d6) |

## Open a dispatch

After the Lead creates the work item and opens an unwatched Herdr lane pane, it
stores the complete contract:

```sh
maestro dispatch open <work-id> --objective "<observable outcome>" --owned-scope "<paths or responsibility>" --excluded-scope "<explicit non-goals>" --mutation "<no-write or write-bounded paths>" --stop-condition "<done or blocked boundary>" --lane delivery --evidence-required "source: <falsifier>" --pane <pane-id> --target-session <session-id>
```

For a Codex pane whose session does not exist until its first turn, the Lead
opens the pane-bound dispatch without `--target-session`, sends the real stored
contract as the first prompt, then verifies the holder after acceptance.

The Lead reads the stored contract and work context before sending it:

```sh
maestro dispatch show <dispatch-id>
maestro dispatch list <work-id>
```

## Accept the lane

The Peer begins by accepting exactly the stored dispatch:

```sh
maestro dispatch accept <dispatch-id>
```

Acceptance binds the session to the Peer role without taking the work write
lease. A delivery Peer starts the assigned work separately when the contract
requires the write lease.

## Return a handback

The Peer stops at the contract boundary and files the complete return packet:

```sh
maestro handback file <dispatch-id> --status DONE --claim "<current belief>" --proof "source: <falsifier>" --assumptions "None" --residual-risks "None" --incidental-findings "None"
```

The Lead reads the handback, checks its evidence, decides whether the work is
complete, and only then closes or reuses the lane pane. A handback is a claim,
not acceptance. A Peer that discovers a topology dependency returns
`DEPENDENCY_REQUEST`; one that discovers the assignment needs several
independent judgments returns `COUNCIL_REQUEST`. The Lead either opens the
required work or council, or records why it declined with `maestro work note`.

## Herdr procedure

The full `~/maestro/lane.md` procedure has the Lead create an unwatched lane
tab, split panes, start the requested harness, resolve session identity, store
and send the exact dispatch, wait for `working`, then wait for any terminal
Herdr state. Both `idle` and `done` mean to read the handback; `blocked` needs
inspection. A wait that returns without a state is re-armed. No Maestro verb
pushes a brief into a pane or calls Herdr.

## Councils

Concurrent dispatches in one generation on the same work item form a council.
The council remains sealed until every lane returns, which prevents one view
from biasing another. The Lead reconciles the complete set; it does not count
votes.

When the returned views conflict or the risk warrants cross-examination, the
Lead opens a second generation on the same work item. Each new dispatch quotes
the other Peers' handbacks verbatim and asks one targeted question. Peers never
prompt each other. They answer by handback with `DONE` and a `CONFIRM` claim,
`CHALLENGE`, or `REOPEN_REQUEST` (`CONFIRM` is claim text, not a status), and
the Lead records the final decision, dissent, and next
proof. A third round requires a new question.

```mermaid
flowchart LR
  Gen1["Generation 1 sealed"] --> Returns["Every handback returned"]
  Returns --> Unsealed["Views unsealed"]
  Unsealed --> Gen2["Generation 2 targeted dispatches"]
  Gen2 --> Answers["DONE with CONFIRM claim / CHALLENGE / REOPEN_REQUEST"]
  Answers --> Reconcile["Lead reconciles decision, dissent, proof"]
```

Implementation of the ruling belongs on separate work, not as a later
dispatch that silently changes council membership.

Team roles and their Room-ledger lifecycle are documented separately in
[Supervised teams](/guides/supervised-teams/). Dispatch Peers remain bounded
lanes and are not baseline team-readiness seats.
