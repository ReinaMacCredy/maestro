<!-- maestro:cli-reference-version: 1.1.0 -->
<!-- maestro:cli-reference-sha256: 64b7374fb924d1c7c9bb5b3cde87258249b0be79bef7ed6d27e17c21135a0436 -->
<!-- generated; do not edit by hand; regenerate: cargo test --test cli_reference_freshness regenerate_cli_md -- --ignored -->
# maestro CLI reference

Authoritative signatures generated from the binary's clap model,
filtered for the `maestro-design` skill. Every listed verb and flag is exact;
a spelling not found here is outside this skill's CLI surface.
`<X>` required, `[X]` optional, `...` repeatable.

## maestro design

- `maestro design list` -- List shipped DESIGN.md style tokens
- `maestro design init [--style <STYLE>] [--dry-run] [--force]` -- Write repo-root DESIGN.md from a shipped style

## maestro status

- `maestro status [--json]` -- Show the repo's current agent handoff and next action

## maestro feature

- `maestro feature new <TITLE> [--description <DESCRIPTION>] [--question <QUESTION>]... [--project <PROJECT>] [--id-only]` -- Propose a new feature (-> proposed)
- `maestro feature set <ID> [--acceptance <ACCEPTANCE>]... [--area <AREA>]... [--non-goal <NON_GOAL>]... [--question <QUESTION>]... [--description <DESCRIPTION>] [--request <REQUEST>] [--type <INPUT_TYPE>]` -- Author a proposed feature's contract (replace or append fields)
- `maestro feature finalize <ID>` -- Write or refresh the clean design handoff before accept/prepare
- `maestro feature reopen <ID>` -- Reopen a DB-backed finalized feature into .maestro/workbench/<id>
- `maestro feature show <ID>` -- Show a feature's status, full contract, and task counts
- `maestro feature spec <ID> [--section <SECTION>] [--append <TEXT>] [--replace <TEXT>]` -- Render a feature's spec-of-record, or fill one section (--section with --append/--replace)
- `maestro feature list [--all]` -- List features with their statuses and task counts

## maestro decision

- `maestro decision audit [--compressed] [--json]` -- Audit decision records for repairable conditions
- `maestro decision set draft [--from <PATH>] [--from-text <TEXT>] [--output <PATH>] [--json]` -- Draft a DecisionSet from YAML, fenced YAML, or plain text
- `maestro decision set lock --from <PATH> [--dry-run] [--json] [--show]` -- Atomically lock a DecisionSet and its child decisions
- `maestro decision set repair <ID> --from <PATH> [--dry-run] [--json]` -- Repair one compressed summary into a DecisionSet replacement
- `maestro decision set show <ID> [--json]` -- Show a locked DecisionSet by id
- `maestro decision new <TITLE> [--context <CONTEXT>] [--feature <FEATURE>] [--lock] [--decision <DECISION>] [--rejected <REJECTED>]... [--preview <PREVIEW>] [--supersedes <SUPERSEDES>]... [--allow-summary-decision] [--project <PROJECT>] [--id-only]` -- Open a structured decision fork (mints a decision card)
- `maestro decision lock <ID> --decision <DECISION> [--rejected <REJECTED>]... [--preview <PREVIEW>] [--supersedes <SUPERSEDES>]... [--allow-summary-decision]` -- Lock an open decision with the chosen answer
- `maestro decision supersede <OLD_ID> --decision <DECISION> --reason <REASON> [--title <TITLE>] [--rejected <REJECTED>]... [--preview <PREVIEW>] [--id-only]` -- Replace a locked decision by superseding it
- `maestro decision show <ID> [--include-set]` -- Show a decision card by id
- `maestro decision list [--all] [--feature <FEATURE>]` -- List decision cards (recent 20 by activity unless --all)

## maestro card

- `maestro card list [--parent <PARENT>] [--type <TYPE>] [--assignee <ASSIGNEE>] [--status <STATUS>] [--project <PROJECT>] [--grep <TERM>] [--archived] [--all] [--json]` -- List cards filtered by parent, type, assignee, or coarse status
- `maestro card show <ID> [--json] [--compact-json]` -- Show a card's header, edges, and body

## maestro active

- `maestro active [--all] [--connect]` -- Show what other live sessions are doing (cross-session awareness)

## maestro link

- `maestro link add <CARD-A> <CARD-B>` -- Add a non-blocking related link between two live cards
- `maestro link remove <FROM> <TO>` -- Remove a related link between two live cards

## maestro msg

- `maestro msg send <TO> <TEXT> [--from <CARD>]` -- Send a message to a linked card (sender is your current card)
- `maestro msg read [CARD]` -- Read unread messages; with no card, aggregate every linked partner
- `maestro msg list [CARD]` -- Channel overview, or one partner's full timeline

## maestro archive

- `maestro archive candidates [--json]` -- List archive candidates and their gate status
- `maestro archive check <ID> [--json]` -- Show the archive gate result for one target

## maestro loop

- `maestro loop list` -- List shipped and project custom recipes
- `maestro loop next [--json] [--compact] [--phase <PHASE>]` -- Recommend the next loop recipe without mutating state
- `maestro loop improve [--json]` -- Plan loop improvement proposals without mutating state
- `maestro loop show <NAME> [--compact] [--phase <PHASE>] [--json]` -- Print one shipped or project custom recipe
- `maestro loop validate <NAME>` -- Validate one structured shipped or project custom loop recipe
- `maestro loop outcome --recipe <RECIPE> --phase <PHASE> --selected-unit <SELECTED_UNIT> [--constraint <CONSTRAINTS>]... [--proof-result <PROOF_RESULT>] --failure-class <FAILURE_CLASS> [--blocker-class <BLOCKER_CLASS>] [--retry-count <RETRY_COUNT>] [--duration-ms <DURATION_MS>] [--learning-candidate <LEARNING_CANDIDATE>] [--source-ref <SOURCE_REF>]... [--run <RUN>] [--json]` -- Record a write-side loop outcome event
