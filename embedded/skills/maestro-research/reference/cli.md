<!-- maestro:cli-reference-version: 1.1.0 -->
<!-- maestro:cli-reference-sha256: 5ff292344c57f978f60807eef9417a811a2fb4e0188b4e01139d06daee74c3f6 -->
<!-- generated; do not edit by hand; regenerate: cargo test --test cli_reference_freshness regenerate_cli_md -- --ignored -->
# maestro CLI reference

Authoritative signatures generated from the binary's clap model,
filtered for the `maestro-research` skill. Every listed verb and flag is exact;
a spelling not found here is outside this skill's CLI surface.
`<X>` required, `[X]` optional, `...` repeatable.

## maestro intake

- `maestro intake --from <PATH|-> [--json]` -- Classify an external plan/spec/prompt into a Maestro lifecycle route

## maestro research

- `maestro research check <CARD_ID> [--intended-project <PROJECT>] [--json]` -- Read-only validation for a card's research.md receipt

## maestro status

- `maestro status [--json]` -- Show the repo's current agent handoff and next action

## maestro feature

- `maestro feature new <TITLE> [--description <DESCRIPTION>] [--question <QUESTION>]... [--project <PROJECT>] [--id-only]` -- Propose a new feature (-> proposed)
- `maestro feature note <ID> <TEXT>` -- Append a dated note to a feature's notes.md
- `maestro feature show <ID>` -- Show a feature's status, full contract, and task counts
- `maestro feature list [--all]` -- List features with their statuses and task counts

## maestro grep

- `maestro grep <QUERY>... [--json]` -- Search Maestro memory and source with the indexed grep engine

## maestro card

- `maestro card list [--parent <PARENT>] [--type <TYPE>] [--assignee <ASSIGNEE>] [--status <STATUS>] [--project <PROJECT>] [--grep <TERM>] [--archived] [--all] [--json]` -- List cards filtered by parent, type, assignee, or coarse status
- `maestro card note <ID> <TEXT>` -- Append a dated note to a card's notes.md
- `maestro card create <TITLE>... -t|--type <TYPE> [--parent <PARENT>] [--description <TEXT>] [--active-form <TEXT>] [--project <PROJECT>] [--kind <KIND>] [--id-only]` -- Create a card of any type
- `maestro card show <ID> [--json] [--compact-json]` -- Show a card's header, edges, and body

## maestro active

- `maestro active [--all] [--connect] [--card <CARD_ID>]` -- Show what other live sessions are doing (cross-session awareness)
