<!-- maestro:cli-reference-version: 1.0.0 -->
<!-- maestro:cli-reference-sha256: c96775eed21f6f924d2c33e18fc3247865ba06f0795ea202dbb9393a5f5cbbf5 -->
<!-- generated; do not edit by hand; regenerate: cargo test --test cli_reference_freshness regenerate_cli_md -- --ignored -->
# maestro CLI reference

Authoritative signatures generated from the binary's clap model.
Every verb and flag is listed; a spelling not found here does not exist.
`<X>` required, `[X]` optional, `...` repeatable.

## maestro init

- `maestro init [--dry-run] [--merge] [--force] [--yes]` -- Scaffold .maestro/ and extract bundled resources into this repo

## maestro install

- `maestro install [AGENT] [--agent <AGENT>]` -- Install maestro hooks and config for an agent (claude, codex)

## maestro upgrade

- `maestro upgrade [--check] [--verbose] [--force]` -- Upgrade the maestro binary and refresh bundled resources

## maestro sync

- `maestro sync [--dry-run] [--global-skills]` -- Resync bundled resources to this binary's versions (offline)

## maestro migrate-v2

- `maestro migrate-v2` -- Migrate v1 Maestro artifacts to the reduced v2 layout

## maestro migrate

- `maestro migrate` -- Fold the legacy v2 trees (features/tasks/decisions/backlog) into the card store

## maestro uninstall

- `maestro uninstall [AGENT] [--agent <AGENT>]` -- Remove maestro hooks and config for an agent

## maestro doctor

- `maestro doctor` -- Diagnose the maestro installation and report problems

## maestro shell-init

- `maestro shell-init` -- Print the shell init snippet for maestro

## maestro status

- `maestro status [--json]` -- Show the repo's current agent handoff and next action

## maestro resume

- `maestro resume [--task <TASK_ID>] [--feature <FEATURE_ID>] [--full] [--handoff] [--write] [--json]` -- Print a clean-session resume packet from current repo artifacts

## maestro task

- `maestro task create <TITLE> [--feature <FEATURE>] [--lane <LANE>] [--risk <RISK>] [--check <CHECK>]... [--covers <COVERS>]... [--id-only]` -- Create a task (-> draft)
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
- `maestro task list [--blocked] [--blocked-by <BLOCKED_BY>] [--blocks <BLOCKS>] [--feature <FEATURE>] [--ready] [--all] [--watch] [--interval <INTERVAL>]` -- List tasks, with optional filters
- `maestro task watch [ID] [--interval <INTERVAL>]` -- Watch tasks live, refreshing on an interval
- `maestro task doctor` -- Check the task blocker graph for cycles and dangling refs
- `maestro task archive <ID> [--dry-run]` -- Archive a done task out of the live scan (-> .maestro/archive/tasks)
- `maestro task unarchive <ID>` -- Restore an archived task to the live scan
- `maestro task help create` -- Create a task (-> draft)
- `maestro task help set` -- Author task checks or change its feature link
- `maestro task help explore` -- Move a draft into exploring (-> exploring)
- `maestro task help accept` -- Lock acceptance and mark the task ready (-> ready)
- `maestro task help claim` -- Claim a ready, unblocked task to work on it (-> in_progress)
- `maestro task help complete` -- Submit work for verification (-> needs_verification)
- `maestro task help verify` -- Run the evidence gate; on pass marks the task verified
- `maestro task help next` -- Print the next task action for the current repo
- `maestro task help note` -- Append a dated note to a task's notes.md
- `maestro task help update` -- Record progress (summary and/or claims) without changing state
- `maestro task help block` -- Add a blocker to a task
- `maestro task help unblock` -- Resolve a blocker by its blk- id
- `maestro task help reject` -- Terminally reject a task (-> rejected)
- `maestro task help abandon` -- Terminally abandon a task (-> abandoned)
- `maestro task help supersede` -- Replace a task with another (-> superseded)
- `maestro task help show` -- Show a task's detail: state, claim, blockers
- `maestro task help list` -- List tasks, with optional filters
- `maestro task help watch` -- Watch tasks live, refreshing on an interval
- `maestro task help doctor` -- Check the task blocker graph for cycles and dangling refs
- `maestro task help archive` -- Archive a done task out of the live scan (-> .maestro/archive/tasks)
- `maestro task help unarchive` -- Restore an archived task to the live scan
- `maestro task help help` -- Print this message or the help of the given subcommand(s)

## maestro event

- `maestro event create [--task-id <TASK_ID>] [--message <MESSAGE>] [--payload <PAYLOAD>] [--claim <CLAIM>]... [--run <RUN>]` -- Record a run event, optionally bound to a task and carrying claims
- `maestro event intervention --note <NOTE> [--topic <TOPIC>] [--run <RUN>]` -- Record an explicit human correction/intervention event
- `maestro event help create` -- Record a run event, optionally bound to a task and carrying claims
- `maestro event help intervention` -- Record an explicit human correction/intervention event
- `maestro event help help` -- Print this message or the help of the given subcommand(s)

## maestro feature

- `maestro feature new <TITLE> [--description <DESCRIPTION>] [--question <QUESTION>]... [--id-only]` -- Propose a new feature (-> proposed)
- `maestro feature set <ID> [--acceptance <ACCEPTANCE>]... [--area <AREA>]... [--non-goal <NON_GOAL>]... [--question <QUESTION>]... [--clear-questions] [--add-acceptance <ADD_ACCEPTANCE>]... [--add-area <ADD_AREA>]... [--add-non-goal <ADD_NON_GOAL>]... [--add-question <ADD_QUESTION>]... [--edit-acceptance <AC_ID>]... [--text <TEXT>]... [--description <DESCRIPTION>] [--request <REQUEST>] [--type <INPUT_TYPE>]` -- Author a proposed feature's contract (replace or append fields)
- `maestro feature accept <ID> [--qa <SURFACE>] [--reason <REASON>] [--dry-run]` -- Accept a feature into ready, freezing its contract (-> ready; gated)
- `maestro feature prepare <ID> [--from <PLAN_FILE>] [--draft]` -- Prepare an accepted feature into a ready implementation queue
- `maestro feature amend <ID> [--add-acceptance <ADD_ACCEPTANCE>]... [--add-area <ADD_AREA>]... [--add-non-goal <ADD_NON_GOAL>]... [--add-question <ADD_QUESTION>]... --reason <REASON>` -- Grow a frozen contract additively with an audit reason (ready/in_progress)
- `maestro feature start <ID>` -- Start work on a ready feature (-> in_progress)
- `maestro feature verify <ID> [--prove <AC_ID>]... [--evidence <EVIDENCE>]... [--waive <AC_ID>]... [--reason <REASON>]...` -- Sweep or record proof for a feature's acceptance contract
- `maestro feature note <ID> <TEXT>` -- Append a dated note to a feature's notes.md
- `maestro feature ship <ID> [--outcome <OUTCOME>] [--dry-run]` -- Ship an in-progress feature (-> shipped; gated)
- `maestro feature cancel <ID> --reason <REASON> [--dry-run]` -- Cancel a non-terminal feature, abandoning its live child tasks (-> cancelled)
- `maestro feature show <ID>` -- Show a feature's status, full contract, and task counts
- `maestro feature spec <ID> [--section <SECTION>] [--append <TEXT>] [--replace <TEXT>]` -- Render a feature's spec-of-record, or fill one section (--section with --append/--replace)
- `maestro feature list [--all]` -- List features with their statuses and task counts
- `maestro feature archive [ID] [--closed] [--dry-run]` -- Archive a terminal feature and its terminal child tasks (-> .maestro/archive/features)
- `maestro feature unarchive <ID>` -- Restore an archived feature and its archived child tasks
- `maestro feature help new` -- Propose a new feature (-> proposed)
- `maestro feature help set` -- Author a proposed feature's contract (replace or append fields)
- `maestro feature help accept` -- Accept a feature into ready, freezing its contract (-> ready; gated)
- `maestro feature help prepare` -- Prepare an accepted feature into a ready implementation queue
- `maestro feature help amend` -- Grow a frozen contract additively with an audit reason (ready/in_progress)
- `maestro feature help start` -- Start work on a ready feature (-> in_progress)
- `maestro feature help verify` -- Sweep or record proof for a feature's acceptance contract
- `maestro feature help note` -- Append a dated note to a feature's notes.md
- `maestro feature help ship` -- Ship an in-progress feature (-> shipped; gated)
- `maestro feature help cancel` -- Cancel a non-terminal feature, abandoning its live child tasks (-> cancelled)
- `maestro feature help show` -- Show a feature's status, full contract, and task counts
- `maestro feature help spec` -- Render a feature's spec-of-record, or fill one section (--section with --append/--replace)
- `maestro feature help list` -- List features with their statuses and task counts
- `maestro feature help archive` -- Archive a terminal feature and its terminal child tasks (-> .maestro/archive/features)
- `maestro feature help unarchive` -- Restore an archived feature and its archived child tasks
- `maestro feature help help` -- Print this message or the help of the given subcommand(s)

## maestro decision

- `maestro decision new <TITLE> [--context <CONTEXT>] [--feature <FEATURE>] [--lock] [--decision <DECISION>] [--rejected <REJECTED>]... [--preview <PREVIEW>] [--supersedes <SUPERSEDES>]... [--id-only]` -- Open a structured decision fork (mints a decision card)
- `maestro decision lock <ID> --decision <DECISION> [--rejected <REJECTED>]... [--preview <PREVIEW>] [--supersedes <SUPERSEDES>]...` -- Lock an open decision with the chosen answer
- `maestro decision show <ID>` -- Show a decision card by id
- `maestro decision list` -- List decision cards
- `maestro decision help new` -- Open a structured decision fork (mints a decision card)
- `maestro decision help lock` -- Lock an open decision with the chosen answer
- `maestro decision help show` -- Show a decision card by id
- `maestro decision help list` -- List decision cards
- `maestro decision help help` -- Print this message or the help of the given subcommand(s)

## maestro card

- `maestro card ready [FEATURE] [--json]` -- List workable cards with no open blockers
- `maestro card list [--parent <PARENT>] [--type <TYPE>] [--assignee <ASSIGNEE>] [--status <STATUS>] [--grep <TERM>] [--archived] [--json]` -- List cards filtered by parent, type, assignee, or coarse status
- `maestro card dep add <CHILD> <PARENT>` -- Add a blocking edge: CHILD waits until PARENT closes
- `maestro card dep remove <CHILD> <PARENT>` -- Remove a blocking edge so CHILD no longer waits on PARENT
- `maestro card dep help add` -- Add a blocking edge: CHILD waits until PARENT closes
- `maestro card dep help remove` -- Remove a blocking edge so CHILD no longer waits on PARENT
- `maestro card dep help help` -- Print this message or the help of the given subcommand(s)
- `maestro card archive [FEATURE] [--loose]` -- Archive a feature card and its child cards
- `maestro card claim <ID>` -- Claim a workable card for this session
- `maestro card note <ID> <TEXT>` -- Append a dated note to a card's notes.md
- `maestro card create <TITLE> -t|--type <TYPE> [--parent <PARENT>] [--description <TEXT>] [--id-only]` -- Create a card of any type
- `maestro card show <ID> [--json] [--compact-json]` -- Show a card's header, edges, and body
- `maestro card update [ID] [--status <STATUS>] [--title <TITLE>] [--description <TEXT>] [--claim] [--json]` -- Update a card's status, title, description, or claim
- `maestro card close <ID>` -- Close a card: status -> closed
- `maestro card help ready` -- List workable cards with no open blockers
- `maestro card help list` -- List cards filtered by parent, type, assignee, or coarse status
- `maestro card help dep add` -- Add a blocking edge: CHILD waits until PARENT closes
- `maestro card help dep remove` -- Remove a blocking edge so CHILD no longer waits on PARENT
- `maestro card help archive` -- Archive a feature card and its child cards
- `maestro card help claim` -- Claim a workable card for this session
- `maestro card help note` -- Append a dated note to a card's notes.md
- `maestro card help create` -- Create a card of any type
- `maestro card help show` -- Show a card's header, edges, and body
- `maestro card help update` -- Update a card's status, title, description, or claim
- `maestro card help close` -- Close a card: status -> closed
- `maestro card help help` -- Print this message or the help of the given subcommand(s)

## maestro ready

- `maestro ready [FEATURE] [--json]` -- List workable cards with no open blockers (card store)

## maestro list

- `maestro list [--parent <PARENT>] [--type <TYPE>] [--assignee <ASSIGNEE>] [--status <STATUS>] [--grep <TERM>] [--archived] [--json]` -- List cards filtered by parent, type, assignee, or coarse status (card store)

## maestro dep

- `maestro dep add <CHILD> <PARENT>` -- Add a blocking edge: CHILD waits until PARENT closes
- `maestro dep remove <CHILD> <PARENT>` -- Remove a blocking edge so CHILD no longer waits on PARENT
- `maestro dep help add` -- Add a blocking edge: CHILD waits until PARENT closes
- `maestro dep help remove` -- Remove a blocking edge so CHILD no longer waits on PARENT
- `maestro dep help help` -- Print this message or the help of the given subcommand(s)

## maestro active

- `maestro active [--all]` -- Show what other live sessions are doing (cross-session awareness)

## maestro link

- `maestro link add <FROM> <TO>` -- Add a non-blocking related link between two live cards
- `maestro link remove <FROM> <TO>` -- Remove a related link between two live cards
- `maestro link help add` -- Add a non-blocking related link between two live cards
- `maestro link help remove` -- Remove a related link between two live cards
- `maestro link help help` -- Print this message or the help of the given subcommand(s)

## maestro archive

- `maestro archive [FEATURE] [--loose]` -- Archive a feature card and its child cards (card store)

## maestro claim

- `maestro claim <ID>` -- Claim a workable card for this session (card store)

## maestro note

- `maestro note <ID> <TEXT>` -- Append a dated note to a card's notes.md (card store)

## maestro create

- `maestro create <TITLE> -t|--type <TYPE> [--parent <PARENT>] [--description <TEXT>] [--id-only]` -- Create a card of any type (card store)

## maestro show

- `maestro show <ID> [--json] [--compact-json]` -- Show a card's header, edges, and body (card store)

## maestro update

- `maestro update [ID] [--status <STATUS>] [--title <TITLE>] [--description <TEXT>] [--claim] [--json]` -- Update a card's status, title, description, or claim (card store)

## maestro close

- `maestro close <ID>` -- Close a card: status -> closed (card store)

## maestro harness

- `maestro harness list [--all]` -- List proposals (proposed + accepted; --all adds the terminal ledger)
- `maestro harness show <ID>` -- Show a proposal's detail and history
- `maestro harness set [--claims-only]` -- Set harness policy flags
- `maestro harness propose --title <TITLE> --evidence <EVIDENCE>... [--topic <TOPIC>]` -- File an agent-authored repo audit proposal
- `maestro harness apply <ID> [--check <CHECK>]...` -- Accept a proposal and spawn a linked task (-> accepted)
- `maestro harness unapply <ID> [--reason <REASON>]` -- Undo an accepted proposal before its linked task is claimed
- `maestro harness dismiss <ID> --reason <REASON>` -- Dismiss a noisy proposal and suppress its fingerprint
- `maestro harness measure <ID> [--force]` -- Re-run the detector to close or revert a proposal (-> measured)
- `maestro harness help list` -- List proposals (proposed + accepted; --all adds the terminal ledger)
- `maestro harness help show` -- Show a proposal's detail and history
- `maestro harness help set` -- Set harness policy flags
- `maestro harness help propose` -- File an agent-authored repo audit proposal
- `maestro harness help apply` -- Accept a proposal and spawn a linked task (-> accepted)
- `maestro harness help unapply` -- Undo an accepted proposal before its linked task is claimed
- `maestro harness help dismiss` -- Dismiss a noisy proposal and suppress its fingerprint
- `maestro harness help measure` -- Re-run the detector to close or revert a proposal (-> measured)
- `maestro harness help help` -- Print this message or the help of the given subcommand(s)

## maestro query

- `maestro query matrix` -- Show the feature x task matrix (FEATURE/TASK/STATE/PROOF/TITLE)
- `maestro query friction` -- Summarize recorded run friction (events, prompts, corrections)
- `maestro query decisions` -- List decision cards (ID/STATUS/HOME/TITLE)
- `maestro query backlog` -- List improvement backlog items (ID/TITLE)
- `maestro query proof [TASK_ID] [--task-id <TASK_ID>]` -- Show a task's proof status
- `maestro query graph [ID] [--dot]` -- Walk a card's typed edges (parent/blocks/related/supersedes)
- `maestro query help matrix` -- Show the feature x task matrix (FEATURE/TASK/STATE/PROOF/TITLE)
- `maestro query help friction` -- Summarize recorded run friction (events, prompts, corrections)
- `maestro query help decisions` -- List decision cards (ID/STATUS/HOME/TITLE)
- `maestro query help backlog` -- List improvement backlog items (ID/TITLE)
- `maestro query help proof` -- Show a task's proof status
- `maestro query help graph` -- Walk a card's typed edges (parent/blocks/related/supersedes)
- `maestro query help help` -- Print this message or the help of the given subcommand(s)

## maestro index

- `maestro index rebuild` -- Rebuild the text index over live + archived cards from scratch
- `maestro index help rebuild` -- Rebuild the text index over live + archived cards from scratch
- `maestro index help help` -- Print this message or the help of the given subcommand(s)

## maestro mcp

- `maestro mcp serve` -- Run the MCP server over stdio
- `maestro mcp stdin` -- Run the MCP server over stdio (same as serve)
- `maestro mcp tools` -- List the MCP tool names maestro exposes
- `maestro mcp list` -- List the MCP tool names maestro exposes (same as tools)
- `maestro mcp help serve` -- Run the MCP server over stdio
- `maestro mcp help stdin` -- Run the MCP server over stdio (same as serve)
- `maestro mcp help tools` -- List the MCP tool names maestro exposes
- `maestro mcp help list` -- List the MCP tool names maestro exposes (same as tools)
- `maestro mcp help help` -- Print this message or the help of the given subcommand(s)

## maestro hook

- `maestro hook record [--event <EVENT>] [--skill <SKILL>] [--session <SESSION>]`
- `maestro hook help record`
- `maestro hook help help` -- Print this message or the help of the given subcommand(s)

## maestro watch

- `maestro watch snapshot`
- `maestro watch help snapshot`
- `maestro watch help help` -- Print this message or the help of the given subcommand(s)

## maestro verify

- `maestro verify [ID]` -- Verify a task against its recorded proof

## maestro version

- `maestro version` -- Print the maestro version and binary path

## maestro help

- `maestro help init` -- Scaffold .maestro/ and extract bundled resources into this repo
- `maestro help install` -- Install maestro hooks and config for an agent (claude, codex)
- `maestro help upgrade` -- Upgrade the maestro binary and refresh bundled resources
- `maestro help sync` -- Resync bundled resources to this binary's versions (offline)
- `maestro help migrate-v2` -- Migrate v1 Maestro artifacts to the reduced v2 layout
- `maestro help migrate` -- Fold the legacy v2 trees (features/tasks/decisions/backlog) into the card store
- `maestro help uninstall` -- Remove maestro hooks and config for an agent
- `maestro help doctor` -- Diagnose the maestro installation and report problems
- `maestro help shell-init` -- Print the shell init snippet for maestro
- `maestro help status` -- Show the repo's current agent handoff and next action
- `maestro help resume` -- Print a clean-session resume packet from current repo artifacts
- `maestro help task create` -- Create a task (-> draft)
- `maestro help task set` -- Author task checks or change its feature link
- `maestro help task explore` -- Move a draft into exploring (-> exploring)
- `maestro help task accept` -- Lock acceptance and mark the task ready (-> ready)
- `maestro help task claim` -- Claim a ready, unblocked task to work on it (-> in_progress)
- `maestro help task complete` -- Submit work for verification (-> needs_verification)
- `maestro help task verify` -- Run the evidence gate; on pass marks the task verified
- `maestro help task next` -- Print the next task action for the current repo
- `maestro help task note` -- Append a dated note to a task's notes.md
- `maestro help task update` -- Record progress (summary and/or claims) without changing state
- `maestro help task block` -- Add a blocker to a task
- `maestro help task unblock` -- Resolve a blocker by its blk- id
- `maestro help task reject` -- Terminally reject a task (-> rejected)
- `maestro help task abandon` -- Terminally abandon a task (-> abandoned)
- `maestro help task supersede` -- Replace a task with another (-> superseded)
- `maestro help task show` -- Show a task's detail: state, claim, blockers
- `maestro help task list` -- List tasks, with optional filters
- `maestro help task watch` -- Watch tasks live, refreshing on an interval
- `maestro help task doctor` -- Check the task blocker graph for cycles and dangling refs
- `maestro help task archive` -- Archive a done task out of the live scan (-> .maestro/archive/tasks)
- `maestro help task unarchive` -- Restore an archived task to the live scan
- `maestro help event create` -- Record a run event, optionally bound to a task and carrying claims
- `maestro help event intervention` -- Record an explicit human correction/intervention event
- `maestro help feature new` -- Propose a new feature (-> proposed)
- `maestro help feature set` -- Author a proposed feature's contract (replace or append fields)
- `maestro help feature accept` -- Accept a feature into ready, freezing its contract (-> ready; gated)
- `maestro help feature prepare` -- Prepare an accepted feature into a ready implementation queue
- `maestro help feature amend` -- Grow a frozen contract additively with an audit reason (ready/in_progress)
- `maestro help feature start` -- Start work on a ready feature (-> in_progress)
- `maestro help feature verify` -- Sweep or record proof for a feature's acceptance contract
- `maestro help feature note` -- Append a dated note to a feature's notes.md
- `maestro help feature ship` -- Ship an in-progress feature (-> shipped; gated)
- `maestro help feature cancel` -- Cancel a non-terminal feature, abandoning its live child tasks (-> cancelled)
- `maestro help feature show` -- Show a feature's status, full contract, and task counts
- `maestro help feature spec` -- Render a feature's spec-of-record, or fill one section (--section with --append/--replace)
- `maestro help feature list` -- List features with their statuses and task counts
- `maestro help feature archive` -- Archive a terminal feature and its terminal child tasks (-> .maestro/archive/features)
- `maestro help feature unarchive` -- Restore an archived feature and its archived child tasks
- `maestro help decision new` -- Open a structured decision fork (mints a decision card)
- `maestro help decision lock` -- Lock an open decision with the chosen answer
- `maestro help decision show` -- Show a decision card by id
- `maestro help decision list` -- List decision cards
- `maestro help card ready` -- List workable cards with no open blockers
- `maestro help card list` -- List cards filtered by parent, type, assignee, or coarse status
- `maestro help card dep add` -- Add a blocking edge: CHILD waits until PARENT closes
- `maestro help card dep remove` -- Remove a blocking edge so CHILD no longer waits on PARENT
- `maestro help card archive` -- Archive a feature card and its child cards
- `maestro help card claim` -- Claim a workable card for this session
- `maestro help card note` -- Append a dated note to a card's notes.md
- `maestro help card create` -- Create a card of any type
- `maestro help card show` -- Show a card's header, edges, and body
- `maestro help card update` -- Update a card's status, title, description, or claim
- `maestro help card close` -- Close a card: status -> closed
- `maestro help ready` -- List workable cards with no open blockers (card store)
- `maestro help list` -- List cards filtered by parent, type, assignee, or coarse status (card store)
- `maestro help dep add` -- Add a blocking edge: CHILD waits until PARENT closes
- `maestro help dep remove` -- Remove a blocking edge so CHILD no longer waits on PARENT
- `maestro help active` -- Show what other live sessions are doing (cross-session awareness)
- `maestro help link add` -- Add a non-blocking related link between two live cards
- `maestro help link remove` -- Remove a related link between two live cards
- `maestro help archive` -- Archive a feature card and its child cards (card store)
- `maestro help claim` -- Claim a workable card for this session (card store)
- `maestro help note` -- Append a dated note to a card's notes.md (card store)
- `maestro help create` -- Create a card of any type (card store)
- `maestro help show` -- Show a card's header, edges, and body (card store)
- `maestro help update` -- Update a card's status, title, description, or claim (card store)
- `maestro help close` -- Close a card: status -> closed (card store)
- `maestro help harness list` -- List proposals (proposed + accepted; --all adds the terminal ledger)
- `maestro help harness show` -- Show a proposal's detail and history
- `maestro help harness set` -- Set harness policy flags
- `maestro help harness propose` -- File an agent-authored repo audit proposal
- `maestro help harness apply` -- Accept a proposal and spawn a linked task (-> accepted)
- `maestro help harness unapply` -- Undo an accepted proposal before its linked task is claimed
- `maestro help harness dismiss` -- Dismiss a noisy proposal and suppress its fingerprint
- `maestro help harness measure` -- Re-run the detector to close or revert a proposal (-> measured)
- `maestro help query matrix` -- Show the feature x task matrix (FEATURE/TASK/STATE/PROOF/TITLE)
- `maestro help query friction` -- Summarize recorded run friction (events, prompts, corrections)
- `maestro help query decisions` -- List decision cards (ID/STATUS/HOME/TITLE)
- `maestro help query backlog` -- List improvement backlog items (ID/TITLE)
- `maestro help query proof` -- Show a task's proof status
- `maestro help query graph` -- Walk a card's typed edges (parent/blocks/related/supersedes)
- `maestro help index rebuild` -- Rebuild the text index over live + archived cards from scratch
- `maestro help mcp serve` -- Run the MCP server over stdio
- `maestro help mcp stdin` -- Run the MCP server over stdio (same as serve)
- `maestro help mcp tools` -- List the MCP tool names maestro exposes
- `maestro help mcp list` -- List the MCP tool names maestro exposes (same as tools)
- `maestro help hook record`
- `maestro help watch snapshot`
- `maestro help verify` -- Verify a task against its recorded proof
- `maestro help version` -- Print the maestro version and binary path
- `maestro help help` -- Print this message or the help of the given subcommand(s)
