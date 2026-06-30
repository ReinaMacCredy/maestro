# External Intake

When the user brings a spec, plan, or PRD authored elsewhere, route open forks
through `maestro-design` first. This skill consumes the approved contract and
drives the active lifecycle; there is no CLI parser for external documents.

1. Use `maestro-design` to create the feature, preserve the source text, decide
   open forks, and author observable acceptance criteria.
2. Return here after the contract is stable.
3. Read `.maestro/cards/<id>/handoff.md` first. If it is missing or stale, run
   `maestro feature finalize <id>`.
4. Run `qa-baseline`, `feature accept`, `feature prepare`, work, verify,
   `qa-slice`, and `feature close`.

Completion criterion: `request.md` exists on the card, every open fork from the
source has either a locked decision or an explicit feature question, and the
active lifecycle starts from a fresh handoff.

`request.md` travels with the card through archive and unarchive.
