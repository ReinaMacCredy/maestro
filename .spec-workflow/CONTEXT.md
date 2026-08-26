# Maestro TS Rewrite

Vocabulary for the TypeScript rebuild of maestro: a local-first, passive CLI
orchestrator for agent work, rebuilt as a plugin-first system.

## Language

**Kernel**:
The mechanism-only core: store, event log, readiness projection, CLI dispatch,
plugin loader. Holds no opinion about how work should be done.
_Avoid_: core (ambiguous), engine

**Policy**:
An opinionated rule about work (proof matching, QA, test-first) shipped as a
removable plugin. The strong default set is the "default policy", never part of
the kernel.
_Avoid_: gate config, mode

**Recipe**:
A prompt-first markdown workflow document served by a plugin and interpreted by
the agent. Never executable by maestro itself.
_Avoid_: script, pipeline

**Work item**:
The single tracking entity: tree (parent-child) + DAG (`blocked_by`) + `kind`
as data (feature/task/bug/idea/...) + acceptance as data + notes + lease.
Replaces card/task/feature (ADR-0003).
_Avoid_: card, task, feature (as entity names)

**Lease**:
The claim a session takes on a work item via `work start`; expires when the
holding session is no longer alive. The collision-prevention primitive.
_Avoid_: lock (reserved for decisions), assign

**Decision**:
The separate record of a fork settled with the owner: draft -> locked ->
superseded(link); parent-child children keep "lock all" forks individually
visible. Never edited after lock — superseded only.

**Brief**:
The compact SessionStart context block generated from live state (held work,
live peers, enabled policies, next verb) and injected via harness hooks.
Plugins contribute sections through effects. Never protocol prose (ADR-0004).
_Avoid_: harness protocol, HARNESS.md

**Plugin**:
A TS module `{ name, inject, apply(ctx) }` whose registrations are reversible
effects. Installable, disable-able, removable per repo or globally.
_Avoid_: extension, module, addon

**Service**:
A capability claimed at a stable `ctx.<key>` in the context registry; plugins
depend on services via `inject`, never by importing implementations.

**Gate**:
A waterfall listener on a lifecycle event that may short-circuit (block with a
reason) instead of delegating via `next()`. Gates live in policy plugins.
_Avoid_: hook (reserved for agent-harness hooks), check

**Effect**:
A registration (verb, gate, listener, prompt section) paired with a disposer so
plugin unload unwinds it completely.

**Shim**:
The `#!/usr/bin/env bun` launcher installed at `~/.local/bin/maestro`; the real
code runs from source, open-world.

**Legacy binary**:
The last Rust maestro, kept installed as `maestro-legacy` as rollback during the
transition.

**Attention**:
A candidate signal a cheap detector computes at read time (stalled lease,
repeated failure, stale decision, scope collision, unreturned dispatch) and
records once per fingerprint. It names evidence and a smallest action; it is
never delivered and never carries a verdict or a command (intervention ladder
levels 1-2).
_Avoid_: alert (implies severity), violation

**Lane**:
A Herdr pane with one named mandate: `delivery` (may write), `decision`
(no-write recommendation), or `challenge` (adversarial findings, no fixes).
The Lead opens the pane; Maestro records its mandate and boundaries in the
dispatch envelope.
_Avoid_: sub-agent, worker, role (roles derive from leases, not a field)

**Envelope / Handback**:
The dispatch contract given to a lane (objective, owned scope, excluded scope,
mutation, stop condition, lane) and the structured return (status vocabulary,
assumptions not verified, residual risks), both stored as durable records.
_Avoid_: prompt, report
