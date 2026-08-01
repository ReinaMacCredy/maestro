<!-- maestro:cli-reference-version: 1.1.0 -->
<!-- maestro:cli-reference-sha256: bce28abe35d41fea1df5972fc14e2019223af5ddfada7fe60a9dc6de83afabe7 -->
<!-- generated; do not edit by hand; regenerate: cargo test --test cli_reference_freshness regenerate_cli_md -- --ignored -->
# maestro CLI reference

Authoritative signatures generated from the binary's clap model,
filtered for the `maestro-setup` skill. Every listed verb and flag is exact;
a spelling not found here is outside this skill's CLI surface.
`<X>` required, `[X]` optional, `...` repeatable.

## maestro init

- `maestro init [--dry-run] [--merge] [--force] [--yes]` -- Scaffold .maestro/ and extract bundled resources into this repo

## maestro install

- `maestro install [AGENT] [--agent <AGENT>] [--dry-run]` -- Install maestro hooks and config for an agent (claude, codex, droid)

## maestro upgrade

- `maestro upgrade [--check] [--verbose] [--force]` -- Upgrade the maestro binary and refresh bundled resources

## maestro sync

- `maestro sync [--dry-run] [--global-skills] [--adopt-unmanaged]` -- Resync bundled resources to this binary's versions (offline)

## maestro uninstall

- `maestro uninstall [AGENT] [--agent <AGENT>]` -- Remove maestro hooks and config for an agent

## maestro doctor

- `maestro doctor` -- Diagnose the maestro installation and report problems

## maestro shell-init

- `maestro shell-init` -- Print the shell init snippet for maestro

## maestro status

- `maestro status [--json]` -- Show the repo's current agent handoff and next action

## maestro active

- `maestro active [--all] [--connect] [--card <CARD_ID>]` -- Show what other live sessions are doing (cross-session awareness)

## maestro loop

- `maestro loop list` -- List shipped and project custom recipes
- `maestro loop next [--json] [--chain] [--compact] [--full] [--phase <PHASE>]` -- Retired successor route; returns a typed refusal
- `maestro loop improve [--json]` -- Plan loop improvement proposals without mutating state
- `maestro loop show <NAME> [--compact] [--full] [--phase <PHASE>] [--json]` -- Print one shipped or project custom recipe
- `maestro loop validate <NAME>` -- Validate one structured shipped or project custom loop recipe
- `maestro loop outcome --recipe <RECIPE> --phase <PHASE> --selected-unit <SELECTED_UNIT> [--constraint <CONSTRAINTS>]... [--proof-result <PROOF_RESULT>] [--failure-class <FAILURE_CLASS>] [--blocker-class <BLOCKER_CLASS>] [--transition-to <TRANSITION_TO>] [--transition-reason <TRANSITION_REASON>] [--trigger <TRIGGER>] [--return-condition <RETURN_CONDITION>]... [--evidence-ref <EVIDENCE_REF>]... [--retry-count <RETRY_COUNT>] [--duration-ms <DURATION_MS>] [--learning-candidate <LEARNING_CANDIDATE>] [--source-ref <SOURCE_REF>]... [--run <RUN>] [--json]` -- Record a write-side loop outcome event
