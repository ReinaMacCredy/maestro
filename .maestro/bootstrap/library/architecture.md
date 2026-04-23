# Architecture

High-level architecture for the Mission Control reliability and cross-CLI orchestration mission.

**What belongs here:** Component relationships, runtime-state boundaries, recovery data flow, and invariants agents need before editing code.

---

## System Overview

Mission Control currently behaves mostly as a lifecycle/state persistence layer. This mission extends it into a more reliable orchestration substrate by adding explicit runtime supervision, cross-CLI session/hook parity, automatic recovery, operator controls, and checkpoint-aware recovery semantics.

The product remains a CLI/TUI application. It still does not become a full autonomous agent runner, but it gains enough runtime modeling to represent active, stale, failed, recoverable, and resumed work coherently.

## Architectural Direction

### 1. Runtime state becomes first-class

Feature status is not enough to represent real execution. The mission should introduce a dedicated runtime-state record for feature execution concerns such as:
- ownership / agent identity
- host or session attribution
- last-seen / lease / freshness timestamps
- runtime condition (`live`, `stale`, `failed`, `recoverable`, etc.)
- recovery/audit metadata references

This runtime state must be persisted separately from feature status and consumed by Mission Control views, recovery logic, and checkpointing.

### 2. Cross-CLI support is adapter-driven

Supported host CLIs (Claude, Codex, Droid, and any others explicitly in scope) should be represented through adapters and normalized metadata rather than host-specific assumptions leaking into usecases.

Key rule:
- host-specific detection/hook capture happens at the boundary
- normalized session/event/runtime metadata flows through usecases and snapshots

### 3. Recovery is explicit and auditable

Automatic recovery must not silently mutate state. Every recovery action should leave a paper trail across:
- runtime-state metadata
- retry history
- preserved agent report state
- event/audit records

Recovery should requeue work safely without producing duplicate active ownership records.

### 4. Operator surfaces read the same truth

`mission-control --json`, one-frame Mission Control, and the interactive TUI must all derive runtime/recovery state from the same persisted runtime model.

If the JSON view says a feature is stale/recoverable, the interactive Mission Control surface and compiled binary must communicate the same state.

### 5. Checkpoints snapshot recovery context, not just lifecycle status

Checkpoint save/load currently centers on feature/assertion states. This mission extends checkpoint semantics so the system can resume or recover interrupted work coherently. Runtime recovery metadata must be captured/restored with safe rules, especially around expired ownership.

## Main Components

### Domain layer

Expected areas of change:
- runtime-state types and validators
- recovery-related lifecycle helpers and invariants
- cross-CLI normalized session/event metadata types
- checkpoint data structures extended for runtime recovery context

### Usecase layer

Expected responsibilities:
- session detection and host-context normalization
- runtime-state persistence and freshness evaluation
- automatic recovery orchestration
- snapshot derivation for Mission Control JSON/TUI
- checkpoint save/load semantics for runtime recovery data

### Adapter layer

Expected responsibilities:
- filesystem persistence for runtime-state and checkpoint extensions
- host/session detection adapters
- hook/event ingestion adapters or normalization helpers
- service wiring in `src/services.ts`

### CLI/TUI layer

Expected responsibilities:
- operator-visible recovery and runtime context
- explicit recovery/retry/resume command paths where applicable
- compiled-binary parity with source-run behavior

## Storage Boundaries

### Product runtime state

Mission Control product runtime continues to live under:

```text
.maestro/missions/{missionId}/
```

This mission may extend that layout with additional runtime-oriented files/directories, but must keep runtime state inside the same mission-scoped namespace.

### Repo mission infrastructure

`.factory/` in this repository is agent/validator infrastructure and mission knowledge. It is not product runtime state.

## Proposed Data Shape Boundaries

At a high level, agents should expect these concerns to remain separate:

- **feature state**: lifecycle of planned work (`pending`, `assigned`, `in-progress`, etc.)
- **runtime state**: who owns active work, freshness, failure, recovery, audit links
- **assertion state**: validation outcomes
- **checkpoint state**: a snapshot of feature/assertion/runtime context for resume/recovery
- **event history**: append-oriented host/tool/runtime signals that can support audit and recovery

## Critical Invariants

1. A feature may have at most one live runtime ownership record at a time.
2. Recovery must preserve previous evidence (`report.json`, retry history, event history) instead of overwriting it.
3. Missing host hook/session context must degrade gracefully and never crash normal CLI flows.
4. Cross-CLI support must not regress existing Claude/Codex behavior.
5. Mission Control surfaces must derive runtime truth from explicit runtime state, not only feature timestamps.
6. Checkpoint restore must never revive expired ownership as currently live work.
7. Operator recovery workflows must be available through supported surfaces rather than requiring manual file edits.

## Verification Focus

Agents should verify changes through:
- temp git repo CLI flows
- persisted runtime/checkpoint file inspection
- `mission-control --json`
- one-frame / PTY Mission Control checks
- compiled `./dist/maestro` parity for user-facing behavior
