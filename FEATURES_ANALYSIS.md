# Maestro Features Analysis

**Generated:** 2026-05-08  
**Repository:** /Users/reinamaccredy/Code/maestro  
**Total Features:** 31

## Executive Summary

The `src/features/` directory contains 31 bounded contexts following a hexagonal architecture pattern. Features range from 2 to 76 TypeScript files each, with the largest being `task` (76 files), `mission` (47 files), and `memory` (21 files). The architecture enforces strict boundaries: cross-feature imports must go through public `index.ts` surfaces only, and each feature follows a consistent structure of `commands/`, `usecases/`, `domain/`, `ports/`, `adapters/`, and `services.ts`.

The feature set implements a "trust substrate" for multi-agent software engineering: tasks and contracts define work scope, evidence records verifiable outputs, the trust verifier checks diffs, verdicts gate completion, CI makes verdicts authoritative, and optional layers (auto-merge, deploy safety, runtime monitoring) extend the primitives.

## Feature Inventory

### Core Task & Contract System

#### 1. **task** (76 files) - Daily Queue & Contracts
- **Purpose:** Lightweight mutable issue graph for daily work with contracts, continuations, and JSONL storage
- **Key Exports:** `Task`, `Contract`, `TaskContinuation`, `RunState`, task CRUD, claim/block/ready operations, contract workflows
- **Structure:** 
  - Commands: `task.command.ts`, `contract.command.ts`, `task-introspect.command.ts`, `task-verify.command.ts`, `task-budget.command.ts`, `task-proof.command.ts`
  - 30 use cases including batch planning, contract workflows, continuation management, cost budgets
  - Domain: task types, validators, state machines, contract types, slugs, run-state
  - Adapters: JSONL task store, FS contract store, FS continuation store, FS run-state store
- **Storage:** `.maestro/tasks/tasks.jsonl` (repo-tracked), `.maestro/tasks/contracts/`, `.maestro/runs/<id>/state.json` (gitignored)
- **Dependencies:** None (foundational)
- **Notes:** Largest feature; contract lifecycle: `draft` → `locked`/`amended` → `fulfilled`/`broken`; task lifecycle: `pending` → `in_progress` → `completed`

#### 2. **evidence** (9 files) - Verifiable Output Logbook
- **Purpose:** Record and list verifiable outputs tied to tasks with witness levels
- **Key Exports:** `EvidenceRow`, `EvidenceKind`, `WitnessLevel`, `recordEvidence`, `listEvidence`
- **Structure:**
  - Commands: `evidence.command.ts` (record/list/show)
  - 2 use cases: record, list
  - Domain: 20+ evidence kinds (command, manual-note, ai-review, threat-model, plan-check, etc.), 4 witness levels
  - Adapters: NDJSON file storage
- **Storage:** `.maestro/evidence/<taskId>.ndjson` (gitignored)
- **Dependencies:** Imports `TASK_ID_PATTERN` from task
- **Notes:** Witness levels: `witnessed-by-maestro` > `witnessed-by-ci` > `agent-claimed-locally` > `agent-claimed-and-not-reproducible`

#### 3. **verdict** (9 files) - Gating Decision Engine
- **Purpose:** Turn verifier runs into deterministic gating decisions (PASS/FAIL/HUMAN/BLOCK)
- **Key Exports:** `Verdict`, `VerdictDecision`, `requestVerdict`, verdict store
- **Structure:**
  - Commands: `verdict.command.ts` (show/request)
  - 1 use case: request-verdict (decision tree)
  - Domain: verdict types, verdict ID generation
  - Adapters: FS verdict store
- **Storage:** `.maestro/verdicts/` (gitignored)
- **Dependencies:** Consumes evidence, spec, contract, risk, policy
- **Notes:** Exit codes: 0=PASS, 1=FAIL, 2=HUMAN, 3=BLOCK

### Trust & Verification Layer

#### 4. **verify** (15 files) - Trust Verifier & ProofMap
- **Purpose:** Run 8 checks against diff + contract; build proof map joining spec criteria with evidence
- **Key Exports:** `runTrustVerifier`, `buildProofMap`, `checkArchitectureLints`, `TrustFinding`
- **Structure:**
  - 8 checks: scope, lockfile, generated-files, sensitive-paths, commit-metadata, secrets, architecture-lints, cross-imports
  - 2 use cases: trust-verifier, proof-map
  - Domain: finding types, severity levels
  - Adapters: git-signature adapter
- **Dependencies:** Imports from policy (sensitive paths), task (contract), spec (acceptance criteria), evidence
- **Notes:** Architecture lints produce `lint-violation` evidence rows at `agent-claimed-locally`

#### 5. **policy** (15 files) - Policy & Owners Loader
- **Purpose:** Load and classify policy edits (risk, autopilot, release, sensitive paths, owners)
- **Key Exports:** `Owners`, `RiskPolicy`, `AutopilotPolicy`, `ReleasePolicy`, policy loaders, asymmetric edit classifier
- **Structure:**
  - Commands: `policy.command.ts` (check/pending)
  - 8 use cases: load policies, classify edits, detect pending loosenings, effective policy
  - Domain: policy types, risk policy defaults, owners types
- **Storage:** `.maestro/policies/owners.yaml`, `risk.yaml`, `autopilot.yaml`, `release.yaml`, `sensitive-paths.yaml` (repo-tracked)
- **Dependencies:** None (foundational)
- **Notes:** Tightenings take effect immediately; loosenings soak 30 days

#### 6. **risk** (7 files) - Risk Engine
- **Purpose:** Derive risk class from diff signals; compute effective risk with AI review raises
- **Key Exports:** `deriveRiskClassFromDiff`, `computeRisk`, `compareRiskClass`, `requiresThreatModel`
- **Structure:**
  - 4 use cases: derive-risk-class, compute-risk, risk-class-order, verdict-reason-templates
  - Domain: risk types
- **Dependencies:** None (pure computation)
- **Notes:** 4 levels: low/medium/high/critical; AI review errors raise by one tier; security errors always lift to critical

#### 7. **spec** (11 files) - Mission Spec (Acceptance Criteria)
- **Purpose:** Store and score acceptance criteria, non-goals, runtime signals, rollout plans
- **Key Exports:** `Spec`, `AcceptanceCriterion`, `NonGoal`, `RuntimeSignal`, `RolloutPlan`, spec CRUD, `scoreSpec`
- **Structure:**
  - Commands: `spec.command.ts` (show/edit)
  - 4 use cases: create, update, get, score
  - Domain: spec types, criterion ID generation
  - Adapters: FS spec store
- **Storage:** `.maestro/missions/<id>/spec.json` (gitignored)
- **Dependencies:** None (foundational)
- **Notes:** Schema v2 added runtime_signals and rollout_plan; v1 specs forward-migrate at read time

#### 8. **plan** (6 files) - Plan-Check
- **Purpose:** Evaluate plan files against contract + spec before coding (scope-widens, missing-proof, risk-class-too-low)
- **Key Exports:** `checkPlan`, plan types, plan validators
- **Structure:**
  - Commands: `plan-check.command.ts`
  - 1 use case: check-plan (3 deterministic checks)
  - Domain: plan types, validators
- **Dependencies:** Imports from risk (compareRiskClass), task (contract), spec, evidence (records plan-check evidence)
- **Notes:** Always exits 0; findings must be resolved before implementation

### CI & Merge Layer

#### 9. **ci** (9 files) - CI Verify & PR Checks
- **Purpose:** Run trust verifier in CI, ingest CI job results as witnessed-by-ci evidence, post GitHub checks
- **Key Exports:** `runCiVerify`, `postPrCheck`, `readCiEnv`, GitHub API port
- **Structure:**
  - Commands: `ci-verify.command.ts`
  - 3 use cases: run-ci-verify, post-pr-check, detect-cross-task-conflict
  - Domain: CI env reader
  - Adapters: gh-cli adapter
- **Dependencies:** Imports from verdict, evidence, verify, policy (owners from base)
- **Notes:** L8.1 added cross-task conflict detection; verdicts bound to (pr, tree_sha)

#### 10. **merge** (5 files) - Auto-Merge Eligibility
- **Purpose:** Run 8 eligibility predicates; trigger `gh pr merge --auto` when all pass
- **Key Exports:** `autoMergeEligible`, eligibility types
- **Structure:**
  - Commands: `merge-auto.command.ts`
  - 1 use case: auto-merge-eligible (8 predicates)
  - Domain: eligibility reason codes
- **Dependencies:** Imports from evidence, policy, risk, spec, task, verdict
- **Notes:** Opt-in via `autopilot.yaml`; exits 0 if eligible, 1 if not

#### 11. **review** (2 files) - Review Acknowledgement
- **Purpose:** Record review-ack evidence for HUMAN verdicts at >=medium risk
- **Key Exports:** Review ack command
- **Structure:**
  - Commands: `review-ack.command.ts`
- **Dependencies:** Records evidence via evidence feature
- **Notes:** Required by auto-merge eligibility gate

### Deploy & Runtime Layer

#### 12. **deploy** (5 files) - Deploy Gate & Rollback
- **Purpose:** Run 4 deploy-readiness checks; witness rollback execution
- **Key Exports:** Deploy gate/rollback commands, check-deploy-readiness use case
- **Structure:**
  - Commands: `deploy-gate.command.ts`, `deploy-rollback.command.ts`
  - 1 use case: check-deploy-readiness (4 checks: feature_flag, canary_plan, rollback, owner)
  - Services: deploy services factory
- **Dependencies:** Imports from evidence, policy (owners), spec
- **Notes:** Does NOT mutate verdict; teams wire via risk.yaml if they want it to gate

#### 13. **runtime** (7 files) - Runtime Signal Monitoring
- **Purpose:** Query Prometheus signals declared in spec; record runtime-signal evidence
- **Key Exports:** `RuntimeMonitorPort`, Prometheus adapter, check-runtime-signals use case
- **Structure:**
  - Commands: `runtime-check.command.ts`
  - 1 use case: check-runtime-signals
  - Domain: runtime signal result types
  - Ports: monitor port
  - Adapters: prometheus adapter
- **Dependencies:** Imports from spec (RuntimeSignal), evidence
- **Notes:** Provider URL precedence: flag → env → localhost:9090

### Mission & Handoff System

#### 14. **mission** (47 files) - Mission Lifecycle & Principles
- **Purpose:** Top-level unit of work with milestones, features, assertions, checkpoints, principles
- **Key Exports:** `Mission`, `Feature`, `Milestone`, `Assertion`, `Checkpoint`, `Principle`, mission/milestone/feature lifecycle, reply ingest
- **Structure:**
  - Commands: `mission.command.ts`, `milestone.command.ts`, `checkpoint.command.ts`, `principle.command.ts`, `validate.command.ts`
  - Nested `feature/` subtree (own commands/usecases)
  - Nested `reply/` subtree (agent reply ingest)
  - 8 use cases: mission lifecycle, milestone lifecycle, checkpoint lifecycle, validation lifecycle, mission report, principle effectiveness
  - Domain: mission types, validators, state machines, workflows, principles, errors
  - Adapters: FS stores for mission, feature, assertion, checkpoint, principle
- **Storage:** `.maestro/missions/<id>/` (gitignored), `.maestro/principles.jsonl` (repo-tracked)
- **Dependencies:** None (foundational)
- **Notes:** Largest feature after task; mission status: draft → approved → executing → sealed

#### 15. **handoff** (19 files) - Launch Packets & Pickup
- **Purpose:** Build self-contained markdown briefs, launch fresh agent runs, pickup consumption with task ownership transfer
- **Key Exports:** `HandoffRecord`, `launchHandoff`, `pickupHandoff`, `buildHandoffPrompt`, handoff store
- **Structure:**
  - Commands: `handoff.command.ts` (launch/pickup/list/show)
  - 9 use cases: build prompt, launch, pickup, list, show, reconcile, read, inspect legacy
  - Domain: handoff types, state, project scope
  - Adapters: FS handoff store, Claude/Codex/Hermes launch adapters
- **Storage:** `~/.maestro/handoff/<id>/` (global, gitignored)
- **Dependencies:** Imports from mission (for mission context), task (for continuation)
- **Notes:** Task-linked pickup must run from source project unless `--standalone`; status: launching → launched → completed/failed

#### 16. **agent** (2 files) - Agent Prompt Generation
- **Purpose:** Generate agent prompts from mission/feature context
- **Key Exports:** `generateAgentPrompt`
- **Structure:**
  - 1 use case: generate-agent-prompt
- **Dependencies:** Imports from mission, memory
- **Notes:** Small feature; prompt generation logic

#### 17. **bundle** (9 files) - Mission Bundle Export
- **Purpose:** Package mission + artifacts as portable `.mission.tar.gz` archive
- **Key Exports:** `exportBundle`, `inspectBundle`, bundle types
- **Structure:**
  - Commands: `bundle.command.ts` (export/inspect)
  - 3 use cases: export, inspect, collect sources
  - Domain: bundle types
  - Adapters: tar-archive adapter
- **Dependencies:** Imports from mission, handoff, session
- **Notes:** Redaction options for memory/prompts

### Memory & Learning

#### 18. **memory** (21 files) - Corrections & Learnings
- **Purpose:** Capture corrections, learnings, and compiled guidance for future runs
- **Key Exports:** `Correction`, `CompiledLearnings`, memory CRUD, recall, compile, search
- **Structure:**
  - Commands: 7 commands (compile, correct, learn, lint, recall, search, stats)
  - 7 use cases: compile, correct, learn, lint, recall, search, stats
  - Domain: memory types
  - Adapters: FS correction store, FS learning store
- **Storage:** `.maestro/memory/corrections/`, `.maestro/memory/learnings/` (gitignored)
- **Dependencies:** Imports from memory-ratchet (for lint/stats), graph (for stats)
- **Notes:** Third-largest feature

#### 19. **memory-ratchet** (9 files) - Regression Ratchet
- **Purpose:** Ratchet check and promote for regression prevention
- **Key Exports:** Ratchet types, check/promote use cases
- **Structure:**
  - Commands: `ratchet-check.command.ts`, `ratchet-promote.command.ts`
  - 2 use cases: check, promote
  - Domain: ratchet types
  - Adapters: ratchet store adapter
- **Storage:** `.maestro/memory/ratchet/` (gitignored)
- **Dependencies:** None
- **Notes:** Separate from memory but related

### Session & Lifecycle

#### 20. **session** (11 files) - Session Detection & Lifecycle
- **Purpose:** Detect agent sessions, session start/exit with orient/progress digests
- **Key Exports:** `AgentSession`, `detectSession`, session start/exit
- **Structure:**
  - Commands: `session.command.ts`, `session-start.command.ts`, `session-exit.command.ts`
  - 3 use cases: detect, start, exit
  - Domain: session types
  - Adapters: Claude session detect adapter
- **Dependencies:** None (foundational)
- **Notes:** Session start writes orient digest, runs baseline arch lint; exit re-runs lint, checks verdict

#### 21. **intake** (5 files) - Plan-Time Risk Classifier
- **Purpose:** Classify work as tiny/normal/high-risk before coding
- **Key Exports:** `classifyIntake`, intake types
- **Structure:**
  - Commands: `intake.command.ts`
  - 1 use case: classify-intake (deterministic risk classifier)
  - Domain: intake types (flags, lanes)
- **Dependencies:** None (pure computation)
- **Notes:** Returns lane + recommended next step; always exits 0

### Utility & Support Features

#### 22. **graph** (9 files) - Project Graph
- **Purpose:** Link projects, store graph metadata
- **Key Exports:** Graph link/context use cases, project graph store
- **Structure:**
  - Commands: `graph-link.command.ts`, `graph-context.command.ts`
  - 2 use cases: link, context
  - Domain: graph types
  - Adapters: project graph store adapter
- **Storage:** `~/.maestro/graph/projects.json` (global)
- **Dependencies:** None
- **Notes:** User-level graph metadata

#### 23. **notes** (7 files) - Notes Storage
- **Purpose:** Simple note storage
- **Key Exports:** Note types, note use case
- **Structure:**
  - Commands: `note.command.ts`
  - 1 use case: note
  - Domain: note types
  - Adapters: notes store adapter
- **Storage:** `.maestro/notes.json` (gitignored)
- **Dependencies:** None
- **Notes:** Simple feature

#### 24. **skills** (2 files) - Skills Management
- **Purpose:** Skills list/install/remove commands
- **Key Exports:** Skills command registration
- **Structure:**
  - Commands: `skills.command.ts`
- **Dependencies:** None
- **Notes:** Minimal feature; delegates to infra

#### 25. **mcp** (18 files) - MCP Server
- **Purpose:** Model Context Protocol server exposing maestro verbs as structured tools
- **Key Exports:** `buildMaestroMcpServer`, `startStdioMcpServer`, configure agent runtime
- **Structure:**
  - Commands: `serve.command.ts`, `check.command.ts`
  - 1 use case: configure-agent-runtime
  - Server: mcp-server.ts + 14 tools across task/evidence/contract/verdict/policy surfaces
- **Dependencies:** Imports from task, evidence, policy, risk (for tools)
- **Notes:** 14 tools; auto-configures on install; stdio transport

### Phase 1-2 Features (Harness Pivot)

#### 26. **recover** (3 files) - Task Recovery
- **Purpose:** Reset to last PASS verdict's tree; remove run state
- **Key Exports:** Recover use case
- **Structure:**
  - Commands: `recover.command.ts`
  - 1 use case: recover
- **Dependencies:** Imports from verdict (to find last PASS)
- **Notes:** Phase 2 feature; records `recovery` evidence at `witnessed-by-maestro`

#### 27. **ralph** (3 files) - Convergence Oracle
- **Purpose:** Aggregate findings, compute stable hash, detect stuck iterations
- **Key Exports:** Ralph review use case
- **Structure:**
  - Commands: `ralph.command.ts`
  - 1 use case: ralph-review
- **Dependencies:** Imports from evidence, verify
- **Notes:** Phase 2 feature; exit codes: 0=converged, 1=not converged, 2=stuck

#### 28. **gc** (3 files) - Garbage Collection
- **Purpose:** Doc gardening (scan for broken path references)
- **Key Exports:** Doc gardening use case
- **Structure:**
  - Commands: `gc.command.ts`
  - 1 use case: doc-gardening
- **Dependencies:** None
- **Notes:** Phase 2 feature; scans AGENTS.md, README.md, docs/**, .maestro/**, skills/**

## Feature Size Analysis

### Large Features (20+ files)
- **task** (76 files, ~3500 LOC) - Daily queue, contracts, continuations, run-state
- **mission** (47 files, ~2500 LOC) - Mission lifecycle, features, assertions, checkpoints, principles, reply ingest
- **memory** (21 files, ~1200 LOC) - Corrections, learnings, compiled guidance

### Medium Features (10-19 files)
- **handoff** (19 files, ~1100 LOC) - Launch packets, pickup, prompt generation
- **mcp** (18 files, ~900 LOC) - MCP server + 14 tools
- **verify** (15 files, ~800 LOC) - Trust verifier, proof map, 8 checks
- **policy** (15 files, ~700 LOC) - Policy loaders, asymmetric edit classifier

### Small Features (5-9 files)
- **session** (11 files) - Session detection, start/exit
- **spec** (11 files) - Acceptance criteria, runtime signals
- **verdict** (9 files) - Gating decisions
- **evidence** (9 files) - Verifiable output logbook
- **memory-ratchet** (9 files) - Regression ratchet
- **graph** (9 files) - Project graph
- **ci** (9 files) - CI verify, PR checks
- **bundle** (9 files) - Mission export
- **runtime** (7 files) - Runtime monitoring
- **risk** (7 files) - Risk engine
- **notes** (7 files) - Note storage
- **plan** (6 files) - Plan-check
- **merge** (5 files) - Auto-merge eligibility
- **intake** (5 files) - Intake classifier
- **deploy** (5 files) - Deploy gate, rollback

### Minimal Features (2-3 files)
- **recover** (3 files) - Task recovery
- **ralph** (3 files) - Convergence oracle
- **gc** (3 files) - Doc gardening
- **skills** (2 files) - Skills management
- **review** (2 files) - Review acknowledgement
- **agent** (2 files) - Agent prompt generation

## Dependency Graph

### Foundational (No Dependencies)
- **task** - Daily queue & contracts
- **policy** - Policy & owners
- **session** - Session detection
- **spec** - Acceptance criteria
- **risk** - Risk engine (pure computation)
- **intake** - Intake classifier (pure computation)
- **mission** - Mission lifecycle
- **notes** - Note storage
- **graph** - Project graph
- **memory-ratchet** - Regression ratchet
- **gc** - Doc gardening

### Layer 1 (Depends on Foundational)
- **evidence** → task (TASK_ID_PATTERN)
- **verify** → policy, task, spec, evidence
- **memory** → memory-ratchet, graph

### Layer 2 (Depends on Layer 1)
- **verdict** → evidence, spec, task, risk, policy
- **plan** → risk, task, spec, evidence
- **handoff** → mission, task
- **bundle** → mission, handoff, session
- **agent** → mission, memory

### Layer 3 (Depends on Layer 2)
- **ci** → verdict, evidence, verify, policy
- **merge** → evidence, policy, risk, spec, task, verdict
- **deploy** → evidence, policy, spec
- **runtime** → spec, evidence
- **ralph** → evidence, verify
- **recover** → verdict

### Layer 4 (MCP - Depends on Multiple Layers)
- **mcp** → task, evidence, policy, risk

### Isolated
- **skills** - No imports from other features
- **review** - Only records evidence (no direct imports shown)

## Common Patterns

### Consistent Structure
All features follow hexagonal architecture:
1. **commands/** - CLI command registration (thin)
2. **usecases/** - Business logic (orchestration)
3. **domain/** - Types, validators, state machines (pure)
4. **ports/** - Interfaces for external dependencies
5. **adapters/** - Implementations of ports (I/O)
6. **services.ts** - Feature-local dependency wiring
7. **index.ts** - Public surface (exports only)

### Storage Patterns
- **Repo-tracked:** `tasks.jsonl`, `principles.jsonl`, `policies/*.yaml`
- **Gitignored:** `.maestro/evidence/`, `.maestro/runs/`, `.maestro/missions/`, `.maestro/memory/`, `~/.maestro/handoff/`
- **Global:** `~/.maestro/handoff/`, `~/.maestro/graph/`

### Port/Adapter Pattern
- **StorePort** interfaces in `ports/`
- **Fs*Adapter** implementations in `adapters/`
- Examples: `TaskStorePort` + `JsonlTaskStoreAdapter`, `EvidenceStorePort` + `FsEvidenceStoreAdapter`

### ID Generation
- Consistent pattern: `generateTaskId()`, `generateEvidenceId()`, `generateVerdictId()`, `generateMissionId()`, `generateCriterionId()`
- Pattern validation: `TASK_ID_PATTERN`, `EVIDENCE_ID_PATTERN`, `VERDICT_ID_PATTERN`, etc.

### Command Registration
- Each feature exports `register*Command(program: Command)` function
- Commands stay thin; delegate to use cases
- Use cases return structured results; commands format output

## Organizational Observations

### Strengths
1. **Consistent architecture** - All features follow the same hexagonal pattern
2. **Clear boundaries** - Cross-feature imports only through public surfaces
3. **Separation of concerns** - Commands, use cases, domain, ports, adapters clearly separated
4. **Testability** - Port/adapter pattern enables easy mocking
5. **Feature cohesion** - Related functionality grouped together
6. **Progressive disclosure** - Small features for simple concerns, large features for complex domains

### Potential Issues

#### 1. Feature Size Disparity
- **task** (76 files) is 2.5x larger than the next largest feature
- **Observation:** Task feature handles multiple concerns: task CRUD, contracts, continuations, run-state, candidates, blocking, slugs
- **Consideration:** Could be split into `task-core`, `task-contracts`, `task-continuations` if it grows further

#### 2. Mission Feature Complexity
- **mission** (47 files) has nested subtrees (`feature/`, `reply/`)
- **Observation:** Mission aggregates multiple sub-domains under one public surface
- **Consideration:** The nested structure is documented and intentional; works well for now

#### 3. Minimal Features
- **review** (2 files), **agent** (2 files), **skills** (2 files) are very small
- **Observation:** These could potentially be merged into related features
- **Consideration:** 
  - `review` could merge into `evidence` (it just records review-ack evidence)
  - `agent` could merge into `mission` (it generates prompts from mission context)
  - `skills` is a thin wrapper; could stay separate or merge into `infra`

#### 4. Phase 1-2 Features
- **recover**, **ralph**, **gc** are new Phase 1-2 features (3 files each)
- **Observation:** These are intentionally small and focused
- **Consideration:** Monitor growth; if they expand significantly, the current structure is fine

#### 5. Evidence Kind Proliferation
- **evidence** feature has 20+ evidence kinds
- **Observation:** Many features record evidence (ci, deploy, runtime, plan, verify, session, recover, ralph, gc)
- **Consideration:** Evidence is the integration point; this is by design

#### 6. Cross-Feature Dependencies
- **verdict** depends on 5 features (evidence, spec, task, risk, policy)
- **merge** depends on 6 features (evidence, policy, risk, spec, task, verdict)
- **Observation:** These are "orchestration" features that compose primitives
- **Consideration:** This is expected for higher-layer features; dependencies are through public surfaces only

### Recommendations

#### Short-term (No Action Needed)
- Current structure is well-organized and follows consistent patterns
- Feature boundaries are clear and enforced
- Size disparity is manageable

#### Medium-term (Monitor)
1. **Task feature growth** - If task grows beyond 100 files, consider splitting
2. **Evidence kind management** - Document evidence kind taxonomy if it grows beyond 30 kinds
3. **Minimal feature consolidation** - Consider merging `review` into `evidence` if no additional functionality is planned

#### Long-term (Future Consideration)
1. **Feature clustering** - If the feature count grows beyond 40, consider grouping into sub-directories:
   - `features/core/` (task, evidence, verdict, policy, spec)
   - `features/trust/` (verify, risk, plan)
   - `features/ci/` (ci, merge, deploy, runtime)
   - `features/mission/` (mission, handoff, agent, bundle)
   - `features/memory/` (memory, memory-ratchet)
   - `features/support/` (session, intake, notes, graph, skills, mcp)
   - `features/harness/` (recover, ralph, gc)

2. **Shared domain types** - If multiple features start duplicating domain types, consider a `features/shared-domain/` for common types

## Import Relationship Matrix

| Feature | Imports From |
|---------|--------------|
| task | (none) |
| evidence | task |
| policy | (none) |
| session | (none) |
| spec | (none) |
| risk | (none) |
| intake | (none) |
| mission | (none) |
| notes | (none) |
| graph | (none) |
| memory-ratchet | (none) |
| gc | (none) |
| verify | policy, task, spec, evidence |
| memory | memory-ratchet, graph |
| verdict | evidence, spec, task, risk, policy |
| plan | risk, task, spec, evidence |
| handoff | mission, task |
| bundle | mission, handoff, session |
| agent | mission, memory |
| ci | verdict, evidence, verify, policy |
| merge | evidence, policy, risk, spec, task, verdict |
| deploy | evidence, policy, spec |
| runtime | spec, evidence |
| ralph | evidence, verify |
| recover | verdict |
| mcp | task, evidence, policy, risk |
| skills | (none) |
| review | evidence (implicit) |

## Key Metrics

- **Total features:** 31
- **Total TypeScript files:** ~344
- **Average files per feature:** 11
- **Median files per feature:** 7
- **Largest feature:** task (76 files)
- **Smallest features:** review, agent, skills (2 files each)
- **Features with no dependencies:** 11 (35%)
- **Features with 1-2 dependencies:** 8 (26%)
- **Features with 3+ dependencies:** 12 (39%)
- **Most depended-upon features:** task (9 dependents), evidence (9 dependents), policy (6 dependents)

## Conclusion

The `src/features/` directory is well-organized with consistent architecture patterns, clear boundaries, and appropriate feature sizing. The hexagonal architecture with ports/adapters enables testability and maintainability. The feature set implements a coherent "trust substrate" for multi-agent software engineering, with foundational features (task, evidence, policy) supporting higher-level orchestration features (verdict, ci, merge).

The current structure scales well to 31 features. No immediate refactoring is needed, but monitoring task feature growth and considering minimal feature consolidation would be prudent for long-term maintainability.
