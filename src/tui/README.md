# Mission Control TUI Architecture

This directory restores the old TypeScript/OpenTUI Mission Control frontend.
Rust remains Maestro's backend and source of truth.

## Current Flow

```text
maestro mission-control --json
  -> Rust snapshot: cards, tasks, runs, proof, repo state

maestro mission-control --renderer opentui --preview tasks
  -> Rust writes a temp snapshot file
  -> Bun runs src/tui/sidecar.ts
  -> current-snapshot.ts adapts Rust JSON to the old MissionControlSnapshot
  -> old OpenTUI reducer/components render the frame
```

`maestro mission-control --json` never calls TypeScript. The TypeScript sidecar
only renders data that Rust has already projected.

## What Lives Where

- `src/interfaces/cli/mission_control.rs`: Rust command adapter. It builds the
  Rust snapshot, writes a temp JSON file, and launches the sidecar for
  `--renderer opentui`.
- `src/interfaces/tui/mission_control.rs`: Rust snapshot/read-only renderer.
  This remains the canonical data projection and the fallback renderer.
- `src/tui/sidecar.ts`: Bun entry point for preview, render-check, and
  interactive OpenTUI modes.
- `src/tui/current-snapshot.ts`: adapter from
  `maestro.mission_control.snapshot.v1` to the old TypeScript
  `MissionControlSnapshot` view model. It sets the default TUI background mode
  to `transparent`.
- `src/tui/state/reducer.ts`: restored old UI state machine.
- `src/tui/opentui/**`: restored old OpenTUI render loop, preview capture, and
  components.
- `src/shared`, `src/infra`, `src/features`, `src/repo`, and `src/service`:
  minimal TypeScript compatibility surfaces for old UI imports. These are not
  the Maestro backend.

## Read-Only Boundary

Safe inspection paths:

- `maestro mission-control --json`
- `maestro mission-control --preview --renderer rust`
- `maestro mission-control --preview tasks --renderer opentui`
- `maestro mission-control --render-check --renderer opentui`

The restored interactive code still contains old action paths, but the
compatibility write functions throw read-only errors. Do not hide durable writes
inside `current-snapshot.ts`, preview rendering, or config/proof adapters.

## Theme Settings

Mission Control currently exposes one background theme setting:

- `transparent`: default. The terminal background shows through normal dashboard
  chrome.
- `current`: uses the current solid Mission Control panel colors.

Legacy config values still map cleanly: `terminal` becomes `transparent`, and
`solid` becomes `current`.

## Editing Guide

1. Displayed data from current Maestro artifacts starts in
   `src/tui/current-snapshot.ts`.
2. Keyboard/modal behavior starts in `src/tui/state/reducer.ts` and
   `src/tui/app/input-dispatch.ts`.
3. Preview screen routing starts in `src/tui/app/preview-state.ts`.
4. OpenTUI layout and panel content starts in
   `src/tui/opentui/components/mission-control-screen.tsx` and
   `src/tui/opentui/components/builders.ts`.
5. CLI flags start in `src/interfaces/cli/mod.rs` and dispatch through
   `src/interfaces/cli/mission_control.rs`.

## Verification

Install TypeScript dependencies first:

```bash
bun install
```

Then run:

```bash
bun run tui:check
cargo run --quiet -- mission-control --renderer opentui --preview tasks --size 120x40 --format plain
cargo run --quiet -- mission-control --renderer opentui --render-check --size 120x40
cargo test --test mission_control_integration --no-fail-fast
```
