# Final Main Input: Agent Jobs, Loop Recipes, and Remaining Reconciliation

Status: post-main design input, non-authoritative  
Owner: main conductor  
Boundary: design only; this file does not lock a Decision, change acceptance,
authorize build, or approve implementation.

## Why this input exists

The post-main design correctly selects one public `maestro` Skill with seven
progressively disclosed internal jobs:

```text
Setup | Research | Design | Review | Execute | Recover | Adapt
```

The remaining reconciliation must cover not only the Skill packages but also
the Loop Recipe system that currently routes and connects them. The final
canonical design must make Research -> Design -> approved Execute feel like one
continuous Maestro experience without turning a Skill or Recipe into a second
lifecycle, authority source, scheduler, cursor, or next-action engine.

## Critical contradiction requiring a main Decision

The canonical design says both:

- the exact shipped Recipe census contains the 15 current Recipes; and
- Wayfinding is a Recipe.

The current 15-Recipe census is:

```text
adversarial-review
audit
conflict-handoff
design-relay
design
feature-fanout
generate-filter
intake-triage
learning
loop-until-done
progress
ship
synthesize
unattended
work
```

`wayfinding` is not in that set. Main must open one material fork and discuss it
with the user before the Final Build Approval Packet:

1. add Wayfinding as a sixteenth Recipe;
2. replace one genuinely overlapping current Recipe with Wayfinding while
   preserving all non-overlapping behavior; or
3. supersede the locked statement that Wayfinding is a Recipe.

Do not silently keep “15 Recipes” while describing Wayfinding as an additional
Recipe. Do not demote Wayfinding to an ungoverned Skill procedure merely to
preserve the count.

## Required Job versus Recipe law

Jobs and Recipes are different axes and must not be forced into a one-to-one
mapping.

- A **job** is an Integration-owned, progressively disclosed instruction branch
  describing the user's current purpose.
- A **Recipe** is an Orchestration-owned, versioned restrictive procedure over
  canonical Packet, Operation, Result, Evidence, and Gate semantics.
- One job may use several Recipes.
- One Recipe may be usable from several jobs.
- A Recipe may tighten an eligible frontier or add hard stops. It cannot create
  eligibility, authority, lifecycle transitions, retry rights, or a second
  Recommendation.
- The seven jobs are not seven lifecycle phases. The existing six-phase Recipe
  grammar is a separate orchestration profile.

The final design needs an exact many-to-many Job/Recipe relation with entry
conditions, permitted return jobs, hard stops, and proof. Recipe selection must
never become client-side next-action authority.

## Candidate mapping for main review

This is evidence for the fork, not a locked final mapping.

| Current Recipe | Candidate job entry | Reconciliation concern |
| --- | --- | --- |
| `adversarial-review` | Review | Preserve independent refutation; remove legacy Task/proof mutation wording. |
| `audit` | Review | Default read-only; findings cannot silently become implementation. |
| `conflict-handoff` | Execute, Recover | Coordination evidence only; no Recipe-owned ownership, blocker, Lease, merge, or authority. |
| `design-relay` | Design | Advisor output remains evidence only; no user consent or Decision authority. |
| `design` | Design | Research freshness and approval boundaries remain external canonical gates. |
| `feature-fanout` | Design, Execute | Pressure-test overlap with Wayfinding and canonical Step DAG/Projection. |
| `generate-filter` | Design, Review, Adapt | Generation and judging are advisory; only Decision/Contract owners materialize results. |
| `intake-triage` | Research, Design | Intake remains non-authoritative and cannot publish Contract scope. |
| `learning` | Review, Adapt | Memory is advisory; no silent Skill, Recipe, Harness, or policy mutation. |
| `loop-until-done` | Execute, Recover | Bounded client continuation only; no daemon, queue, cursor, retry engine, or worker launcher. |
| `progress` | Execute | Rewrite Card/Task/Progress vocabulary into Work/Step/Projection semantics. |
| `ship` | Execute, Review | External effects require exact authority and Evidence; Review cannot self-approve Execute. |
| `synthesize` | Execute, Recover | Merge/worktree activity remains external effect handling, not Recipe-owned state. |
| `unattended` | Execute, Recover | Projection selects each next action; the Recipe only restricts operating limits. |
| `work` | Execute | Rewrite accepted Task/Card flow into exact Work/Step/Attempt/Submission/Gate behavior. |
| candidate `wayfinding` | Research, Design | Bounded Destination, Investigation Steps, fog/unknowns, one Step per Run; no build approval or execution authority. |

Main must give every current Recipe one final disposition: retain, rewrite,
replace, migration-only, external pattern, or remove. If Wayfinding replaces a
Recipe, the replaced Recipe's unique behavior and every consumer must be mapped
explicitly.

## Smooth cross-job transition contract

The design currently proves job names and lazy loading but does not yet freeze
how an Agent moves between jobs. There is no exact `recommended_job`, job-route,
or job-handoff contract in the canonical design.

Recommended ownership:

1. Projection remains the sole owner of the recommended canonical next action.
2. Integration applies one total deterministic mapping from the fresh Packet's
   recommended Operation, refusal, blockers, and context to one internal job.
3. The mapping chooses only which instruction Resource to load. It grants no
   authority and cannot alter the Packet recommendation.
4. At every job boundary the Agent re-reads a fresh Packet and passes exact
   artifact identities, not chat memory.
5. `maestro_cli_search` discovers the CLI rendering only after the canonical
   recommendation/job route is known. It never chooses the job.
6. If the mapping is missing, ambiguous, incompatible, or stale, routing fails
   closed and returns typed recovery guidance.

The final design must freeze the entry, completion, refusal, and return contract
for all seven jobs. At minimum, prove this journey:

```text
Research
  -> current Research artifact and Evidence
  -> fresh Packet
Design
  -> locked Decisions, candidate Contract Components, Build Handoff
  -> explicit exact build approval
  -> fresh Packet
Execute
```

`Review` may be entered from Design or Execute. `Recover` may interrupt any job
but returns only through a fresh Packet. `Adapt` creates or materially revises a
reusable extension; ordinary use of an existing adaptation stays with Setup,
Review, Execute, or Recover as applicable.

Do not persist an active Skill job as canonical lifecycle state. A job route is
an ephemeral, reproducible Integration rendering over current canonical facts.

## Recipe schema semantics that must be rewritten

The existing Recipe YAML files contain fields that can accidentally preserve
v1 authority or scheduler behavior:

- `router.status`, `router.priority`, and `router.confidence`;
- `transitions` and `invocations`;
- `authority_scope` and `autonomy` prose;
- `allowed_verbs` containing legacy CLI strings;
- `progress_tasks` and phase outputs;
- Card, Feature, Task, Progress, proof, witness, close, archive, ready, next,
  Work Lease, and other v1 names.

The vNext disposition must be exact:

- Recipe priority cannot compete with Projection Recommendation.
- Recipe transitions cannot mutate Work/Step lifecycle or switch jobs by
  themselves.
- `authority_scope` is a restriction/requirement declaration, never a Grant or
  Mandate.
- `allowed_verbs` cannot define Operations through prose CLI strings; retained
  actions reference exact canonical Action/Ceremony identities and the CLI is
  only their rendering.
- `progress_tasks` cannot become a hidden queue, cursor, Step store, or
  completion authority.
- `autonomy` expresses operating limits only and cannot authorize external
  effects or retries.
- Hard stops may tighten canonical policy but cannot weaken it.
- Every v1 Recipe byte, profile, schema v2/v3 representation, resolver marker,
  test, installed copy, cache, and consumer receives migration/removal treatment.

The `maestro.standard-six-phase.v1` successor must explicitly preserve or
replace the six-phase grammar independently of the seven-job vocabulary.

## Other forgotten surfaces beside Loop Recipes

### 1. Nested Skill Resource disposition

The current seven Skill packages include many nested references: DDD, TDD and
its deep modules, domain modeling, grilling, PRD, work, verify, QA baseline,
QA slice, feature, intake, simplify, architecture review, examples, and one
`reference/cli.md` per package. The final Resource census must map every file
exactly once into:

- `jobs/<job>.md`;
- governed methods such as DDD, TDD, and Extension Law;
- an Orchestration Recipe reference;
- a migration/proof-only fixture; or
- removal.

All Agent-facing `reference/cli.md` files are removed from active resources and
retained only as migration/proof evidence where required.

### 2. Skill activation events and observability

Current Skills emit activation events using old public names such as
`--skill maestro-audit` or `--skill maestro-card`. The single-Skill target needs
an exact migration and observability contract, for example public
`skill = maestro` plus a bounded non-bearer internal job identity. Job and Recipe
telemetry cannot become authority, lifecycle, liveness, or billing truth.
Old activation names must not remain active aliases.

### 3. Review mode separation

`maestro-audit` and `maestro-witness` both feed the new Review job, but they have
different safety contracts. The final design must preserve separate Review
modes or exact procedures:

- ordinary audit/refutation is read-only and produces findings/Evidence;
- close review checks acceptance, proof, QA, risk, and independence;
- Review never self-authorizes close or implementation;
- failed close review routes to Design, Execute, or Recover through a fresh
  Packet;
- advisor output remains evidence and never impersonates human authority.

### 4. Context-budget and lazy-loading proof

One mega Skill is safe only if the router stays small. Freeze a bounded root
resource and prove that only the selected job, selected method, and selected
Recipe references load. Add positive and negative routing fixtures, maximum
root-context measurement, no eager loading of all seven jobs, and no duplicated
bootstrap/CLI catalog text.

### 5. Resume, interruption, and recovery UX

Cross-session continuation must reconstruct from canonical artifacts and a
fresh Packet. It must not depend on remembered job, chat summary, Recipe cursor,
`STATE.md`, or `LOOP.md`. Define behavior for interruption before job output,
after output but before Result publication, stale handoff, rejected submission,
partial external effect, and `in_doubt` reconciliation.

### 6. Human, CLI, TUI, shell, and MCP parity

The seven jobs are Agent UX labels, not seven new domain commands. Decide where
humans can see them, if anywhere. CLI/TUI may render the same route and reason,
but no adapter may independently choose a job or next action. MCP remains
exactly the two read-only global Tools; projects install zero MCP.

### 7. Setup, Recover, and Adapt coverage

Do not automatically invent one Recipe per job. Explicitly decide whether these
jobs need a dedicated Recipe or operate directly from Packet plus canonical
Operations. Whichever choice is made must prove complete journeys for clean
setup, adoption, update, repair, rollback, stale installation, project-specific
adaptation, extension upgrade/removal, and recovery from partial effects.

### 8. Installed-copy and removal census

Removal must cover old Skill directories and names, installed copies, links,
caches, backups, hook event consumers, docs, tests, generated references,
Recipe resolvers, host configurations, TUI labels, shell helpers, and retained
old binaries. Retention is explicit; no active runtime alias translates old
Skill or Recipe names.

## Required acceptance/proof additions

Before regenerating the Final Build Approval Packet, prove:

1. exact one public Skill and seven internal jobs;
2. one resolved exact Recipe count including the Wayfinding disposition;
3. every current Recipe and nested Skill Resource classified exactly once;
4. a total deterministic Packet-to-job instruction mapping with ambiguity
   refusal;
5. Research -> Design -> exact approval -> Execute continuity without chat
   memory or hidden state;
6. Review and Recover return only through fresh Projection;
7. Recipe schema fields cannot own authority, lifecycle, next action, retries,
   scheduling, cursor state, mutation, or recovery;
8. old Skill activation/event names and Recipe/CLI vocabulary are migration-only
   or removed;
9. context-budget and lazy-load bounds pass on every supported Agent host;
10. CLI, JSON, two-MCP, TUI, hook, shell, Skill, and Recipe parity holds.

## Main conductor action

Treat this file as new post-main evidence. Re-read live artifacts, deduplicate
anything already settled, and open one material Decision at a time for genuine
choices. At minimum, the Wayfinding/Recipe-count contradiction and the exact
Packet-to-job routing ownership require explicit resolution. Repair mechanical
census, wording, migration, and proof gaps directly.

Do not produce the Final Build Approval Packet until these points are resolved
and the complete edge sweep is clean. Discuss any irreversible public Recipe
name/count or job-routing UX choice with the user before locking it.

## Live canonical coverage audit

Audit basis at 2026-07-13 09:40 +07:

```text
feature: maestro-whole-flow-architecture-refoundation
state: proposed
tasks: 0
decisions: 179 total / 129 locked / 50 superseded / 0 open
design sha256: 2191b03d207ef2790e83c50c54fb0cbf582146fef5ad46763a368b28d6b1b8c1
decisions sha256: e80eaf1c279a02da58edf573f6890cf7df251e1d00a11f480aea0b0e06c191fc
```

The main thread reports that it has closed the literal Resource-contract fork
and is in top-down recomposition/final-advisor closeout. This new evidence means
the claim “all material forks are closed” must be re-evaluated before the final
packet.

| Concern in this file | Live coverage | Evidence and required disposition |
| --- | --- | --- |
| One public Skill and seven internal jobs | covered and locked | Effective 7305; design defines exact 1/7 topology, lazy selected-branch loading, no old aliases, routing and host-parity proof. |
| Job is not lifecycle/authority/Recipe owner | covered | Design defines Internal Job as an Integration branch and gives Orchestration sole Recipe ownership. |
| Jobs and Recipes are many-to-many | partial | Ownership separation exists, but no exact Job/Recipe relation, eligible entry jobs, return jobs, ambiguity law, or exhaustive mapping exists. |
| Exact current Recipe census | covered for v1 only | Design enumerates the 15 current Recipes and one profile exactly. |
| Wayfinding disposition and final Recipe count | contradictory | Design says Wayfinding is a Recipe while the exact 15-member Recipe census omits it. No Decision resolves add/replace/supersede. |
| Smooth Research -> Design -> approval -> Execute routing | partial | Build proof mentions `research-to-design`, execution and recovery journeys, but no normative transition/mapping contract defines how the one Skill chooses the next job. |
| Packet-to-job routing owner and refusal | missing | No `recommended_job`, job-route, job-handoff or equivalent total deterministic mapping is defined. Main must decide whether Integration derives an instruction branch from the exact Projection recommendation or another shape preserves Projection sole-next authority. |
| Recipe schema authority boundary | substantially covered | Design makes Recipes restrictive/advisory, migration-only for current v2/v3 bytes and unable to create actions, authority, workers, retries, lifecycle or next action. |
| Field-level Recipe rewrite | partial | The design does not explicitly dispose `router.priority/status`, `transitions`, `invocations`, `authority_scope`, `autonomy`, prose `allowed_verbs`, or `progress_tasks`, nor prove that legacy commands are removed from active vNext Recipes. |
| Six-phase profile versus seven jobs | partial | Current profile is inventoried and Recipe phases are non-lifecycle, but the final successor/retention law does not explicitly state that six phases and seven jobs are independent axes. |
| Per-Recipe job mapping and final disposition | missing | The 15 Recipes are retained as one group. None has an exact new-job entry/return mapping or individual retain/rewrite/replace/remove disposition. |
| Nested Skill Resource inventory | partial | Design preserves exact v1 Skill/reference bytes and requires one Capability Bundle, seven job resources and governed method/Recipe references, but it does not map each current nested file to an exact new Resource or removal. |
| Agent-facing `reference/cli.md` removal | covered at policy level | Two-MCP/CLI correction removes active Agent `cli.md`; exact per-file removal/migration rows still belong in the Resource census. |
| DDD, TDD, Extension Law | covered at semantic level | DDD/TDD are methods, Extension Law is hard boundary. Exact Resource paths/bytes remain implementation-contract material. |
| Skill activation events after single-Skill cutover | missing | Current event census includes SkillActivation/skill_activation and old Skills emit their public names; no exact `maestro` plus internal-job observability migration is defined. |
| Review audit versus witness behavior | partial | Review and witness-review proof are named, and advisor authority is generally constrained, but there is no exact internal Review-mode split preserving read-only audit versus independent close review. |
| Context budget and lazy loading | covered as proof, not literal budget | Design requires unrelated branches unloaded, host routing/context measurements and no duplicate semantics. A numeric/fixture budget may remain implementation-stage proof if it cannot alter public behavior. |
| Resume/interruption across jobs | partial | Canonical fresh Packet, stale-state and recovery laws exist; job-specific resume behavior and prohibition on persisted job/Recipe cursor are not materialized as one journey contract. |
| Setup/Recover/Adapt Recipe coverage | missing choice | The jobs exist, but design does not state whether they need dedicated Recipes or operate from Packet plus Operations. Avoid implicitly adding one Recipe per job. |
| CLI/JSON/two-MCP/TUI/hook/shell parity | covered broadly | Exact two-MCP and CLI execution surface plus adapter parity are locked. Job-rendering parity still depends on the missing route mapping. |
| Installed copies and legacy removal | covered broadly | Old Skill names/copies, Agent `cli.md`, host/project MCP, caches and consumers have migration/removal laws; Recipe/job-specific consumer closure still needs the new mapping. |

### Audit conclusion

The canonical design covers most constitutional boundaries, but it does not yet
cover the complete Agent-job/Recipe UX. Two issues are material and should
reopen the design before final approval:

1. resolve the public Wayfinding Recipe identity/count contradiction; and
2. freeze ownership and behavior of the smooth Packet-to-internal-job route.

The exact Job/Recipe matrix, Recipe field dispositions, activation-event
migration, nested Resource mapping, Review-mode split and job-specific journeys
may be resolved as children of those Decisions when tightly dependent; otherwise
repair them as separate sequential forks or proof/census material. They must not
remain implicit.

## Settled brainstorm decision FB-1: continuation and relay Recipes

User selection: locked for the side-brainstorm handoff. This is not yet a
canonical Maestro Decision and must be materialized by the main conductor only
after the consolidated brainstorm handoff.

The earlier candidate classification in this file that placed `design-relay`,
`unattended`, and `loop-until-done` together as external patterns is superseded
for the handoff by this selection:

1. Retain `Design Relay` as one distinct Orchestration-owned Recipe entered
   from the Design job. It consumes an exact bounded mandate; advisors and
   subagents provide Evidence only; the main conductor remains the sole
   Decision writer; and every relay result returns to Design through a fresh
   Packet. The Recipe grants no authority and stops at mandate, confidence,
   security, migration-loss, public-UX, acceptance, or scope boundaries.
2. Replace `unattended` and `loop-until-done` with one
   `Bounded Continuation` Recipe and two exact profiles:
   - `attended`: the user is available for a material choice or hard stop;
   - `unattended`: no mid-run user interaction is assumed, operating limits are
     stricter, and ambiguity or missing authority stops rather than guesses.
3. Every continuation tick is exactly:
   `fresh Packet -> one Projection-recommended Operation -> Result/Evidence ->
   fresh Packet`.
4. Bounded Continuation owns no cursor, queue, scheduler, worker launcher,
   retry engine, next-action choice, lifecycle, authority, or hidden state.
   State reconstructs from canonical artifacts. If the external Agent or
   conductor stops invoking ticks, continuation stops.
5. Profiles may only tighten cadence, attempts, time, cost, subagent count,
   connector permissions, denylist and hard stops. They cannot widen the Packet
   frontier, authority, effect permission, retry rights or completion meaning.
6. v1 `loop-until-done` maps to the attended profile and v1 `unattended` maps
   to the unattended profile as byte-preserved migration provenance. Neither
   old name remains an active post-cutover alias unless the later public naming
   fork deliberately selects one as the canonical new name.

The working target Recipe family after FB-1 is:

```text
Wayfinding
Adversarial Review
Generate & Filter
Fanout
Conflict Handoff
Synthesize
Ship
Learning
Design Relay
Bounded Continuation
  - attended profile
  - unattended profile
```

The exact final public Recipe names and the remaining current-Recipe
dispositions are separate downstream forks. FB-1 locks the semantic merge and
boundaries, not spelling or a final count independent of those dispositions.

## Settled brainstorm decision FB-2: generic-job Recipe absorption

User selection: locked for the side-brainstorm handoff. This refines the earlier
candidate mapping in this file after direct review of the five current Recipe
definitions.

### Absorb or remove

- `design` is absorbed into the Design job. Its one-fork-at-a-time discipline,
  Decision supersession, contradiction sweep, acceptance coverage and explicit
  build-approval boundary survive as Design-job behavior. Its Recipe-local
  router priority, lifecycle transition and Feature/Card command vocabulary do
  not survive. Specialized Design orchestration uses Wayfinding, Generate &
  Filter or Design Relay.
- `work` is absorbed into the Execute job. Smallest-correct-change discipline,
  TDD, scoped mutation, Evidence and verification survive. Task/Card lifecycle,
  Recipe-local claiming, direct Design/Audit transitions and generic work-loop
  authority do not survive. Design, Review or Recover re-entry occurs only
  through a fresh Packet.
- `progress` is removed as a Recipe and as a separate work model. Low-ceremony
  work uses the same canonical Work/Step model with one or a few Steps and the
  Execute job. `Progress Card`, `progress.yml`, active-task cursor and
  checklist-owned completion become migration/proof-only inputs or removal
  targets. Low ceremony is an authoring UX, not another lifecycle.

### Retain and rewrite

- `audit` remains an Orchestration-owned `Audit` Recipe entered through the
  Review job. It owns the bounded multi-probe procedure, evidence-backed
  finding classification and dedupe, but remains read-only by default. A
  finding cannot create or mutate Work directly; remediation requires a fresh
  Packet and separately authorized Action Request. Audit owns no transition to
  Execute and is distinct from independent close/witness review.
- `intake-triage` remains an Orchestration-owned `Intake Triage` Recipe entered
  from the appropriate Research or Review route. It preserves the untrusted
  input boundary, total item coverage, read-only classification, canonical
  dedupe and duplicate/new/escalate disposition. Classifier output is
  non-authoritative. Admitting candidate Work requires a fresh typed authorized
  Action Request; raw text never drives a command or lifecycle transition.

The resulting working Recipe family after FB-1 and FB-2 is:

```text
Wayfinding
Adversarial Review
Audit
Generate & Filter
Fanout
Conflict Handoff
Synthesize
Ship
Learning
Intake Triage
Design Relay
Bounded Continuation
  - attended profile
  - unattended profile
```

This is a working semantic set of twelve Recipes plus two profiles. The final
count remains subject to review of the remaining current Recipes and exact
public naming; no old Recipe name gains an automatic runtime alias.

## Settled brainstorm decision FB-3: progressive Skill tree and linked QA methods

User selection: locked for the side-brainstorm handoff. The one public Skill is
a capability-complete Resource tree on disk, not one eagerly loaded mega
prompt.

### Logical Skill Resource structure

```text
skills/maestro/
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
    domain-model.md
    grilling.md
    tdd.md
    tdd/interface-design.md
    tdd/mocking.md
    tdd/refactoring.md
    tdd/test-design.md
    qa-baseline.md
    qa-replay.md
    close-review.md
    verification.md
    simplify.md
    extension-law.md
```

Orchestration owns Recipe Resources separately from the Skill tree. The Skill
references exact Recipe identities and never owns or copies Recipe bytes.
`~/.maestro/skills/maestro` remains the one active Skill materialization, with
the approved host activation paths resolving to that same identity.

The root `SKILL.md` remains bounded. It contains only the seven-job purpose and
trigger capsules, exact route/refusal boundary, Packet-first rule,
`maestro_cli_search` discovery rule, CLI execution boundary and completion
contract. It contains no `.maestro/MAESTRO.md` read-pointer duplication, CLI
catalog, identity hashes, Recipe body, private lifecycle or authority law.

Only the selected `jobs/<job>.md`, selected methods and selected exact Recipe
references load. Unrelated job branches remain unloaded. Context measurement,
positive/negative routing fixtures and no-eager-load proof are mandatory.

### Preserve the two-pass QA model

QA is not an eighth job, separate lifecycle, Store, authority source or Recipe.
It is two linked methods across existing jobs:

```text
Design + QA Baseline
  -> exact build approval
Execute + optional TDD
  -> Result/Evidence
Review + QA Replay
  -> Gate derivation
```

`QA Baseline` runs during Design before Execute authorization. It creates stable
Scenario Specs covering actor/trust boundary, setup, action or journey,
observable oracle, required Evidence, related acceptance/Gate and recovery
behavior. Every Scenario Spec binds to the exact candidate Contract Root.

A genuinely non-behavioral scope uses a typed, reasoned NotApplicable
Assessment bound to the exact Contract Root, not an ambient waiver. A later
behavioral amendment invalidates it.

`QA Replay` runs from Review after Execute against the exact current Contract
Root and artifact/Run/environment. It replays affected Scenario Specs and emits
source-qualified Observations plus pass, fail, indeterminate or error
Assessments. A stale baseline, changed Contract Root, mismatched artifact or
missing Evidence refuses applicability. Gate alone derives satisfaction.

TDD remains an Execute method for Step-level RED/GREEN development Evidence.
QA Baseline and Replay remain acceptance-level real-journey protection. Neither
substitutes for the other.

A failing QA Replay cannot be repaired inside Review. Review reads a fresh
Packet and returns to Design, Execute or Recover. Review cannot self-authorize
implementation, close or ship; Ship consumes the resulting Gate state but does
not own QA meaning.

### Resource and migration implications

Every current nested Skill reference receives one exact target disposition into
a job Resource, method Resource, Recipe reference, migration/proof-only fixture
or removal. Current `qa-baseline.md` and `qa-slice.md` are rewritten into the
linked `qa-baseline.md` and `qa-replay.md` methods with byte-preserved migration
provenance. All Agent-facing `reference/cli.md` files remain inactive
migration/proof inputs only.

## Settled brainstorm decision FB-4: Integration-owned JobRouteV1

User selection: Option C, locked for the side-brainstorm handoff.

`JobRouteV1` is a non-authoritative Integration projection that selects exactly
which internal instruction Resource to load. It is not canonical lifecycle
state, a Projection replacement, a persisted active-job cursor, an authority
record, an Operation selector or a Recipe transition.

```text
JobRouteStatusV1 = Selected | Ambiguous | Blocked
InternalJobV1 = Setup | Research | Design | Review | Execute | Recover | Adapt
JobRouteBasisV1 = Bootstrap | ExplicitRequest | PacketReason | RecoveryState
```

One route binds status, optional exact job, basis, optional fresh Packet
identity, closed reason code and exact job/method Resource identities. Selected
means one job only. Ambiguous or Blocked loads no mutation-capable job and
returns bounded guidance.

Routing order is:

1. Missing, stale or incompatible installation/bootstrap routes only to Setup;
   this is the sole permitted pre-Packet entry exception.
2. `in_doubt`, stale Submission, interrupted effect or durable
   `recovery_required` routes to Recover and cannot be overridden by an Execute
   request.
3. An explicit read-only Research or Review request may select that instruction
   branch without changing the canonical Packet recommendation.
4. Otherwise Integration applies one closed total mapping from exact Packet
   reason codes to one job, including setup-required, context-unknown,
   design-required, review-required, step-runnable, recovery-required and
   extension-required families.
5. Missing, conflicting or non-total mapping returns Ambiguous or Blocked. The
   router does not eagerly load both branches or guess a mutation path.

Projection remains the sole next-action authority. JobRoute may only render the
current recommendation into an instruction branch and cannot widen, replace or
reorder the advertised frontier. `maestro_cli_search` runs only after the job
and canonical Operation are known and discovers rendering only.

The required journey proof is:

```text
Research
  -> fresh Packet/design-required
Design + QA Baseline
  -> approval-required refusal
exact build approval
  -> fresh Packet/step-runnable
Execute + optional TDD
  -> fresh Packet/review-required
Review + QA Replay
  -> fresh Packet/ship-ready or recovery-required
Execute + Ship | Recover
```

Research versus Review, Design versus Adapt, Setup versus Adapt, and Execute
versus Recover receive positive, negative and ambiguous routing fixtures. Job
telemetry is non-bearer observability only and cannot make a route current.

## Settled brainstorm decision FB-5: closed Review modes and auto review-repair routing

User selection: Option B plus the compound-intent refinement, locked for the
side-brainstorm handoff.

`ReviewModeV1` is the closed instruction-mode set:

```text
Inspect | Audit | AdversarialReview | QAReplay | CloseReview
```

The selected mode is an ephemeral Review instruction route, not lifecycle,
authority, Gate state or a persisted reviewer cursor.

- `Inspect` performs one bounded read-only inspection and emits findings or a
  no-finding result.
- `Audit` invokes the Audit Recipe for a bounded multi-lens sweep and
  evidence-backed candidate findings.
- `AdversarialReview` invokes the Adversarial Review Recipe to try to refute
  exact claims and emits a typed Assessment.
- `QAReplay` invokes the linked QA Replay method against exact Scenario Specs,
  Contract Root, artifact/Run and environment.
- `CloseReview` replaces the valid behavior of v1 witness review: it checks
  acceptance mapping, Evidence, QA, risk requirements, exact artifact refs and
  reviewer independence, then emits pass, fail, indeterminate or error
  Assessment. It does not emit bearer approval or close Work.

Gate derives satisfaction from a current applicable Assessment. A passing
CloseReview may cause a fresh Packet to recommend a close/ship Operation, but
Review cannot perform or authorize that transition. Contract/Gate policy owns
risk tier, human-demo, expert and independence requirements; Review cannot
weaken them.

### Compound `review and fix` intent

The root Skill and `JobRouteV1` recognize a closed non-authoritative compound
intent such as `ReviewRepairRecheck`. They still select exactly one job per
tick and begin with least-authority read-only Review.

```text
Review
  -> Evidence-backed findings
  -> fresh Packet
  -> no finding: stop
  -> in-scope executable finding: Execute
  -> contract/acceptance defect: Design
  -> stale/interrupted/in_doubt: Recover
  -> missing authority or ambiguity: Blocked
Execute or Recover
  -> Result/Evidence
  -> fresh Packet
Review recheck
```

Bounded Continuation may drive these ticks in attended or unattended profile,
but the continuation hint is guidance only. It holds no cursor, queue,
authority, pre-approved job list or pre-authorized batch. Each mutation requires
the fresh Packet and its own typed authorized Action Request.

The root Skill carries a compact rule for compound intents: start read-only,
record findings, reproject, route each actionable finding separately, re-enter
Review after mutation, and stop on scope, authority, independence or ambiguity.

If the final Gate requires independence, the worker that repaired the finding
cannot satisfy independent CloseReview. Reviewer identity and exact reviewed
inputs bind into the Assessment provenance. Review never silently fixes and
self-approves in one authority step.

## Settled brainstorm decision FB-6: closed typed vNext Recipe manifest

User selection: Option B, locked for the side-brainstorm handoff.

vNext Recipes use one closed typed `RecipeManifestV1` under the already selected
ManifestIdentityV1 protocol. Recipe source remains declarative and portable;
unknown fields, unknown enum members, unresolved identities and semantic cycles
fail closed.

The manifest binds at least:

```text
RecipeManifestV1
  recipe_identity
  semantic_version
  profile_identity
  eligible_internal_jobs
  purpose_resource
  exact trigger reason-code set
  exact required input-contract identities
  restrictive Operation-filter identities
  six fixed phase-guidance resources
  required Observation/Assessment classes
  hard-stop predicates
  completion predicates
  return reason-code set
  operating-limit profile identities
  exact Resource dependencies
  migration/removal/proof identities
```

The standard phase grammar remains exactly:

```text
Perceive -> Choose -> Act -> Observe -> Learn -> Continue
```

These phases are instructional orchestration positions, not Work/Step states,
a persisted Recipe cursor, completion authority or a seven-job mapping. A
session reconstructs its position from fresh canonical artifacts and Packet
facts. Phase guidance cannot define a new Operation.

Recipe evaluation is pure and produces only one of:

```text
NotApplicable
RestrictiveAdvice
HardStop
```

RestrictiveAdvice may intersect the exact Packet-advertised frontier with a
closed allowset, add preconditions, order candidates only as input to canonical
Projection, select exact guidance Resources and tighten operating limits.
Projection remains the sole final Recommendation owner. A Recipe cannot add an
Operation absent from the Packet, widen authority, reinterpret Result/Evidence,
or emit an executable next action independently.

The following v1 fields or meanings do not survive as active semantics:

- `router.status`, `router.priority` and `router.confidence` as competing
  routing or Recommendation;
- lifecycle/job `transitions` and `invocations`;
- prose `authority_scope` as a Grant, Mandate or permission;
- prose CLI strings in `allowed_verbs` or `forbidden_verbs` as Operation
  definitions;
- `progress_tasks` as persisted Steps, queue, cursor or completion state;
- `autonomy` as mutation or external-effect authority;
- Recipe-local retries, workers, schedulers, stores, migration, recovery or
  Evidence meanings.

Required authority and applicability are exact references to canonical Packet,
Contract, Gate and Authority predicates. CLI spellings are generated only by
the running binary after the canonical Operation is selected. Hard stops and
operating-limit profiles may tighten but never weaken canonical policy.

The two Bounded Continuation profiles may restrict attempts, elapsed budget,
cost, subagent count, connector permissions, denylist and user-availability
behavior. They still evaluate one fresh Packet per tick and own no retry right
or cursor.

Migration preserves every v1 Recipe/profile/schema/resolver byte and consumer
with provenance, but activates no legacy phase, priority, transition, CLI verb,
authority, task, cursor or recommendation. Each retained Recipe is rewritten
into the closed manifest; each absorbed/replaced/removed Recipe has exact
consumer and behavior disposition. Proof includes deterministic identity,
unknown-field rejection, frontier-subset property, hard-stop monotonicity,
six-phase/job independence, cross-adapter parity and zero hidden state.

## Settled brainstorm decision FB-7: Wayfinding and Fanout remain separate

User selection: keep them separate because one plans/discovers and the other
executes. Locked for the side-brainstorm handoff.

- `Wayfinding` is a Research/Design Recipe for a bounded Destination whose
  material questions, prerequisites or fog are not yet fully known. It creates
  or sharpens Investigation Steps and Evidence, returns candidate design input,
  and cannot approve or execute the destination.
- `Fanout` is an Execute Recipe for an already accepted exact Contract whose
  fresh Projection exposes two or more independently runnable Steps. It creates
  no Steps, priority, dependency, authority or Recommendation.

Fanout may only restrict a current runnable frontier after proving no dependency
edge, shared mutable target, overlapping file/Store ownership, incompatible
external effect, Lease conflict or unisolated proof boundary. Every selected
Step retains its own StepAttempt, Lease fence, Result and Evidence.

Maestro and the Recipe do not launch workers. Fanout emits exact bounded worker
contracts/advice; an external Agent or conductor chooses whether to create
subagents/worktrees. Workers cannot close Work or mutate another worker's Step.

`feature-fanout` is replaced by canonical `Fanout` vocabulary using Work, Step,
Projection, Lease and StepAttempt. Its v1 Feature/Task/Card commands, router
priority and lifecycle transitions are migration-only. Parallel results return
to `Synthesize` or ordinary Review through fresh Packets; unresolved design
questions return to Design rather than being hidden inside execution lanes.

Required negative proof covers one-Step frontiers, unresolved material choices,
shared files or targets, dependency edges, same-effect domains, stale Packets,
Lease collision, worker self-close, hidden process launch and attempted
cross-worker authority.

## Settled brainstorm decision FB-8: Synthesize remains separate from Fanout

User selection: keep Synthesize as a separate Recipe, locked for the
side-brainstorm handoff. Exact public spelling (`Synthesize` versus a clearer
candidate such as `Integrate`) remains a later naming fork.

Fanout is the `1 -> N` execution split over independently runnable Steps.
Synthesize is the optional `N -> 1` integration procedure over returned lane
Results. Neither owns the other and neither implies that the other must run.

Synthesize begins only from exact non-stale lane handoffs/results. It verifies
Step/Contract binding, artifact or branch identity, base/head compatibility,
dependency and custody constraints, per-lane proof and integration order. An
external Agent/conductor performs Git or artifact integration one bounded lane
at a time; Maestro and the Recipe do not run a hidden merge process.

After each integration boundary, Synthesize records the exact integrated
artifact and Evidence, rechecks currentness and reads a fresh Packet. Conflicts,
stale inputs, behavior mismatch, dirty/unknown lanes or intent-dependent merge
choices route to Recover and/or Conflict Handoff rather than being guessed.
Successful integration returns to Review/QA Replay before Ship.

Synthesize is skipped for one-lane work, outputs that remain intentionally
independent, or a provider with a separately proven canonical transactional
integration boundary. It may also integrate lawful results not originally
created by Fanout, so it cannot be modeled only as Fanout's final phase.

The Recipe owns no worker queue, branch cursor, hidden process, merge authority,
worker approval, Work close or ship transition. Required proof covers stale
head/base, partial merge, conflict, crash between lanes, integration regression,
duplicate handoff, missing result, unrelated dirty state and final artifact-to-
Contract closure.

## Settled brainstorm decision FB-9: Learning remains a Recipe distinct from Adapt

User selection: Option B, locked for the side-brainstorm handoff.

`Learning` remains a Review/Recover/Adapt-adjacent Recipe that turns exact,
verified experience into a typed promotion candidate. `Adapt` remains the
internal job for creating or materially revising a reusable extension such as a
Recipe, Adapter, Integration or project policy. Learning discovers and packages
a lesson; it does not itself change the reusable system.

Learning starts only from durable, current provenance such as applicable
Evidence, Assessment, Result, Decision, recovery outcome or a demonstrated
repeated-failure pattern. Chat recollection, speculation and unsourced advice
are not promotable inputs. Its output is one closed disposition:

```text
NoLearning
ProvenanceNoteCandidate
MemoryCandidate
DesignFinding
RegressionFinding
AdaptationCandidate
```

The Recipe deduplicates against current candidates and binds every candidate to
the exact source identities, applicability scope, confidence/unknowns and
invalidation conditions. These fields are evidence metadata only and do not
make the candidate authoritative.

Routing after Learning always uses a fresh Packet:

```text
one-off operational lesson       -> provenance note candidate
reusable sourced knowledge       -> Memory candidate
Contract or acceptance defect    -> Design finding
missing regression protection    -> Design or Execute finding
extension or policy improvement  -> Adaptation candidate -> Adapt
stale, conflicting or in_doubt   -> Recover
insufficient durable evidence    -> NoLearning
```

Promotion is a separate typed authorized Action Request owned by the canonical
destination. Learning cannot directly write canonical Memory, lock a Decision,
amend a Contract, modify Skill/Recipe/Harness/policy, create executable Steps,
authorize a retry or make a next-action recommendation. Memory remains advisory
and cannot become authority, lifecycle state or hidden retrieval truth.

The v1 `learning` Recipe is rewritten into `RecipeManifestV1`. Its router
priority, transition to `work`, legacy CLI spellings and direct memory/Decision
write semantics are migration-only. Required proof covers unsourced input,
duplicate lesson, stale Evidence, scope leakage, conflicting evidence, invalid
promotion, silent Skill mutation and correct routing of a material extension
change to Adapt.

## Settled brainstorm decision FB-10: Conflict Handoff remains distinct from Recover

User selection: Option B, locked for the side-brainstorm handoff.

`Conflict Handoff` remains a distinct Coordination Recipe eligible from Execute
or Recover. It handles overlapping ownership, incompatible concurrent intent,
shared mutable targets and integration disputes. `Recover` remains the internal
job for reconstructing canonical state after stale, interrupted, rejected or
`in_doubt` execution/effect conditions. A coordination conflict is not itself a
failed Run or recovery state.

Conflict Handoff starts from exact current identities for the affected Work,
Steps, Attempts, Leases, actors, artifacts/paths, effect domains and observed
overlap. It produces a non-bearer `ConflictHandoffV1` containing:

```text
conflict identity and class
exact conflicting parties and scoped targets
current ownership/Lease/Attempt facts
each party's durable intent and completed Evidence
safe pause or isolation requirements
candidate resolution boundaries
required authority and fresh-Packet return conditions
```

The handoff may require a lane to pause, isolate its workspace, relinquish an
expired/revoked Lease through the canonical owner or wait for a dependency. It
cannot assign ownership, transfer a Lease, amend the Step DAG, select a winner,
merge artifacts, cancel another actor, authorize mutation or resolve a material
intent choice. Those actions require their own typed authorized Action Requests.

Deterministic non-semantic conflicts may return restrictive resolution advice
when the canonical policy already selects an answer, for example ordering two
independent integrations by an existing dependency. Conflicting product intent,
acceptance, authority or irreversible effects fail closed and route through a
fresh Packet to Design, Recover or an authorized human decision as applicable.

The v1 `conflict-handoff` Recipe is rewritten into `RecipeManifestV1`. Inbox
messages, actor claims, prose ownership, Recipe transitions and legacy CLI
commands remain advisory migration evidence only. Task blockers/Step
dependencies, Lease fences and canonical custody records remain the sole
execution-order and ownership facts.

Required proof covers same-file and same-effect collisions, false-positive
overlap, expired versus live Lease, stale handoff, actor disappearance, partial
lane completion, conflicting acceptance intent, unauthorized winner selection,
cross-lane mutation and resumption only after a fresh Packet confirms the
conflict is resolved or safely isolated.

## Settled brainstorm decision FB-11: Ship remains a distinct restrictive Recipe

User selection: Option B, locked for the side-brainstorm handoff.

`Ship` remains a distinct Recipe eligible from Execute when a fresh Packet
advertises an exact ship/release Operation, including after Review has produced
the required current Assessment. Review may prepare or verify ship readiness
but cannot perform, approve or self-authorize the external effect.

Ship binds one exact Contract Root, accepted artifact identity, source/tree
identity, required Evidence and Assessments, destination/release channel,
Authority facts and rollback/reconciliation policy. Before dispatch it fails
closed on stale inputs, missing acceptance coverage, unverified artifact,
ambiguous destination, insufficient authority, unresolved risk or an existing
`in_doubt` Effect Intent.

The Recipe provides restrictive procedure around canonical effect handling:

```text
fresh Packet and ship Operation
-> freeze exact artifact and destination
-> verify current Gate and Authority predicates
-> create durable Effect Intent through an authorized Action Request
-> DispatchAttempt performs the bounded external effect
-> bind provider Receipt and post-dispatch Evidence
-> fresh Packet
-> success, Recover, or authorized ReconciliationAttempt
```

Every Run has exactly one owner under the closed `ExecutionAttempt` union.
Dispatch Runs belong to `DispatchAttempt` and bind the durable Effect Intent
plus dispatch fence. Reconciliation Runs belong to `ReconciliationAttempt` and
bind the same durable Effect Intent plus a fresh authorized Action Request.
Their originating Step Binding is provenance only and grants no Lease,
authority, Evidence applicability, lifecycle authority or right to mutate
current Step state.

Ship cannot mint or widen a Grant, infer approval from a passing Review, create
a retry right, treat timeout as failure, repeat an uncertain effect, mark Work
complete, update current Step state or substitute a provider Receipt for
canonical Evidence. Remote uncertainty and the Effect Intent survive removal,
amendment or supersession of the originating Step/Contract. Reconciliation
requires fresh authority and can never revive stale Step authority.

The v1 `ship` Recipe is rewritten into `RecipeManifestV1`. Its Feature/Task,
witness, close/archive, direct Git/release commands, autonomy wording and
transition semantics are migration-only. Provider-specific commands live in
Adapters/Integrations and are rendered only after canonical Operation
selection.

Required proof covers stale Review, artifact substitution, missing authority,
double dispatch, timeout and unknown provider response, crash before/after
Receipt persistence, partial multi-destination release, revoked authority,
rollback failure, reconciliation after Contract supersession and verification
that Review cannot self-approve or execute Ship.

## Settled brainstorm decision FB-12: Generate and Filter becomes a governed method

User selection: Option B, locked for the side-brainstorm handoff.

The useful behavior of v1 `generate-filter` is retained as the progressively
loaded `GenerateAndFilter` governed method available to Design, Review and
Adapt. It is not a vNext Recipe because it neither restricts a canonical
Operation frontier nor needs cross-session orchestration state; it is a bounded
reasoning technique inside an already selected job.

The method freezes an exact rubric before option generation, creates genuinely
divergent candidates, performs a logically fresh judging pass, selects at most
one survivor and records concrete rejection reasons. Its typed advisory output
is:

```text
GenerateFilterAssessmentV1
  source fork and constraint identities
  frozen rubric identity
  CandidateSetV1
  judging provenance and independence class
  survivor or NoSurvivor
  rejected alternatives with criterion-level reasons
  unknowns and invalidation conditions
```

The root `maestro` Skill loads this method only when the selected job encounters
a taste-heavy, naming, UX, API-shape, report-structure or extension-design fork
with multiple plausible answers. Ordinary deterministic choices do not load it.

The method cannot create or lock a Decision, amend a Contract, select a job,
change lifecycle state, call or schedule workers, choose the canonical next
Operation, authorize mutation or preserve a private cursor. After the
Assessment, a fresh Packet routes any materialization through the canonical
Design/Decision or Adapt owner and its own typed authorized Action Request.

The v1 `generate-filter` Recipe and manifest identity are replaced, not kept as
an active alias. Its fixed-rubric, divergent-generation, fresh-judge and
rejected-alternative semantics migrate into the method Resource; its router,
transitions, progress tasks, direct Decision writes, CLI strings and autonomy
claims are migration-only.

Required proof covers rubric written after viewing candidates, convergent or
duplicate options, generator-as-judge bias, multiple surviving choices,
unsupported rejection, no-survivor handling, attempted Decision lock, eager
method loading and deterministic job behavior when the method is not applicable.

## Settled brainstorm decision FB-13: Adversarial Review becomes a Review method

User selection: Option A after reconsidering the cleaner ownership boundary,
locked for the side-brainstorm handoff.

This decision supersedes only the FB-5 clause stating that
`AdversarialReview` invokes an Orchestration Recipe. The closed Review mode and
all other FB-5 semantics remain effective. History is preserved rather than
rewriting FB-5 in place.

`AdversarialReview` remains a `ReviewModeV1` value but loads the governed
`AdversarialReview` method Resource inside the Review job. It is not a vNext
Recipe because the behavior is bounded read-only analysis over frozen inputs,
owns no canonical Operation and requires no cross-session orchestration state.

The method binds an exact claim set, rubric/locked checks, Contract and artifact
identities, bounded source/Evidence packet, risk/independence requirements and
invalidation conditions. It may partition the input into bounded refutation
packets and returns one or more typed Assessments:

```text
AdversarialAssessmentV1
  reviewed claim and exact frozen-input identities
  reviewer identity and independence class
  verdict: Upheld | Refuted | Indeterminate | Error
  supporting or counterexample Evidence
  scope, unknowns and invalidation conditions
```

An external Agent/conductor may run fresh-context reviewers. The method and
Maestro do not spawn workers, preserve reviewer sessions or treat a number of
reviewers as proof of correctness. Independence is validated from exact
provenance against Contract/Gate policy; majority voting cannot override a
reproducible refutation or satisfy a missing required independence class.

Review aggregation is fail-closed for the relevant claim set. `Refuted`,
`Indeterminate`, `Error`, stale input or missing required independence cannot
produce a passing Assessment. Gate remains the sole owner of satisfaction; a
fresh Packet routes resulting work to Design, Execute or Recover.

The method cannot verify or block a Step directly, mutate implementation,
create a Decision, authorize a fix, select the next canonical Operation or
self-satisfy CloseReview. A repair actor cannot satisfy an independent review
requirement for its own change.

The v1 `adversarial-review` Recipe and active manifest identity are replaced.
Its claim partitioning, fixed-input refutation, independence and evidence-backed
verdict semantics migrate into the method Resource. Its router, transitions,
progress tasks, Task block/verify writes, CLI strings and autonomy claims are
migration-only with no active alias.

Required proof covers context leakage between reviewers, changed inputs during
review, reviewer/repairer identity collision, reproducible refutation hidden by
majority, unsupported upheld verdict, missing Evidence, indeterminate/error
handling, attempted Step mutation, eager method loading and stale Assessment
rejection.

## Settled brainstorm decision FB-14: Audit becomes a Review method

User selection: Option B, locked for the side-brainstorm handoff.

This decision supersedes only the FB-2 clause retaining an Audit Recipe and the
FB-5 clause stating that `Audit` mode invokes that Recipe. The Audit capability,
closed Review mode and all other FB-2/FB-5 semantics remain effective. History
is preserved rather than rewriting those decisions in place.

`Audit` remains a `ReviewModeV1` value but loads the governed `Audit` method
Resource inside the Review job. It is not a vNext Recipe because it is a bounded
read-only discovery and classification technique over an exact scope, owns no
canonical Operation and requires no private cross-session orchestration state.

The method binds the requested scope, Contract/artifact/source identities,
applicable acceptance and risk lenses, available Evidence, probe budget and
invalidation conditions. It selects the highest-value non-destructive probes
and returns:

```text
AuditAssessmentV1
  exact audited scope and input identities
  applied lenses and probe provenance
  findings with severity, impact and reproduction Evidence
  NoFinding for covered probes
  blocked or unexamined surfaces and unknowns
  invalidation conditions
```

Audit discovers unknown defects across one or more lenses. Adversarial Review
instead tries to refute an already stated bounded claim. They remain separate
methods because their input contracts, stopping conditions and negative proof
differ, even though both run inside Review.

The method cannot create Work/Steps, write blockers, mutate implementation,
lock a Decision, promote a finding to accepted truth, authorize a fix, select a
job or choose the canonical next Operation. A fresh Packet routes an applicable
finding to Design, Execute or Recover; non-actionable and insufficiently
supported candidates remain explicit dispositions rather than chat-only work.

Long or resumable audits use canonical Work/Steps and, where applicable,
Bounded Continuation. Audit itself keeps no scan cursor, private backlog,
scheduler, lifecycle or completion authority. Completed coverage is derived
from bound probe Results/Evidence, not remembered chat state.

The v1 `audit` Recipe and active manifest identity are replaced. Its bounded
scope, risk-lens selection, evidence-backed probes, severity and finding
classification semantics migrate into the method Resource. Its router,
transitions, progress tasks, direct Task/note writes, CLI strings and autonomy
claims are migration-only with no active alias.

Required proof covers incomplete scope represented as no-findings, unsupported
severity, destructive probe, stale source/artifact, duplicate finding,
chat-only finding, silent fix, attempted Work creation, resumable audit without
a private cursor, eager method loading and correct distinction from
AdversarialReview.

## Settled brainstorm decision FB-15: Intake Triage remains a trust-boundary Recipe

User selection: Option B, locked for the side-brainstorm handoff.

`Intake Triage` remains a distinct Recipe eligible from Research and Design for
a bounded set of external, imported or otherwise untrusted items. Integration
owns source acquisition and normalization into immutable `IntakeEnvelopeV1`
records; the Recipe owns only restrictive classification, deduplication advice
and exhaustive disposition of that bounded input set.

Each envelope binds the exact source/provider identity, acquisition Receipt,
content identity, media/schema classification, trust label and original actor
or authority claims as inert provenance. Embedded prompts, commands, approval
language, actor fields and lifecycle words remain data and grant no authority.

For each input item, the Recipe returns exactly one typed disposition:

```text
Duplicate(existing canonical identity)
Candidate(candidate class and normalized summary)
Escalate(required trust, scope or authority decision)
Reject(reason and Evidence)
Quarantine(reason and recovery requirements)
```

The Recipe proves that expected inventory count equals classified inventory
count and that every item is classified exactly once. Deduplication references
canonical identities and Evidence; textual similarity alone cannot silently
fold or discard an item. Unknown schema, ambiguous provenance, unsafe payload,
conflicting disposition or missing inventory fails closed.

`Candidate` is non-authoritative. Creating or amending Work, Step, Contract,
Decision, Memory or policy requires a fresh Packet and a separate typed
authorized Action Request owned by the canonical destination. Intake Triage
cannot execute commands from input, create executable work, select a job,
approve scope, assign severity as truth, authorize external effects or choose
the canonical next Operation.

Batch continuation may use Bounded Continuation, but Intake Triage retains no
private queue, cursor, scheduler or retry engine. Coverage is reconstructed from
the exact envelope inventory and durable dispositions. New or changed input
creates a new inventory identity rather than mutating a completed batch in
place.

The v1 `intake-triage` Recipe is rewritten into `RecipeManifestV1`. Its useful
trust-boundary, exhaustive coverage, structured classification and central
dedupe semantics remain. Its router, transitions, progress tasks, direct
Card/Task/blocker writes, CLI strings and trusted-conductor authority prose are
migration-only.

Required proof covers prompt injection, embedded privileged commands, forged
actor/approval fields, missing and duplicate dispositions, unsafe media,
unknown schema, false dedupe, stale store comparison, candidate self-promotion,
partial batch interruption, changed source bytes and reconstruction without a
private cursor.

## Settled brainstorm decision FB-16: Setup uses one mode-driven Recipe

User selection: Option B, locked for the side-brainstorm handoff.

The Setup job uses one progressively loaded `Setup` Recipe with the closed mode
set:

```text
Install | Adopt | Update | Repair | Rollback | Uninstall
```

One Recipe preserves a common preflight, plan, visible diff, backup, mutation,
verification and recovery discipline without creating six overlapping
orchestration authorities. Mode selection is an Integration rendering from the
fresh bootstrap/Packet reason and explicit user request; it grants no mutation
authority.

Distribution owns release/resource manifests, compatibility, installed-copy
inventory, archive/export formats, snapshots and package identity. Integration
owns host-specific placement and discovery for Agent Skills, the two global
read-only MCP Tools and project bootstrap resources. Persistence owns durable
local receipts and safe-write mechanics. The Recipe owns none of those states;
it only restricts their canonical Operations and orders their proof boundaries.

Every mode begins with a read-only plan binding exact source release/resources,
target host/project, current installed-copy identities, ownership policy,
compatibility findings, intended mutations and rollback boundary. User-owned or
diverged files require an explicit apply/replace decision; passive checks,
ordinary binary updates and unrelated modes cannot silently mutate them.

Mutation occurs through separate typed authorized Action Requests and durable
Results/Receipts. The Recipe verifies each installed copy against the expected
Resource identity and host contract before advancing. Partial application,
identity mismatch, stale plan, unsafe link/path, permission failure or
unverified target fails closed and returns through a fresh Packet to Repair,
Rollback or Recover.

Snapshot retention is exactly the two most recent verified rollback snapshots
per canonical retention scope. A new snapshot does not evict an older one until
the new installation/update is verified and the resulting rollback boundary is
durable. Rollback binds one exact snapshot and produces a new Receipt; it never
silently treats old bytes as the current release. Uninstall preserves required
recovery/export evidence and removes only manifest-owned resources unless the
user separately authorizes disposition of diverged files.

Global Agent-host placement follows each supported host's canonical global
location, including the equivalent of current `~/.agents/skills` and
`~/.claude/skills` conventions. The two MCP Tools install at global host scope
for autoload; projects install zero MCP servers. Project adoption/bootstrap
remains explicit and does not duplicate the global MCP installation.

Required proof covers clean install, adoption into an existing project, update
with user-diverged resources, stale and incompatible installation, partial
multi-host application, repair, two-snapshot retention, rollback after failed
verification, uninstall with diverged files, unsafe symlink/path, installed-copy
identity drift and zero project-scoped MCP installation.

## Settled brainstorm decision FB-17: Recover has no generic Recipe

User selection: Option A, locked for the side-brainstorm handoff.

Recover remains one of the seven internal jobs but does not gain a generic
`Recover` Recipe. A fresh Packet carries a closed recovery reason and the exact
canonical owner/eligible Operations. JobRoute selects the Recover instruction
branch; the job then follows that owner rather than interpreting all failures
through a second recovery engine.

Recovery ownership remains distributed by concept:

```text
stale or rejected Submission       -> Execution
interrupted StepAttempt            -> Execution
in_doubt DispatchAttempt/effect    -> Effect/Reconciliation
partial install/update             -> Setup/Distribution
ownership or integration collision -> Conflict Handoff/Coordination
stale Contract or Step Binding     -> Contract/Projection
```

Recover may load exact owner-specific guidance or an already selected
specialized Recipe such as Setup, Conflict Handoff or Ship. It cannot infer a
retry, revive a Lease, choose reconciliation, compensate an effect, amend a
Contract or transfer authority merely because the route says Recover.

The Packet recovery case binds current artifact identities, the failed/stale or
uncertain attempt/effect, fences and receipts, known remote state, applicable
authority, prohibited operations and exact reason codes. Missing, ambiguous,
conflicting or stale recovery facts remain Blocked and fail closed.

Every recovery mutation uses a typed authorized Action Request owned by the
responsible domain. After each bounded operation the Agent reads a fresh Packet;
Recover retains no private cursor, retry counter, queue, scheduler, checkpoint
or remembered return job. Return to Research, Design, Review, Execute, Setup or
Adapt occurs only through fresh Projection and JobRoute.

Resume after interruption reconstructs exclusively from canonical artifacts,
Results, Receipts and Evidence. Chat summaries, remembered job state,
Recipe-local state and host session state are not recovery authority or proof.

Required proof covers stale Submission, expired Lease, interrupted local Run,
unknown dispatch outcome, effect reconciliation after Contract supersession,
partial install, integration conflict, ambiguous recovery owner, unauthorized
retry, stale recovery Packet and cross-session reconstruction with no hidden
cursor.

## Settled brainstorm decision FB-18: Adapt has no generic Recipe

User selection: Option A, locked for the side-brainstorm handoff.

Adapt remains one of the seven internal jobs but does not gain a generic
`Adapt` Recipe. Its progressively loaded core is the governed `ExtensionLaw`
method, which classifies a proposed reusable capability, checks canonical
ownership and produces a non-authoritative extension candidate. The candidate
then uses the existing Design, Execute, Review and Setup boundaries instead of
entering a duplicate extension lifecycle.

The closed extension classes are the canonical Recipe, Adapter, Integration
and project-policy/resource classes selected by the main design. ExtensionLaw
must prove one owner per capability, exact data and authority boundaries,
portable identity, declared dependencies, no hidden lifecycle/Projection/
authorization/Evidence/retry/scheduling/cursor/migration/recovery semantics and
explicit install, upgrade, rollback and removal disposition.

The method returns a typed `ExtensionCandidateV1` or a refusal:

```text
extension class and proposed canonical owner
purpose, public boundary and exact dependencies
required Contract/Decision changes
Resource/manifest candidates
authority and effect requirements
migration, compatibility and removal obligations
proof requirements and unresolved forks
```

Learning may route an evidence-backed `AdaptationCandidate` to Adapt. Direct
user intent or Research may also enter Adapt, but no candidate can
self-authorize. Material scope or architecture choices route through a fresh
Packet to Design; an accepted exact Contract routes to Execute; Review checks
the built boundary; Setup installs, upgrades, rolls back or removes the verified
extension.

Adapt cannot publish a Contract, create executable Steps, mutate an installed
extension, register an Integration, grant authority, choose the next Operation
or treat successful validation as build approval. It owns no registry, private
extension state, job cursor or lifecycle. Ordinary use of an existing extension
does not enter Adapt.

Required proof covers Recipe with private next-action semantics, Adapter with
hidden retries, Integration self-authorization, project policy that weakens a
canonical Gate, dependency ambiguity, identity collision, extension upgrade
and removal, user-diverged installed copy, candidate self-promotion and a full
Adapt -> Design -> Execute -> Review -> Setup journey using only fresh Packets.

## Settled brainstorm decision FB-19: public Recipe name is Synthesize Results

User selection: Option A, locked for the side-brainstorm handoff.

The public Recipe title is `Synthesize Results` with stable candidate identifier
`synthesize`. It describes the optional `N -> 1` integration of exact lane
Results without limiting the Recipe to Git merges or colliding with the
architecture concept `Integration` used for host, Tool and provider boundaries.

All FB-8 behavior remains effective. Agent-facing descriptions and search terms
must explain that Synthesize Results validates and combines independently
produced lane artifacts before Review; it is not summarization, majority voting,
worker orchestration or hidden merge authority. Legacy `synthesize` references
receive exact migration treatment rather than becoming an ambiguous alias.

## Evidence audit: lossless v1 Skill-pack distillation candidate

Status: evidence and candidate architecture only; not yet a settled brainstorm
decision.

Live source census at this review contains exactly seven Skill packages and 35
files under `embedded/skills`: seven top-level `SKILL.md` files, seven generated
`reference/cli.md` catalogs and 21 semantic reference files, totaling 3,853
lines. The seven packages are `ask-maestro`, `maestro-setup`,
`maestro-research`, `maestro-design`, `maestro-audit`, `maestro-card` and
`maestro-witness`.

The effective main design currently defines the one-Skill/seven-job topology,
ownership prohibitions, lazy loading and coarse journeys, but it does not yet
materialize most of the old pack's instruction-level value. Treating all old
bytes as migration-only without extracting this behavior would lose proven
Agent UX even though the migration census remained byte-complete.

### Exact source-family disposition candidate

| v1 source | Extracted active destination | Inactive/removal disposition |
| --- | --- | --- |
| `ask-maestro/SKILL.md` | bounded root `SKILL.md` job capsules, refusal and resume rules | old public name and command routing removed |
| `maestro-setup/SKILL.md` | `jobs/setup.md` plus exact Setup Recipe reference | direct v1 commands and public activation removed |
| `maestro-research/SKILL.md` | `jobs/research.md` research-readiness, hosting, stakeholder, unknown and validity contract | same-card/v1 verb assumptions rewritten |
| `maestro-research/reference/examples.md` | positive/negative Research routing and staleness fixtures | no active prose authority |
| `maestro-design/SKILL.md` | `jobs/design.md` plus selected Design methods and Recipe references | Card/Feature/Task commands and old public activation removed |
| five `maestro-design/reference/*.md` semantic files | DDD, domain model, grilling, PRD synthesis and architecture-deepening methods | v1 artifact/CLI spellings rewritten |
| `maestro-audit/SKILL.md` | Review job Audit-mode capsule plus Audit method | direct proposal writes and Audit Recipe identity removed per FB-14 |
| `maestro-audit/reference/architecture-review.md` | Architecture Review method and optional report-rendering Adapter pattern | hard-coded host/report commands do not become core authority |
| `maestro-card/SKILL.md` | Execute job ground rules plus Review/Recover/Recipe references | monolithic Card lifecycle and old public activation removed |
| `maestro-card/reference/work.md` | Execute job, steering, coordination, low-ceremony and Evidence guidance | Card/Task/Progress lifecycle and command strings rewritten |
| `maestro-card/reference/tdd.md` plus five nested TDD files | TDD method family: vertical slices, behavior tests, interface design, boundary mocking, refactor and deep modules | upstream provenance retained; old card bindings rewritten |
| `maestro-card/reference/qa-baseline.md` | QA Baseline method in Design | v1 QA file/verb contract becomes migration/proof evidence |
| `maestro-card/reference/qa-slice.md` | QA Replay method in Review | v1 slice storage and close commands removed |
| `maestro-card/reference/verify.md` | Verification method plus Adversarial Review method input | direct verify/block writes removed |
| `maestro-card/reference/simplify.md` | Execute Simplify method at GREEN | fixed subagent fan-out is an external pattern, not required runtime behavior |
| `maestro-card/reference/loop.md` | Bounded Continuation profiles, operating limits, report and recurrence-guard candidates | v1 autonomy, Work Lease, cursor/TTL and direct authority claims removed |
| `maestro-card/reference/feature.md` | unique Gate, Fanout, QA, Ship and amendment guidance distributed to their canonical owners | file itself migration-only; no vNext Feature lifecycle guide |
| `maestro-card/reference/intake.md` | Intake Triage/Research entry guidance | direct Feature/Card continuation removed |
| `maestro-witness/SKILL.md` | Close Review method: exact-input mapping, risk and independence checks | witness/advisor sidecars, `Gate: APPROVED`, auto-spawn and direct close removed |
| all seven `reference/cli.md` files | behavior-discovery and migration fixtures only | zero active Agent-facing CLI catalog; `maestro_cli_search` replaces them |

Every source file therefore has exactly one primary disposition while extracted
behavior may be referenced from multiple jobs through one governed Resource
identity. Byte preservation for migration does not imply active retention.

### Candidate job playbooks distilled from proven behavior

#### Root Skill

Keep only Packet-first routing, seven concise purpose/trigger capsules,
least-authority selection, ambiguity refusal, selected-branch loading,
`maestro_cli_search` after Operation selection and fresh-Packet resume. Preserve
the useful `ask-maestro` idea that the router chooses a job but never replaces
the chosen job. Remove all v1 command catalogs and old Skill names.

#### Setup

Preserve exact target-root/host confirmation, read-only preflight and visible
diff, bounded repository-instruction read-in, user-owned-file protection,
installed-copy verification, compatibility diagnosis and explicit repair/
rollback/uninstall reporting. Route mutation through FB-16's Setup Recipe and
canonical Distribution/Integration owners; never copy v1 verbs as authority.

#### Research

Preserve breadth-before-depth triggers, explicit skip with sourced risk,
hosting selection, problem/users/stakeholders/current context/constraints,
three-way unknown classification, assumptions, landscape versus first design
fork, stakeholder-action status, `as_of` and invalidation rules, prior-art
stop/pivot and one exact readiness disposition. Recast the old `research.md`
receipt as a typed current Research output bound to canonical identities rather
than a privileged filename.

#### Design

Preserve the working thesis from user corrections, evidence-first current-state
mapping, full-durable-target versus build-stage separation, one fork at a time,
artifact-level option previews, detail floor, concrete examples, edge pressure,
recommendation, immediate Decision materialization, additive supersession,
consumer inventory before removal, independent review for hard-to-reverse
choices, current language/relationships/ambiguities, acceptance/non-goals/
affected areas, no-fork edge sweep and explicit build-approval stop.

Design loads governed methods only as needed: DDD fitness gate, strategic-first
domain modeling, one-question grilling, PRD synthesis without rediscovery,
architecture deepening, throwaway probe, and FB-12 Generate and Filter. Design
Relay and Wayfinding remain Orchestration-owned Recipe references.

#### Review

Preserve the closed Review modes without mixing them: Inspect for bounded
inspection; Audit for unknown-finding discovery across correctness, security,
performance, test coverage, technical debt, dependency, developer-experience
and documentation lenses; Adversarial Review for frozen-claim refutation; QA
Replay for affected Scenario Specs; Close Review for acceptance/Evidence/QA/
risk/independence closure. Findings must be reproducible, current, deduplicated
against canonical work and carry unexamined scope. Review never fixes, blocks,
approves, closes or ships directly.

Architecture Review remains an Audit specialization using the useful depth,
leverage, locality, deletion-test and before/after comparison vocabulary. HTML
or visual report generation is an optional Adapter/external presentation
pattern, not required Review semantics.

#### Execute

Preserve the design-to-execution gate, fresh overlap/conflict check, exact
Work/Step/Contract binding, one StepAttempt and live Lease fence per execution
Run, explicit dependencies rather than Inbox ordering, durable user steering,
smallest falsifying proof, low-ceremony use of the same Work/Step model and
bounded handoff/reporting. Remove Card/Task/Progress as active vNext semantics.

Behavior-changing Steps default to the TDD method. Preserve vertical tracer
slices, tests through public behavior, project ubiquitous language in test
names, boundary-only mocking, testable/deep interfaces, never refactor while
RED and explicit RED/GREEN Evidence. Non-behavioral work needs a typed reasoned
NotApplicable/skip bound to exact scope. Run Simplify once at GREEN over the
owned change, using reuse, simplification, efficiency and altitude lenses,
without turning cleanup into correctness review or scope expansion.

#### Recover

Preserve source reconstruction rather than chat memory, explicit stale/
conflict/interrupted/`in_doubt` classification, known versus unknown remote
state, failure/operating limits, exact owner and prohibited actions, and fresh
Packet return. Do not import Work Lease TTL, fixed retry rights or a generic
Recover Recipe. Load the owner-specific canonical Operation or Setup, Conflict
Handoff or Ship/Reconciliation Recipe selected under FB-17.

#### Adapt

Preserve reusable-lesson intake, architecture/deep-module vocabulary and the
requirement that broadly useful behavior become durable rather than chat-only.
Apply FB-18's Extension Law method: one capability owner, exact class and
dependencies, no private lifecycle/authority/Projection/Evidence/retry/
scheduling/cursor/migration/recovery, and explicit compatibility, proof,
installation, upgrade, rollback and removal. Material extension changes return
through Design, Execute, Review and Setup rather than a private Adapt lifecycle.

### Expanded governed method tree candidate

The FB-3 tree should be expanded rather than replaced:

```text
methods/
  ddd.md
  domain-model.md
  grilling.md
  prd.md
  architecture-deepening.md
  architecture-review.md
  probe.md
  generate-filter.md
  audit.md
  adversarial-review.md
  tdd.md
  tdd/test-design.md
  tdd/interface-design.md
  tdd/mocking.md
  tdd/refactoring.md
  tdd/deep-modules.md
  qa-baseline.md
  qa-replay.md
  close-review.md
  verification.md
  simplify.md
  extension-law.md
```

Methods remain progressively loaded and non-authoritative. Duplication between
job files and methods is prohibited: jobs state trigger, inputs, stopping and
return contract; methods contain reusable technique; Recipes contain bounded
cross-Operation orchestration; canonical domains retain lifecycle and authority.

### Candidate acceptance additions

1. All 35 v1 Skill files appear exactly once in the Resource/disposition ledger.
2. All 21 semantic reference files have an extracted active destination or an
   explicit rationale-only/migration-only disposition.
3. Positive and negative fixtures prove each old high-value behavior remains
   reachable from exactly the intended job/method/Recipe.
4. No active Resource contains a v1 command catalog, Card/Feature/Task/Progress
   lifecycle authority, old public Skill activation or direct Gate approval.
5. Root plus one selected job/method/Recipe stays within the context budget;
   unrelated branches never load.
6. DDD, TDD, Research readiness, two-pass QA, architecture review, grilling,
   PRD synthesis, simplify, verification and close-review independence each
   retain dedicated falsifying fixtures.

## Settled brainstorm decision FB-20: lossless Skill-pack distillation

User selection: Option B, locked for the side-brainstorm handoff.

The evidence audit immediately above becomes the required Skill Resource
materialization contract. It expands FB-3 without replacing its one-Skill,
seven-job, progressive-loading, QA or ownership laws. Every one of the 35 v1
Skill-pack files must appear exactly once in the final Resource/disposition
ledger, and all 21 semantic references must have an extracted active
destination or an explicit rationale-only/migration-only disposition.

Lossless means preservation of every still-valid behavioral capability and
failure shield, not verbatim active reuse. Proven Research, Design, DDD, domain
modeling, grilling, PRD, architecture review/deepening, TDD, QA, verification,
simplification, close-review, Setup, continuation, coordination and recovery
guidance is rewritten against vNext Work/Step/Contract/Packet/Operation/Result/
Evidence/Gate/Authority semantics and remains reachable through exactly the
intended job, method or Recipe.

The bounded root Skill remains small. Rich behavior lives in progressively
loaded job and method Resources. Jobs own purpose, trigger, input, refusal,
completion and return guidance; methods own reusable techniques; Recipes own
only restrictive cross-Operation orchestration; canonical domains retain all
lifecycle, authority, Evidence, persistence and Projection meaning.

The seven generated `reference/cli.md` files, old public Skill names, direct v1
command procedures and Card/Feature/Task/Progress authority are inactive
migration/proof inputs. They cannot re-enter through aliases, copied prose or
host-specific fallbacks. CLI rendering is discovered only through
`maestro_cli_search` after the canonical job and Operation are known.

The expanded method tree and candidate acceptance additions in the evidence
audit are incorporated into this decision. Build planning must include literal
positive/negative fixtures for every retained high-value behavior, exact lazy-
loading/context-budget proof and a consumer-zero removal check for every old
active package/name/catalog path.

## Settled brainstorm decision FB-21: exact vNext Recipe census is ten

User selection: Option A, locked for the side-brainstorm handoff.

The closed active vNext Recipe set is exactly:

```text
Bounded Continuation
Conflict Handoff
Design Relay
Fanout
Intake Triage
Learning
Setup
Ship
Synthesize Results
Wayfinding
```

`Bounded Continuation` has exactly two operating-limit profiles, `attended` and
`unattended`. Profiles are not additional Recipes, jobs, lifecycle states or
authority classes.

The exact v1 disposition closure is:

| v1 Recipe | vNext disposition |
| --- | --- |
| `adversarial-review` | replace with governed Adversarial Review method |
| `audit` | replace with governed Audit method |
| `conflict-handoff` | retain and rewrite as Conflict Handoff |
| `design-relay` | retain and rewrite as Design Relay |
| `design` | absorb into Design job |
| `feature-fanout` | replace with Fanout |
| `generate-filter` | replace with governed Generate and Filter method |
| `intake-triage` | retain and rewrite as Intake Triage |
| `learning` | retain and rewrite as Learning |
| `loop-until-done` | merge into Bounded Continuation |
| `progress` | remove; low ceremony uses canonical Work/Step |
| `ship` | retain and rewrite as Ship |
| `synthesize` | retain semantics under public title Synthesize Results |
| `unattended` | merge into Bounded Continuation `unattended` profile |
| `work` | absorb into Execute job and canonical Work/Step execution |

Wayfinding and Setup are the two admitted active additions. No eleventh Recipe,
active legacy alias, hidden default, host-specific fallback or unclassified v1
Recipe survives. Every retained Recipe is a `RecipeManifestV1` Resource under
Orchestration; every absorbed/replaced/removed source remains connected only to
exact migration provenance and consumer-removal proof.

Acceptance requires literal catalog equality at source, Bundle, installed-copy,
search/rendering and supported-host surfaces; positive reachability for all ten;
negative refusal for every old active name; exact two-profile equality for
Bounded Continuation; and proof that methods/jobs cannot be invoked through the
Recipe catalog as aliases.

## Settled brainstorm decision FB-22: typed non-bearer Skill activation observability

User selection: Option B, locked for the side-brainstorm handoff.

Active observability uses one typed `SkillActivationV1` Observation. It records
the route and Resources that were actually selected; it does not participate in
selection, applicability, authority, lifecycle, billing truth or resume.

The closed semantic shape binds:

```text
skill_identity: maestro
selected_job: Setup | Research | Design | Review | Execute | Recover | Adapt
route_status: Selected
route_basis_ref: Bootstrap | ExplicitRequest | PacketReason | RecoveryState
route_reason_code
loaded_job_resource_identity
loaded_method_resource_identities
referenced_recipe_identities
host/session provenance
observed_at
```

`route_basis_ref` points to the exact non-authoritative Integration basis and,
when applicable, the fresh Packet/bootstrap fact identity; it does not copy
authority or make the route current. An Ambiguous or Blocked route emits the
corresponding route/refusal Observation rather than a false Skill activation.

The event is produced only after `JobRouteV1` selects exactly one job and the
loader resolves the exact Resource closure. It cannot alter that closure,
trigger eager loading, manufacture a method/Recipe reference, recommend an
Operation, authorize a mutation, satisfy a Gate or prove successful work.

Resume ignores prior activation events and reconstructs from current canonical
artifacts plus a fresh Packet or the sole Setup bootstrap exception. Telemetry
may explain why a previous session loaded Design plus DDD, but it cannot restore
that job, Packet, method state or Recipe phase.

The seven v1 public activation names are inactive. They survive only in
source-qualified migration provenance attached to imported historical events;
no active event, alias, analytics normalizer, host adapter or fallback may emit
or translate them. Unknown job/method/Recipe identities fail closed in active
event validation.

Required proof covers all seven jobs, Setup before Packet, explicit read-only
Research/Review routing, ambiguous/blocked refusal, exact loaded-Resource
closure, no extra context load for telemetry, old-name rejection, stale event
ignored on resume and cross-host event-schema parity.

## Settled brainstorm decision FB-23: structural lazy loading plus measured context budgets

User selection: Option B, locked for the side-brainstorm handoff.

Context safety has two layers. A host-independent structural load law is part of
the Skill contract; a measured `ContextBudgetProfileV1` is bound into each
supported-host Release proof. Tokenizer-specific numeric thresholds do not
become product semantics, but a Release cannot activate on a host whose measured
closure exceeds its admitted profile.

The structural load sequence is exact:

```text
activation                  -> root SKILL only
Selected JobRoute           -> exactly one jobs/<job>.md
applicable governed method  -> only that method and declared required child
applicable Recipe           -> only its compact reference/guidance closure
```

No job may import another whole job. No method may import a job, Recipe body or
unrelated sibling method. Nested methods use exact backward dependencies and
load only the child required by the current case; selecting TDD does not eagerly
load mocking, interface design, test design, refactoring and deep modules.
Recipe references do not load the ten-Recipe catalog or another Recipe.

Every Release profile records at least UTF-8 bytes and tokenizer-observed token
counts for root-only, root plus each job, every reachable job/method/Recipe
closure, the maximum ordinary activation closure and declared deep exceptional
closures. It binds the supported host/tokenizer identity, Resource identities,
measurement procedure, thresholds and measured results.

Positive fixtures prove required guidance is reachable. Negative fixtures
assert the exact unloaded Resource set for every route. Representative closures
include Review plus Audit with no Execute/TDD/QA; Execute plus TDD and only the
needed TDD child; Design plus DDD/domain model without Setup/Ship; and Setup
before Packet without loading other jobs.

Loading additional guidance cannot be triggered by telemetry, prose mentions,
search results, historical activation events or legacy names. It requires a
closed declared Resource dependency and current applicable route/method/Recipe
selection. Missing, cyclic, unknown or budget-exceeding closure fails closed
with typed repair guidance.

Budget regression is a Release gate. A host-specific threshold may evolve only
through a new measured Release profile; it cannot silently widen at runtime or
cause a smaller host to load a truncated, behaviorally different Skill.

Required proof covers all seven root/job paths, every method and Recipe
reference, nested TDD selective loading, dependency-cycle rejection, stale
Resource identity, tokenizer/profile mismatch, root duplication, eager catalog
load, budget regression and identical semantic guidance across admitted hosts.

## Settled brainstorm decision FB-24: shared visible non-authoritative JobRoute rendering

User selection: Option B, locked for the side-brainstorm handoff.

Human and Agent adapters may render the same Integration-owned `JobRouteV1`
companion with the current Projection output. It is visible guidance, not a
canonical Packet field, lifecycle state, Recommendation, mode or command
namespace.

A routine human rendering distinguishes the two concepts explicitly:

```text
Next action: <Projection-owned recommendation or refusal>
Agent guidance: <Setup | Research | Design | Review | Execute | Recover | Adapt>
Reason: <closed JobRoute reason code>
```

CLI human text, JSON companion fields, TUI and the one Agent Skill render the
same route identity and reason produced by Integration. Where the final locked
`maestro_packet` response envelope permits a companion, MCP exposes that exact
same non-authoritative route without changing AgentPacket identity or
Recommendation; otherwise the root Skill derives it through the same
Integration contract from the returned Packet. No adapter independently maps
reason codes, chooses a job or caches a current route.

There are no seven public job commands, public modes or discoverable Skills.
An explicit human request for read-only Research or Review may use the FB-4
least-authority route rule, but cannot alter the current executable frontier or
authorize mutation. Requests for other jobs remain intent input to the normal
Packet/Projection/Action path rather than imperative mode switches.

Ambiguous, incompatible, stale or missing route facts render the typed Blocked
or Ambiguous refusal and load no guessed branch. Setup remains the sole
pre-Packet bootstrap exception. After any Action Result or bounded read-only job
output, adapters discard the route and recompute only from a fresh Packet or
current bootstrap facts.

Required proof covers semantic parity across CLI text/JSON, TUI, Skill and the
admitted MCP envelope; explicit separation of next action from Agent guidance;
read-only Research/Review intent; ambiguous refusal; stale-route non-reuse; zero
job commands/modes; and verification that rendering cannot mutate, recommend,
authorize or eagerly load a branch.

## Settled brainstorm decision FB-25: snapshot cutover and sealed legacy quarantine

User selection: Option B, locked for the side-brainstorm handoff.

The one-Skill/two-global-MCP cutover is a coherent InstallationDomain
Distribution transition. Old Skill packages, activation links, MCP descriptors,
host configuration, caches and managed copies leave every active discovery root
as part of the verified activation; a disabled marker inside a discoverable root
is not an acceptable removal.

The transition sequence is:

```text
byte-total installed inventory
-> coherent pre-cutover Installation snapshot
-> stage one maestro Skill and exact two-MCP host descriptors
-> verify Resource/Bundle/Release and host compatibility
-> atomically publish the new same-domain current claim/Receipt
-> move legacy names outside all discovery roots into sealed quarantine
-> reconnect/reobserve admitted hosts
-> prove old-name refusal and new catalog equality
-> prune only after consumer-zero and rollback-safety gates
```

Candidate/staged bytes grant no currentness. Semantic activation succeeds only
after the complete same-domain target set verifies. Crash, partial host update,
alias ambiguity, installed-copy drift or reconnect mismatch returns an honest
stale/conflict/recovery-required/`in_doubt` Result according to the canonical
effect boundary; it never claims a mixed installation as current.

Ordinary rollback restores one exact coherent prior Installation snapshot,
including matching Skill, two-MCP descriptors/config and protected compatible
Release closure. It never mixes the new Skill with old MCP/config or translates
legacy names at runtime. Exactly the two most recent eligible prior successful
snapshots remain ordinary selectable snapshots under FB-16 and the canonical
two-snapshot law.

Sealed v1 migration, audit, legal, export or unfinished-recovery material may be
retention-protected outside discovery roots. Such material is not active,
ordinary selectable, searchable as a Skill, a fallback catalog or authority.
User-diverged/unmanaged files are preserved in quarantine with source path,
hash, custody and recovery disposition; they are never silently deleted or
promoted into the new Skill.

Final removal requires exact consumer-zero across host catalogs, activation
links, caches, backups, hooks/events, docs, tests, generated references, Recipe
resolvers, shell/TUI labels, installers/updaters and retained old binaries, plus
current custody, fresh authorization and rollback/protected-retention safety.

Required proof covers clean and diverged installations, both supported host
roots, partial activation crashes, stale candidate, alias escape/substitution,
old-name discovery refusal, host reconnect skew, rollback-of-rollback,
two-snapshot capacity, protected migration material outside the catalog and
consumer-zero removal without data loss.

## Settled brainstorm decision FB-26: one primary Recipe plus optional Continuation overlay

User selection: Option B, locked for the side-brainstorm handoff.

Each fresh tick evaluates one closed `RecipeApplicationV1`:

```text
primary: None | exactly one of the nine non-Continuation Recipes
continuation:
  None
  | BoundedContinuation(attended)
  | BoundedContinuation(unattended)
```

`Bounded Continuation` is the sole admitted overlay. It constrains whether and
under what operating limits an external Agent/conductor may request another
fresh tick. The primary Recipe supplies the current bounded procedure. Neither
field is lifecycle state, a persisted cursor, authority, worker launch or a
second Recommendation.

Composition is monotonic and deterministic. Operation allowsets/frontiers are
intersected, preconditions and hard stops are accumulated, budgets/permissions
take the stricter value and any contradiction or empty result returns
`HardStop`. No Recipe may weaken, subtract or override a canonical or sibling
restriction. Projection remains the sole final Recommendation owner.

Two simultaneously applicable primary Recipes are Ambiguous unless current
canonical facts make exactly one strictly applicable. There is no priority,
first-match, host ordering or user-prose guess. Recipes cannot invoke, import,
nest or transition directly into another Recipe. Every Result/output ends the
application; a fresh Packet reconstructs the next application from current
facts.

Examples:

```text
Ship + BoundedContinuation(unattended)
DesignRelay + BoundedContinuation(attended)
Setup + no continuation
no primary + BoundedContinuation(attended)
```

Ship still requires exact current effect authority; the unattended profile
cannot create it. A Design Relay mandate remains a canonical Authority/operating
constraint rather than Recipe memory. If a later tick needs Wayfinding, the
fresh application selects Wayfinding as the new primary while the mandate and,
if still applicable, Continuation overlay remain independently revalidated.

Required proof covers all nine primary Recipes with and without each permitted
Continuation profile, zero-primary continuation, two-primary ambiguity,
frontier intersection, unioned hard stops, stricter budget/permission merge,
empty intersection, stale application non-reuse, forbidden Recipe nesting and
Projection ownership after composition.

## Settled brainstorm decision FB-27: closed Job-Recipe eligibility and reason-coded return

User selection: Option A, locked for the side-brainstorm handoff.

The exact eligibility matrix is:

| Recipe | Eligible internal jobs |
| --- | --- |
| Bounded Continuation overlay | Setup, Research, Design, Review, Execute, Recover, Adapt, subject to profile and current authority/limits |
| Conflict Handoff | Execute, Recover |
| Design Relay | Design |
| Fanout | Execute |
| Intake Triage | Research, Design |
| Learning | Review, Recover, Adapt |
| Setup | Setup |
| Ship | Execute |
| Synthesize Results | Execute, Recover |
| Wayfinding | Research, Design |

The matrix is closed for vNext. A Recipe manifest references only its admitted
job set; unknown jobs, additional job edges and host-specific eligibility fail
closed. Extension of this set requires a new Resource/Release and the normal
Extension Law plus architectural review; manifest prose cannot widen it.

Recipes never name, persist or transition directly to a return job. Their
completion and hard-stop outputs use exact closed return reason codes such as
`review_required`, `recovery_required`, `design_required`, `authority_required`,
`setup_required` or capability-specific completion/refusal codes. A fresh
Packet plus Integration-owned `JobRouteV1` maps those current facts to exactly
one next instruction job or Ambiguous/Blocked.

Bounded Continuation eligibility across seven jobs does not pre-authorize seven
jobs or preserve a job chain. Each new tick revalidates the Packet/bootstrap,
JobRoute, primary Recipe, continuation profile, authority and operating limits.
Setup before Packet remains restricted to the exact bootstrap exception.

No Recipe may smuggle a job change through phase names, `continue` prose,
output labels, invocation fields, CLI commands or telemetry. Return reason codes
are observations/guidance inputs only and cannot override canonical Result,
Evidence, Gate or Projection facts.

Required proof covers every admitted matrix edge, every non-edge refusal,
reason-code closure, missing/unknown/stale reason, ambiguous route, zero direct
job transitions, Bounded Continuation revalidation across all seven jobs and
semantic parity across installed manifests and supported hosts.

## Settled brainstorm decision FB-28: closed Job-Method eligibility matrix

User selection: Option A, locked for the side-brainstorm handoff.

The exact direct method eligibility is:

| Internal job | Direct governed methods |
| --- | --- |
| Setup | none; use the Setup Recipe and job guidance |
| Research | none; research-readiness is the bounded job contract |
| Design | DDD, Domain Model, Grilling, PRD, Architecture Deepening, Probe, Generate and Filter, QA Baseline |
| Review | Audit, Architecture Review, Adversarial Review, Generate and Filter, QA Replay, Close Review, Verification; Inspect is the bounded default mode and needs no separate method Resource |
| Execute | TDD, Simplify |
| Recover | none; follow the exact canonical recovery owner and specialized Recipe/guidance |
| Adapt | Extension Law, Generate and Filter |

The TDD child Resources are reachable only through the selected TDD method in
Execute:

```text
Test Design
Interface Design
Mocking
Refactoring
Deep Modules
```

A child loads only when the current Step's exact behavior, interface, external
boundary, GREEN refactor or module-depth question requires it. TDD selection
does not load all children, and no child is directly invocable as a job-level
method.

DDD and Domain Model run in Design; Execute consumes the published ubiquitous
language and Contract rather than reopening the model. Architecture Review
discovers/evaluates candidates in Review; Architecture Deepening designs the
selected solution in Design. QA Baseline binds Scenario Specs in Design; QA
Replay evaluates them in Review. Generate and Filter is the sole method with
direct eligibility across Design, Review and Adapt because each may contain a
bounded judgment-heavy candidate choice.

Method output is advisory Evidence, Assessment or candidate material. It cannot
change job, lifecycle, authority, Gate or canonical Recommendation. Completion
returns through current artifacts and a fresh Packet/JobRoute. Unknown method
edges, direct TDD-child activation, host-specific additions and copied method
bodies in jobs fail closed or violate Resource census equality.

Required proof covers every admitted direct edge and non-edge, all five TDD
children and selective negative loading, DDD/Execute separation, Architecture
Review/Deepening separation, QA Baseline/Replay separation, shared Generate and
Filter identity across three jobs, zero method-driven job transitions and
cross-host Resource-identity parity.

## Settled brainstorm decision FB-29: preserve the exact MCP Packet envelope

User selection: Option B, locked for the side-brainstorm handoff.

This Decision supersedes only FB-24's conditional clause that allowed an MCP
JobRoute companion when the final envelope permitted one. The effective rule is
now unconditional: `maestro_packet` returns only the exact closed
`McpPacketReadEnvelopeV1` selected by locked Decision
`dec-post-main-two-operation-global-mcp-b46b`. Its tagged branches remain
`Packet | NoActiveStore | Unavailable | Stale | Incompatible`; no
`JobRouteV1`, Skill-loading hint, adapter route or other companion field is
added to that public schema.

After receiving a valid Packet, the one root `maestro` Skill asks the shared
Integration-owned routing function to derive a fresh non-authoritative
`JobRouteV1` from the exact Packet plus the admitted explicit read-only intent.
The same closed reason map and renderer may be used by CLI/JSON and TUI on their
own UX/read envelopes, but no adapter independently maps reason codes, persists
a current job or changes the canonical Packet identity, hash, Recommendation or
validity.

Setup remains the sole pre-Packet bootstrap exception. A Packet refusal yields
the corresponding blocked/bootstrap guidance and cannot be converted into a
guessed job. Every Action Result or bounded read-only output invalidates the
derived route; the Skill must read a fresh Packet and recompute. The derived
route grants no authority, applicability, Evidence status, lifecycle change or
right to mutate.

The public agent sequence is therefore:

```text
maestro_packet -> exact canonical Packet/refusal
               -> shared Integration JobRouteV1 derivation
               -> load one selected job and only its required methods/Recipes
```

No third MCP Tool is introduced. The exact global Tool set remains
`{maestro_packet, maestro_cli_search}`. Required proof covers byte/schema
identity of `McpPacketReadEnvelopeV1`; rejection of extra route fields; route
parity from identical Packet/intent inputs across supported hosts; refusal and
stale-Packet behavior; zero eager loading before route selection; and proof that
the derived route cannot mutate, authorize or alter Projection.

## Settled brainstorm correction FB-30: distinguish Adopt from Migrate

Edge-sweep correction, locked as a lossless capability repair for the
side-brainstorm handoff.

This correction supersedes only FB-16's six-member Setup mode list and its
omission of explicit migration proof. The closed `SetupModeV1` set is exactly:

```text
Install | Adopt | Migrate | Update | Repair | Rollback | Uninstall
```

`Adopt` takes an existing vNext-compatible but unmanaged target through exact
inventory, custody/adoption authority, visible Plan, transfer or managed-block
claim, verification and Receipt. It does not translate generations, import
legacy authority, reinterpret v1 state or perform a cutover.

`Migrate` is the explicit offline v1/legacy-to-vNext journey. It delegates all
semantic mapping, quarantine, sibling-store construction, equivalence proof,
activation, rollback and legacy-removal rules to canonical Migration,
Persistence, Distribution and owning domains. The Setup Recipe may only
restrict and order the advertised Operations and proof boundaries; it owns no
migration reader, mapping, state, authority, activation or rollback semantics.

A legacy store, legacy public Skill catalog, old MCP descriptor or mixed-
generation installation can never be accepted through Adopt. A proven
vNext-compatible unmanaged target cannot be forced through semantic Migrate
merely to bypass adoption authority or custody checks. Ambiguous generation,
custody or target identity blocks both modes and routes through exact inspect or
Recover guidance.

Required proof adds positive and negative Adopt-versus-Migrate classification;
byte-total v1 export/import and quarantine; mixed-generation refusal; sibling-
store semantic equivalence; crash cuts before and after activation; coherent
rollback; no legacy alias or authority promotion; and proof that both modes use
the same authorized Distribution/Migration Operations rather than private
Setup mutation.

## Settled brainstorm correction FB-31: Intake owns durable intake records

Edge-sweep ownership correction, locked for the side-brainstorm handoff.

This correction supersedes only FB-15's statement that Integration owns source
acquisition and immutable `IntakeEnvelopeV1` records. The already locked main
boundary remains authoritative: Intake owns immutable untrusted
`SourceArtifactV1`, intake findings and dispositions; Evidence owns
authenticated `ObservationKindV1::ResearchSource`; Research owns its separate
question and synthesis revisions.

Integration and host Adapters may transport bytes, validate framing and media,
apply bounded redaction and construct a typed candidate input for the exact
advertised Action. They cannot publish a SourceArtifact, disposition,
Observation or acquisition Receipt directly. Every durable publication uses
the canonical owner's separate typed authorized Action and Store transaction.
An external provider Receipt or actor claim is inert provenance until admitted
by the owning contract; it never grants authority.

Intake Triage consumes one exact bounded inventory of current
`SourceArtifactV1` references plus applicable Evidence. It emits restrictive
classification advice and exhaustive proposed dispositions only. The canonical
Intake Action owns any published disposition. No separate durable
`IntakeEnvelopeV1`, Integration store, adapter queue or hidden intake lifecycle
survives.

Required proof adds transport-without-publication, malformed and prompt-
injected source input, actor/Receipt nonpromotion, exact SourceArtifact and
ResearchSource ownership, typed-Action-only publication, interruption without
an adapter queue and adapter parity over the same canonical records.

## Settled brainstorm correction FB-32: Distribution owns placement and receipts

Edge-sweep ownership correction, locked for the side-brainstorm handoff.

This correction supersedes only FB-16's assignment of host-specific placement
to Integration and durable installation receipts/safe writes to Persistence.
The final corrected main topology remains authoritative.

Distribution owns the staged target-mutation protocol, exact Plans, preimages,
visible diffs, custody checks, backup/recovery requirements, filesystem and
manager effects, verification, activation and domain-local Distribution
Receipt publication. Installation owns the User-Agent InstallationDomain
current claims and installation topology; each RepositoryDomain owns its
project bootstrap/current claims. Adapter owns immutable host-facing descriptor
and activation-metadata Resources, while Distribution alone materializes them
at authorized targets.

Integration owns only non-authoritative host capability discovery, request
transport and route/rendering. It cannot select or mutate a target, infer
custody/currentness, publish a Receipt, perform a safe write or create a
cross-domain Plan. Persistence supplies canonical Store/object durability,
transaction and recovery primitives; it does not own installation meaning,
target custody or Distribution Receipt semantics.

The Setup Recipe references these owners and may tighten/order their advertised
Operations. It owns no placement, path, Receipt, snapshot, write primitive,
activation, currentness or rollback state. Required proof adds owner-boundary
tests, zero Integration writes, domain-local Receipt publication, Adapter-
Resource versus Distribution-deployment separation, Persistence semantic
nonownership and cross-domain refusal.

## Effective side-decision closure

This section is the effective read head for the side brainstorm. It preserves
all earlier evidence and rejected alternatives but prevents historical audit or
candidate wording from being mistaken for an open fork. Side decisions are
handoff evidence until the main conductor records their canonical Decision or
superseding Decision; they are not runtime authority or build approval.

| Earlier statement | Effective disposition |
| --- | --- |
| The initial Wayfinding/Recipe-count contradiction and 15-current-Recipe audit | Closed by FB-7 and FB-21: Wayfinding is a Recipe and the exact active vNext catalog contains ten Recipes. The 15-item table is the complete v1 source-disposition input, not the vNext catalog. |
| FB-1's temporary statement that old continuation names may remain until a naming fork | Superseded by FB-21 and FB-25: `loop-until-done` and `unattended` have no active aliases; they merge into Bounded Continuation and its `unattended` profile. |
| FB-2 and FB-5 retain/invoke Audit as a Recipe | Superseded only on that point by FB-14: Audit is a governed Review method. |
| FB-5 invokes Adversarial Review as a Recipe | Superseded only on that point by FB-13: Adversarial Review is a governed Review method. |
| FB-2 routes Intake Triage through Research or Review | Superseded by FB-15 and FB-27: exact eligibility is Research or Design. |
| FB-8 leaves the public Synthesize name open | Superseded by FB-19: public title `Synthesize Results`, candidate stable id `synthesize`. |
| The Skill-pack audit labels its mappings and method tree candidate-only | Promoted into the side contract by FB-20 and made exact by FB-28 plus the file-level ledger below. |
| FB-24 conditionally permits an MCP JobRoute companion | Superseded only on that point by FB-29: the MCP Packet envelope remains byte/schema exact and route derivation happens afterward in shared Integration. |
| FB-16's six Setup modes omit the canonical Migration journey | Superseded only on that point by FB-30: Adopt and Migrate are distinct, producing the exact seven-mode Setup set. |
| FB-15 assigns durable intake acquisition/records to Integration | Superseded only on that point by FB-31: Intake owns SourceArtifact/disposition, Evidence owns ResearchSource Observation and Integration only transports. |
| FB-16 assigns host placement to Integration and receipts/safe writes to Persistence | Superseded only on that point by FB-32: Distribution and the exact domain owners retain placement, currentness and Receipt semantics; Integration renders and Persistence supplies durability primitives only. |
| FB-7 says Wayfinding creates/sharpens Investigation Steps and FB-8 says Synthesize records integration Evidence | Read under FB-6 and the canonical mutation law: both Recipes emit proposals/restrictions and require owning Actions; neither publishes Step, Evidence, artifact or Result directly. |
| FB-22 says a Skill activation Observation is emitted after routing | Read under the frozen Observation publication law: publication, when applicable and authorized, uses the exact Evidence acquisition Action; routing and passive Packet reads never mutate merely to record telemetry. |
| Any implication that a Recipe can directly return, persist or invoke a job | Closed by FB-4, FB-26 and FB-27: a fresh Packet plus `JobRouteV1` selects one instruction branch; Recipe output is reason-coded restrictive advice only. |

The effective side closure is therefore FB-1 through FB-32 with the narrow
supersessions above. There are zero open side Decisions and zero unresolved
public names, counts, ownership axes or composition rules.

## Exact 35-file legacy Skill Resource disposition ledger

The live `embedded/skills` source set contains exactly 35 files. Every path
below appears exactly once. A disposition applies to the v1 source Resource;
the named destination is a newly identified vNext Resource and does not reuse
the legacy source identity. `migration-only` means sealed provenance/proof and
never active installation or discovery.

| Exact v1 source path | Disposition | Exact active destination or treatment |
| --- | --- | --- |
| `ask-maestro/SKILL.md` | rewrite | compact public `maestro` Skill router |
| `ask-maestro/reference/cli.md` | migration-only | removed from active Agent context; CLI discovery uses `maestro_cli_search` |
| `maestro-audit/SKILL.md` | replace | Review job plus governed Audit method |
| `maestro-audit/reference/architecture-review.md` | rewrite | `methods/architecture-review.md` |
| `maestro-audit/reference/cli.md` | migration-only | removed from active Agent context; CLI discovery uses `maestro_cli_search` |
| `maestro-card/SKILL.md` | replace | Execute job plus its governed method references |
| `maestro-card/reference/cli.md` | migration-only | removed from active Agent context; CLI discovery uses `maestro_cli_search` |
| `maestro-card/reference/feature.md` | replace | Design/Execute Work, Contract, acceptance and handoff guidance with no Card/Feature lifecycle authority |
| `maestro-card/reference/intake.md` | rewrite | Intake Triage Recipe guidance |
| `maestro-card/reference/loop.md` | replace | Bounded Continuation attended/unattended profiles and operating-limit guidance |
| `maestro-card/reference/qa-baseline.md` | rewrite | `methods/qa-baseline.md` |
| `maestro-card/reference/qa-slice.md` | replace | `methods/qa-replay.md` |
| `maestro-card/reference/simplify.md` | rewrite | `methods/simplify.md` |
| `maestro-card/reference/tdd.md` | rewrite | `methods/tdd.md` |
| `maestro-card/reference/tdd/deep-modules.md` | rewrite | `methods/tdd/deep-modules.md` |
| `maestro-card/reference/tdd/interface-design.md` | rewrite | `methods/tdd/interface-design.md` |
| `maestro-card/reference/tdd/mocking.md` | rewrite | `methods/tdd/mocking.md` |
| `maestro-card/reference/tdd/refactoring.md` | rewrite | `methods/tdd/refactoring.md` |
| `maestro-card/reference/tdd/tests.md` | replace | `methods/tdd/test-design.md` |
| `maestro-card/reference/verify.md` | rewrite | `methods/verification.md` |
| `maestro-card/reference/work.md` | rewrite | Execute job guidance over canonical Work/Step/Packet/Operation/Result |
| `maestro-design/SKILL.md` | rewrite | Design job guidance |
| `maestro-design/reference/cli.md` | migration-only | removed from active Agent context; CLI discovery uses `maestro_cli_search` |
| `maestro-design/reference/ddd.md` | rewrite | `methods/ddd.md` |
| `maestro-design/reference/deepening-candidate.md` | replace | `methods/architecture-deepening.md` |
| `maestro-design/reference/domain-model.md` | rewrite | `methods/domain-model.md` |
| `maestro-design/reference/grilling.md` | rewrite | `methods/grilling.md` |
| `maestro-design/reference/prd.md` | rewrite | `methods/prd.md` |
| `maestro-research/SKILL.md` | rewrite | Research job guidance |
| `maestro-research/reference/cli.md` | migration-only | removed from active Agent context; CLI discovery uses `maestro_cli_search` |
| `maestro-research/reference/examples.md` | rewrite | progressively loaded Research examples Resource |
| `maestro-setup/SKILL.md` | replace | Setup job guidance plus identity-only reference to the Setup Recipe |
| `maestro-setup/reference/cli.md` | migration-only | removed from active Agent context; CLI discovery uses `maestro_cli_search` |
| `maestro-witness/SKILL.md` | replace | Review job plus governed Close Review method |
| `maestro-witness/reference/cli.md` | migration-only | removed from active Agent context; CLI discovery uses `maestro_cli_search` |

Ledger proof requires source path-set equality at 35, unique row membership,
the live sorted source-stream digest, positive reachability of every rewritten
behavior, rejection of every migration-only Resource from active Bundle and
host catalogs, and consumer-zero proof before old discovery paths disappear.

## Final side no-fork edge sweep

Sweep basis after FB-32:

```text
scratch completeness gate: PASS
embedded source inventory: 204 expected / 204 classified / 0 remaining
direct-consumer inventory: 325 expected / 325 classified / 0 remaining
closed installed/cache/mirror census: 28,102 expected / 28,102 classified
legacy Skill source ledger: 35 expected / 35 classified / 0 remaining
side Decisions: FB-1..FB-32 / 0 open
```

| Edge surface | Result | Effective closure |
| --- | --- | --- |
| Product and authority boundary | clean | Jobs, methods, Recipes, Skill, Harness, Adapter, Integration and Distribution own no lifecycle, Gate truth, authority, mutation, retry, scheduler, cursor, private store or recovery semantics. |
| Public Skill topology | clean | One discoverable `maestro` Skill, seven internal jobs, selected-branch loading only, zero legacy aliases. |
| Recipe topology | clean in side design; canonical supersession required | Exact ten-Recipe catalog and complete 15-source v1 disposition are fixed by FB-21. This changes the older active-name/count treatment. |
| Feature acceptance | repair required after canonical successor | Live feature acceptance `ac-2` still says 15 Recipes and one profile. After the Recipe-catalog Decision locks, replace that clause with exactly ten active vNext Recipes and exactly two Bounded Continuation profiles; retain 15 only as the v1 source-disposition count. Add JobRoute, matrix, lazy-loading and 35-file ledger proof without weakening the existing E204/C325/28,102 gates. |
| Recipe selection/composition | clean in side design; canonical supersession required | FB-26 permits one primary plus the sole Bounded Continuation overlay; this must supersede the older canonical `CoreOnly | exactly one explicit Recipe` selection clause rather than reinterpret it. |
| Recipe grammar and return | clean | FB-6 and FB-27 close fields, eligibility and reason-coded return; no direct job transition or private continuation state survives. |
| Packet and next-action ownership | clean | Projection remains sole Recommendation owner. `JobRouteV1` is fresh non-authoritative Integration guidance only. |
| MCP surface | clean | Exactly `{maestro_packet, maestro_cli_search}`; FB-29 preserves the closed `McpPacketReadEnvelopeV1` and forbids a route companion or third Tool. |
| Review and repair | clean | Five Review modes, Audit and Adversarial Review as methods, QA Replay and Close Review preserved, compound review/fix auto-routes one fresh job per tick. |
| Job-to-Recipe relation | clean | Closed exhaustive matrix in FB-27; every non-edge refuses. |
| Job-to-Method relation | clean in side design; narrow canonical supersession required | FB-28 makes TDD Execute-only. Main Decision 7305 says TDD guides Execute/Review; the successor must replace only that Review eligibility wording while preserving all other 7305 laws. |
| Legacy Skill behavior | clean | All 35 paths classified exactly once; all 21 semantic references have an active rewritten destination or migration-only treatment. |
| Activation observability | clean | FB-22 refines the already frozen `ObservationKindV1::SkillActivation`; it adds no 44th kind, remains non-bearer and publishes only through the exact Evidence acquisition Action rather than passive route telemetry. |
| Context loading | clean | Structural progressive loading plus measured release/host budget profiles and negative-loading fixtures; no eager sibling/catalog load. |
| Setup, installation and rollback | clean | One Setup Recipe with seven modes, explicit Adopt/Migrate separation, global one-Skill/two-MCP closure, Repository bootstrap separation, and exactly two ordinary selectable prior snapshots. |
| Intake and Research ownership | clean | FB-31 imports the locked Intake/Research/Evidence split; Integration transports but publishes no durable intake or research state. |
| Distribution and placement ownership | clean | FB-32 imports the corrected domain topology: Distribution mutates exact targets and publishes domain-local Receipts; Adapter supplies Resources, Integration renders and Persistence supplies durability primitives. |
| Cutover and removal | clean | Coherent snapshot cutover, sealed quarantine outside discovery roots, old-name refusal, consumer-zero removal and protected migration evidence. |
| Recovery and effects | clean | Recover routes to canonical owners; Ship preserves exact Effect Intent/Attempt laws; no stale Step authority or blind retry. |
| Human and agent journeys | clean | Research to Design, approval gate, Execute, Review, Ship or Recover is fresh-Packet routed; Setup and explicit read-only Research/Review exceptions are bounded. |
| Acceptance and proof | clean | Catalog equality, path-set equality, positive/negative routing, stale/refusal, authority non-escalation, context budgets, cross-host parity, crash/rollback and removal proof are explicit. |
| Remaining material choice | none | The three canonical conflicts above have selected outcomes and require proper successor Decisions, not more side brainstorming. FB-30 repairs a mandatory full-scope omission rather than adding optional scope. |

No critical or high side finding remains unresolved. Historical prose is
retained as evidence but is governed by the Effective side-decision closure.
The edge sweep authorizes no implementation and does not claim that the main
canonical design has already materialized these selections.

## Main materialization handoff

The main conductor must re-read live artifacts and use this file as reviewed
post-main evidence. Locked main Decisions remain authoritative until replaced
through their canonical successor path. Materialize in dependency order, one
fork at a time, with fresh adversarial review and direct readback after each:

1. **vNext Recipe catalog and v1 disposition.** Canonicalize FB-1, FB-2,
   FB-7 through FB-19, FB-21 and the ownership correction FB-31. Freeze the
   exact ten active names, two Continuation profiles and complete 15-source v1
   disposition. Supersede only
   older clauses that leave `unattended`, `feature-fanout`, Audit,
   Adversarial Review, Generate/Filter or the old count active. After locking,
   update feature acceptance `ac-2`: ten is the vNext active Recipe count,
   fifteen is only the complete v1 source-disposition count and Bounded
   Continuation has exactly two profiles.
2. **Recipe grammar and composition.** Canonicalize FB-6 and FB-26. Replace
   the older one-exact-Recipe selector with one optional primary plus the sole
   optional Bounded Continuation overlay; preserve Projection ownership and
   every existing no-state/no-worker/no-retry law.
3. **Packet-to-job routing and public rendering.** Canonicalize FB-4, FB-24
   and FB-29. Add Integration-owned `JobRouteV1`, refusal and visible route
   parity without changing `AgentPacketV1`, Projection or the exact MCP Packet
   envelope.
4. **Review and compound-intent routing.** Canonicalize effective FB-5,
   FB-13 and FB-14. Preserve one job per tick, reviewer independence and fresh
   Packet re-entry.
5. **Closed Job-Recipe relation.** Canonicalize FB-27 only after the Recipe
   catalog and route contract are fixed.
6. **Capability Resource tree and Job-Method relation.** Canonicalize FB-3,
   FB-20 and FB-28 using the exact 35-file ledger. Narrowly supersede only the
   TDD-in-Review wording in Decision 7305; preserve its one-Skill/seven-job,
   lazy-loading, migration, no-alias and parity laws.
7. **Activation and context proof contracts.** Canonicalize FB-22 and FB-23.
   Reuse the existing `SkillActivation` Observation kind and frozen identity
   law; do not change the 43-kind catalog unless literal materialization proves
   an unavoidable semantic delta.
8. **Distribution cutover and removal closure.** Materialize FB-16, FB-25,
   FB-30 and FB-32 against the final corrected InstallationDomain topology,
   two-snapshot law,
   custody protocol and E204/C325/28,102 evidence without cross-domain
   authority or implicit retention.
9. Recompose the one canonical design top-down; rerun affected advisors,
   Capability Census, migration/rollback, adapter parity, journeys, Unknowns
   Lens and the complete no-fork edge sweep. Only then regenerate the Final
   Build Approval Packet and stop for the exact approval token.

The main conductor must not batch independent forks, weaken locked core laws,
implement source, create executable build Tasks, reconcile/finalize/accept the
feature, commit, push, install, release or infer build approval from this
handoff.
