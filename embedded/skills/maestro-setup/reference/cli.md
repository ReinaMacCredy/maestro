<!-- maestro:cli-reference-version: 1.1.0 -->
<!-- maestro:cli-reference-sha256: 11115eb909da70322d8f274e4f500d1ab17ab15f96b73c1fa0187f11389cf15f -->
<!-- generated; do not edit by hand; regenerate: cargo test --test cli_reference_freshness regenerate_cli_md -- --ignored -->
# maestro CLI reference

Authoritative signatures generated from the binary's clap model,
filtered for the `maestro-setup` skill. Every listed verb and flag is exact;
a spelling not found here is outside this skill's CLI surface.
`<X>` required, `[X]` optional, `...` repeatable.

## maestro init

- `maestro init [--dry-run] [--merge] [--force] [--yes]` -- Scaffold .maestro/ and extract bundled resources into this repo

## maestro install

- `maestro install [AGENT] [--agent <AGENT>]` -- Install maestro hooks and config for an agent (claude, codex, droid)

## maestro upgrade

- `maestro upgrade [--check] [--verbose] [--force]` -- Upgrade the maestro binary and refresh bundled resources

## maestro sync

- `maestro sync [--dry-run] [--global-skills]` -- Resync bundled resources to this binary's versions (offline)

## maestro uninstall

- `maestro uninstall [AGENT] [--agent <AGENT>]` -- Remove maestro hooks and config for an agent

## maestro doctor

- `maestro doctor` -- Diagnose the maestro installation and report problems

## maestro shell-init

- `maestro shell-init` -- Print the shell init snippet for maestro

## maestro status

- `maestro status [--json]` -- Show the repo's current agent handoff and next action

## maestro active

- `maestro active [--all] [--connect]` -- Show what other live sessions are doing (cross-session awareness)

## maestro loop

- `maestro loop list` -- List shipped and project custom recipes
- `maestro loop next [--json]` -- Recommend the next loop recipe without mutating state
- `maestro loop show <NAME>` -- Print one shipped or project custom recipe
