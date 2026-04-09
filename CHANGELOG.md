# Changelog

## 1.0.1 — UKI v5.2 handoff system

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

- The old handoff format (pre-1.0.0) was deleted in Phase 1. Existing
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

## 1.0.0 — Phase 1 strip

This release is the v1 cutover to the conductor model. Maestro is no longer a
harness that spawns workers; it is a shared mission/memory artifact that
external workers (Claude Code, Codex, Gemini CLI, etc.) read from and write to
via the CLI. The worker-execution layer has been removed wholesale, which is
why Phase 1 ships as a major bump.

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
UKI v5.2 format records; this 1.0.0 release intentionally has no handoff
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
