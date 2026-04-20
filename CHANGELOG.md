# Changelog

## 0.44.3 - Native handoff launcher

- Replace the old UKI queue-based handoff flow with `maestro handoff <task>`,
  which builds a self-contained markdown brief and launches a fresh Codex or
  Claude run, optionally in a sibling worktree.
- Persist launch artifacts under `.maestro/launches/` (`prompt.md`,
  `launch.json`, `output.log`) and remove the old `handoff create`, `pickup`,
  and `list` subcommands plus pending-handoff Mission Control surfaces.
- Drop handoff replay injection from worker prompt generation and switch
  bundle export to snapshot launch records instead of UKI handoff records.
- Update built-in mission-planning guidance, bootstrap templates, README,
  AGENTS, and TUI docs to teach the new launcher-based workflow.
- Keep legacy `.maestro/handoffs/` artifacts ignored but unused, with `status`
  and `doctor` warning when they are still present.

## 0.40.0 - Install hardening and task workflow improvements

- Windows install flow: harden running-exe replacement, tolerate EBUSY/EPERM
  on prior `.old` locks, and keep local verification paths honest when the
  install directory is overridden.
- `maestro update` honors the configured install path instead of defaulting
  to the system location.
- Task workflow: auto-claim on `task update --status in_progress`, surface
  blocker errors before ownership checks, extract stale-session recovery
  into a reusable helper.
- CI: stabilize the Windows matrix (bundle/tar/rename/build paths, skill
  path encoding, boundary check root, last-mile test failures).
- Docs: sync README features, commands, storage model, and TUI screen set
  with current state.

## 0.39.0 - Windows CLI support

- Publish a Windows x64 release asset (`maestro-windows-x64.exe`) and
  extend the platform resolver to find it.
- Add a Windows install flow that handles replacing a running executable
  via a `.old`-rename-and-swap dance, including rollback on verification
  failure.
- Add a PowerShell installer (`scripts/install.ps1`) mirroring the POSIX
  `install.sh` flow and surfacing user-PATH guidance.
- Drop `sh -c` usage from cross-platform code paths and fix POSIX-leaky
  tests so the CI matrix runs green on `windows-latest`.

## 0.38.0 - Rename Feature.workerType to Feature.agentType (BREAKING)

- Rename `Feature.workerType` to `Feature.agentType` across domain types,
  Zod schemas, TUI DTOs, CLI JSON output, and generated prompt headers.
  The field names a brand classifier (codex-cli, claude-code, subagent,
  human, maestro:worker-base etc.) which is the `agent` concept in the
  conductor model, not the `worker` (role) concept.
- Rename exported `WORKER_TYPE_PATTERN` constant to `AGENT_TYPE_PATTERN`.
- CLI stdout now prints `Agent type: <value>` (was `Worker type:`).
- Generated prompt header is `**Agent Type:** <value>` (was
  `**Worker Type:**`).
- Breaking surfaces: mission plan YAML/JSON input contract, persisted
  `.maestro/missions/<id>/features/*.json` on-disk format, `feature
  prompt --json` output shape.
- Migration: run `bun scripts/migrate-feature-agent-type.ts` once to
  rewrite existing local feature JSON; the script is idempotent.
- What survives unchanged (intentional, role-level): `maestro:worker-base`
  skill name, `WorkerReport`, `WorkerReply`, `generateWorkerPrompt`,
  `.maestro/missions/<id>/workers/` directory, `# Worker Assignment:`
  prompt H1 header, `Worker skill '...' not found` error message, and
  all general worker-as-role prose in shipped skills.

## 0.37.5 - Strip pre-conductor worker-dispatch config surface

- Remove the dead worker-dispatch config surface left behind by the
  2026-04-08 conductor refactor: `execution.defaultWorker`,
  `workers: { transport, command, args, env }`, `WorkerConfig`,
  `CliWorkerConfig`, `recommendWorkerFit`, `getWorkerGuidance`,
  `formatWorkerLabel`, the Mission Control `workers` config tab, and
  the default-worker picker modal.
- Remove residual `cassAvailable` fields from `StatusReport` and
  `MissionControlConfigSummary`; `status --json` no longer emits the
  field.
- Remove dead `HIDDEN_CONFIG_KEY_PATTERNS` regex entries for config
  keys that no longer exist (`supervision.*`, `rotateWorkerOnRetry`,
  `workers.*.outputMode`).
- Net delete of ~693 lines; maestro stays a conductor that never spawns
  workers.

## 0.37.4 - Inline GitHub release notes

- Publish GitHub releases with readable inline notes instead of only the
  default compare link.
- Prefer the matching `CHANGELOG.md` section for release notes when it
  exists, and fall back to inline commit bullets when it does not.

## 0.36.0 - Mission bundles (phase 1: export + inspect)

- Add `maestro bundle export <missionId>` that packages a mission and
  its artifacts (plan, features, workers, replies, handoffs, principles,
  memory) as a portable `.mission.tar.gz` archive with a v1 manifest.
- Add `maestro bundle inspect <path>` that prints the manifest of a
  bundle without extracting it.
- Support `--out`, `--base <ref>` (include `diff.patch` from
  `<ref>..HEAD`), `--redact <memory|prompts|replies>`, and `--json` on
  the export subcommand.
- Manifest `schemaVersion: 1` captures bundle id, created-at, maestro
  version, mission summary, per-section stats, redaction flags, and an
  optional git patch descriptor so future readers can reject unknown
  versions cleanly.

## 0.35.4 — Typecheck and release metadata alignment

- Fix the repo's standing `bun run typecheck` failures across source and test
  files.
- Align the changelog back to the `0.x.y` version scheme used by
  `package.json` and `src/shared/version.ts`.
- Fix `scripts/bump.ts` so it reports the pre-bump and post-bump versions
  correctly.

## 0.35.3 — Built-in skill handoff link docs

- Add active handoff links between built-in skills.

## 0.35.2 — Mission Control cleanup

Phase 3 of the conductor refactor. The Mission Control TUI was in an
intermediate state after Phase 1 removed the worker execution layer:
the snapshot pipeline still populated empty worker / runtime /
progress-log panes and the preview screen set still advertised
`runtime`, `workers`, and `output` screens whose data stores had been
deleted. This release cleans up the dead screens, the orphaned DTOs,
and the types that described them.

### Removed preview screens

- `runtime`, `workers`, `output` are no longer valid `--preview` values.
  The TUI now renders 7 screens: `dashboard`, `features`,
  `dependencies`, `handoffs`, `config`, `memory`, `graph`.
- The `proc`, `process`, `processes`, `worker`, `out` screen aliases
  were removed. Surviving aliases: `feat`, `handoff`, `cfg`, `deps`,
  `mem`.

### Removed modal kinds

- `"processes"`, `"runtime-output"`, `"workers"` modal kinds were
  deleted from the Mission Control reducer along with their
  command-palette entries, reducer cases, modal builders, and
  input-dispatch hotkeys. The surviving modal kinds are `none`,
  `command-palette`, `feature-action`, `feature-browser`, `overview`,
  `dependencies`, `handoffs`, `config`, `memory`, `graph`.

### Orphaned types removed

From `src/domain/worker-types.ts`:

- `TransportType` union (collapsed to the literal `"cli"` on
  `CliWorkerConfig`).
- `A2aWorkerConfig`, the old `WorkerConfig` union variant.
- `WorkerResult`, `ExecutionRecord`, `RuntimeEventRecord`.
- `WorkerProgressEvent`, `WorkerProgressEventKind`, `FailureClass`.

Kept because the config inspector and `DEFAULT_CONFIG` still render
them: `CliWorkerConfig`, `WorkerConfig` (now an alias for
`CliWorkerConfig`), `WorkerOutputMode`, `ExecutionConfig`,
`SupervisionConfig`, `SupervisionLevel`, `ParallelConfig`.

From `src/tui/state/types.ts`:

- `MissionControlWorkerPane`, `MissionControlRuntimeProcessRow`.
- `MissionControlWorkerHealthRow`, `MissionControlWorkerHealthStatus`,
  `MissionControlWorkerHealthCheck`.
- `activeWorker`, `runtimeProcesses`, `workerHealth` fields on
  `MissionControlSnapshot`.
- `transport` field on `MissionControlSessionSidebar`.
- `runtimeState`, `lastSeenAgeMs`, `failureReason`, `retryCount`,
  `agent`, `sessionId` fields on `TaskPreviewPane`.

### Other removals

- `src/usecases/worker-selection.usecase.ts` and its test. No callers
  survived the Phase 1 strip.
- `validateSupervisionConfig`, `validateParallelConfig`, and
  `isCliWorkerConfig` exports in `src/domain/worker-validators.ts`.
- `workerHealth` input to `buildConfigInspector`. The inspector now
  derives worker availability directly from the CLI worker config via
  `cachedWhich`.
- `workerEvents` input to `deriveEvents`. The timeline reads only
  mission, feature, assertion, and checkpoint timestamps.
- The agent-per-feature aggregation in `buildMissionOverview`
  (`agentSummary` always returns an empty array).
- Live feature auto-follow in `createInitialState` and the reducer's
  `update-snapshot` handler (runtime state is gone, so there is no
  live feature to follow).
- Session sidebar transport/session/agent rendering in the OpenTUI
  builders.
- Faster polling cadence in `getSnapshotPollIntervalMs` (now always
  returns the 5s default because there is no live runtime to track).

### Behavior changes

- `maestro mission-control --preview output --feature <id>` now fails
  with "Unknown preview screen" instead of rendering a runtime output
  stream. The worker output pane was already empty after Phase 1.
- `maestro mission-control --preview runtime` and `--preview workers`
  fail the same way.
- The `O` hotkey inside the runtime modal no longer does anything
  (neither modal exists).

## 0.35.1 — UKI v5.2 handoff system

Phase 2 of the conductor refactor. The single `maestro handoff` subcommand
is back, producing deterministic, machine-readable UKI v5.2 records that
external workers (Claude Code children, Codex, Gemini CLI) can consume via
a single compressed string.

### Added

- `src/lib/uki-format.ts` -- deterministic compressor + parser for the
  UKI v5.2 12-slot single-string format. Pure (no clock, no random, no
  I/O). Validator returns violation list without throwing.
- `src/domain/uki-types.ts` -- `UkiHandoff`, `UkiSlots`, `CreateUkiHandoffInput`,
  `UkiHandoffStatus` domain types.
- `src/ports/handoff-store.port.ts` -- new v2 `HandoffStorePort` (different
  shape than the pre-Phase-1 port; flat-file JSON records under
  `.maestro/handoffs/<id>.json`).
- `src/adapters/handoff-store.adapter.ts` -- filesystem adapter that caches
  the compressed UKI string on each record at create time.
- `src/usecases/{create,pickup,list}-uki-handoff.usecase.ts`.
- `maestro handoff create` with structured slot flags, auto-filling agent
  and session id from `SessionDetectPort`.
- `maestro handoff pickup` with `--json` (default), `--markdown` (human
  briefing), and `--uki` (raw compressed string for piping) output modes.
  `--claim` transitions `pending -> picked-up` atomically.
- `maestro handoff list` with `--status pending|picked-up|completed`.
- Mission Control home pane now shows pending UKI handoffs sourced from
  the new store (`buildSnapshot` and `buildHomeSnapshot` both populate
  `pendingHandoffs` by listing pending records and projecting each via
  `mapUkiHandoffToHomeHandoff`).

### Notes

- The old handoff format (pre-0.35.0) was deleted in Phase 1. Existing
  `.maestro/handoffs/` contents from before the strip become orphaned
  when upgrading -- the new format is a flat-file JSON shape (not the
  previous directory-per-handoff layout) and the compressed string is
  entirely different. There is no migration path by design (documented
  in the plan at `~/.claude/plans/drifting-humming-dream.md`).
- The UKI v5.2 string must contain exactly 11 pipes, zero colons, zero
  newlines, and every `_`-joined token half is capped at 4 words (R2).
- `CS` (confidence) is scoped: `CS-work_X`, `CS-summary_Y`, or
  `CS-work_X~summary_Y`. Bare `CS-N.NN` is rejected (R5).
- `ARTIFACTS` must contain at least one of `commit_`, `branch_`,
  `version_`, or `file_` (R7).
- `STANCE_COLLAPSE` is always emitted; if the caller does not supply a
  value the compressor defaults to `NONE_DETECTED_LOW_FRICTION` (R6).

## 0.35.0 — Phase 1 strip

This release is the conductor-model cutover. Maestro is no longer a harness
that spawns workers; it is a shared mission/memory artifact that external
workers (Claude Code, Codex, Gemini CLI, etc.) read from and write to via the
CLI. The worker-execution layer has been removed wholesale.

### Removed CLI subcommands

- `maestro feature run` (the sequential feature execution engine)
- `maestro handoff`
- `maestro handoff-pickup`
- `maestro handoff-dig`
- `maestro handoff-drop`
- `maestro handoff-cleanup`
- `maestro handoff-report`
- `maestro a2a` (agent-to-agent debug command)

Phase 2 will re-introduce a single `maestro handoff` command that produces
UKI v5.2 format records; this 0.35.0 release intentionally has no handoff
surface at all.

### Removed ports and adapters

- `TransportPort` (`cli-transport`, `a2a-transport`, `multi-transport`)
- `RuntimeStorePort` / `RuntimeEventStorePort` (worker runtime + event stores)
- `ExecutionStorePort` (historic execution records)
- `HandoffStorePort` and `HandoffEnvelope` / `HandoffSession` / `HandoffPlan`
  / `Handoff` domain types (Phase 2 will re-introduce a `HandoffStorePort`
  with a completely different shape keyed on UKI records)
- `CassPort` and all CASS knowledge-store integration
- Runtime supervision stack: `runtime-supervision.usecase`,
  `runtime-recovery.usecase`, `live-runtime-tracking.usecase`
- Worker dispatch: `run-features.usecase`
- Handoff use-cases: `create-handoff`, `pickup-handoff`, `dig-handoff`,
  `report-handoff`, plus the orphaned `generate-prompt` usecase that only
  wrapped the deleted `handoff-pickup --claim` workflow

### Behavior changes

- `feature-lifecycle.updateFeature` no longer writes to a runtime store.
  Feature status updates now touch the feature store only; the runtime
  lease / last-seen / failure-reason fields are gone because there is no
  runtime to supervise.
- `session-detect` simplified: the cwd-fallback, session-id prefix resolve,
  and staleness warning flows are gone. The adapter only reads
  `CLAUDECODE` / `CODEX_THREAD_ID` env vars. Explicit `--session <id>`
  arguments are required wherever a session must be identified outside of
  those two environments.
- `generate-worker-prompt.usecase` no longer takes a `runtimeStore`
  parameter. The memory-injection path (`safeRecallMemory` ->
  `appendMemorySection`) is unchanged and continues to auto-wire into
  `maestro feature prompt <id>`.
- Mission Control worker / runtime / output panes are empty until Phase 3
  removes them outright. The dashboard, features, dependencies, config,
  memory, handoffs, and graph screens remain fully functional.
- Top-level CLI description changed from "Cross-agent handoff CLI" to
  "Conductor CLI".

### Removed devDependencies

- `@a2a-js/sdk`
- `express` and `@types/express`

These shipped only to support the now-deleted A2A transport.

### Upgrade notes

- Existing `.maestro/handoffs/` records are orphaned by this release. There
  is no migration path; they will be re-formatted by Phase 2 when the UKI
  handoff store ships.
- Any scripts that shell out to `maestro feature run`, `maestro handoff *`,
  or `maestro a2a *` must be updated. Use `maestro feature prompt <id>` to
  generate a worker prompt, then run the actual worker in a separate
  terminal (Claude Code, Codex, etc.).
