# Long-term maintainability roadmap

## Current state

Root design record: `./SPEC-maintainability-roadmap.md` (authoritative). This
card spec is a compact mirror, refreshed whenever a decision locks or a
workstream is added or retired. Last refresh: 2026-06-12 (all decisions
locked).

The 2026-06-12 repo-wide audit produced four findings: the architecture doc
describes a `CardType` trait the code does not implement, `scripts/verify-all.sh`
defaults to macOS-only `/private/tmp`, `maestro resume` rendering uses
production `writeln!(...).unwrap()` calls, and temporary domain compatibility
aliases linger in CLI imports. The idea cards cited in earlier drafts were
never persisted; the root SPEC's Evidence Map is the record.

The 2026-06-12 `review-swarm` + `simplify-code` pass (8 `gpt-5.4 xhigh`
agents) produced 20 roadmap candidates (SRI-1..20 in the root SPEC). The
WS6/WS7/WS8 subsets were first-party verified on 2026-06-12 during the
D7/D8/D9 walks; verification refuted one WS7 claim (`list_archived`) and
partially refuted SRI-2 (installer), validating the unverified-until-confirmed
intake rule.

The root SPEC also carries the full schema-evolution design record as a WS5
Design Annex (merged 2026-06-12 from `SPEC-schema-evolution.md`, which is now
historical evidence only).

## Problem

Maestro needs a durable path from "audit found debt" to "design record" to
"small implementation slice". Without that path, long-term debt either stays
as scattered findings or becomes a broad rewrite.

The root SPEC keeps one shared roadmap and splits the findings into
workstreams:

- WS1 Architecture Truth
- WS2 Portable Full Verification
- WS3 Panic-Free Production Rendering
- WS4 Compatibility Alias Retirement
- WS5 Schema Evolution and Artifact Compatibility
- WS6 Runtime Trust Boundaries
- WS7 Read-Model and Hot-Path Efficiency
- WS8 Domain Ownership and Cleanup
- WS9 Multi-Agent Scale Boundaries (added 2026-06-12 via D12 after the
  pre-approval growth-gap review)

## Decisions

All locked 2026-06-12; full records with previews live in the root SPEC.

- D1: one umbrella roadmap SPEC, schema evolution merged as WS5, future
  implementation cut into small approved slices.
  Card: `dec-roadmap-shape-for-maintainability-work-d659`.
- D2 (WS1): CardType dispatch truth is the enum + exhaustive match;
  `ARCHITECTURE.md` section 2 gets rewritten to match; the trait survives
  only as a revisit trigger.
  Card: `dec-cardtype-dispatch-truth-for-ws1-38dd`.
- D3 (WS2): `verify-all.sh` default temp root becomes `mktemp -d` +
  `cd`/`pwd -P` canonicalization (portable, symlink-free).
  Card: `dec-verify-all-default-temp-root-portability-4f7a`.
- D4 (WS3): resume rendering cleanup + src-wide no-production-unwrap guard,
  empty allowlist at adoption.
  Card: `dec-production-unwrap-policy-for-rendering-8a9c`.
- D5 (WS4): re-point both CLI alias imports + interfaces import-boundary
  rule in one slice; alias deletion itself scoped to D9.
  Card: `dec-cli-alias-import-retirement-mode-b905`.
- D6 (WS5): six annex sub-decisions, all locked:
  - D6.1 versioned readers + in-memory normalization (reading just works);
    envelope-as-contract is the long-term direction.
    Card: `dec-schema-compatibility-posture-b913`.
  - D6.2 embedded schema packs as the editable contract source + Rust
    compat kernel. Card: `dec-embedded-schema-packs-as-contract-source-9d42`.
  - D6.3 storage hub runtime pattern: bounded supported versions, read-only
    commands never rewrite disk, explicit migrate with backup.
    Card: `dec-storage-hub-runtime-pattern-2019`.
  - D6.4 no YAML-to-SQLite pivot; the disposable projection (D6.4-B) is the
    only sanctioned future shape, owned by WS7.
    Card: `dec-yaml-to-sqlite-storage-pivot-d75e`.
  - D6.5 all-artifact coordinated rollout under 11 advisor lock conditions
    (advisor review recorded 2026-06-12). Owed before implementation: the
    per-family supported-old-version matrix (condition 8).
    Card: `dec-schema-compatibility-rollout-strategy-7456`.
  - D6.6 unknown fields tolerated on read, preserved through rewrites,
    surfaced by `maestro doctor` (closes SRI-6).
    Card: `dec-unknown-field-passthrough-rule-for-card-e0ef`.
- D7 (WS6): invited repos only - passive writes require an existing
  `.maestro/`, failed checks also stamp, hook exec path pinned; installer
  and skill-sync patterns unchanged.
  Card: `dec-passive-side-effect-policy-for-runtime-2a34`.
- D8 (WS7): load once per command, no new on-disk caches; count+mtime
  stamps advisory-only; persisted accelerators follow the grep-index
  precedent; before/after measure on every fix.
  Card: `dec-hot-path-read-model-scope-and-freshness-8a65`.
- D9 (WS8): retire it all - task shim, lib.rs aliases, tautological guard
  tables, root migration playbook, stage comments (DN3 vocabulary stays),
  card_support dead code; nothing is downstream API. Bridge dedup stays the
  separately sequenced WS8 slice.
  Card: `dec-true-api-versus-retired-transition-60e3`.
- D10: owner-first intake routing - owned surface -> workstream evidence +
  SRI series (unverified until confirmed); unowned -> backlog idea card;
  new workstreams only via a walked decision.
  Card: `dec-intake-routing-policy-for-future-audit-6d54`.
- D11: the feature card closes only when every workstream is closed or
  explicitly parked with a recorded reason; bundle archives on close.
  Card: `dec-roadmap-feature-exit-condition-29ef`.
- D12: WS9 Multi-Agent Scale Boundaries added via the D10 walked-decision
  path after the user's pre-approval growth-gap review; dependency hygiene
  and CLI verb governance filed as backlog ideas
  (`idea-dependency-hygiene-policy-for-third-780c`,
  `idea-cli-verb-governance-when-a-new-command-0f06`). Note: the WS9
  evidence was corrected in-session - snapshot CAS + write-lock markers
  already shipped (hb-011, `c0a48988`); the root SPEC carries the corrected
  record. Card: `dec-add-ws9-multi-agent-scale-boundaries-to-a8cf`.
- D13 (WS9): write-safety scope is audit + document - every write path
  audited for snapshot/CAS routing with a guard test, concurrency model
  documented, multi-file coordination only on demonstrated interleaving.
  Card: `dec-concurrent-write-safety-scope-for-ws9-8928`.
- D14 (WS9): agent-facing read verbs (`list`, `ready`, siblings) gain
  `--json` with stable versioned fields (additive-only per the D6.3 change
  policy); human text stays free to improve.
  Card: `dec-machine-readable-output-contract-for-a75c`.

## Status

APPROVED 2026-06-12. Design phase complete: D1-D14 locked (including the six
D6 sub-decisions and the WS9 additions from the growth-gap review). The
Approval Gate is satisfied: root SPEC status flipped to APPROVED and
`VERIFY-maintainability-roadmap.md` created as the review gate.

Implementation is delegated to implementing agents on branch
`maintainability-roadmap`; the conducting session reviews the result against
the VERIFY file when the user returns, then the branch merges locally. No
push, tag, or publish without separate approval. Standing sub-gate: the
per-family supported-old-version matrix (D6.5 condition 8) must be written
into the root SPEC before any WS5 slice.
