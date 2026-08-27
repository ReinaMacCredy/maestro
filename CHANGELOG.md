# Changelog

All notable changes to Maestro are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## Unreleased history

The 0.107.x series was the Rust implementation. Version 0.108.0 begins the
TypeScript-on-Bun line and continues the existing version sequence.

## [Unreleased]

Nothing yet.

## [0.109.0] - 2026-08-28

### Added

- `scripts/install.sh`: one-command source install
  (`curl -fsSL .../scripts/install.sh | sh`) that clones the repository into
  `~/.maestro/source` and runs the installer from it, so `maestro update`
  keeps following that checkout; `install.sh --help`; a Bun 1.4 floor.
- Expanded SLP with a no-write shadow lane, explicit `COUNCIL_REQUEST`
  handbacks, sealed council cross-examination, one Lead per scope, and
  searchable Lead handoff receipts; `bundle open` refuses a successor whose
  handoff packet still holds placeholders.
- Bound the single Supervisor's scope and authority in `IDENTITY.md`, deny
  Claude sub-agent tools in the Supervisor room, and seed `OWNER.md` with the
  interview questions the room asks on its first session.
- `~/maestro/lead.md`: the Supervisor hands owner intent to a repository Lead
  through Herdr, prompting a live Lead or starting one, without writing to the
  project store.
- The intake contract (d700): the Lead scores the ROI questions from the
  repository and the prompt, asks at most one owner-boundary question, and
  announces the score, the route, the deciding fact, and the adjacent route
  not taken with the phrase that switches to it.
- Attention kinds `LEAD_COLLISION`, `HUMAN_DECISION_REQUIRED` (from
  `decision draft --needs-owner`), and `DECISION_REVIEW_DUE` (from
  `decision lock --review-at`); `SCOPE_COLLISION` now also compares the
  declared scopes of open delivery dispatches across work items.
- Typed return details: `handback file --request` is required for
  `BLOCKED`, `DEPENDENCY_REQUEST`, `COUNCIL_REQUEST`, and `REOPEN_REQUEST`,
  and `HANDBACK_UNREVIEWED` names the recovery path per status; optional
  `--candidate` on `handback file` and `work done`; `--dissent` on decisions.
- `dispatch confirm`: an untargeted dispatch accept records a claim that only
  the opener can confirm before the lane can work or return.
- A Claude `PreToolUse` hook denies sub-agents to a session that holds an
  accepted open dispatch; Leads and Codex are unaffected.
- `install` and `update` warn, and treat unreadable registered repositories as
  unsafe, before replacing the shared runtime.
- Method skills carry a `review-date`; `maestro doctor` reports overdue ones.
- Docs: an owner-seat SLP scenarios guide, an enforced-versus-soft-audited
  boundary table, build-time Mermaid diagrams with no runtime CDN dependency.

### Changed

- Cross-role decisions now use Herdr for transport and Maestro decisions or
  work notes as the durable record; one dispatch ends with exactly one
  handback (d697).
- Repeated failures now route Peer-held work to the Lead and Lead-held work to
  the Supervisor.
- `decision draft --work` warns when that work's council is still sealed; the
  recipe forbids storing the Lead's view before first views return.
- The managed instruction block names the repository's own `AGENTS.md` and
  `CLAUDE.md` as its Workspace Protocol.
- Documented quickfix, Light, and Full method tiers with explicit handoff and
  evidence boundaries.

### Fixed

- Sealed council returns are hidden from `handback list --json` and attention;
  a no-write lane holder cannot take the work lease; `--atomic-reason` is only
  written when `work start` succeeds; `update` treats untracked `src/` and
  `bin/` files as dirt; the doctor reads Codex hook trust from
  `~/.codex/config.toml`.

## [0.108.0] - 2026-08-27

### Added

- Rebuilt Maestro in TypeScript on Bun around a mechanism-only kernel, removable
  verb and policy plugins, and prompt-first Markdown recipes and skills.
- Added durable work, decisions, bundles, dispatches, handbacks, councils,
  attention packets, observer mode, a cross-repository brief, and the
  Supervisor, Lead, and Peer role model.
- Added source-checkout installation, fast-forward-only updates, read-only
  diagnostics, repository uninstall, a private Supervisor room, and four
  installed method skills.
- Added a bounded stdio MCP interface for finding and running Maestro verbs.
- Added read-only import of Rust-era stores plus idempotent promotion of legacy
  work, decisions, supersession links, receipts, and archived snapshots.

### Changed

- Coordination lanes now use Herdr panes for agent lifecycle and wake-up while
  Maestro remains the durable contract and evidence record.
- Attention is computed when read instead of delivered through a daemon or
  mailbox. Returned but unreviewed handbacks now receive immediate attention.
- The Rust-era stores are preserved under `legacy/rust/` for import and
  provenance.

### Fixed

- Serialized store transitions and ID allocation so concurrent work, decision,
  and dispatch operations cannot lose leases, collide IDs, or leak raw SQLite
  errors.
- Scoped council membership to the current concurrent generation and applied
  decision supersession only when the replacement locks.
- Rejected blank required arguments and invalid lane names with actionable
  errors.
- Hardened installer source trust, symlink boundaries, configuration writes,
  file permissions, rollback behavior, and runtime restamping.
- Made observer search fail closed when its index cannot be refreshed.
- Validated malformed MCP frames without terminating the server.
- Preserved native chronology and evidence when Rust data is promoted, including
  repeated promotion and missing-receipt handling.
- Corrected lane session resolution, handback status guidance, and command
  contracts in generated documentation.
