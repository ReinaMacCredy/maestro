# Changelog

All notable changes to Maestro are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## Unreleased history

The 0.107.x series was the Rust implementation. Version 0.108.0 begins the
TypeScript-on-Bun line and continues the existing version sequence.

## [Unreleased]

Nothing yet.

## [0.111.1] - 2026-08-29

### Changed

- Every Claude agent SLP starts now carries `--autocompact 250000` (owner
  order, room d24): the Model table's Claude cells, the Claude command strings
  in the lane and lead templates, and the `hm` shell function, which previously
  started the Supervisor with no flags at all. Codex command strings are
  unchanged, since Codex has no equivalent flag. `hm` takes the window but not
  a model: model names live in `OWNER.md`, and the `--kind` plus the flags
  after `--` are named as one owner edit point so switching to `--kind codex`
  does not leave Claude flags behind.

## [0.111.0] - 2026-08-29

The public-adoption release (d8). Maestro is now licensed, has a security
policy, and no longer executes code a repository carries.

### Added

- MIT `LICENSE` (owner choice, room d16), `SECURITY.md` with a private
  reporting channel and the boundaries a report is judged against, and
  `CONTRIBUTING.md` naming the bun-only toolchain, the test-first rule, and the
  A1-A3 gates CI enforces.
- Plugin trust boundary (d712, d713). A global or repository plugin executes
  only when `~/.maestro/trust.json` holds a grant matching its canonical path
  and a sha256 over every file in the artifact. `maestro plugin trust` and
  `plugin untrust` manage grants; `plugin add` trusts the bytes it just cloned
  and no longer imports them; `plugin new` scaffolds untrusted. An untrusted
  plugin is named from the filesystem and never imported, so `plugin list`
  executes nothing.
- `maestro handback review <h-id> --note <text>` (d17) records that the opener
  read a return packet and clears its `HANDBACK_UNREVIEWED` finding. Only the
  session that opened the dispatch may file it, and a second review is a no-op.
- PR CI over the trees the root suite never compiled: the desktop TypeScript
  build and tests, the desktop Rust crate on macOS, and the documentation site.
- A store records its schema generation in `PRAGMA user_version`, and a Maestro
  older than the store refuses every command with `STORE_TOO_NEW` rather than
  writing into a shape it does not know.

### Changed

- `scripts/install.sh` installs the newest pushed release tag by version order
  onto a `maestro-release` branch instead of the tip of `main` (d714).
  `MAESTRO_REF` remains the development escape hatch, and `maestro update`
  moves a pinned checkout tag to tag while every other branch keeps its
  upstream fast-forward. Pinning buys a release and a reproducible install; it
  is not tamper-proofing, because `install.sh` itself is fetched from `main`.
- The desktop data layer runs every verb under `MAESTRO_READ_ONLY=1`, so
  watching a store no longer writes a session row, heartbeats liveness, or
  loads the watched repository's plugins.
- A Lead reports a closed card carrying relayed room intent with one
  `[from lead][done w<id> re d<room-id>]` prompt (d22); `maestro brief` prints
  attention findings only, so a closure was otherwise invisible to the room.
- The `HANDBACK_UNREVIEWED` packet's smallest action is the review verb on
  every branch; the status-specific next move moved into its question.
- A lane's cwd decides which store it reads, so the lane procedure states the
  room lane's cwd and makes a lane compare its stored contract against the one
  it was sent before accepting (d717).

### Fixed

- `LEAD_COLLISION` missed a holder pair where one card was parentless and the
  other a child, so two sessions could hold active work in one repository with
  `maestro attention` reporting nothing (d21). A pair sharing a parent stays
  `SCOPE_COLLISION` alone, so one incident is still one packet.
- `STALLED_LEASE` raised nothing when the holder session was dead, leaving an
  abandoned card active forever; a dead holder now raises regardless of
  freshness, as an unreturned dispatch already did.
- `plugin list` reported an untrusted plugin twice, once as untrusted and again
  as a missing source.
- Tests 47 and 308 exceeded Bun's 5s default on an idle machine and now carry
  explicit timeouts.

## [0.110.0] - 2026-08-28

### Added

- `policy-card-budget` (d703): a removable policy on the new `work.add` gate
  that refuses a new card while the store already holds `limit` (default 3)
  open cards with no live holder and no accepted dispatch; enabled here.
  lane.md: a finding returned in a handback is closed by that handback and
  becomes a card only when it is the next thing the Lead will do.
- Declared councils (d706): `dispatch open --council-members <n>` and
  `--council-anchor <id>` record membership instead of inferring it from
  timing; the seal is a property of the declared council, whatever order the
  lanes open and return. One handback per dispatch is now a store constraint,
  and only the session that opened a dispatch may cancel it.
- Enforcement registry: the boundary table in `concepts/lanes.md` gives every
  row an id and cites the test that attacks it, or says `soft-audited`;
  meta-tests refuse a row without an attacking test and an SLP mechanism
  token that no row claims.
- Roles travel in pane names (d709): the room finds a repository Lead only as
  the Herdr agent it started as `lead-<repo basename>`, the Lead starts each
  lane as `peer-<dispatch id>` after `dispatch open --pane`, a peer-named
  session refuses any prompt that is not its stored contract, and the hook
  brief prints a store-derived `role:` line on every prompt (open dispatch
  ids, closed ones as a count; a returned holder is still a peer). The
  repository mirror block names the Lead the same way.
- The Supervisor room is a recorded store fact: the installer writes
  `meta.kind=room` into the room store; `doctor` applies the room contract
  there (files, registry, hooks, deny list, store) and never prescribes
  `maestro install`; `install` and `uninstall` refuse with `INSTALL_IN_ROOM`;
  the room hook brief carries a Supervisor intake line; the room `AGENTS.md`
  names the repository-only verbs.
- SLP Model reference (d711, guidance only): four rungs (cheap, strong,
  diverse, lead) with dated per-harness examples the owner keeps current in
  `OWNER.md`, a thinking level per lane, and the effort flags to pass; the
  Lead picks a lane's model the way it picks a sub-agent's, the room picks the
  Lead's model, nothing records or enforces the choice.

### Changed

- Session identity (d705) is never adopted from an incidental signal: without
  an explicit id a process mints `anon-<uuid>`; the pid stays liveness-only.
- Read-only opens fall back to a `query_only` handle for a cleanly closed WAL
  store, so declared-pure verbs answer the same with or without the sidecars.
- The Peer `PreToolUse` deny now covers a session on any dispatch row in any
  state, matching the role line.
- lane.md: a lane never talks to the Lead through the terminal; its returns
  are the handback and `--request`.

### Fixed

- `work cancel` on a parent cascades to its open or active descendants in one
  transaction with a prefixed reason and one event per row.
- `work add` records the session, so a session whose only verb was `work add`
  appears in `status --live`.
- `dispatch confirm` accepts a targeted dispatch already held by the named
  session instead of failing with `CLAIM_MISMATCH`.
- The `role:` line no longer lists every dispatch the Lead ever opened.

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
