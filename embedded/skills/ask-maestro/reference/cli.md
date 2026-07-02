<!-- maestro:cli-reference-version: 1.1.0 -->
<!-- maestro:cli-reference-sha256: fb1311633e35994eeade25bc69e81e396dc1976bec48a7bf4ab985385f639fd4 -->
<!-- generated; do not edit by hand; regenerate: cargo test --test cli_reference_freshness regenerate_cli_md -- --ignored -->
# maestro CLI reference

Authoritative signatures generated from the binary's clap model,
filtered for the `ask-maestro` skill. Every listed verb and flag is exact;
a spelling not found here is outside this skill's CLI surface.
`<X>` required, `[X]` optional, `...` repeatable.

## maestro init

- `maestro init [--dry-run] [--merge] [--force] [--yes]` -- Scaffold .maestro/ and extract bundled resources into this repo

## maestro install

- `maestro install [AGENT] [--agent <AGENT>]` -- Install maestro hooks and config for an agent (claude, codex, droid)

## maestro sync

- `maestro sync [--dry-run] [--global-skills]` -- Resync bundled resources to this binary's versions (offline)

## maestro doctor

- `maestro doctor` -- Diagnose the maestro installation and report problems

## maestro status

- `maestro status [--json]` -- Show the repo's current agent handoff and next action

## maestro task

- `maestro task add <TITLE> [--card <CARD>] [--project <PROJECT>] [--id-only]` -- Add a low-ceremony task ready to start
- `maestro task setup --task <TASK>... [--start] [--atomic] [--reason <REASON>] [--project <PROJECT>]` -- Set up a low-ceremony Progress checklist before work starts
- `maestro task start <REF_OR_ID>` -- Start a ready task (alias for claim)
- `maestro task done <REF_OR_ID> [--summary <SUMMARY>] --proof <PROOF>...` -- Mark a low-ceremony task done when it has no explicit gate
- `maestro task show [REF_OR_ID]` -- Show a task's detail: state, claim, blockers
- `maestro task list [--blocked] [--blocked-by <BLOCKED_BY>] [--blocks <BLOCKS>] [--feature <FEATURE>] [--ready] [--mine] [--all] [--json] [--interval <INTERVAL>]` -- List tasks, with optional filters

## maestro feature

- `maestro feature new <TITLE> [--description <DESCRIPTION>] [--question <QUESTION>]... [--project <PROJECT>] [--id-only]` -- Propose a new feature (-> proposed)
- `maestro feature set <ID> [--acceptance <ACCEPTANCE>]... [--area <AREA>]... [--non-goal <NON_GOAL>]... [--question <QUESTION>]... [--description <DESCRIPTION>] [--request <REQUEST>] [--type <INPUT_TYPE>]` -- Author a proposed feature's contract (replace or append fields)
- `maestro feature finalize <ID>` -- Write or refresh the clean design handoff before accept/prepare
- `maestro feature show <ID>` -- Show a feature's status, full contract, and task counts
- `maestro feature list [--all]` -- List features with their statuses and task counts

## maestro qa

- `maestro qa baseline <ID> [--observed <OBSERVED>] [--observed-file <PATH>] [--observed-stdin]` -- Write a feature QA baseline from explicit observed behavior
- `maestro qa slice <ID> [--scenario <SCENARIO>]... [--observed <OBSERVED>] [--observed-file <PATH>] [--observed-stdin]` -- Append counting QA slice evidence for baseline scenarios

## maestro decision

- `maestro decision new <TITLE> [--context <CONTEXT>] [--feature <FEATURE>] [--lock] [--decision <DECISION>] [--rejected <REJECTED>]... [--preview <PREVIEW>] [--supersedes <SUPERSEDES>]... [--project <PROJECT>] [--id-only]` -- Open a structured decision fork (mints a decision card)
- `maestro decision lock <ID> --decision <DECISION> [--rejected <REJECTED>]... [--preview <PREVIEW>] [--supersedes <SUPERSEDES>]...` -- Lock an open decision with the chosen answer
- `maestro decision show <ID>` -- Show a decision card by id

## maestro card

- `maestro card list [--parent <PARENT>] [--type <TYPE>] [--assignee <ASSIGNEE>] [--status <STATUS>] [--project <PROJECT>] [--grep <TERM>] [--archived] [--all] [--json]` -- List cards filtered by parent, type, assignee, or coarse status
- `maestro card show <ID> [--json] [--compact-json]` -- Show a card's header, edges, and body

## maestro active

- `maestro active [--all] [--connect]` -- Show what other live sessions are doing (cross-session awareness)

## maestro link

- `maestro link add <CARD-A> <CARD-B>` -- Add a non-blocking related link between two live cards

## maestro msg

- `maestro msg send <TO> <TEXT> [--from <CARD>]` -- Send a message to a linked card (sender is your current card)
- `maestro msg read [CARD]` -- Read unread messages; with no card, aggregate every linked partner

## maestro harness

- `maestro harness list [--all]` -- List proposals (proposed + accepted; --all adds the terminal ledger)
- `maestro harness show <ID>` -- Show a proposal's detail and history
- `maestro harness apply <ID> [--check <CHECK>]...` -- Accept a proposal and spawn a linked task (-> accepted)

## maestro loop

- `maestro loop list` -- List shipped and project custom recipes
- `maestro loop next [--json] [--compact] [--phase <PHASE>]` -- Recommend the next loop recipe without mutating state
- `maestro loop improve [--json]` -- Plan loop improvement proposals without mutating state
- `maestro loop show <NAME> [--compact] [--phase <PHASE>] [--json]` -- Print one shipped or project custom recipe
- `maestro loop outcome --recipe <RECIPE> --phase <PHASE> --selected-unit <SELECTED_UNIT> [--constraint <CONSTRAINTS>]... [--proof-result <PROOF_RESULT>] --failure-class <FAILURE_CLASS> [--blocker-class <BLOCKER_CLASS>] [--retry-count <RETRY_COUNT>] [--duration-ms <DURATION_MS>] [--learning-candidate <LEARNING_CANDIDATE>] [--source-ref <SOURCE_REF>]... [--run <RUN>] [--json]` -- Record a write-side loop outcome event
