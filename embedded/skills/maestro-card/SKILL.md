---
name: maestro-card
version: 1.6.0
description: "Use for active Maestro card work: pick up and deliver work cards (claim, update, complete, verify), run the feature-card lifecycle (accept, prepare, amend, ship), and capture qa-baseline/qa-slice gate evidence."
---

# Maestro Card

Everything in a Maestro repo is a card in one flat store
(`.maestro/cards/<id>/card.yaml`): features, tasks, bugs, chores, ideas, and
decisions. This skill covers the active-work cluster: the work loop, the
feature lifecycle, proof, and the QA gates. Design (`maestro-design`), audit
(`maestro-audit`), and setup (`maestro-setup`) have their own skills.

Activate:
`maestro hook record --event skill_activation --skill maestro-card`

## Route

Read the reference for the job at hand; they share the ground rules below.

- Pick up, progress, finish, or unblock a work card (task/bug/chore):
  [reference/work.md](reference/work.md). Its implement step is test-first
  (red-green-refactor) whenever the card's `--check` names observable
  behavior: [reference/tdd.md](reference/tdd.md).
- Work the backlog unattended while the user is away or asleep:
  [reference/loop.md](reference/loop.md)
- Author, accept, prepare, amend, ship, or archive a feature card:
  [reference/feature.md](reference/feature.md)
- Prove a claim, repair failed proof, or verify adversarially:
  [reference/verify.md](reference/verify.md)
- Capture the behavior contract before `feature accept`:
  [reference/qa-baseline.md](reference/qa-baseline.md)
- Replay scenarios and record slice evidence before `feature ship`:
  [reference/qa-slice.md](reference/qa-slice.md)

## Shared Ground

- Exact command signatures live in [reference/cli.md](reference/cli.md),
  generated from the binary. A verb or flag not listed there does not exist;
  read it instead of probing `--help`.
- Discover work with the flat card verbs: `maestro ready`, `maestro list`,
  `maestro show`. Take and annotate work with `maestro claim`,
  `maestro note`, and `maestro dep add`.
- Ids are stable and opaque (`card-<hash>`; features keep their creation
  slug). The dotted alias `show` prints is display-only; never address a card
  with it.
- Never chain a guessed id: use only ids read from verb output (`create
  --id-only`, `ready`, `list`, `show`). When a lookup misses, re-list and
  read the real id; do not retry spelling variations.
- Do not hand-edit `card.yaml` or the verb-guarded sidecars (`qa.md`,
  state history). Use verbs so gates and audit trails stay intact.
- Terminal words are per type — feature `shipped`/`cancelled`, work
  `verified`/`rejected`/`abandoned`/`superseded`, decision
  `locked`/`superseded`, loose task/bug/chore `closed` — and all of them read
  as coarse `closed` on the board. `maestro close` fits only task/bug/chore;
  when the user says "close" a feature, that means `feature ship` or
  `feature cancel`. `maestro archive` moves an already-terminal feature's
  records to `.maestro/archive/`; it is a location move, never an implied
  side effect of closing — archive only on explicit user intent.
- When the user corrects your behavior, record it:
  `maestro event intervention --note "<what was wrong>" [--topic <slug>]`.

## External intake

When the user brings a spec, plan, or PRD authored elsewhere, the agent does
the conversion; there is no CLI parser for external documents.

1. `maestro feature new "<title>" --description "<problem>"`, then copy the
   document verbatim: `cp <doc> .maestro/cards/<id>/request.md`.
2. `maestro feature set <id> --request "distilled from request.md (<doc
   title>)" --type prd` (`prd`, `spec`, or `plan`), then author
   acceptance, areas, and non-goals through `feature set`.
3. Rewrite the document's bullets into observable acceptance criteria; never
   copy narrative lines as acceptance. The verbatim text stays in
   `request.md`; the contract carries only checkable behavior.

`request.md` travels with the card through archive and unarchive.

## Pipeline

`maestro-design -> [maestro-card: qa-baseline -> feature accept -> work ->
verify -> qa-slice -> feature ship]`
