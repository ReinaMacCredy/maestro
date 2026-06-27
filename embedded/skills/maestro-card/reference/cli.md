<!-- maestro:cli-reference-version: 1.1.0 -->
<!-- maestro:cli-reference-sha256: b3ae6239b5461588dc4700cea42f6170d7ffcaa73f3975cacfe2a51a819b425e -->
<!-- generated; do not edit by hand; regenerate: cargo test --test cli_reference_freshness regenerate_cli_md -- --ignored -->
# maestro CLI reference

Authoritative signatures generated from the binary's clap model,
filtered for the `maestro-card` skill. Every listed verb and flag is exact;
a spelling not found here is outside this skill's CLI surface.
`<X>` required, `[X]` optional, `...` repeatable.

## maestro status

- `maestro status [--json]` -- Show the repo's current agent handoff and next action

## maestro task

- `maestro task create <TITLE> [--feature <FEATURE>] [--card <CARD>] [--lane <LANE>] [--risk <RISK>] [--check <CHECK>]... [--covers <COVERS>]... [--project <PROJECT>] [--id-only]` -- Create a task (-> draft)
- `maestro task set <ID> [--check <CHECK>]... [--feature <FEATURE>] [--no-feature] [--covers <COVERS>]... [--verify-command <VERIFY_COMMAND>] [--clear-verify-command]` -- Author task checks or change its feature link
- `maestro task explore <ID>` -- Move a draft into exploring (-> exploring)
- `maestro task accept <ID>` -- Lock acceptance and mark the task ready (-> ready)
- `maestro task claim [ID] [--next]` -- Claim a ready, unblocked task to work on it (-> in_progress)
- `maestro task complete <ID> --summary <SUMMARY> --claim <CLAIM>... [--proof <PROOF>]...` -- Submit work for verification (-> needs_verification)
- `maestro task verify [ID]` -- Run the evidence gate; on pass marks the task verified
- `maestro task next [--json]` -- Print the next task action for the current repo
- `maestro task note <ID> <TEXT>` -- Append a dated note to a task's notes.md
- `maestro task update <ID> [--summary <SUMMARY>] [--claim <CLAIM>]...` -- Record progress (summary and/or claims) without changing state
- `maestro task block <ID> --reason <REASON> [--by <BY>]` -- Add a blocker to a task
- `maestro task unblock <ID> --blocker <BLOCKER>` -- Resolve a blocker by its blk- id
- `maestro task reject <ID> --reason <REASON>` -- Terminally reject a task (-> rejected)
- `maestro task abandon <ID> --reason <REASON>` -- Terminally abandon a task (-> abandoned)
- `maestro task supersede <ID> --by <BY> --reason <REASON>` -- Replace a task with another (-> superseded)
- `maestro task show [ID]` -- Show a task's detail: state, claim, blockers
- `maestro task list [--blocked] [--blocked-by <BLOCKED_BY>] [--blocks <BLOCKS>] [--feature <FEATURE>] [--ready] [--mine] [--all] [--interval <INTERVAL>]` -- List tasks, with optional filters
- `maestro task watch [ID] [--interval <INTERVAL>]` -- Watch tasks live, refreshing on an interval
- `maestro task proof [TASK_ID] [--task-id <TASK_ID>]` -- Show a task's proof status
- `maestro task doctor` -- Check the task blocker graph for cycles and dangling refs

## maestro event

- `maestro event create [--task-id <TASK_ID>] [--message <MESSAGE>] [--payload <PAYLOAD>] [--claim <CLAIM>]... [--run <RUN>]` -- Record a run event, optionally bound to a task and carrying claims
- `maestro event intervention --note <NOTE> [--topic <TOPIC>] [--run <RUN>]` -- Record an explicit human correction/intervention event

## maestro feature

- `maestro feature finalize <ID>` -- Write or refresh the clean design handoff before accept/prepare
- `maestro feature accept <ID> [--qa <SURFACE>] [--reason <REASON>] [--dry-run]` -- Accept a feature into ready, freezing its contract (-> ready; gated)
- `maestro feature prepare <ID> [--from <PLAN_FILE>] [--draft] [--task <TASK>]... [--check <CHECK>]... [--covers <COVERS>]... [--blocker <BLOCKER>]... [--after <AFTER>]...` -- Prepare an accepted feature into a ready implementation queue
- `maestro feature amend <ID> [--add-acceptance <ADD_ACCEPTANCE>]... [--add-area <ADD_AREA>]... [--add-non-goal <ADD_NON_GOAL>]... [--add-question <ADD_QUESTION>]... --reason <REASON>` -- Grow a frozen contract additively with an audit reason (ready/in_progress)
- `maestro feature start <ID>` -- Start work on a ready feature (-> in_progress)
- `maestro feature verify <ID> [--prove <AC_ID>]... [--evidence <EVIDENCE>]... [--waive <AC_ID>]... [--reason <REASON>]... [--no-close] [--outcome <OUTCOME>]` -- Sweep or record proof for a feature's acceptance contract
- `maestro feature proof add <ID> --ac <AC> --evidence <EVIDENCE> [--no-close] [--outcome <OUTCOME>]` -- Record explicit feature acceptance proof
- `maestro feature proof waive <ID> --ac <AC> --reason <REASON>` -- Waive a feature acceptance item with an explicit reason
- `maestro feature note <ID> <TEXT>` -- Append a dated note to a feature's notes.md
- `maestro feature close <ID> [--outcome <OUTCOME>] [--dry-run]` -- Close an in-progress feature (-> closed; gated)
- `maestro feature cancel <ID> --reason <REASON> [--dry-run]` -- Cancel a non-terminal feature, abandoning its live child tasks (-> cancelled)
- `maestro feature show <ID>` -- Show a feature's status, full contract, and task counts
- `maestro feature list [--all]` -- List features with their statuses and task counts
- `maestro feature archive [ID] [--closed] [--dry-run]` -- Archive a terminal feature and its terminal child tasks (-> .maestro/archive/features)
- `maestro feature unarchive <ID>` -- Restore an archived feature and its archived child tasks

## maestro qa

- `maestro qa baseline <ID> [--observed <OBSERVED>] [--observed-file <PATH>] [--observed-stdin]` -- Write a feature QA baseline from explicit observed behavior
- `maestro qa slice <ID> [--scenario <SCENARIO>]... [--observed <OBSERVED>] [--observed-file <PATH>] [--observed-stdin]` -- Append counting QA slice evidence for baseline scenarios

## maestro memory

- `maestro memory create --from <SOURCE_OR_SUGGESTION> [--summary <SUMMARY>] [--lesson <LESSON>] [--signal-type <SIGNAL_TYPE>] [--scope-kind <SCOPE_KIND>] [--scope-ref <SCOPE_REF>]... [--target-surface <TARGET_SURFACE>] [--id-only]` -- Create a Memory card from an explicit source or suggestion id
- `maestro memory list [--all]` -- List Memory cards
- `maestro memory show <ID>` -- Show a Memory card, candidate metadata, and lesson
- `maestro memory search [QUERY]...` -- Search approved Memory
- `maestro memory promote <MEMORY_OR_PROMOTION_ID> [--plan] [--apply] [--scorer-receipt <REF>] [--review-evidence <EVIDENCE>]` -- Plan or apply a gated Memory promotion
- `maestro memory maintain [--level <L0|L1|L2|L3>] [--scope-kind <SCOPE_KIND>] [--scope-ref <SCOPE_REF>]... [--source-ref <SOURCE_REF>]... --reason <REASON> [--proof-link <PROOF_LINK>]... [--run-link <RUN_LINK>]... [--human-approved] [--tokens <TOKENS>] [--wall-minutes <WALL_MINUTES>] [--max-source-refs <MAX_SOURCE_REFS>] [--max-files <MAX_FILES>] [--subagents <SUBAGENTS>] [--id-only]` -- Create a bounded Memory maintenance contract
- `maestro memory dream [--level <L0|L1|L2|L3>] [--scope-kind <SCOPE_KIND>] [--scope-ref <SCOPE_REF>]... [--source-ref <SOURCE_REF>]... --reason <REASON> [--proof-link <PROOF_LINK>]... [--run-link <RUN_LINK>]... [--human-approved] [--tokens <TOKENS>] [--wall-minutes <WALL_MINUTES>] [--max-source-refs <MAX_SOURCE_REFS>] [--max-files <MAX_FILES>] [--subagents <SUBAGENTS>] [--id-only]` -- Create a bounded Memory-dream maintenance contract
- `maestro memory scorer attach <ID> --contract-file <PATH>` -- Attach an inline scorer contract file to memory/candidate.yml
- `maestro memory suggest list [--all]` -- List Memory suggestions
- `maestro memory suggest create [--source-ref <SOURCE_REF>]... --signal-type <SIGNAL_TYPE> --summary <SUMMARY> [--scope-kind <SCOPE_KIND>] [--scope-ref <SCOPE_REF>]... [--target-surface <TARGET_SURFACE>] [--dedupe-key <DEDUPE_KEY>] [--expires-at <EXPIRES_AT>]` -- Create or upsert a visible Memory suggestion
- `maestro memory suggest dismiss <ID> --reason <REASON>` -- Dismiss an open Memory suggestion

## maestro scorer

- `maestro scorer run <CONTRACT_REF>` -- Run a typed scorer contract reference
- `maestro scorer show <RECEIPT_REF>` -- Show one scorer receipt by path or memory-id#receipt-id
- `maestro scorer list --memory <MEMORY_ID>` -- List scorer receipts for a Memory card

## maestro card

- `maestro card ready [FEATURE] [--json] [--project <PROJECT>]` -- List workable cards with no open blockers
- `maestro card list [--parent <PARENT>] [--type <TYPE>] [--assignee <ASSIGNEE>] [--status <STATUS>] [--project <PROJECT>] [--grep <TERM>] [--archived] [--all] [--json]` -- List cards filtered by parent, type, assignee, or coarse status
- `maestro card dep add <CHILD> <PARENT>` -- Add a blocking edge: CHILD waits until PARENT closes
- `maestro card dep remove <CHILD> <PARENT>` -- Remove a blocking edge so CHILD no longer waits on PARENT
- `maestro card archive [FEATURE] [--loose]` -- Archive a feature card and its child cards
- `maestro card claim <ID>` -- Claim a workable card for this session
- `maestro card assign <ID> [WHO] [--clear]` -- Suggest an owner for a workable card (advisory; never blocks a claim)
- `maestro card note <ID> <TEXT>` -- Append a dated note to a card's notes.md
- `maestro card create <TITLE>... -t|--type <TYPE> [--parent <PARENT>] [--description <TEXT>] [--active-form <TEXT>] [--project <PROJECT>] [--kind <KIND>] [--id-only]` -- Create a card of any type
- `maestro card show <ID> [--json] [--compact-json]` -- Show a card's header, edges, and body
- `maestro card update [ID] [--status <STATUS>] [--title <TITLE>] [--description <TEXT>] [--active-form <TEXT>] [--claim] [--json]` -- Update a card's status, title, description, or claim
- `maestro card close <ID>` -- Close a card: status -> closed
- `maestro card graph [ID] [--dot]` -- Walk a card's typed edges (parent/blocks/related/supersedes)

## maestro active

- `maestro active [--all] [--connect]` -- Show what other live sessions are doing (cross-session awareness)

## maestro link

- `maestro link add <CARD-A> <CARD-B>` -- Add a non-blocking related link between two live cards
- `maestro link remove <FROM> <TO>` -- Remove a related link between two live cards

## maestro msg

- `maestro msg send <TO> <TEXT> [--from <CARD>]` -- Send a message to a linked card (sender is your current card)
- `maestro msg read [CARD]` -- Read unread messages; with no card, aggregate every linked partner
- `maestro msg list [CARD]` -- Channel overview, or one partner's full timeline

## maestro conflict

- `maestro conflict <PEER> [REASON] [--clear]` -- Flag a work conflict on a peer card so it holds off (no link, no git)

## maestro harness

- `maestro harness list [--all]` -- List proposals (proposed + accepted; --all adds the terminal ledger)
- `maestro harness show <ID>` -- Show a proposal's detail and history
- `maestro harness apply <ID> [--check <CHECK>]...` -- Accept a proposal and spawn a linked task (-> accepted)
- `maestro harness dismiss <ID> --reason <REASON>` -- Dismiss a noisy proposal and suppress its fingerprint
- `maestro harness measure <ID> [--force]` -- Re-run the detector to close or revert a proposal (-> measured)

## maestro watch

- `maestro watch [ID] [--interval <INTERVAL>]` -- Live dependency-tree board (bare) or a one-shot snapshot; optional feature-id focuses one feature
- `maestro watch snapshot [ID]` -- Render the live board once and exit
