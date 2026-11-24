---
title: Work, decisions, and evidence
description: The records Maestro keeps for each task, each settled choice, and the proof behind them.
---

Maestro keeps three kinds of durable record in the repository store: work
items, decisions, and the proof a work item was closed with. Conversation
stays in the chat; only what must survive goes in the store.

## Work items

A work item, or card, is one bounded task with an id such as `w1`. It is
`open` when created, `active` while a session holds its lease, and `done` or
`cancelled` at the end. A lease is the claim one session holds on an active
item, so two sessions do not work on the same card.

```sh
maestro work add "<title>" --start
maestro work note <id> "<what changed or what you learned>"
maestro work done <id> --check "<command>"
```

- `work add --start` creates the card and claims it in one command. Plain
  `work add` creates an open card, and `work start <id>` claims it later.
- `--parent <id>` places a card under another one, and `--blocked-by <id>` or
  `work block <id> --by <id>` makes it wait for another card.
- `work release <id>` gives up a lease without finishing, and
  `work reclaim <id> --reason "<why>"` takes over the lease of a session that
  stopped.
- `work cancel <id> --reason "<why>"` closes a card and its open children
  without finishing them.
- `work list` prints open and active cards with their holder, blockers and
  gates, then counts the done and cancelled ones. `work show <id>` prints one
  card with its evidence, children, and last notes.

With `policy-breakdown` on, a card whose write-like children are still open
cannot be started or finished.

## Proof

`work done` records what the card was closed with. The `policy-proof` gate, on
by default, requires one of three forms:

| Form | What is recorded |
| --- | --- |
| `--check "<command>"` | Maestro runs the command in the repository root and records its exit code, duration, commit, and whether the tree was clean (`proof kind: check`). |
| `--evidence "<text>"` | The text as given (`proof kind: text`). |
| `--claim "<claim>" --proof "<proof>"` | One or more claims, each paired with its proof. |

A failing check is recorded and the card still closes. Read the result line
`work done` prints, or the `check:` line in `work show`, before you call the
work finished.

## Evidence layers

Name the narrowest layer a claim was proven at:

| Layer | What it proves |
| --- | --- |
| `source` | source inspection, test, lint or type check |
| `artifact` | the built or packaged output was read back |
| `installed` | the installed bytes or stamp match the artifact |
| `live` | the running process matches the installed layer |
| `journey` | the real user path reaches the intended result |

Do not claim a higher layer from a lower one. A passing source test does not by
itself prove the installed runtime or the user journey.

## Decisions

A decision records a settled choice and the reason for it:

```sh
maestro decision add "<choice>" --rationale "<why>" --work <id>
```

`decision add` records and locks the choice in one command. A locked decision
does not change. To change course, record a new one with
`--supersedes <decision-id>`; the old record stays so a later reader can see
what changed and why.

A choice that must wait for the owner starts as a draft:

```sh
maestro decision draft "<choice>" --needs-owner --work <id>
maestro decision lock <decision-id>
```

`maestro attention` lists a draft marked `--needs-owner` until it is locked or
withdrawn with `decision withdraw <id> --reason "<why>"`. `decision list`
prints the 20 most recent decisions and the total; `--all` lists every one and
`--work <id>` lists the ones linked to one card.

## Bundles

A bundle carries large, multi-session, or risky work. It is a folder under
`.maestro/bundle/<id>` with three files: `SPEC.md` (what to build, in units),
`NOTES.md` (what happened), and `VERIFY.md` (one row per unit with its check
and result).

```sh
maestro bundle open <id> --work <work-id>
maestro bundle check <id>
```

`bundle check` fails when a SPEC unit has no VERIFY row; `--final` also fails
when a row has no result. When `work done` closes the last open card linked to
a bundle, the bundle archives itself and reports any VERIFY row that has no
result. `bundle close <id>` archives a bundle by hand. `bundle list` prints
active bundles, most recently updated first, and counts the archived ones.

## Lessons

A lesson records a correction to a method, so the next edit to that method
can answer it:

```sh
maestro lesson file "<what happened>" --target "<recipe, rule or skill>" \
  --expected "<what should have happened>" --why "<why that is right>" \
  --evidence <work-or-decision-id>
maestro lesson list
maestro lesson process <lesson-id> --commit <commit>
```

`lesson process` marks a lesson answered by the commit that carries the edit,
or by `--answer "<why no edit>"`. Nothing deletes a lesson. `maestro attention`
reports lessons that are still pending.

## Store ownership

Each repository keeps its work, notes, decisions, bundles, and lessons in
`<git-common-root>/.maestro/maestro.db`, shared by linked worktrees. The Hub
room at `~/maestro` keeps the global memory and, when SLP is enabled, the
record of each team. Never edit a store by hand; use Maestro commands so the
state and its activity line change in the same transaction.

Teams use a separate four-state lifecycle, `OPEN -> ACTIVE -> RETURNED ->
DONE`, described in [Supervised teams](/guides/supervised-teams/).
