<!-- maestro:cli-reference-version: 1.1.0 -->
<!-- maestro:cli-reference-sha256: 8b77833bb1e93cf938de221c0895f5bd3fed6e4b34eb7aa7443264be011bb915 -->
<!-- generated; do not edit by hand; regenerate: cargo test --test cli_reference_freshness regenerate_cli_md -- --ignored -->
# maestro CLI reference

Authoritative signatures generated from the binary's clap model,
filtered for the `maestro-witness` skill. Every listed verb and flag is exact;
a spelling not found here is outside this skill's CLI surface.
`<X>` required, `[X]` optional, `...` repeatable.

## maestro status

- `maestro status [--json]` -- Show the repo's current agent handoff and next action

## maestro task

- `maestro task show [REF_OR_ID]` -- Show a task's detail: state, claim, blockers
- `maestro task list [--blocked] [--blocked-by <BLOCKED_BY>] [--blocks <BLOCKS>] [--feature <FEATURE>] [--ready] [--mine] [--all] [--json] [--interval <INTERVAL>]` -- List tasks, with optional filters
- `maestro task proof [TASK_ID] [--task-id <TASK_ID>]` -- Show a task's proof status

## maestro feature

- `maestro feature verify <ID> [--prove <AC_ID>]... [--evidence <EVIDENCE>]... [--waive <AC_ID>]... [--reason <REASON>]... [--no-close] [--outcome <OUTCOME>]` -- Sweep or record proof for a feature's acceptance contract
- `maestro feature proof add <ID> --ac <AC> --evidence <EVIDENCE> [--no-close] [--outcome <OUTCOME>]` -- Record explicit feature acceptance proof
- `maestro feature proof waive <ID> --ac <AC> --reason <REASON>` -- Waive a feature acceptance item with an explicit reason
- `maestro feature close <ID> [--outcome <OUTCOME>] [--dry-run]` -- Close an in-progress feature (-> closed; gated)
- `maestro feature show <ID>` -- Show a feature's status, full contract, and task counts
- `maestro feature design <ID> [--section <SECTION>] [--append <TEXT>] [--replace <TEXT>]` -- Render a feature's design-of-record, render one section, or fill one section (--section with --append/--replace)

## maestro qa

- `maestro qa baseline <ID> [--observed <OBSERVED>] [--observed-file <PATH>] [--observed-stdin]` -- Write a feature QA baseline from explicit observed behavior
- `maestro qa slice <ID> [--scenario <SCENARIO>]... [--observed <OBSERVED>] [--observed-file <PATH>] [--observed-stdin]` -- Append counting QA slice evidence for baseline scenarios

## maestro card

- `maestro card show <ID> [--json] [--compact-json]` -- Show a card's header, edges, and body

## maestro active

- `maestro active [--all] [--connect] [--card <CARD_ID>]` -- Show what other live sessions are doing (cross-session awareness)

## maestro harness

- `maestro harness propose --title <TITLE> --evidence <EVIDENCE>... [--topic <TOPIC>]` -- File an agent-authored repo audit proposal
