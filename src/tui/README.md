# Mission Control TUI Architecture

This directory contains the read-only Mission Control interface for Maestro. The TUI is not a separate product or runtime. It is a projection layer over the same filesystem-backed services that power the CLI.

## What lives where

The Mission Control flow crosses a few directories on purpose:

- `src/index.ts` registers the `mission-control` command with Commander.
- `src/infra/commands/mission-control.command.ts` is the CLI entry point for interactive mode, preview mode, JSON output, and render checks.
- `src/services.ts` provides the shared service graph used by both the CLI and the TUI snapshot loader.
- `src/tui/state/snapshot.ts` builds the read model that the UI consumes.
- `src/tui/state/reducer.ts` owns interactive UI state such as focus, modal selection, and copy mode.
- `src/tui/app/preview-state.ts` maps preview flags like `--preview features` into a deterministic reducer state.
- `src/tui/opentui/app/interactive.tsx` runs the interactive OpenTUI loop.
- `src/tui/opentui/app/preview.ts` renders a single non-interactive frame.
- `src/tui/opentui/components/mission-control-screen.tsx` turns state plus snapshot data into panels and overlays.

If you are new to this code, start with the command handler, then the snapshot builder, then the reducer, then the screen component.

## Mental model

Mission Control is built in four layers:

1. Command layer: parse flags, choose mission or home mode, and select interactive vs preview vs JSON output.
2. Snapshot layer: collect data from feature stores and infra adapters into a `MissionControlSnapshot`.
3. UI state layer: keep track of selection, modal state, and keyboard or mouse navigation without mutating domain state directly.
4. Render layer: convert snapshot plus UI state into terminal frames.

The important boundary is that the snapshot is the source of truth for display data, while the reducer is the source of truth for temporary UI behavior.

## End-to-end flow

### 1. Command registration

`src/index.ts` calls `registerMissionControlCommand`, so every Mission Control entry point starts from the normal CLI boot path after `initServices(process.cwd())`.

### 2. Command handling

`src/infra/commands/mission-control.command.ts` does the orchestration work:

- resolves `--json`, `--preview`, `--render-check`, `--feature`, `--size`, and `--format`
- builds snapshot dependencies from the shared services object
- creates a snapshot loader with cached config and git ports
- keeps read-only outputs redacted through `redactSnapshotForReadOutput`
- dispatches to `renderDashboard`, `renderPreviewFrame`, or `runRenderCheck`

This file is also where mission selection falls back from explicit `--mission` to an executing or paused mission, then to the newest mission, and finally to home mode when no mission is resolved.

### 3. Snapshot building

`src/tui/state/snapshot.ts` is the core read-model builder. It gathers mission state, features, assertions, checkpoints, config, git state, memory data, launch-linked principle outcomes, tasks, replies, and principle effectiveness into a single snapshot object.

Two rules matter here:

- `buildSnapshot()` and `buildHomeSnapshot()` are read-model builders first. They should not become a place for recovery logic or workflow mutations.
- Reply ingest is the one sanctioned side effect, and it is gated by `SnapshotBuildOptions.includeReplies`.

That split keeps `maestro mission-control --json` and `maestro mission-control --preview` safe as inspection paths while still allowing the interactive principles workflow to opt into reply processing when needed.

### 4. Preview and interactive state

Preview and interactive mode share the same state machine:

- `src/tui/state/reducer.ts` defines `AppState`, modal variants, actions, and the pure `reduce()` function.
- `src/tui/app/preview-state.ts` seeds the reducer for preview screens such as `dashboard`, `features`, `tasks`, and `principles`.
- `src/tui/opentui/app/interactive.tsx` drives keyboard and mouse input through the reducer, reloads snapshots, and performs the limited write actions that interactive Mission Control supports.

That separation is intentional:

- preview mode is deterministic because it starts from a snapshot and a synthetic reducer state
- interactive mode is live because it polls snapshots and feeds real user input into the same reducer

## Read-only contract

Mission Control is designed as a read-only dashboard with narrow exceptions.

Safe inspection paths:

- `maestro mission-control --json`
- `maestro mission-control --preview`
- `maestro mission-control --preview all`
- `maestro mission-control --render-check`

Interactive mode can still trigger specific workflows, but those writes happen in the interactive layer, not in the snapshot projection layer. Current examples include:

- updating a feature status with `updateFeature`
- generating a worker prompt from the dispatch modal
- previewing and applying config edits

If you add a new interactive action, keep the write in `interactive.tsx` or another explicit use-case boundary. Do not hide it inside snapshot construction or preview rendering.

## Home mode vs mission mode

Mission Control renders two related dashboards:

- mission mode: a specific mission snapshot with features, milestones, dependencies, dispatch queue, timeline, and other mission-scoped panels
- home mode: a repo-wide overview when there is no selected mission

`src/infra/commands/mission-control.command.ts` chooses between those modes through `loadMissionControlSnapshot()`, which calls either `buildSnapshot()` or `buildHomeSnapshot()`.

`src/tui/app/preview-state.ts` also enforces which preview screens are legal in each mode. For example, `dependencies`, `dispatch`, and `timeline` require mission mode.

## Rendering pipeline

Rendering is intentionally thin:

- `src/tui/opentui/app/mission-control-app.tsx` is a small adapter that injects terminal dimensions and passes props to the main screen component.
- `src/tui/opentui/components/mission-control-screen.tsx` composes the page layout, panels, footer, and modal overlay.
- `src/tui/opentui/components/builders.ts` provides most of the view-model shaping for individual panels and overlay content.
- `src/tui/opentui/testing/frame-capture.tsx` is used by preview and render-check flows to capture deterministic frames.

The screen component should stay focused on layout and presentation. If you find domain or store logic creeping into it, that logic probably belongs back in `snapshot.ts`, the reducer, or a builder.

## Editing guide

When making Mission Control changes, use this checklist:

1. If the change affects displayed data, start in `src/tui/state/types.ts` and `src/tui/state/snapshot.ts`.
2. If the change affects keyboard or modal behavior, start in `src/tui/state/reducer.ts`.
3. If the change affects preview-only selection or screen routing, update `src/tui/app/preview-state.ts`.
4. If the change affects rendering, update the OpenTUI components and builders.
5. If the change adds or changes a CLI flag, update `src/infra/commands/mission-control.command.ts`.

## Contributor guardrails

- Keep cross-feature imports on the public surface, for example `@/features/mission`, not deep paths into another feature.
- Keep snapshot builders side-effect free unless the behavior is explicitly sanctioned and gated.
- Keep preview and JSON modes read-only.
- Prefer adding derived display fields in the snapshot layer instead of recalculating them in React components.
- Use the reducer for UI behavior, not ad hoc mutable state in components.

## Verification commands

The normal Mission Control verification commands are:

```bash
bun run build
./dist/maestro mission-control --preview --size 120x40 --format plain
./dist/maestro mission-control --render-check --size 120x40
```

This documentation change was requested with test execution skipped, so the commands above are the expected checks for future code changes rather than steps run in this edit.
