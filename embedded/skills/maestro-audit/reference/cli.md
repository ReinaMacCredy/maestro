<!-- maestro:cli-reference-version: 1.1.0 -->
<!-- maestro:cli-reference-sha256: 4efa6130d8107f1dcafd8c1b093029b78ed957bf94ce88dcb18b03b0e394adab -->
<!-- generated; do not edit by hand; regenerate: cargo test --test cli_reference_freshness regenerate_cli_md -- --ignored -->
# maestro CLI reference

Authoritative signatures generated from the binary's clap model,
filtered for the `maestro-audit` skill. Every listed verb and flag is exact;
a spelling not found here is outside this skill's CLI surface.
`<X>` required, `[X]` optional, `...` repeatable.

## maestro status

- `maestro status [--json]` -- Show the repo's current agent handoff and next action

## maestro task

- `maestro task show [ID]` -- Show a task's detail: state, claim, blockers
- `maestro task list [--blocked] [--blocked-by <BLOCKED_BY>] [--blocks <BLOCKS>] [--feature <FEATURE>] [--ready] [--mine] [--all] [--interval <INTERVAL>]` -- List tasks, with optional filters

## maestro feature

- `maestro feature show <ID>` -- Show a feature's status, full contract, and task counts
- `maestro feature list [--all]` -- List features with their statuses and task counts

## maestro decision

- `maestro decision show <ID>` -- Show a decision card by id
- `maestro decision list [--all] [--feature <FEATURE>]` -- List decision cards (recent 20 by activity unless --all)

## maestro card

- `maestro card list [--parent <PARENT>] [--type <TYPE>] [--assignee <ASSIGNEE>] [--status <STATUS>] [--project <PROJECT>] [--grep <TERM>] [--archived] [--all] [--json]` -- List cards filtered by parent, type, assignee, or coarse status
- `maestro card show <ID> [--json] [--compact-json]` -- Show a card's header, edges, and body

## maestro active

- `maestro active [--all] [--connect]` -- Show what other live sessions are doing (cross-session awareness)

## maestro harness

- `maestro harness list [--all]` -- List proposals (proposed + accepted; --all adds the terminal ledger)
- `maestro harness show <ID>` -- Show a proposal's detail and history
- `maestro harness propose --title <TITLE> --evidence <EVIDENCE>... [--topic <TOPIC>]` -- File an agent-authored repo audit proposal
- `maestro harness apply <ID> [--check <CHECK>]...` -- Accept a proposal and spawn a linked task (-> accepted)

## maestro query

- `maestro query matrix` -- Show the feature x task matrix (FEATURE/TASK/STATE/PROOF/TITLE)
- `maestro query friction` -- Summarize recorded run friction (events, prompts, corrections)
- `maestro query backlog` -- List improvement backlog items (ID/TITLE)

## maestro lean

- `maestro lean [TARGET] [--card]` -- Lean reach-ladder tooling: show/set the session strictness mode, emit review/audit guidance, or harvest debt markers
