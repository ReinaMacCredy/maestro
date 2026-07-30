# Maestro vNext Embedded and Agent UX Reconciliation Notes

Status: side-conversation workbench, design-only, non-authoritative

This file accumulates the post-main Embedded, Agent UX, Skill-system,
Distribution, migration, and removal reconciliation being developed with the
user in a side conversation. It is evidence and candidate synthesis for later
delivery to the canonical main conductor. It does not override locked Maestro
Decisions, the canonical feature design, or the main conductor's Decision
authority.

Do not use this file to authorize implementation, build work, feature
reconciliation/finalization/acceptance, installation, release, or mutation of
external systems.

## Live baseline

Baseline captured on 2026-07-13, Asia/Ho_Chi_Minh:

- Post-refoundation scratch SHA-256:
  `12e5d7f361512aec4d69131a148464d3e163f86936645229aea77daa738b2cd7`
- Canonical feature design SHA-256:
  `77fed03def8c3428b9f1adf0be028767163a3262de3c9176439ce7f74415c7b1`
- Canonical Decision store SHA-256:
  `269ab54bd85ea3766f1518bad90daa78f63c4d913c70edd9425590c50a3834ed`
- Canonical feature contract SHA-256:
  `4f626af6a836e2ee314106ad64bf4369a3ff78ac75b0ff76f948d9781c8bfea8`
- Live Decision census: 155 total, 113 effective locked, 42 superseded,
  zero open.
- Feature state: `proposed`; zero executable build Tasks.

Live canonical artifacts always override this baseline and this file.

## Pre-Main Evidence Relay gate

The evidence portion of the terminal `<MAIN_CONTINUATION_PROMPT>` in
`SPEC-MAESTRO-VNEXT-POST-REFOUNDATION-BRAINSTORM.md` passed on 2026-07-13 and
is materialized in that file's `Post-main evidence closure` section. The live
closure is:

- Embedded universe: 204 expected, 204 classified, zero remaining.
- Direct-consumer universe: 325 expected, 325 classified, zero remaining.
- Installed/cache/mirror universe: 28,102 expected, 28,102 classified, zero
  remaining; 27,883 regular files plus 219 symlinks; zero cross-category
  overlap and zero missing.
- The independent path census and adversarial omission sweep completed. They
  corrected the historical 302-file direct-consumer and 4,816-location
  partial-ledger limits without rewriting their lineage.

The stable full reread of the effective Decision closure and canonical design
remains a main-conductor pre-Decision obligation. Passing this evidence gate
does not authorize a Decision, implementation, build approval, feature
reconciliation, finalization, acceptance, installation, or release.

## Why this lane exists

The canonical design already places Embedded resources, Agent UX, Skills,
Harness, Recipes, hooks, shell, schemas, installation, update, migration, and
removal inside the full product scope. However, much of that coverage remains
at constitutional, ownership, or census level. The asset-level product design
is not yet sufficiently explicit about:

- what an agent receives after installation;
- the final public Skill jobs and routing model;
- the exact distinction among Skill, Harness, Recipe, pattern/playbook,
  schema, hook, shell, and agent instruction assets;
- package identity, versioning, compatibility, and provenance;
- ownership and disposition of every embedded and installed asset;
- adoption and custody of user-edited copies;
- update, backup, rollback, uninstall, and clean-install behavior;
- migration of installed copies, caches, mirrors, and worktrees; and
- the complete legacy-removal consumer graph.

This lane closes those gaps without reopening settled Work, Step, Contract,
Authority, Execution, Evidence, Projection, or other effective core semantics.

## Live Embedded inventory

The current `embedded/` universe contains exactly 204 files:

| Family | Files | Current reconciliation stance |
| --- | ---: | --- |
| Design resources | 77 | Deep disposition and packaging reconciliation required |
| Schemas | 57 | Machine-contract ownership and generation/removal reconciliation required |
| Skills and references | 35 | Final Skill jobs, routing, composition, and package topology required |
| Loop Recipes, including one profile | 16 | Rewrite against canonical Packet/Operation boundaries |
| Playbook/pattern resources | 11 | External-pattern boundary, provenance, and delivery treatment required |
| Harness resources | 2 | Thin operating-contract target required |
| Hook resources | 2 | Acquisition-adapter-only target required |
| Shell resources | 2 | Invocation-adapter-only target required |
| Root agent instructions | 2 | Thin routing/instruction target required |

The 204/204 count proves current embedded-file inventory coverage only. It does
not prove final architecture, installed-copy coverage, consumer-total removal,
or final disposition correctness.

## Canonical constraints imported from main

The side lane imports these settled constraints and does not reopen them:

- Maestro is a local-first work-governance kernel, not a general-purpose agent
  runtime, daemon, scheduler, queue, worker launcher, Eval subsystem, Council,
  or secrets manager.
- Work and Step are the public work model.
- Projection is the sole next-action authority.
- Public governed submission uses the closed Packet/Operation/Result contract;
  Operation contains the disjoint Action and Ceremony branches.
- Every mutation uses one typed authorized request. Unknown, generic, nested,
  batched, Recipe-defined, adapter-defined, argv, shell, and plugin semantics
  fail closed.
- Skill, Harness, Recipe, adapter, hook, shell, TUI, MCP, connector, and
  Distribution resources cannot own private lifecycle, next-action authority,
  authorization, Evidence semantics, retries, scheduling, cursor state,
  mutable semantic state, migration, or recovery outside their exact boundary.
- Installed-resource mutation requires explicit identity, version, hash,
  ownership, alias-closed target resolution, visible diff, backup, explicit
  apply, verification, rollback, and removal treatment.
- Passive update checks and binary updates cannot silently mutate user-owned
  Harness, Skills, hooks, Recipes, configuration, or instructions.
- vNext is a clean runtime break. Legacy names and semantics are migration
  inputs, not normal-runtime compatibility authority.
- Retention is never implicit.

## Target Agent journey

The candidate Agent UX should make the normal journey:

```text
install or capability handshake
  -> receive thin instructions and exact resource identities
  -> route to one public capability job
  -> obtain a fresh Agent Packet or supported Ceremony capability
  -> inspect exact requirements when needed
  -> prepare and submit one Operation
  -> receive one canonical Result
  -> recover exact in-doubt work or request a fresh Packet
```

An agent should not need to memorize overlapping lifecycle rules, private Skill
state, hidden command sequences, or the full Maestro command tree.

## Reconciliation workstreams

### Agent UX

Design the complete human and agent journey across installation, capability
handshake, routing, Packet reading, inspect, preparation, one-Operation
submission, Result interpretation, recovery, and continuation.

### Skill system

For every current and proposed Skill, decide:

- public capability job;
- canonical semantic owner;
- invocation and routing contract;
- composition boundary;
- Packet/Operation/Result interaction;
- package identity, version, hash, compatibility, and provenance;
- installation targets and custody;
- migration and removal consumers; and
- one explicit disposition: retain, rewrite, replace, migration-only, or
  remove.

Skills remain procedures over canonical contracts. They do not own semantics.

### Resource architecture

Every embedded and installed asset requires one total resource record:

```text
Resource identity
Resource family and package
Canonical semantic owner
Distribution owner
User job
Source hash and version
Compatibility and provenance
Install targets and alias closure
Maestro-owned or user-owned custody
Adoption and local-modification treatment
Dry-run and visible diff
Backup, apply, verification, rollback, and uninstall
Migration and quarantine
Removal consumers
Explicit disposition
```

### Boundary taxonomy

- Harness: thin operating contract and router.
- Skill: agent-facing capability procedure.
- Recipe: bounded sequencing advice over canonical Projection and Operation.
- Pattern/playbook: binary-served external implementation knowledge, never a
  runtime semantic extension.
- Schema: versioned machine contract, never lifecycle or authority.
- Hook: acquisition adapter only.
- Shell: invocation adapter only.
- Root agent instruction: thin routing and safety contract.
- Distribution: resource identity, custody, planning, staging, installation,
  update, rollback, uninstall, and recovery at its exact boundary.

### Distribution lifecycle

The full target lifecycle is:

```text
inspect and capability handshake
  -> plan, alias-closed target resolution, and visible diff
  -> authorize exact typed Distribution Action
  -> snapshot the Installation Realm
  -> stage exact immutable resources or intended absence
  -> verify the complete candidate
  -> activate at the Realm boundary
  -> commit Installation Receipt
  -> prune only after committed success
```

No read path, passive update, binary refresh, or cache lookup may perform a
hidden resource mutation.

### Migration and removal

The consumer census must cover:

- repository-local installed copies;
- global installed copies;
- worktree copies;
- Codex, Claude, and other agent mirrors;
- generated CLI and MCP references;
- user-edited Harness and Skill files;
- retired caches and backups;
- package and runtime assets;
- schemas and generated fixtures;
- tests, docs, examples, completions, and release/install scripts; and
- archive, export, migration, rollback, and retained-old-binary readers.

No asset is removed until its live consumer set is empty or every remaining
consumer is explicitly retained as sealed migration or audit tooling.

## Initial architectural recommendation

Start by defining one immutable, content-bound Resource Bundle and Resource
Manifest model shared by Skills, Harness, Recipes, patterns, schemas, hooks,
shell assets, and agent instructions. The bundle should bind exact asset
identity, package membership, semantic owner, Distribution treatment,
compatibility, provenance, target policy, custody, migration, rollback, and
removal data.

This common substrate should be settled before choosing the final Skill catalog
or package names. Otherwise each asset family may invent a separate version,
install, adoption, and rollback protocol.

## Settled side choices

These choices are settled between the user and the side-conversation assistant
for later review by the main conductor. They are not canonical Maestro
Decisions until the main conductor reconciles and records them.

### ER-1: Two-level Resource and Bundle identity

Chosen option: B, one immutable identity per Resource plus one content-bound
identity per exact Resource Bundle.

`ResourceDescriptorV1` identifies one source asset and binds at least:

- exact resource identity and content hash;
- resource kind and package membership;
- canonical semantic owner and Distribution treatment;
- compatibility and dependency constraints;
- provenance and licensing commitments;
- install-target policy and custody class;
- migration, rollback, uninstall, and removal disposition; and
- explicit retain, rewrite, replace, migration-only, or remove treatment.

`ResourceBundleManifestV1` identifies one exact, immutable, canonically ordered
set of `ResourceDescriptorV1` values and binds at least:

- bundle kind, version, and compatibility range;
- exact member set and no-duplicate/no-omission proof;
- exact dependency Bundle identities;
- package-level provenance and policy commitments;
- supported target classes; and
- migration, rollback, uninstall, and removal closure.

Bundle identity uses the already locked `ManifestIdentityV1` byte and hashing
protocol. It creates no second hashing algorithm, latest selector, mutable
registry, runtime extension system, authority, or activation right.

Installed state is deliberately outside source Resource and Bundle identity.
One `InstallationReceiptV1` binds the exact Bundle identity, alias-closed target
identity, before and after state, custody/adoption decision, backup, staged
apply result, verification, activation, rollback or uninstall state, and
authorization receipts. A path, installed copy, local edit, cache, mirror, or
backup cannot redefine the source Resource or Bundle identity.

Required failure behavior:

- duplicate target ownership, duplicate members, missing members, dependency
  cycles, unknown resource kinds, unsupported compatibility, unresolved
  provenance, or incomplete migration/removal treatment fail closed;
- two Bundles cannot silently claim the same target or overwrite one another;
- local divergence is explicit and routes to adopt, preserve, quarantine,
  replace, or refuse under a later custody Decision; and
- unknown installed files are never implicitly retained or promoted into a
  Bundle.

Example: the `maestro-design` package contains individually identified
`SKILL.md`, CLI reference, DDD reference, and domain-model reference resources.
Their exact set forms one Skill Bundle, which in turn belongs to an exact
release-level Embedded Bundle. Updating one member changes both its Resource
identity and every containing Bundle identity, while preserving the installed
copy's separate custody and history.

### ER-2: Closed typed Bundle hierarchy

Chosen option: B, one release-rooted hierarchy of closed Bundle kinds with a
content-bound, strictly backward dependency DAG.

`BundleKindV1` is the closed union:

```text
Release
AgentBootstrap
Capability
SharedContract
Adapter
ExternalPattern
Migration
```

The hierarchy law is:

```text
EmbeddedReleaseBundleV1
  -> exact BundleManifestV1 members
      -> exact ResourceDescriptorV1 members
```

- One `EmbeddedReleaseBundleV1` is the sole root of one resource release.
- Every non-release Bundle belongs to exactly one release membership set.
- Every Resource has exactly one owning Bundle; another Bundle may reference
  it only through its exact identity and may not duplicate ownership or bytes.
- Bundle dependencies bind exact Bundle identities, are finite, acyclic, and
  strictly backward under the release's canonical order.
- A Bundle never selects targets, installs itself, activates itself, or grants
  authority. Distribution creates a separately authorized Installation Plan
  and Receipt for exact Bundle identities.
- Missing dependencies, cycles, duplicate ownership, unknown Bundle kinds,
  unsupported compatibility, incomplete provenance, or unresolved
  migration/removal treatment fail closed.
- No `latest`, semver-only selector, mutable package registry, plugin Bundle,
  runtime registration, dependency fallback, or adapter-selected Bundle is
  permitted.

Bundle responsibilities are fixed:

- `Release`: exact root bill of materials only.
- `AgentBootstrap`: thin root-agent instructions and thin Harness routing.
- `Capability`: one public Agent capability job and its procedures; it owns no
  core semantics.
- `SharedContract`: public or shared machine schemas and generated contracts.
- `Adapter`: hook, shell, agent-host, or other Integration transport assets.
- `ExternalPattern`: read-only implementation/design knowledge with exact
  provenance and no runtime authority.
- `Migration`: legacy readers, maps, fixtures, quarantine, and rollback-only
  resources that cannot activate as normal-runtime contracts.

Candidate packaging of the current 204-file source inventory, subject to later
per-resource disposition and Skill-job reconciliation:

```text
Embedded Release
  Agent Bootstrap
    embedded/AGENTS.md
    embedded/CLAUDE.md
    embedded/harness/*

  Current Skill Source Bundles
    ask-maestro
    maestro-audit
    maestro-card
    maestro-design
    maestro-research
    maestro-setup
    maestro-witness

  Shared Contract
    active vNext machine schemas

  Migration Contract
    v1/retired schemas and fixtures

  Recipe Bundle
    current Recipes and standard profile, pending final disposition

  Hook Adapter
    embedded/hooks/*

  Shell Adapter
    embedded/shell/*

  Implementation Pattern
    embedded/playbook/*

  Design Reference Pattern
    embedded/design/styles/*
    embedded/design/vendor/* including manifest and license
```

The seven current Skill names and the current Recipe packaging are inventory
inputs only, not guaranteed vNext public packages. The later Skill-system fork
may rewrite, merge, replace, migrate, or remove them while preserving exact
source history and consumers.

Physical repository folders, archive layout, compression, or binary embedding
do not define Resource or Bundle identity. A build may embed deterministic
bytes directly or in an archive, but the canonical Manifest values define the
identity. Changing one Resource changes its Resource identity, every owning or
containing Bundle identity, and the Release identity; unrelated Bundles remain
unchanged.

Installation selects exact Bundles from an exact Release, resolves the exact
dependency closure, computes alias-closed targets, shows a dry-run diff, backs
up prior state, stages and verifies resources, activates them through
Distribution, and writes an `InstallationReceiptV1`. Not every Bundle must be
installed into every target: target selection is delivery treatment, not
product-scope reduction.

### ER-3: Portable progressive Maestro Tool discovery

Chosen option: B2, a host-installed three-tool bootstrap kernel followed by
progressive exact-tool discovery. Agent-facing `reference/cli.md` catalogs are
removed from the target Skill path; the CLI remains a supported fallback
Adapter and may generate human help on demand.

Every supported Agent host receives these stable bootstrap capabilities:

```text
maestro_packet
maestro_tool_search
maestro_tool_call
```

- `maestro_packet` reads one bounded current Agent Packet. Projection remains
  the sole owner of blockers, readiness, and Recommendation.
- `maestro_tool_search` searches only the exact installed, Release-bound Tool
  Catalog. Exact Tool-id lookup is the normal Packet route; fuzzy intent search
  is discovery only and establishes no eligibility, Recommendation, authority,
  currentness, reservation, or right to execute.
- On hosts that support dynamic MCP tool refresh, exact search activates the
  selected typed descriptor for that connection, the server emits
  `notifications/tools/list_changed`, and the host refreshes `tools/list`.
- `maestro_tool_call` is the portability fallback when the host cannot inject a
  discovered typed Tool dynamically. It accepts only an exact installed Tool
  descriptor id and hash, validates the exact input schema, routes to that
  closed typed handler, and attributes Result and Evidence to the inner Tool.
  It is not a generic Action, shell, plugin, authority path, or open dispatch
  registry.
- Unknown, uninstalled, disabled, incompatible, unpinned, mixed-Release,
  wrong-host-protocol, wrong-schema, or hash-mismatched Tools fail closed.
- Tool discovery and connection-local activation are ephemeral Adapter state
  only. They create no Work, Step, Recommendation, Action Request, Lease,
  authority, lifecycle, retry, Evidence applicability, or canonical cursor.

The portable bootstrap requires three independent discovery channels:

1. Distribution registers the Maestro MCP server with the exact supported
   Agent host, so the host discovers the kernel through MCP `tools/list` before
   model execution.
2. Distribution installs the concise `maestro` Skill metadata where the host
   supports Skills, so the model can associate governed user intent with the
   bootstrap Tools.
3. Distribution installs a thin repository Harness/root-agent pointer that
   detects missing bootstrap and stops rather than silently bypassing Maestro.

An `InstallationReceiptV1` cannot claim the Agent host ready merely because
files or configuration were written. It must bind the exact Release and host,
MCP protocol negotiation, bootstrap Tool descriptor identities, successful
`tools/list` probe, Skill metadata disposition, Harness pointer disposition,
and supported fallback mode. A configured but undiscoverable installation is
`installed_but_not_discoverable`, not ready.

Host behavior is closed by capability:

```text
dynamic MCP discovery
  Packet -> exact Tool search -> typed Tool injection -> typed call

fixed-tool MCP host
  Packet -> exact Tool search -> maestro_tool_call -> closed typed handler

no MCP, shell available
  Skill/Harness -> exact Release-bound CLI Adapter fallback

guidance only, no MCP or shell
  inspection only; mutation fails closed

no MCP, Skill, Harness, or shell bootstrap
  unsupported/uninstalled; the Agent cannot be expected to know Maestro exists
```

The canonical Tool/Action registry is the one source for MCP schemas, Tool
Search descriptors, fallback validation, CLI parsing/help, TUI controls, Packet
capability requirements, and compatibility proof. Adapter renderings never
become Action authority. No host-specific preloaded Tool Search, including a
Codex-native one, is assumed by the Maestro product contract.

### ER-4: CLI-first Agent execution with a two-Tool MCP surface

Chosen option: C. ER-4 supersedes ER-3's three-Tool kernel, dynamic Tool
activation, and `maestro_tool_call` fallback. ER-3 remains historical evidence;
its portable host-registration, discoverability proof, exact Release binding,
and fail-closed missing-bootstrap requirements continue where consistent.

The complete public Maestro MCP surface is exactly:

```text
maestro_packet
maestro_cli_search
```

- `maestro_packet` reads one bounded current Agent Packet. Projection alone
  owns the Recommendation. When the Recommendation is executable through CLI,
  the Packet includes the exact versioned command id, typed Action Spec id,
  argument-vector template, required structured inputs, and output schema.
- `maestro_cli_search` searches the exact structured CLI Capability Catalog of
  the installed running binary. Exact command-id lookup is preferred. Fuzzy
  intent search is discovery only and cannot establish current eligibility,
  Recommendation, authority, expected state, or permission to execute.
- Search returns structured `argv_template` arrays, schemas, effect class,
  authority requirements, compatibility, binary version, and Release identity.
  It never returns arbitrary shell fragments, pipelines, substitutions, or
  executable user-controlled text.
- The catalog is generated from the same canonical Action/Query registry and
  CLI command tree used by parsing and help. Search never scrapes `cli.md`,
  `--help` prose, Skill text, or cached installed documentation.
- The Agent executes selected work through the shell-visible Maestro CLI. CLI
  parsing is an Adapter boundary only; canonical typed Action Request,
  expected-state, authority, Lease/fence, effect, Result, and Evidence checks
  remain in their owning domain handlers.
- There is no public lifecycle MCP Tool, dynamic Tool injection, Tool-list
  mutation, generic MCP call gateway, hidden Tool alias, private plugin Tool,
  or alternate MCP mutation path.
- Current individual Maestro MCP Tools are removal/migration inputs. Their
  public registrations disappear in vNext; each capability maps exactly once
  to a canonical handler and CLI command or receives an explicit replace,
  migration-only, or remove disposition.
- Agent-facing `reference/cli.md` files are removed. Human Markdown help may be
  generated on demand from the exact running catalog but is not a shipped Skill
  dependency or authority source.

Host capability is explicit:

```text
MCP + shell
  Packet/Search discovery -> exact CLI argv -> canonical typed handler

shell without MCP
  thin Skill/Harness bootstrap -> exact CLI packet/search fallback -> handler

MCP without shell
  Packet and CLI discovery only; mutation unsupported and readiness fails

no MCP, shell, Skill, or Harness bootstrap
  unsupported/uninstalled; Agent cannot be expected to know Maestro exists
```

The installer registers only the two read-only MCP descriptors, installs the
Skill and thin Harness pointers, and proves the exact `tools/list` census plus
shell-visible binary identity. A ready Installation Receipt binds the same
Release identity across MCP descriptors, CLI Capability Catalog, binary, Skill,
and Harness. A mismatch, missing shell, stale binary, extra Maestro MCP Tool,
or missing descriptor is not ready.

Required proof includes an exact two-Tool `tools/list` census; zero legacy or
hidden MCP registrations; full current-MCP-to-CLI/handler disposition coverage;
Packet command rendering parity; exact and fuzzy search fixtures; structured
argv injection resistance; JSON stdin/stdout and exit-contract tests; stale,
mixed-binary, wrong-Release, unknown-command, unsupported-host, and no-shell
refusal; Skill/Harness removal of old Tool names; clean install and reconnect;
upgrade from cached v1 Tool schemas; and exact rollback to the prior MCP/CLI
Bundle without semantic reinterpretation.

### ER-5: Thin `MAESTRO.md` bootstrap and method-aware router capsules

Chosen option: rename the public repository bootstrap to
`.maestro/MAESTRO.md`, keep it and the public `maestro` Skill context-minimal,
and retain enough DDD, TDD, and Wayfinding trigger detail for reliable routing.

- The public installed bootstrap is `.maestro/MAESTRO.md`; its owning source
  Resource is `embedded/bootstrap/MAESTRO.md` under the AgentBootstrap Bundle.
  `Harness` remains an internal architectural term only.
- The root `AGENTS.md` managed block is the one owner of the instruction to read
  `.maestro/MAESTRO.md`. `SKILL.md` does not repeat that pointer.
- `MAESTRO.md` contains only the canonical-state/Projection boundary, the two
  MCP discovery names, CLI-first execution rule, no-guess rule, and missing or
  incompatible bootstrap stop. It contains no command catalog, lifecycle
  manual, method workflow, migration procedure, or high-entropy identity.
- Resource, Bundle, Release, schema, compatibility, and installed-copy
  identities live in `ResourceBundleManifestV1`, `InstallationReceiptV1`, and
  machine validation. They are omitted from normal Agent-facing Markdown and
  routine Packets unless an exact incompatibility diagnosis requires them.
- The public `maestro` Skill is one concise router. It names the six jobs and
  gives DDD, TDD, and Wayfinding one trigger capsule each: purpose, activation
  condition, and hard boundary. Detailed steps, examples, schemas, and edge
  cases remain progressively disclosed in their exact resource.
- DDD is advertised for genuine core behavior, precise domain language, and
  material invariants; it is skipped for mechanical CRUD/formatting/adapter
  work and cannot lock Decisions or authorize implementation.
- TDD is advertised for behavior-changing executable Steps as vertical
  RED-GREEN-REFACTOR evidence; passing tests do not themselves satisfy a Gate
  or complete a Step.
- Wayfinding is advertised for a bounded multi-session destination with
  dependent questions or material fog; it names Destination, Investigation
  Steps, fog, and Projection-owned frontier, resolves at most one Investigation
  Step per Run, and does not approve or execute the destination.

The legacy `.maestro/harness/HARNESS.md` and
`embedded/harness/HARNESS.md` paths are migration inputs only. Migration never
activates both old and new bootstrap files, never silently merges user edits,
and explicitly preserves, adopts, quarantines, replaces, refuses, or restores
the legacy bytes. Rollback restores the exact prior active path and root-agent
pointer.

Required proof includes exact root-pointer uniqueness; absence of the duplicate
`Read .maestro/MAESTRO.md` instruction from the Skill; Agent-facing token-budget
fixtures; Manifest/Receipt identity verification without Markdown frontmatter;
positive and negative DDD/TDD/Wayfinding routing fixtures; no-detail eager-load
proof; clean install, edited legacy migration, dual-file refusal, archive and
restore, rollback, and Linux case-sensitive path tests.

### ER-6: Project-first public Agent job vocabulary

Chosen option: B with the project-first correction. The canonical public
`maestro` Skill exposes exactly these six jobs:

```text
Research
Design
Review
Execute
Recover
Adapt
```

- `Advance` is not a job. New and continuing work both begin with a fresh
  `maestro_packet`, whose Projection-owned Recommendation routes to one job.
- `Research` gathers local or external evidence, constraints, stakeholders, and
  unknowns before a material decision. It cannot lock a Decision or publish a
  Contract.
- `Design` turns evidence into domain language, Decisions, Contract Components,
  acceptance, proof gates, and a finalization candidate. DDD and Wayfinding may
  be loaded here when their trigger capsules match.
- `Review` explains or challenges current state, blockers, history, design,
  code, Evidence, proof, migration readiness, or capability conformance. It is
  read-only by default; repair routes through a fresh Packet to Design,
  Execute, Recover, or Adapt.
- `Execute` performs one eligible CLI Action through a fresh typed authorized
  Action Request. Behavior-changing Steps load TDD when applicable.
- `Recover` handles stale state, crashes, rejected submissions, uncertain
  effects, quarantine, migration failure, and rollback without blind retry or
  revived stale authority.
- `Adapt` makes Maestro fit a consuming project's domain, toolchain, workflow,
  integrations, and Distribution needs through governed Recipes, Profiles,
  Adapters, Integrations, Patterns, and Resource Bundles. The Maestro repository
  itself is only one consumer; Adapt is not limited to changing Maestro core.

`Extension Law` is a mandatory built-in method for Adapt and an available lens
for Design, Review, Execute, and Recover when extension artifacts are involved.
An adaptation cannot create private lifecycle, Recommendation, authority,
Evidence meaning, retry, scheduling, cursor, mutation, migration, recovery, or
state-store semantics outside its exact boundary.

Normal use of an existing adaptation remains the owning job: installing or
configuring an existing Recipe is Execute; inspecting it is Review; recovering
its failure is Recover. Creating or materially revising the reusable adaptation
is Adapt.

Required proof includes positive, negative, and ambiguous invocation fixtures
for all six names; continuation routing without an Advance branch; Research vs
Review, Design vs Adapt, Execute vs Recover, and existing-use vs new-adaptation
boundary cases; project examples across web, iOS, data, monorepo, CI/release,
and external trackers; current seven-Skill migration mapping; no private
semantics mutants; Agent Packet job parity; and token-budget proof that only one
job plus explicitly triggered methods loads.

### ER-7: One public Maestro Skill and no active legacy aliases

Chosen option: B. Maestro vNext ships and activates exactly one public Agent
Skill named `maestro`. The seven v1 Skill packages are source and migration
inputs only; no active redirect, alias, shim, duplicate description, or second
router survives.

The closed disposition is:

| v1 Skill | vNext disposition |
| --- | --- |
| `ask-maestro` | remove; routing is owned by `maestro/SKILL.md` plus Packet |
| `maestro-research` | rewrite into `jobs/research.md` and exact research resources |
| `maestro-design` | rewrite into `jobs/design.md`, DDD, Wayfinding, and exact design resources |
| `maestro-audit` | rewrite into `jobs/review.md` and exact review resources |
| `maestro-card` | split into Execute, Recover, TDD, Review, Recipes, and canonical CLI actions |
| `maestro-setup` | split into Execute, Review, Recover, Adapt, Distribution guidance, and migration |
| `maestro-witness` | replace with Review plus canonical acceptance/proof/close Gates |

Every nested v1 resource receives exactly one retain, rewrite, replace,
migration-only, or remove disposition. The table does not authorize whole-folder
retention. ER-4 removes every Skill-local `reference/cli.md`; CLI discovery uses
the running binary's structured Catalog.

Legacy installation treatment is identity-sensitive:

- an exact known unmodified seven-Skill installation is archived byte-for-byte,
  deactivated, replaced atomically at the activation boundary by the one
  `maestro` Skill, and recorded in a Migration/Installation Receipt;
- a modified, unknown, partial, mixed-version, unreadable, or provenance-
  ambiguous legacy copy is preserved and quarantined inactive until an explicit
  adopt, preserve, replace, remove, or refuse disposition is authorized;
- new and legacy Skills are never simultaneously active in one Agent host or
  search root;
- rollback deactivates the new Skill, restores the exact archived legacy Bundle
  and host registration, and verifies its previous active census and identities.

Old prompts or cached invocations do not justify an active alias. Migration
diagnostics may explain the replacement and point to the one `maestro` Skill,
but they own no routing or execution semantics.

Required proof includes exactly one discoverable public Skill; absence of all
seven legacy descriptions and directories from active host roots; complete
nested-resource disposition coverage; no loss of research, DDD, TDD, QA,
review, setup, witness, recovery, Recipe, or Distribution behavior; positive
and negative old-prompt diagnostics; modified-copy preservation; partial and
mixed-version refusal; atomic activation; clean install; upgrade; uninstall;
archive/export; exact rollback; and context-budget comparison against v1.

### ER-8: Baseline plus optional external design-pattern distribution

Chosen option: B. The 77 current `embedded/design` resources split into two
closed Bundles with different installation treatment:

```text
DesignBaselinePattern
  embedded/design/styles/neutral/DESIGN.md

BrandDesignReferencePack
  embedded/design/vendor/awesome-design-md/manifest.yml
  embedded/design/vendor/awesome-design-md/LICENSE
  embedded/design/vendor/awesome-design-md/design-md/*/DESIGN.md  # 74 files
```

- `DesignBaselinePattern` is a first-party small baseline included in the
  Release. It is available as a fallback for UI projects without a project
  design system, but target selection does not materialize or activate it for
  every non-UI project and it never overrides a project-owned `DESIGN.md`.
- `BrandDesignReferencePack` is one optional `ExternalPattern` Bundle, not part
  of AgentBootstrap or the public Skill Bundle and not installed or activated by
  default. It preserves the exact upstream repository, commit
  `664b3e78fd1a298ba11973822da988483256d4b4`, MIT license, manifest hashes, and
  all 74 vendored references.
- Design/Adapt may discover installed pattern-catalog CLI capabilities through
  `maestro_cli_search`. Pattern catalog search returns bounded metadata; exact
  show/read progressively discloses only the selected pattern. Public CLI names
  remain a later surface rendering and are not locked by this Decision.
- An external pattern is reference Evidence only. It cannot become the project
  Contract, project `DESIGN.md`, acceptance authority, Recipe, Recommendation,
  or mutation source without a separately authorized Design result.
- Brand selection, Bundle installation, upstream update, project design
  materialization, and replacement of an installed copy are all explicit
  actions. No network fetch, `latest` resolution, automatic brand default,
  whole-library context load, proprietary font/asset acquisition, or silent
  project-file overwrite is permitted.
- A project-owned design artifact and a selected external reference retain
  separate identities and provenance. Local project edits never mutate the
  vendored Resource, and Bundle updates never rewrite the project design.

Migration inventories the neutral file, 74 reference files, manifest, license,
all embedded/archive/cache/installed copies, and every consumer. Exact known
vendor copies may move into the optional Bundle; modified or ambiguous copies
are preserved and quarantined. Rollback restores the exact former packaging and
activation state without promoting a pattern to project authority.

Required proof includes exact `1 + 74 + manifest + license = 77` census; Bundle
identity and license preservation; non-UI clean install with no active vendor
pack; explicit optional install/uninstall; offline exact selected-pattern read;
bounded context showing one pattern only; missing-pack diagnostics; project
`DESIGN.md` noninterference; no automatic network/update/latest behavior;
proprietary-font and brand-authority negative tests; modified-copy quarantine;
archive/export; and exact rollback.

### ER-9: Active SharedContract schemas plus exact legacy Migration packs

Chosen option: B. The 57 current `embedded/schemas` files split by role rather
than remaining one active runtime schema authority.

The exact current census is:

```text
11 legacy artifact families
  11 current.yaml
  11 supported.yaml
  11 retired.yaml
  22 fixtures
   1 AGENTS.md
   1 CLAUDE.md
  --
  57 files
```

- A new vNext `SharedContract` Bundle exposes only public machine boundaries:
  persisted canonical records where public, Agent Packet, CLI JSON input/output,
  the two MCP result contracts, archive/export, Installation and Migration
  Receipts, and public Adapter contracts.
- Public schema renderings derive from one canonical vNext Type/Contract
  Registry. They do not create a parallel meaning, and internal implementation
  structs or database rows do not become public merely because generated schema
  is possible.
- All 33 current `current.yaml`, `supported.yaml`, and `retired.yaml` files are
  exact `Migration` Bundle inputs. All 22 fixtures are migration/proof-only and
  are not installed into consuming projects as current schemas.
- Legacy packs may identify and read exact v1 bytes, preserve provenance,
  quarantine malformed or ambiguous data, drive explicit mapping, prove
  archive/export, and support exact rollback. They cannot create or validate a
  vNext record, govern lifecycle, authorize mutation, influence Projection,
  become a Contract Root, or serve as fallback semantics.
- Legacy `retired.yaml` identifiers remain reserved-name migration/proof input
  and cannot be silently reused with a different vNext meaning. The YAML files
  themselves remain non-authoritative for current runtime behavior.
- `embedded/schemas/AGENTS.md` is repository authoring instruction only and is
  excluded from product Schema Bundles. `embedded/schemas/CLAUDE.md` is removed
  with legacy agent mirrors after its consumers are proved absent.

Migration preserves original byte sequence, locator, container ordering, parse
failure, and exact legacy schema identity before normalization. A successfully
parsed v1 fixture produces only candidate vNext records; current SharedContract,
authority, and activation checks decide whether they become current. Unknown or
ambiguous mappings quarantine instead of guessing or falling back.

Required proof includes exact `33 + 22 + 2 = 57` census; one disposition per
family and file; active-runtime architecture guards against consulting legacy
packs; exact fixture reads with no seeded-byte rewrite; malformed, unknown, and
mixed-version quarantine; reserved-name non-reuse; public CLI/MCP/archive schema
parity from one canonical meaning; no internal-schema accidental publication;
no Card/Feature/Task/Harness leakage into vNext writers; removed instruction-
mirror consumer proof; archive/export; and exact rollback with the full legacy
schema pack.

### ER-10: Maestro-owned replacement with two retained rollback snapshots

Chosen option: A with bounded rollback. A recognized Maestro-managed installed
Resource is Maestro-owned at its declared target and may be replaced by an
explicit authorized install, sync, update, migration, or restore Action even if
its bytes were locally modified. Passive checks never mutate.

The closed target ownership is:

```text
MaestroOwnedTarget
SharedManagedBlock
Unmanaged
```

- `MaestroOwnedTarget` covers exact declared Maestro resources such as
  `.maestro/MAESTRO.md`, the canonical `maestro` Skill, MCP bootstrap descriptors,
  managed hooks, and active SharedContract resources. Explicit mutation may
  replace the whole target. Local edits are not adopted or merged; project
  customization belongs in an Adapt-owned project Resource instead.
- `SharedManagedBlock` covers files such as root `AGENTS.md` or host/shell
  configuration where Maestro owns only an exact bounded block. Maestro replaces
  that block while preserving all bytes outside it. Missing, duplicated,
  overlapping, malformed, or ambiguous markers fail closed instead of granting
  whole-file ownership.
- `Unmanaged` targets are never changed. A path without trusted ownership and
  target provenance is not inferred to be Maestro-owned merely because its name
  resembles a managed path.

Before any activation that would change managed bytes, Maestro records one
coherent pre-activation Installation Snapshot for the entire Installation Realm,
including exact affected bytes, target paths, ownership class, permissions,
managed-block boundaries, active Bundle/Release identities, host registration,
and prior Installation Receipt. Symlinks and unsafe aliases are rejected rather
than followed into a snapshot or replacement.

Retention is exactly the two most recent prior successfully active Installation
Snapshots per Installation Realm. The current active installation is not counted
as a retained snapshot. Pruning happens only after the new activation and Receipt
commit succeed; a failed, crashed, staged-only, or rolled-back activation never
deletes the last recoverable snapshot. Snapshot storage is access-restricted and
must not create a second active installation or authority source.

Rollback selects one of the two exact retained snapshots, stages and verifies
its complete restoration, atomically reactivates it at the Installation boundary,
and writes a new Rollback/Installation Receipt. It never performs a three-way
merge or reinterprets old bytes under the new Release. Rolling back itself first
captures the current active installation, then applies the same two-snapshot
retention rule after successful activation.

ER-10 supersedes earlier modified-copy preserve/quarantine treatment in ER-7,
ER-8, and ER-9 only for targets whose Maestro ownership and exact Installation
Realm provenance are trusted. Foreign, ambiguous, unsafe, or unowned paths still
fail closed and are not overwritten. Recognized modified legacy Skill or pattern
copies are captured in the coherent snapshot and replaced; unknown copies remain
untouched and block conflicting activation.

Required proof includes whole-target replacement; shared-block outside-byte
noninterference; modified and missing managed-target restoration; foreign and
ambiguous refusal; exact current-plus-two-prior retention; third-oldest pruning
only after committed activation; crash at every snapshot/stage/activation/Receipt/
prune boundary; two-step rollback; rollback-of-rollback; permissions and host-
registration restoration; no symlink traversal; no automatic adoption or merge;
passive-check nonmutation; and explicit diagnostics that local customization must
move into an Adapt-owned Resource.

### ER-11: Layered package topology with shared Agent Skill activation

Chosen option: A, with the current cross-host Skill convention retained and
made explicit. The immutable Embedded Release in the running Maestro binary is
the Distribution source. Installation materializes only the exact Bundles
required by each closed Installation Realm; physical copies, links, caches, and
snapshots never redefine Resource, Bundle, or Release identity.

The topology is:

```text
running Maestro binary
  immutable Embedded Release and exact manifests

User Agent Installation Realm
  ~/.maestro/skills/maestro/         active Maestro-owned Skill materialization
  ~/.agents/skills/maestro           universal Codex/compatible-CLI activation
  ~/.claude/skills/maestro           Claude activation
  host MCP registrations             exact two-Tool read-only surface
  one coherent Installation Receipt

Project Installation Realm
  <project>/.maestro/MAESTRO.md
  <project>/AGENTS.md managed block
  project-owned Adapt resources and exact project receipt

Worktree Installation Realm
  the exact checked-out project targets for that worktree
  no implicit mutable resource sharing with another worktree

Distribution cache
  immutable hash-addressed Release/Bundle bytes only

Rollback store
  exactly two prior successfully active snapshots per Installation Realm
```

`~/.agents/skills` is the universal public Agent Skill root for Codex and other
compatible CLIs. Maestro does not create a separate `~/.codex/skills` target.
Claude additionally receives the same exact Skill through
`~/.claude/skills/maestro`. Both host-facing entries activate the one
`~/.maestro/skills/maestro` materialization and therefore cannot drift into two
independent Skill meanings.

Host-facing activation links are permitted only when Maestro created them or an
exact prior Receipt proves their ownership, target, and resolved alias closure.
The display path and fully resolved path are both recorded. A trusted symlinked
root such as the current `~/.claude/skills` layout may be used after alias-closed
resolution, but loops, escapes, collisions, broken links, link substitution, or
an unowned destination fail closed. This is the narrow owned-link exception to
ER-10's rejection of arbitrary or unsafe symlinks.

The User Agent Realm activates and rolls back its Skill materialization, both
host entries, MCP registrations, permissions, and Receipt coherently. It cannot
leave Codex-compatible hosts and Claude on different Releases. Project and
worktree activation cannot mutate the User Agent Realm, and a global Agent
update cannot silently rewrite project-owned Adapt resources.

Distribution cache entries are immutable, content-addressed acceleration only.
They cannot act as the active installation, satisfy a Receipt, select a Release,
authorize mutation, or survive a hash mismatch. Cache eviction changes no
active state. Rollback snapshots are separate from cache, access-restricted,
bound to one Installation Realm, and follow ER-10's exact two-prior retention.

Required proof includes exact binary-to-Bundle provenance; one active Skill
materialization; both host activation paths resolving to the same bytes;
absence of a Maestro-created `~/.codex/skills` target; compatible-CLI discovery
through `~/.agents/skills`; Claude discovery through `~/.claude/skills`; trusted
symlink-root resolution; loop, escape, substitution, and collision refusal;
atomic cross-host activation and rollback; no project/global or worktree/
worktree mutation bleed; cache eviction and corruption non-authority; exact
Receipt coverage; and current-plus-two-prior snapshot retention for every Realm.

### ER-12: Setup as the seventh job and global-only MCP bootstrap

Chosen option: A. ER-12 supersedes ER-6's closed six-job count, ER-5's
six-job router wording, and ER-7's treatment of `maestro-setup` as behavior
scattered only across other branches. Maestro still exposes exactly one public
Agent Skill named `maestro`, now with seven progressively disclosed jobs:

```text
Setup
Research
Design
Review
Execute
Recover
Adapt
```

`Setup` is a job branch at `jobs/setup.md`, not a second public Skill, alias,
router, lifecycle, or privileged installer. It handles the human and Agent
journey for clean installation, project adoption, prerequisite and
compatibility inspection, dry-run planning, explicit installation or update,
discoverability verification, repair, uninstall, migration, and rollback. Any
mutation still uses an exact authorized Distribution Action through the running
CLI and produces the canonical Plan, Result, Receipt, and Evidence.

MCP installation belongs only to the User Agent Installation Realm. Maestro
registers exactly the two read-only Tools `maestro_packet` and
`maestro_cli_search` globally for each supported Agent host so they are
discoverable before the Agent enters a project. A project or worktree never
installs, starts, configures, shadows, vendors, or selects its own Maestro MCP
server, Tool descriptors, Tool aliases, or Tool version.

Project Setup may verify the global MCP registration and report an exact global
Setup Action when it is missing, stale, mixed-Release, or undiscoverable, but it
cannot silently create or mutate that global registration as a side effect of a
project-scoped Action. Project scope installs only the thin
`.maestro/MAESTRO.md` bootstrap, root `AGENTS.md` managed block, selected
project Bundles, Adapt resources, and project Installation Receipt.

The Setup branch cannot infer approval, grant authority, choose the next Work
or Step, change Projection, reinterpret Evidence, hide a mutation in detection,
or maintain private installation state. Passive detection and routine Packet
reads remain non-mutating. Setup completes only when the requested scope has an
exact committed Receipt and its required discovery/compatibility probes pass,
or when it returns a typed blocked/refused Result with recovery guidance.

The v1 `maestro-setup` Skill is rewritten into `jobs/setup.md` plus exact shared
Distribution, migration, Review, Recover, and Adapt resources. Its old public
name, description, router, local CLI reference, and independent activation are
removed under ER-7.

Required proof includes one public Skill and seven job routes; positive,
negative, and ambiguous Setup invocation fixtures; Setup versus Execute, Adapt,
Review, and Recover boundaries; no active `maestro-setup` alias; clean global
and project setup; already-installed idempotence; missing and stale global MCP;
exact two-Tool global census; zero project MCP registrations and descriptors;
project setup with global MCP absent; explicit global-before-project ordering;
no passive mutation; atomic User Agent Realm rollback; project-only rollback;
uninstall; migration from the v1 Setup Skill; and bounded Skill context.

### ER-13: One transactional Distribution flow for every mutation journey

Chosen option: A. Every resource-changing Distribution journey uses one closed
transaction protocol. The supported mutation intents are:

```text
Install | Update | Repair | Migrate | Rollback | Uninstall
```

They differ only in the authorized desired-state Plan. They do not own separate
backup, write, activation, retry, recovery, Receipt, or pruning engines. The
single ordered protocol is:

```text
Inspect
  -> PlanAndDiff
  -> Authorize
  -> Snapshot
  -> Stage
  -> VerifyCandidate
  -> Activate
  -> CommitReceipt
  -> Prune
```

`Inspect` and `PlanAndDiff` are read-only. The Plan binds the exact mutation
intent, Installation Realm, current Receipt and expected-state fence, source
Release and Bundle closure, resolved targets, ownership classes, before/after
hashes, visible diff, compatibility, required authority, verification probes,
rollback source, and removal consequences. A changed target or Receipt makes
the Plan stale and requires a fresh Plan; it is never silently rebased.

`Authorize` accepts one exact typed Action Request for that Plan. It cannot
authorize a broader Realm, newer Plan, different Release, additional target, or
later retry. Detection, update checks, Packet reads, cache reads, and planning
remain non-mutating.

`Snapshot` records the coherent pre-activation Realm required by ER-10. A clean
install records exact prior absence and any shared-file outside-block bytes.
`Stage` writes only to inactive, access-restricted staging targets. `VerifyCandidate`
checks hashes, manifest closure, compatibility, permissions, ownership,
managed-block integrity, Agent discovery, MCP census when global, project
bootstrap when project-scoped, and journey-specific acceptance before any
activation claim.

`Activate` applies the complete verified candidate under one Realm activation
fence. If the platform cannot physically switch every target in one atomic
filesystem operation, a partial switch is never reported as active: the Realm
is durably `recovery_required`, readiness and further ordinary mutation fail
closed, and Recover resumes or restores from the exact journal and snapshot.
There is no blind retry, hidden daemon, private queue, or inferred completion.

`CommitReceipt` is the sole successful terminal commit and binds every activated
target, probe, Result, prior Receipt, snapshot, and authorization. A staged or
partially activated candidate without this Receipt is not successfully active.
`Prune` is post-commit housekeeping only: it enforces exactly two prior
successful snapshots and removes safe staging data. Prune failure cannot undo
the committed state, cannot delete a recoverable snapshot, and is reported as
explicit cleanup debt.

Journey semantics are closed:

- `Install` activates exact Bundles from prior absence or compatible shared
  containers; an already identical installation returns a typed no-change
  Result without creating a fake snapshot or new Receipt generation.
- `Update` replaces one exact active Release with another after compatibility,
  migration, and rollback closure are proved.
- `Repair` restores the exact currently receipted Release and targets; it does
  not adopt local edits or silently update.
- `Migrate` preserves exact legacy bytes and provenance, generates candidate
  vNext state, and activates only after all ambiguity is resolved; otherwise it
  quarantines without claiming success.
- `Rollback` treats one of the two retained snapshots as the exact desired
  state, first snapshots the current active Realm, and never merges or
  reinterprets old bytes.
- `Uninstall` stages intended absence of Maestro-owned targets and removal of
  only Maestro-owned shared blocks/links/registrations. It preserves unmanaged
  bytes, writes an uninstall Receipt/tombstone, and remains rollback-capable
  under the same two-snapshot rule.

Removing a Resource from a future Release is not equivalent to uninstalling one
Realm. Release removal is permitted only after the consumer census is empty or
every remaining consumer is sealed as explicit Migration/audit support. Each
affected active Realm still transitions through Update, Migrate, or Uninstall.

Required proof crosses every mutation intent with clean/no-change, stale Plan,
concurrent edit, authority mismatch, unknown target, staging failure,
verification failure, partial activation, Receipt failure, prune failure,
process crash at every boundary, recovery resume, recovery restore, and two-
snapshot retention. It additionally proves global MCP versus project bootstrap
scope, modified managed targets, shared-block outside-byte preservation,
legacy quarantine, uninstall and rollback-of-uninstall, update then rollback,
rollback-of-rollback, cache loss, and no hidden mutation or retry path.

### ER-14: Manifest-driven Resource and Installation census

Chosen option: A. Completeness is proved through two linked, read-only census
contracts rather than an unbounded filesystem search or trust in the new
manifest alone:

```text
ReleaseResourceCensusV1
InstallationCensusV1
```

`ReleaseResourceCensusV1` is generated and verified from the exact source tree
and build graph before an Embedded Release can be created. It covers every
shipped, generated, referenced, migrated, tested, documented, installed,
archived, exported, restored, or removed Resource and every direct consumer.
Each item appears exactly once as an owned Resource or exactly once as a
consumer reference to an owning Resource identity.

Its closed evidence families include:

- `embedded/` resources, manifests, licenses, fixtures, and generation inputs;
- binary embedding and build-script consumers, catalogs, registries, command
  metadata, MCP descriptors, schema generation, and release packaging;
- installer, updater, sync, repair, rollback, uninstall, archive, export,
  import, migration, retained-old-binary, and removal readers or writers;
- CLI, JSON, MCP, TUI, hook, shell, completion, Recipe, Skill, bootstrap,
  prompt, profile, Adapter, Integration, and Distribution surfaces;
- tests, snapshots, docs, examples, scripts, CI/release checks, and version
  guards; and
- every legacy name, path, alias, schema, backup family, and compatibility
  reader retained solely for migration or audit.

For every entry the Release census binds one canonical owner, exact source or
consumer locator, Resource/Bundle/Release identity where applicable,
provenance, target policy, direct-consumer edges, and exactly one disposition:
`retain`, `rewrite`, `replace`, `migration-only`, or `remove`. No unclassified,
multiply classified, ownerless, or consumerless removal candidate may enter the
Release.

`InstallationCensusV1` is produced by Setup/Inspect for one explicit
Installation Realm without mutation. Its search roots are closed and
provenance-bearing:

- the exact running binary and its Embedded Release;
- the User Agent Realm under `~/.maestro`, the universal
  `~/.agents/skills` activation root, the Claude `~/.claude/skills` activation
  root, and supported host MCP configuration locations declared by each host
  Adapter;
- the explicitly selected project and worktree roots, their `.maestro`
  locations, root-agent managed blocks, receipts, and Adapt resources;
- exact cache and two-snapshot stores declared by Distribution;
- known v1 and retired Maestro roots, aliases, mirrors, backups, lock files,
  Harness/Skill/schema locations, archives, and migration locators; and
- additional explicit import or legacy roots supplied by the user and bound
  into the inspection request.

The scanner resolves aliases without escaping the declared root closure,
records both display and resolved paths, does not follow arbitrary symlinks,
and never searches unrelated home-directory or filesystem content. Absence is
claimed only inside the complete declared root set; Maestro never equates “not
found in known roots” with “no copy exists anywhere on the machine.”

Every discovered installation item is classified exactly once as active,
inactive, stale, legacy, modified-managed, foreign, ambiguous, unsafe,
snapshot, cache, archive, or removal candidate and linked to its Receipt and
source Resource when provable. A Maestro-like collision inside a declared
target root with unknown ownership blocks conflicting mutation and is never
silently adopted. Unknown material outside the declared roots remains out of
scope rather than being touched or falsely classified.

The census closure is:

```text
source Resource
  -> build/direct consumers
  -> owning Bundle and Release
  -> declared install targets
  -> active and inactive installed copies
  -> host/project consumers
  -> Receipt, snapshot, cache, archive, and migration readers
  -> final disposition and removal gate
```

Release removal requires zero live consumers across both census contracts or a
sealed, explicit Migration/audit consumer set with exact retained readers. The
current scratch count of 4,816 classified installed/cache/mirror locations is
historical evidence only until its expected set, root closure, one-time
classification, and final adversarial omission sweep are proved.

Required proof includes deterministic repeated census output; expected equals
classified counts per family and in total; every direct consumer edge resolves;
every item has one owner and one disposition; duplicate, missing, dangling, and
cyclic references fail; build graph versus manifest parity; known-root and
legacy-locator fixtures; current host roots; symlinked Claude root; escape and
loop refusal; custom explicit legacy root; no unrelated-home traversal; empty,
partial, modified, mixed-Release, archived, and rollback states; current and
retained-old-binary readers; removal refusal with one remaining consumer; and a
final adversarial sweep finding no census omission.

### ER-15: Post-main single-Skill catalog superseding candidate

Chosen option: A under the explicit post-main correction boundary. The target
ships and activates exactly one discoverable public Agent Skill named
`maestro`. Its seven progressively disclosed internal jobs remain:

```text
Setup | Research | Design | Review | Execute | Recover | Adapt
```

ER-15 is the side candidate that must supersede
`dec-canonical-vnext-shipped-skill-surface-2ab8` through a new canonical
Decision after this post-main reconciliation is complete. It does not edit,
unlock, reinterpret, or impersonate that currently effective Decision, and it
has no canonical authority before the main conductor performs the superseding
Decision path.

The unaffected `2ab8` laws are retained: Skills are thin Integration adapters;
Packet/Operation/Result and canonical owners remain authoritative; Skill prose
cannot create lifecycle, authority, Gate satisfaction, retry, scheduling,
storage, compatibility translation, or next-action truth; legacy bytes and
installed copies retain provenance; active aliases and silent translation are
forbidden; installed resources remain versioned and hash-bound; and migration,
removal, parity, compatibility, and user-file safety require explicit proof.

The replacement changes only the public Skill topology and job vocabulary:

- `maestro` is both the sole invocation surface and the concise router;
- the seven jobs are internal progressively disclosed resources, not separately
  discoverable Skills or independent model-trigger descriptions;
- no active `maestro-setup`, `maestro-research`, `maestro-design`,
  `maestro-work`, `maestro-audit`, `maestro-witness`, `ask-maestro`, or
  `maestro-card` Skill survives;
- old names may appear only in the offline migration report and exact removal
  diagnostics, never as aliases, redirects, shims, fallback routing, or runtime
  translation; and
- Setup, Research, Design, Review, Execute, Recover, and Adapt retain the exact
  boundaries already settled by ER-12 and the method/Recipe capsules.

The installed Resource is one exact `maestro` Skill tree containing one small
`SKILL.md` router and progressively disclosed job, method, Recipe, example, and
shared-reference resources. Each nested Resource retains its own identity and
owner while the active host catalog exposes only the one top-level Skill. The
pending MCP/public-transport supersession remains a separate fork; ER-15 does
not decide Tool names or counts.

Required proof includes exactly one discoverable public Skill in every
supported host; exactly seven reachable job branches; no eighth branch or
legacy alias; positive, negative, and ambiguous invocation fixtures; job
boundary and completion fixtures; top-level context budget; lazy branch load;
Packet/Operation/Result parity; exact resource and binary identity; global host
activation; clean install, update, migration, rollback and uninstall; user-file
safety; old-name refusal; complete removed-consumer census; and comparison
against the seven-separate-Skill topology proving reduced always-loaded
description cost without reduced product capability.

## Current open questions

The 2026-07-13 live edge sweep reopened the following dependency-ordered
material choices:

1. two read-only MCP discovery operations plus CLI execution versus the locked
   six-operation MCP transport;
2. explicit replacement of modified Maestro-managed targets versus the locked
   custody law that blocks local-edit overwrite without a distinct custody or
   adoption Action;
3. the missing owning Bundle kind for Orchestration-owned Recipe resources;
4. exact two-snapshot rotation versus mandatory retained binaries, resource
   closures, recovery bundles, and unfinished-effect holds; and
5. global host activation and project/worktree topology under the locked
   single-domain RepositoryDomain versus InstallationDomain law.

The Pre-Main Evidence Relay completion gate also remains external evidence that
must pass before any side choice can be reconciled into the canonical design.

## Live edge-sweep checkpoint — 2026-07-13

Stable pre/post read hashes at this checkpoint:

```text
scratch   12e5d7f361512aec4d69131a148464d3e163f86936645229aea77daa738b2cd7
design    d9c830dc3615117afe6586ee11cd9e685224c205534aa0b029f2212bee036015
decisions 77bac7a90fe0ccf4dcc97ddda6e84918eabc5fe02ed3b194a805b3f8c4b29ca5
```

The live feature remains `proposed` with zero build Tasks. The latest stable
closure observed during this sweep is 172 Decisions: 130 locked, 42
superseded, zero open. This supersedes the older
baseline count in this workbench but does not rewrite its historical snapshot.

The delta is material: locked
`dec-canonical-vnext-shipped-skill-surface-2ab8` now fixes seven separately
discoverable Skills: `maestro`, `maestro-setup`, `maestro-research`,
`maestro-design`, `maestro-work`, `maestro-audit`, and `maestro-witness`. It was
written and fully materialized in the canonical design during this side sweep.
It conflicts directly with ER-7 and ER-12 rather than silently superseding
them, because this workbench is non-authoritative.

The subsequent locked delta
`dec-canonical-hermetic-tui-release-bundle-4803` closes a TUI release-bundle
subject and does not change any ER-1 through ER-14 classification above. Main
remained actively writing during the sweep, so every eventual handoff must
repeat the stable hash and effective-closure read rather than treating this
checkpoint as permanently current.

The scratch evidence Completeness Gate now passes under the exact closure
recorded below and in the scratch's `Post-main evidence closure` section.
Family 11 is fully classified. The full effective-Decision and canonical-design
reread remains a main-conductor obligation before the first canonical write;
the evidence result alone is not a design, Decision, or build authority.

### Clause classification against the effective live closure

| Side clause | Classification | Required disposition |
| --- | --- | --- |
| ER-1 Resource plus Bundle identity | new candidate architecture | Reuse locked `ManifestIdentityV1` bytes and Distribution roles; create no second identity or hashing law. |
| ER-2 closed Bundle hierarchy | new candidate architecture | Resolve Recipe ownership before handoff; the current seven-kind union cannot own the listed `Recipe Bundle` under its own laws. |
| ER-3 three-Tool kernel | implementation-order only | Historical superseded side evidence; exclude it from the effective candidate. |
| ER-4 two read-only MCP operations and CLI-first execution | conflicting and requiring supersession | Effective `2f53 -> 49af -> 1aac` preserves exactly six MCP operations and governed Operation submission. |
| ER-5 thin `.maestro/MAESTRO.md` and method capsules | new candidate architecture | Retain the thin bootstrap candidate, but its Tool and Skill wording follows the public-surface forks. |
| ER-6 six-job vocabulary | implementation-order only | Historical side record superseded by ER-12; exclude it from the effective candidate. |
| ER-7 one public Skill | conflicting and requiring supersession | Newly effective `2ab8` locks seven separately discoverable Skills. |
| ER-8 baseline plus optional external design pack | consistent explanatory expansion | Main already assigns external-pattern semantics; exact Bundle, installation and disclosure shape remain post-main materialization. |
| ER-9 vNext SharedContract plus legacy schema packs | consistent explanatory expansion | Preserve the locked byte-total v1 migration and current-schema replacement laws; exact Bundle packaging remains new detail. |
| ER-10 overwrite of modified managed targets | conflicting and requiring supersession | `dec-distribution-custody-2f11`, AC-10 and `2ab8` preserve user-owned modifications and reject silent overwrite. |
| ER-10 exact two routine snapshots | new candidate architecture | Reconcile with retention holds and prior-binary/resource closure before it can become a safe canonical rule. |
| ER-11 host paths and common Skill materialization | consistent explanatory expansion | Paths already exist in the main census; exact activation/link topology must obey canonical target identity and domain separation. |
| ER-11 project/worktree Installation Realms and cross-host atomicity | conflicting and requiring supersession | Projects/worktrees are RepositoryDomains, not Installation Realms; no Plan, Transaction, Receipt or rollback may span domains. |
| ER-12 one Skill with seven jobs | conflicting and requiring supersession | `2ab8` fixes seven Skills and their exact public names. |
| ER-12 global-only two-Tool MCP | conflicting and requiring supersession | Main acceptance and effective transport require exactly six MCP operations; global installation location remains a post-main packaging candidate. |
| ER-13 one Distribution protocol | already locked | Materialize through the exact locked phase, Effect Intent, occurrence, recovery and single-domain laws rather than the simplified side phase names. |
| ER-14 linked release/install census | consistent explanatory expansion | The corrected live census is complete: 204 embedded Resources, 325 direct consumers and 28,102 closed-root physical nodes, each with owner/disposition input and exact proof identity; canonical Bundle and Installation materialization remains a later Decision/design write. |
| ER-15 post-main single-Skill catalog | conflicting and requiring supersession | Explicit side resolution: preserve one public `maestro` Skill and require a new canonical Decision that supersedes `2ab8` after this reconciliation. |

### Automatic non-material corrections from the sweep

- Project and worktree resource mutation belongs to a `RepositoryDomain`.
  `Installation Realm` is reserved for the externally protected global
  InstallationDomain. The ER-11 labels are historical side wording only.
- “One transaction” means one exact Distribution Transaction in one Authority
  Domain. A combined global-plus-project intent decomposes into separate
  Operations, Results and replanning; it has no cross-domain atomic promise.
- Exactly two means two ordinary user-selectable full snapshots in the rotating
  snapshot catalog. Mandatory retention-hold artifacts for unfinished Intents,
  recovery, sealed migration rollback or binary/protocol compatibility are
  separate protected objects, not extra selectable snapshots, and cannot be
  pruned by the rotation rule.
- Post-Receipt pruning atomically removes the third snapshot from the active
  snapshot catalog before reporting snapshot-retention compliance. Failed
  physical garbage collection may leave inaccessible cleanup debt but never a
  third selectable rollback snapshot or deletion of a held recovery object.
- Agent-facing procedure text says governed `Operation`, not generic Action,
  whenever both Action and Ceremony branches are possible. A Step execution is
  still an Action; first install, cutover and recovery may be Ceremonies.
- Installation scanning enumerates only exact declared Maestro targets, legacy
  locators and collisions inside their alias closure. It does not classify or
  ingest unrelated third-party Skills merely because they share an Agent root.

## Skill-design review evidence

The user explicitly selected `writing-great-skills` and `write-a-skill` to
review the proposed six Agent capability jobs. Their guidance changes the
packaging recommendation without changing the then-current six-job product
vocabulary. ER-12 subsequently adds Setup as the seventh branch while retaining
the review's one-router, progressive-disclosure, and context-budget findings.

### Findings

- Six capability jobs do not imply six separately model-invoked Skills.
  Model-invoked Skills permanently load every description and create both
  context cost and trigger competition. `Advance`, `Operate`, and `Recover`
  have adjacent triggers and would be particularly prone to ambiguous
  invocation if independently advertised.
- Keeping the seven historical Skills would preserve sediment and duplicated
  routing, lifecycle, proof, command, and recovery knowledge.
- One monolithic all-purpose `SKILL.md` would avoid description competition but
  create sprawl, weaken the information hierarchy, and load unrelated branches
  on every run.
- Progressive disclosure supplies the stable hybrid: one concise
  model-invoked Maestro router, explicit capability branches behind strong
  context pointers, and shared canonical reference outside branch bodies.
- Each branch needs a leading word, checkable completion criterion, and only
  the steps required for that branch. Branch-specific examples and reference
  move behind one-level pointers. Shared Packet, Operation, Result, Authority,
  Evidence, and Distribution meanings retain one source of truth rather than
  being copied into every branch.
- Deterministic validation, manifest generation, formatting, and parity checks
  belong in versioned scripts when repeatedly required. Semantic judgment and
  authority selection do not.
- The top-level `SKILL.md` target should remain under roughly 100 lines, carry
  only invocation/routing and hard completion gates, use a concise trigger
  description, and point directly to the selected branch resource.

### Refined candidate structure

```text
maestro/
  SKILL.md              model-invoked router, under 100 lines
  branches/
    setup.md
    research.md
    design.md
    review.md
    execute.md
    recover.md
    adapt.md
  methods/
    ddd.md
    tdd.md
    extension-law.md
  recipes/
    wayfinding.md
  examples/
    journeys.md         branch examples loaded only when needed
  scripts/
    validate-bundle.*   deterministic validation only
```

The exact directory shape remains a packaging candidate. The architectural
point is that the seven jobs are product branches and Capability Bundle subjects,
while one small router supplies automatic invocation. A job branch may still
be packaged and versioned independently if the final target-layout and atomic
installation protocol prove that shared routing cannot create split-package
activation.

### Candidate invocation description

```text
Use Maestro to set up, research, design, review, execute, recover, or adapt
governed project work. Use when Maestro artifacts exist or the user invokes
Maestro.
```

This is candidate wording only. Final descriptions must be tested against
positive and negative invocation fixtures and pruned for duplicate branches.

## Candidate Maestro Agent package demo

This is a non-authoritative design preview. It demonstrates the intended
information hierarchy without selecting final public CLI verb names or
changing any currently shipped resource.

### Candidate package topology

```text
maestro/
  SKILL.md
  jobs/
    setup.md
    research.md
    design.md
    review.md
    execute.md
    recover.md
    adapt.md
  methods/
    ddd.md
    tdd.md
    extension-law.md
  recipes/
    wayfinding.md
  reference/
    canonical-semantics.md
  examples/
    journeys.md
  scripts/
    validate-bundle
```

DDD and TDD remain first-class Maestro guidance as they are in v1, while
Extension Law is mandatory for Adapt. They move behind explicit method pointers
rather than being repeated in job bodies. Wayfinding is a Recipe consumed by
Research and Design, not a lifecycle, state store, scheduler, or authority
owner. Continuation begins at `maestro_packet` and creates no Advance job.

### Candidate `SKILL.md`

```markdown
---
name: maestro
description: Use Maestro to set up, research, design, review, execute, recover, or adapt governed project work. Use when Maestro artifacts exist or the user invokes Maestro.
---

# Maestro

## Start

Use `maestro_packet` for the current Recommendation. Use `maestro_cli_search`
only when the installed CLI command is unknown. Execute governed Operations
through the shell-visible Maestro CLI using the exact returned argv shape.

## Jobs

- Setup: install, adopt, inspect, update, repair, migrate, roll back, or remove
  Maestro through explicit Distribution Actions and verify the resulting
  Receipt. Read [jobs/setup.md](jobs/setup.md).
- Research: gather evidence, constraints, stakeholders, and unknowns before a
  material decision. Read [jobs/research.md](jobs/research.md).
- Design: turn evidence into domain language, Decisions, Contract Components,
  acceptance, and proof gates. Read [jobs/design.md](jobs/design.md).
- Review: explain or challenge state, blockers, history, design, code, Evidence,
  proof, or capability conformance without mutation. Read
  [jobs/review.md](jobs/review.md).
- Execute: perform one eligible CLI Action through a fresh authorized Action
  Request. Read [jobs/execute.md](jobs/execute.md).
- Recover: handle stale state, crashes, uncertain effects, quarantine,
  migration failure, or rollback without blind retry. Read
  [jobs/recover.md](jobs/recover.md).
- Adapt: make Maestro fit a project's domain, toolchain, workflow, integrations,
  and Distribution through governed extension artifacts. Read
  [jobs/adapt.md](jobs/adapt.md).

## Built-in methods

### DDD

Use [methods/ddd.md](methods/ddd.md) during Design or Wayfinding for genuine core
behavior, precise domain language, and material invariants. Skip mechanical
CRUD, formatting, and adapter-only work. DDD cannot lock Decisions or authorize
implementation.

### TDD

Use [methods/tdd.md](methods/tdd.md) during Execute for behavior-changing Steps.
Work in vertical RED-GREEN-REFACTOR slices and record attributable Evidence.
Passing tests do not themselves complete a Step or satisfy a Gate.

### Wayfinding

Use [recipes/wayfinding.md](recipes/wayfinding.md) from Research or Design when a
bounded destination spans sessions and the route contains dependent questions
or material fog. Name the Destination, Investigation Steps, and fog; Projection
owns the frontier. Resolve at most one Investigation Step per Run. Wayfinding
does not approve or execute the destination.

### Extension Law

Use [methods/extension-law.md](methods/extension-law.md) for every Adapt job and
whenever another job touches a Recipe, Profile, Adapter, Integration, Pattern,
or Resource Bundle. Adaptations cannot own private lifecycle, Recommendation,
authority, Evidence, retry, scheduling, migration, recovery, or state semantics.

## Stop

Never guess commands, ids, schemas, state, authority, approval, success, or next
action. Finish with one bounded readout, one candidate or authorized Result, a
fresh Packet, or an explicit blocker.
```

The body is intentionally a map. Detailed lifecycle, DDD, TDD, Wayfinding,
recovery, migration, and command catalogs remain out of the hot context until
selected.

### Candidate `methods/ddd.md`

```markdown
# DDD method

Use from Design or a Wayfinding investigation when the subject has genuine core
behavior, precise domain language, and material invariants. Stop cheaply for
mechanical CRUD, formatting, or adapter-only work.

1. Run the DDD fitness gate.
2. Establish ubiquitous language and canonical concept owners.
3. Separate core, supporting, and generic subdomains.
4. Model events, actions, policies, invariants, and context boundaries.
5. Produce candidate Contract Components and Decision alternatives.

DDD output is design input. It cannot lock a Decision, publish a Contract,
authorize an Action, or mutate lifecycle state.

Completion: every introduced domain term has one definition and owner, and
every material invariant is represented in a candidate Contract or explicit
Decision fork.
```

This retains the current gate-first DDD stance and its handoff into executable
work without copying the full v1 reference into the router.

### Candidate `methods/tdd.md`

```markdown
# TDD method

Use from Execute for a behavior-changing Step whose acceptance can be observed
through a supported public boundary.

For each vertical slice:

1. Bind the test to the exact accepted behavior and current Step Binding.
2. Record RED from a test failing for the expected reason.
3. Implement only enough behavior to reach GREEN.
4. Refactor while the relevant proof remains GREEN.
5. Submit RED, GREEN, source, environment, and revision observations as Evidence.

Test success is Evidence, not lifecycle authority. Gate evaluation and the
authorized Result determine applicability and satisfaction.

Completion: every accepted behavior selected for this Step has attributable
RED/GREEN Evidence or an explicit authorized skip basis.
```

This preserves current Maestro TDD semantics while binding Evidence more
strictly to vNext Step, Action, and applicability rules.

### Candidate `methods/extension-law.md`

```markdown
# Extension Law

Use for every Adapt job and whenever Design, Review, Execute, or Recover touches
a Recipe, Profile, Adapter, Integration, Pattern, or Resource Bundle.

An adaptation must name one canonical owner, exact inputs and outputs, versioned
identity, compatibility, migration, removal, and proof. It may compose canonical
Maestro capabilities but cannot create private lifecycle, Recommendation,
authority, Evidence meaning, retry, scheduling, cursor, mutation, migration,
recovery, or state-store semantics.

Completion: every added capability and asset has one owner and explicit
disposition, every boundary maps to canonical Maestro contracts, and negative
proof shows no hidden alternate semantics.
```

Adapt is project-first. A consuming web, iOS, data, monorepo, CI/release, or
other project may add governed capabilities without forking Maestro core.

### Candidate `recipes/wayfinding.md`

```markdown
# Wayfinding Recipe

Use when a bounded destination is too large for one session and the route
contains material questions that cannot all be specified yet. For a small or
already-clear route, remain in ordinary Research or Design.

## Chart

1. Research and name the Destination, then bind it to candidate Contract scope
   through Design.
2. Record known sharp questions as Investigation Steps.
3. Keep unsharp in-scope uncertainty in the Unknowns Lens as fog.
4. Record excluded territory in Contract exclusions.
5. Let Projection derive the unblocked frontier.
6. Stop after charting; do not also resolve a frontier Step in the same Run.

## Continue

1. Read a fresh Packet; never select a frontier client-side.
2. Acquire only the advertised Investigation Step through its authorized
   Action Request and StepAttempt/Lease boundary.
3. Resolve one Research, Prototype, Grilling, or prerequisite Step.
4. Record its Result and Evidence. A candidate answer is not a locked Decision.
5. Materialize newly sharp questions as separately authorized Steps; keep the
   rest as fog.
6. Read a fresh Packet for continuation.

## Tracker projection

An issue tracker may render the map, child Steps, dependencies, assignees, and
frontier. Tracker ids and assignees are non-bearer provenance only. Closing,
assigning, or editing a projected issue grants no Lease, authority, Evidence
applicability, Decision authority, or right to mutate current Step state.

Completion: the destination has no material unresolved question or fog, and a
design finalization candidate can be produced. This is not build approval.
```

Wayfinding detection may be automatic and read-only. Creating a Wayfinding Work,
Investigation Steps, tracker projections, or resolutions requires the matching
authorized Action Request. One Wayfinding Run resolves at most one
Investigation Step; independent frontier Steps may run in separate concurrent
Runs.

## Settled Agent command-discovery policy

ER-4 is effective and supersedes both the original generated Agent-facing
`reference/cli.md` candidate and ER-3's Tool activation/call design. The target
Skill carries no command catalog. It uses a fresh `maestro_packet` for
Recommendation and `maestro_cli_search` only when the installed CLI capability
must be discovered. The Agent executes through the shell-visible CLI. Generated
human help remains optional output from the same registry, not a shipped Skill
dependency or authority source.

## Settled thin `.maestro/MAESTRO.md`

```markdown
# Maestro

This repository is governed by Maestro.

Canonical records own state. Projection alone recommends the next action.
Skills, Recipes, CLI, MCP, hooks, and trackers own no lifecycle or authority.

## Work

- Use `maestro_packet` for what to do next.
- Use `maestro_cli_search` only when the installed CLI command is unknown.
- Execute governed Operations through the shell-visible Maestro CLI.

Use only exact ids, schemas, and argv returned by the current binary. Never
infer authority, approval, success, or current state from prose or chat.

If the Packet, CLI, or installation is missing, stale, or incompatible, stop.
```

The file intentionally carries no frontmatter identity. Its exact bytes and
target are verified by the Resource Bundle Manifest and Installation Receipt.
The root `AGENTS.md` managed block owns the one instruction to read this file;
the Skill does not repeat it.

## Handoff rule

When this side reconciliation is complete:

1. re-read the live canonical design and Decision closure;
2. remove all overlap with effective locked Decisions;
3. classify every side clause as already locked, consistent explanatory
   expansion, genuine conflict requiring supersession, new candidate
   architecture, or implementation-order only;
4. provide exact resource, capability, migration, proof, and removal tables to
   the main conductor;
5. let the main conductor perform every canonical Decision and design write;
   and
6. require the regenerated Final Build Approval Packet before implementation.

## Change log

- 2026-07-13: Settled side choice ER-15 under the clarified post-main boundary:
  retained exactly one public `maestro` Skill with seven internal progressively
  disclosed jobs, preserved every unaffected `2ab8` safety and migration law,
  and required a future canonical superseding Decision rather than aligning the
  side candidate to the newly locked seven-Skill topology.
- 2026-07-13: Ran the live ER-1 through ER-14 edge sweep through a stable
  172-Decision checkpoint
  and the fully materialized `2ab8` skill-surface lock; recorded exact live
  hashes, the still-failing Pre-Main Evidence Relay gate, clause-by-clause
  classifications, six reopened material choices, and non-material corrections
  for Repository/Installation domains, Operation wording, snapshot holds,
  pruning, and bounded installation scanning.
- 2026-07-13: Settled side choice ER-14: required linked Release Resource and
  per-Realm Installation censuses, closed their source, consumer, host,
  project, legacy, cache, snapshot, archive, and removal coverage, prohibited
  unbounded home/disk scanning and false global-absence claims, and retained the
  current 4,816-location count as historical evidence pending closure proof.
- 2026-07-13: Settled side choice ER-13: unified Install, Update, Repair,
  Migrate, Rollback, and Uninstall under one expected-state-fenced Distribution
  transaction from inspection through post-Receipt pruning, closed crash and
  partial-activation recovery, and separated per-Realm uninstall from
  release-level resource removal.
- 2026-07-13: Settled side choice ER-12: added Setup as the seventh
  progressively disclosed job inside the single `maestro` Skill, rewrote the
  v1 Setup Skill into that branch, and made the exact two-Tool MCP registration
  global-only so supported hosts autoload it while projects install no private
  MCP server or descriptors.
- 2026-07-13: Settled side choice ER-11: adopted the layered binary, User
  Agent, project, worktree, cache, and rollback topology; retained one active
  `~/.maestro/skills/maestro` materialization activated through
  `~/.agents/skills/maestro` for Codex/compatible CLIs and
  `~/.claude/skills/maestro` for Claude; prohibited a new Codex-only root; and
  bound both host paths to one atomic Receipt and rollback Realm.
- 2026-07-13: Settled side choice ER-10: explicit authorized mutations replace
  recognized Maestro-owned resources, shared files remain managed-block only,
  foreign or ambiguous targets fail closed, and each Installation Realm retains
  exactly the two most recent successfully active prior snapshots for exact
  rollback, with pruning only after committed activation.
- 2026-07-13: Settled side choice ER-9: created the vNext SharedContract role
  for public machine boundaries, moved all 33 legacy schema descriptors and 22
  fixtures to exact Migration/proof-only treatment, retained retired names as
  non-reuse input, and removed schema instruction mirrors from product Bundles.
- 2026-07-13: Settled side choice ER-8: retained the one first-party neutral
  design baseline in the Release and moved the 74 vendored brand references,
  exact upstream manifest, and MIT license into one optional ExternalPattern
  Bundle with explicit install and single-pattern progressive disclosure.
- 2026-07-13: Settled side choice ER-7: vNext activates exactly one public
  `maestro` Skill; mapped all seven v1 Skills into explicit rewrite, split,
  replace, migration, or removal destinations; prohibited active aliases; and
  added identity-sensitive modified-copy, activation, and rollback treatment.
- 2026-07-13: Settled side choice ER-6 as the project-first public job
  vocabulary `Research / Design / Review / Execute / Recover / Adapt`; removed
  Advance in favor of Packet-owned continuation, made Adapt apply to consuming
  projects rather than Maestro core alone, and added mandatory Extension Law
  guidance for governed extension artifacts.
- 2026-07-13: Settled side choice ER-5: renamed the public bootstrap to
  `.maestro/MAESTRO.md`, removed Agent-facing identity frontmatter, made the root
  `AGENTS.md` block the sole owner of the read pointer, kept the `maestro` Skill
  free of that duplicate instruction, and retained compact purpose/trigger/hard-
  boundary capsules for DDD, TDD, and Wayfinding.
- 2026-07-13: Settled side choice ER-4, superseding ER-3's effective discovery
  and execution path: the complete public Maestro MCP surface is exactly the
  read-only `maestro_packet` and `maestro_cli_search`; Agents execute work
  through the exact running CLI surface, all individual lifecycle MCP Tools and
  generic/dynamic Tool-call paths are removed, and Agent-facing `cli.md` remains
  removed.
- 2026-07-13: Settled side choice ER-3 as a portable three-Tool bootstrap
  kernel (`maestro_packet`, `maestro_tool_search`, `maestro_tool_call`), dynamic
  typed-Tool discovery where supported, schema-validating fixed-host fallback,
  exact host installation proof, and removal of Agent-facing `cli.md` from the
  target Skill path while retaining CLI as a fallback Adapter.
- 2026-07-13: Applied `writing-great-skills` and `write-a-skill`; recorded the
  refined one-router/six-disclosed-branches candidate, invocation-load findings,
  information-hierarchy rules, completion-criterion requirement, and script
  boundary.
- 2026-07-13: Settled side choice ER-2 as one release-rooted, closed typed
  Bundle hierarchy with exact dependency identities and a seven-kind Bundle
  union; added the candidate packaging map for all current Embedded families.
- 2026-07-13: Settled side choice ER-1 as two-level Resource plus Bundle
  identity with installed state in a separate Installation Receipt.
- 2026-07-13: Created the side-conversation workbench with live baseline,
  completeness-gate status, Embedded inventory, imported main constraints,
  target journeys, reconciliation workstreams, initial Resource Bundle
  recommendation, open questions, and handoff boundary.
