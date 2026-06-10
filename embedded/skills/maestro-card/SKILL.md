---
name: maestro-card
version: 1.0.0
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
  [reference/work.md](reference/work.md)
- Author, accept, prepare, amend, ship, or archive a feature card:
  [reference/feature.md](reference/feature.md)
- Prove a claim, repair failed proof, or verify adversarially:
  [reference/verify.md](reference/verify.md)
- Capture the behavior contract before `feature accept`:
  [reference/qa-baseline.md](reference/qa-baseline.md)
- Replay scenarios and record slice evidence before `feature ship`:
  [reference/qa-slice.md](reference/qa-slice.md)

## Shared Ground

- Discover work with the flat card verbs: `maestro ready [<feature>]`,
  `maestro list --parent <feature> [--type T --assignee A --status S]`,
  `maestro show <id>`.
- Take and annotate work with `maestro claim <id>`, `maestro note <id>
  "<text>"`, and `maestro dep add <child> <blocker>`.
- Ids are stable and opaque (`card-<hash>`; features keep their creation
  slug). The dotted alias `show` prints is display-only; never address a card
  with it.
- Do not hand-edit `card.yaml` or the verb-guarded sidecars (`qa.md`,
  state history). Use verbs so gates and audit trails stay intact.
- When the user corrects your behavior, record it:
  `maestro event intervention --note "<what was wrong>" [--topic <slug>]`.

## Pipeline

`maestro-design -> [maestro-card: qa-baseline -> feature accept -> work ->
verify -> qa-slice -> feature ship]`
