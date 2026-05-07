# Proposed Code File Reorganization Plan

Status: proposal only. No source files should be moved until this plan is reviewed and accepted.

## Executive Summary

The current `src/` tree is not uniformly disorganized. The top-level shape is already sensible:

```text
src/
  features/    bounded product contexts
  infra/       CLI plumbing, install/update/init, config/git adapters
  shared/      generic utilities
  tui/         Mission Control read model and renderer
  index.ts     Commander root
  services.ts  composition root
```

The audit found 391 TypeScript/TSX source files under `src/`. Most small and medium features already use the established `commands/`, `usecases/`, `domain/`, `ports/`, `adapters/`, `services.ts`, and `index.ts` shape. The "too many scattered files" problem is mostly concentrated in four areas:

1. `src/features/task/`: 74 files and about 14.1k lines. It mixes daily task CRUD, contract lifecycle, continuation/recovery, candidate matching, NOW writing, cost budget, CLI parsing, and CLI formatting in one feature-level bucket.
2. `src/features/mission/`: 47 files and about 6.2k lines. It already has `feature/` and `reply/` subtrees, but the remaining root-level mission files still mix mission lifecycle, milestones, checkpoints, assertions, principles, workflow defaults, and aggregate reporting.
3. `src/tui/`: 37 files and about 10.2k lines. `state/`, `app/`, and `opentui/` are reasonable, but several files have become multi-concept modules, especially modal building, reducer logic, config inspection, projection, and component builders.
4. A few large single-purpose-but-overloaded files: `src/features/agent/usecases/generate-agent-prompt.usecase.ts`, `src/infra/usecases/manage-agents.usecase.ts`, `src/features/task/adapters/jsonl-task-store.adapter.ts`, `src/features/task/usecases/contract-workflows.usecase.ts`, `src/features/task/commands/task.command.ts`, and `src/features/task/commands/contract.command.ts`.

The recommended approach is not a repo-wide folder churn. It is a low-risk staged reorganization that keeps the existing top-level architecture, deepens a small number of overloaded modules, and creates shallow, intuitive subfolders only where the domain is already visible in filenames.

## Current Source Map

Top source concentrations:

| Area | Files | Approx lines | Current role |
|------|------:|-------------:|--------------|
| `src/features/task/` | 74 | 14,139 | Daily task loop, task contracts, ownership, continuations, candidates, local stores |
| `src/tui/` | 37 | 10,206 | Mission Control snapshots, reducer state, preview, interactive OpenTUI rendering |
| `src/features/mission/` | 47 | 6,200 | Missions, milestones, features, assertions, checkpoints, principles, replies |
| `src/infra/` | 29 | 4,266 | Init, status, install/update/uninstall, config/git, agent install plumbing |
| `src/features/handoff/` | 18 | 2,220 | Launch packets, prompt building, pickup, legacy handoff reads |
| `src/features/policy/` | 15 | 1,532 | Owners, risk/autopilot/release policy loading, pending loosenings |
| `src/shared/` | 23 | 1,460 | Generic filesystem, shell, path, YAML, output, version utilities |

Largest modules:

| File | Approx lines | Problem |
|------|-------------:|---------|
| `src/features/task/commands/task.command.ts` | 2,422 | Many subcommands plus ownership, stale-claim, continuation, contract, status, ready, and formatting orchestration helpers |
| `src/tui/app/modal-builders.ts` | 1,714 | Builds every modal family in one file |
| `src/tui/state/reducer.ts` | 1,429 | Action union, state transitions, navigation, modal behavior, config editor state |
| `src/features/task/adapters/jsonl-task-store.adapter.ts` | 1,103 | Store implementation plus graph normalization, slug uniqueness, locking, batch ID generation, mutation helpers |
| `src/features/task/usecases/contract-workflows.usecase.ts` | 1,081 | Full contract workflow facade plus amendment, close/reopen, ownership transfer, overlap detection, verdict helpers |
| `src/features/task/commands/contract.command.ts` | 1,062 | Contract CLI registration plus draft template parsing, editor integration, formatting, actor/session resolution |
| `src/tui/state/config-inspector.ts` | 791 | Config projection, row building, edit metadata, value display, provenance, doctor checks |
| `src/infra/usecases/manage-agents.usecase.ts` | 783 | Agent block injection plus bundled skill sync, manifest handling, symlink migration, cleanup |
| `src/features/agent/usecases/generate-agent-prompt.usecase.ts` | 726 | Prompt data loading, skill loading, memory recall, principle formatting, prompt composition |

## Existing Functional Flow

The important flows today are:

```text
CLI command registration:
src/index.ts
  -> src/services.ts
  -> src/features/<feature>/services.ts
  -> src/features/<feature>/commands/*.command.ts
  -> src/features/<feature>/usecases/*
  -> ports/adapters/domain

Mission Control:
src/index.ts
  -> src/infra/commands/mission-control.command.ts
  -> src/tui/state/snapshot-loader.ts
  -> src/tui/state/snapshot.ts
  -> src/tui/state/projection.ts and focused projection builders
  -> src/tui/state/reducer.ts and src/tui/app/preview-state.ts
  -> src/tui/opentui/app/* and src/tui/opentui/components/*

Task contract/verdict flow:
src/features/task/commands/*
  -> task stores and contract stores
  -> src/features/evidence/*
  -> src/features/verify/*
  -> src/features/risk/*
  -> src/features/verdict/*

Handoff flow:
src/features/handoff/commands/handoff.command.ts
  -> build/launch/list/pickup usecases
  -> task public surface
  -> mission public surface
```

These flows are worth preserving. The plan should make the folders explain those flows instead of inventing a different architecture.

## Design Principles

1. Preserve the top-level `features/`, `infra/`, `shared/`, and `tui/` seams.
2. Avoid deep nesting. A new folder is justified only when there are at least 3 related files or one large file that clearly contains multiple independent concepts.
3. Prefer subdomain folders over generic buckets when the domain name is obvious: `contracts`, `continuations`, `candidates`, `workflow`, `principles`, `replies`, `modals`.
4. Keep public imports stable where possible by updating `index.ts` barrels first, then moving internal files.
5. Use tiny commits. Every commit should leave `bun run check:boundaries`, targeted tests, and `bun run build` runnable.
6. Do not move generated files or generated template outputs unless a separate generation strategy is accepted.

## Proposed Target Structure

### 1. Task Feature

Current pain: `task` is a bounded context, but it contains several mature subdomains. The current `domain/`, `usecases/`, `ports/`, and `adapters/` buckets are too broad for a developer or agent to know where task contracts, continuation state, candidates, and run state live.

Proposed shape:

```text
src/features/task/
  commands/
    task.command.ts
    task-command-parsers.ts
    task-command-formatters.ts
    task-command-output.ts          # new, optional
    task-command-ownership.ts       # new, optional
    task-command-continuation.ts    # new, optional
    contract/
      contract.command.ts
      contract-l2.command.ts
      contract-formatters.ts        # extracted from contract.command.ts
      contract-draft.ts             # extracted from contract.command.ts
      contract-actor.ts             # extracted from contract.command.ts
    verify/
      task-verify.command.ts
      task-proof.command.ts
      task-budget.command.ts
    shared/
      command-silence.ts
      duration.ts
  domain/
    task/
      task-types.ts
      task-state.ts
      task-id.ts
      task-slug.ts
      task-validators.ts
      task-errors.ts
      task-batch-types.ts
      extract-keywords.ts
    contracts/
      contract-types.ts
      contract-state.ts
      verdict.ts
    candidates/
      task-candidate.ts
    continuations/
      task-continuation-types.ts
    run-state/
      run-state.ts
    now/
      now-md-format.ts
  ports/
    task-store.port.ts
    contracts/
      contract-store.port.ts
      contract-version-store.port.ts
      git-anchor.port.ts
    candidates/
      candidate-store.port.ts
    continuations/
      task-continuation-store.port.ts
      task-continuation-history.port.ts
    run-state/
      run-state-store.port.ts
  adapters/
    task-store/
      jsonl-task-store.adapter.ts
      jsonl-task-store-graph.ts       # extracted helper module
      jsonl-task-store-slugs.ts       # extracted helper module
      jsonl-task-store-locking.ts     # optional only if extraction remains clean
    contracts/
      fs-contract-store.adapter.ts
      fs-contract-version-store.adapter.ts
      git-anchor.adapter.ts
    candidates/
      fs-candidate-store.adapter.ts
    continuations/
      fs-task-continuation-store.adapter.ts
      fs-task-continuation-history-store.adapter.ts
    run-state/
      fs-run-state-store.adapter.ts
    now/
      now-md-writer.adapter.ts
  usecases/
    tasks/
      create-task.usecase.ts
      list-tasks.usecase.ts
      update-task.usecase.ts
      show-task.usecase.ts
      inspect-task.usecase.ts
      claim-task.usecase.ts
      unclaim-task.usecase.ts
      heartbeat-task.usecase.ts
      next-task.usecase.ts
      ready-tasks.usecase.ts
      group-tasks-by-track.usecase.ts
      manage-task-blockers.usecase.ts
      release-owned-tasks.usecase.ts
      delete-task-flow.usecase.ts
      reopen-task-flow.usecase.ts
      prune-local-task-state.usecase.ts
    contracts/
      contract-workflows.usecase.ts
      contract-workflows-create.ts       # extracted
      contract-workflows-amend.ts        # extracted
      contract-workflows-close-reopen.ts # extracted
      contract-workflows-verdict.ts      # extracted
      propose-contract.usecase.ts
      approve-contract.usecase.ts
      amend-contract.usecase.ts
      get-current-contract.usecase.ts
      get-contract-history.usecase.ts
      read-current-contract-with-backfill.ts
      check-cost-budget.ts
    candidates/
      capture-task-candidate.usecase.ts
      match-candidates.usecase.ts
      find-similar-tasks.usecase.ts
    continuations/
      task-continuation.usecase.ts
    planning/
      plan-tasks.usecase.ts
      batch-input-schema.usecase.ts
```

Rationale:

- This keeps the feature shallow at three to four levels and makes each subdomain visible by folder name.
- It helps a developer answer "where is contract behavior?" without scanning 74 files.
- It creates locality for tests: `tests/unit/features/task/contract/*` maps to `usecases/contracts/`, `domain/contracts/`, and `adapters/contracts/`.
- It preserves the current public seam at `src/features/task/index.ts`, so most callers can continue importing from `@/features/task`.

No-brainer move set:

| Move | Why | Calling code to update |
|------|-----|------------------------|
| `domain/contract/*` -> `domain/contracts/*` | Existing contract subfolder already exists; pluralize and make it the root for all contract domain types | All imports of `@/features/task/domain/contract/*` and relative imports from task files/tests |
| contract ports -> `ports/contracts/*` | Contract store, version store, and git anchor are one contract subsystem | `src/features/verdict/usecases/request-verdict.usecase.ts`, `src/tui/state/autopilot-screen.ts`, `src/tui/state/snapshot-loader.ts`, tests |
| contract adapters -> `adapters/contracts/*` | Matches contract ports and store tests | `src/features/task/services.ts`, contract tests, handoff pickup tests |
| contract usecases -> `usecases/contracts/*` | Makes L2/L3 contract workflows discoverable | task commands, plan/policy/verdict tests, contract tests |
| task CRUD/state usecases -> `usecases/tasks/*` | Separates daily task loop from contracts and candidates | `task.command.ts`, tests under `tests/unit/features/task/usecases` |
| candidate files -> `domain/candidates`, `ports/candidates`, `adapters/candidates`, `usecases/candidates` | Candidate matching is a distinct subsystem | ready/match/capture tests and `task.index.ts` |
| continuation files -> `domain/continuations`, `ports/continuations`, `adapters/continuations`, `usecases/continuations` | Continuation state is a distinct subsystem used by task show/claim/reopen | task command, handoff pickup, tests |
| run-state files -> `domain/run-state`, `ports/run-state`, `adapters/run-state` | Cost budget/run state is separate from task CRUD | verdict/ci/plan tests, `services.ts` |
| NOW files -> `domain/now`, `adapters/now` | NOW rendering is output state, not core task domain | `services.ts`, `task.command.ts` |
| command files into `commands/contract`, `commands/verify`, `commands/shared` | Top-level command bucket is currently overloaded | `task.command.ts`, `src/index.ts` for `contract-l2`, tests |

File split recommendations:

- Split `task.command.ts` after moves, not before. First move it intact and update imports. Then extract subcommand registration groups: create/update/list/status, ownership, dependencies/blockers, ready/next, maintenance, continuation helpers. The interface should remain `registerTaskCommand(program)`.
- Split `contract.command.ts` into registration, draft-source/editor parsing, formatting, actor/session resolution, and overlap warnings. The interface should remain `registerContractCommand(taskCmd, program)`.
- Split `contract-workflows.usecase.ts` into a facade plus helper modules. The public interface should remain `buildContractWorkflows(...)`, but create/edit/amend/close/reopen/verdict logic should live in named modules.
- Split `jsonl-task-store.adapter.ts` only if helper extraction reduces complexity without leaking store invariants. Good candidates are slug uniqueness, graph normalization, and batch ID generation. Do not split locking unless the extracted module still has a tiny interface.

### 2. Mission Feature

Current pain: `mission` already has nested `feature/` and `reply/` subtrees, but milestones, checkpoints, validation/assertions, principles, workflows, and reports still sit together at the root.

Proposed shape:

```text
src/features/mission/
  mission/
    commands/mission.command.ts
    domain/mission-types.ts
    domain/mission-state.ts
    domain/mission-id.ts
    domain/mission-defaults.ts
    domain/mission-validators.ts
    adapters/mission-store.adapter.ts
    ports/mission-store.port.ts
    usecases/mission-lifecycle.usecase.ts
    usecases/missions.usecase.ts
    usecases/mission-report.usecase.ts
    usecases/progress-derivation.usecase.ts
  milestones/
    commands/milestone.command.ts
    usecases/milestone-lifecycle.usecase.ts
  assertions/
    commands/validate.command.ts
    adapters/assertion-store.adapter.ts
    ports/assertion-store.port.ts
    usecases/validation-lifecycle.usecase.ts
  checkpoints/
    commands/checkpoint.command.ts
    adapters/checkpoint-store.adapter.ts
    ports/checkpoint-store.port.ts
    usecases/checkpoint-lifecycle.usecase.ts
  principles/
    commands/principle.command.ts
    adapters/principle-store.adapter.ts
    ports/principle-store.port.ts
    domain/default-principles.ts
    domain/principle-types.ts
    domain/principle-validators.ts
    usecases/principle-effectiveness.usecase.ts
  workflows/
    domain/workflow-types.ts
    domain/workflows.ts
  feature/
    ...keep existing subtree for now...
  reply/
    ...keep existing subtree for now...
```

Rationale:

- This makes the mission aggregate understandable by lifecycle concept without burying files under many generic buckets.
- `feature/` and `reply/` remain as-is initially because they are already intentional nested subtrees with tests and guidance.
- The root `src/features/mission/index.ts` can continue to export the same public API.
- This is a real design change relative to current `src/features/mission/AGENTS.md`, so that doc must be updated in the same commit if accepted.

No-brainer move set:

| Move | Why | Calling code to update |
|------|-----|------------------------|
| mission root domain/store/usecases -> `mission/*` | Concentrates mission aggregate lifecycle | many relative imports inside mission, `src/features/agent`, TUI types, tests |
| milestone command/usecase -> `milestones/*` | Milestone lifecycle is a visible CLI surface | `index.ts`, tests |
| validation/assertion files -> `assertions/*` | "validate" command is assertion update/show behavior | `index.ts`, tests |
| checkpoint files -> `checkpoints/*` | Checkpoints have separate store and command | `services.ts`, `index.ts`, tests |
| principle files -> `principles/*` | Principles are separate data and command surface | `services.ts`, `index.ts`, agent prompt, TUI reply projection |
| workflow files -> `workflows/*` | Workflow templates are not mission state machinery | init/mission lifecycle imports |

File split recommendations:

- Do not split `mission-validators.ts` in phase 1. It is large, but splitting before the folders settle would create churn across many tests.
- Consider a later split into `mission-input-schemas.ts`, `agent-report-schema.ts`, and `principle-shared-schemas.ts` only after the move is stable.
- Keep `missions.usecase.ts` as the aggregate read facade. Prior audit notes warned against using it to absorb reply ingest or mutation paths.

### 3. Mission Control TUI

Current pain: folder names are generally reasonable, but the files inside `state/`, `app/`, and `opentui/components/` are too broad.

Proposed shape:

```text
src/tui/
  state/
    snapshot/
      snapshot.ts
      snapshot-loader.ts
      snapshot-demand.ts
      snapshot-poll-cache.ts       # move from lib if accepted
    projections/
      projection.ts
      environment-projection.ts
      memory-projection.ts
      reply-projection.ts
      task-board.ts
      autopilot-screen.ts
      events.ts
    reducer/
      reducer.ts
      modal-reducer.ts             # extracted
      navigation-reducer.ts        # extracted
      config-editor-reducer.ts     # extracted
      types.ts                     # reducer-specific types if useful
    config/
      config-inspector.ts
      config-row-builders.ts       # extracted
      config-edit-meta.ts          # extracted
      config-display.ts            # extracted
    types/
      types.ts
      screen-types.ts
      mission-control-commands.ts
  app/
    preview-state.ts
    preview-contract.ts
    render-check-contract.ts
    interactive-shared.ts
    input-dispatch.ts
    modals/
      modal-builders.ts            # dispatcher only
      memory-modal.ts
      graph-modal.ts
      config-modal.ts
      task-modal.ts
      timeline-modal.ts
      command-palette.ts
  opentui/
    app/
    components/
      screen/
      layout/
      modals/
    testing/
  shared/
    modal-model.ts
    header-animation.ts
```

Rationale:

- The TUI already has good top-level seams. The goal is to make large files navigable by screen family and state concern.
- `state/projections` clarifies which files derive read models from feature services.
- `state/reducer` clarifies which files are pure state transitions.
- `app/modals` gives each modal family a file, which directly addresses the 1,714-line modal builder.
- `opentui/components` can stay presentational, with screen/layout/modal helpers separated only if extraction is straightforward.

No-brainer move set:

| Move | Why | Calling code to update |
|------|-----|------------------------|
| projection files -> `state/projections/*` | All are read-model derivations | `snapshot.ts`, tests under `tests/unit/tui/state` |
| snapshot files -> `state/snapshot/*` | Snapshot loader/build/demand are one subsystem | `infra/commands/mission-control.command.ts`, preview/render tests |
| reducer file -> `state/reducer/reducer.ts` | Opens room for reducer helpers | all imports of `@/tui/state/reducer.js` |
| split `modal-builders.ts` into `app/modals/*` | Each modal family is independently readable | OpenTUI builders, tests |
| config inspector split into `state/config/*` | Config rows/edit metadata/provenance are independent concepts | reducer, modal builders, config tests |

File split recommendations:

- Split `modal-builders.ts` first because it has clear internal functions per modal family and a stable public dispatcher `buildModalOptions(state)`.
- Split `reducer.ts` only after modal split, because many state helpers are coupled to modal kinds.
- Split `config-inspector.ts` into pure row builders and display helpers. Preserve the current public functions `buildConfigInspector`, `getConfigRowsForTab`, and `resolveConfigScopeForKey`.

### 4. Agent Prompt Generation

Current pain: `src/features/agent/usecases/generate-agent-prompt.usecase.ts` is 726 lines and combines data gathering, skill discovery, memory recall, principle rendering, reply contract text, and final prompt composition.

Proposed shape:

```text
src/features/agent/
  prompt/
    generate-agent-prompt.usecase.ts
    prompt-context.ts
    skill-loader.ts
    previous-reports.ts
    memory-section.ts
    principle-section.ts
    reply-contract-section.ts
    prompt-composer.ts
  index.ts
```

Rationale:

- The feature has only two files today, so generic `domain/ports/adapters` buckets would be artificial.
- A `prompt/` folder matches the actual product concept and keeps nesting shallow.
- The public API remains `generateAgentPrompt` from `@/features/agent`.

Calling code to update:

- `src/features/agent/index.ts`
- `src/features/mission/feature/commands/feature.command.ts`
- `src/tui/opentui/app/interactive.tsx`
- `tests/unit/features/agent/usecases/generate-agent-prompt.usecase.test.ts`

### 5. Infra Agent Install/Update Flow

Current pain: `src/infra/usecases/manage-agents.usecase.ts` is 783 lines and mixes four concerns: config block injection, bundled skill manifest sync, skill symlink migration, and stale cleanup.

Proposed shape:

```text
src/infra/usecases/agents/
  manage-agents.usecase.ts       # public inject/remove facade
  agent-config-blocks.ts
  bundled-skill-manifest.ts
  bundled-skill-sync.ts
  managed-skill-links.ts
  managed-skill-cleanup.ts
```

Rationale:

- This belongs in `infra`, not `features/agent`, because it installs agent config and bundled skills rather than generating mission agent prompts.
- It gives maintainers a clear place for install/update mechanics without deep nesting.
- The public interface remains `injectAgentBlocks` and `removeAgentBlocks`.

Calling code to update:

- `src/infra/commands/install.command.ts`
- `src/infra/commands/uninstall.command.ts`
- `src/infra/commands/update.command.ts`
- `tests/unit/infra/usecases/manage-agents.usecase.test.ts`

### 6. Keep Small Features Mostly As-Is

Do not reorganize these in the first implementation pass:

- `bundle`
- `ci`
- `deploy`
- `evidence`
- `graph`
- `handoff`
- `memory`
- `memory-ratchet`
- `merge`
- `notes`
- `plan`
- `policy`
- `review`
- `risk`
- `runtime`
- `session`
- `spec`
- `verdict`
- `verify`
- `shared`

Rationale:

- These already map well to the established feature shape.
- Moving them would create large import churn without much locality gain.
- Some are intentionally small: `review` has two files, `merge` has five files, `deploy` has five files.
- `handoff` is large enough to matter, but it already has a scoped `AGENTS.md`, a coherent launch/pickup/list layout, and no obvious "no-brainer" folder split beyond the existing buckets.

## Public Surface and Import Strategy

The safest migration strategy is:

1. Move files inside a feature.
2. Update the feature's `index.ts` so external callers keep importing from `@/features/<name>`.
3. Update internal relative imports.
4. Update tests that intentionally import internal files.
5. Run `bun run check:boundaries`.

Known external deep imports that need special handling:

| Current import surface | Current callers | Proposed handling |
|------------------------|----------------|-------------------|
| `@/features/task/ports/contract-store.port.js` | `verdict`, `tui`, tests | Export `ContractStoreQueryPort` from `@/features/task` or add a stable `@/features/task/contracts` barrel |
| `@/features/task/usecases/read-current-contract-with-backfill.js` | `verdict`, `tui`, tests | Export from `@/features/task` or `@/features/task/contracts` before moving |
| `@/features/evidence/ports/storage.js` | `ci`, `verdict`, tests | Consider exporting `EvidenceStorePort` from `@/features/evidence`; do not move evidence in phase 1 |
| `@/features/evidence/domain/types.js` | `ci`, tests | Use `@/features/evidence` barrel for `EvidenceRow`, payload types, and witness types |
| `@/features/verdict/domain/types.js` | `ci`, tests | Use `@/features/verdict` barrel for `Verdict` and `VerdictDecision` |
| `@/features/verify/domain/types.js` | task verify command and tests | Export `TrustFinding` from `@/features/verify`; already present |
| `@/features/policy/services.js`, `@/features/risk/services.js`, `@/features/verify/services.js` | verdict request usecase | Prefer passing service functions via deps or importing from public feature index if exposed |

Because the boundary checker enforces cross-feature public surface imports, any accepted implementation should reduce deep cross-feature imports before moving files. That is the first safety step.

## Test and Fixture Impact

The tests contain many internal imports. This is normal for unit tests, but it means file moves are not just source changes.

Important test areas:

| Test area | Impact |
|-----------|--------|
| `tests/unit/features/task/**` | High impact from task reorganization |
| `tests/integration/features/task/**` | Mostly path strings and CLI behavior, lower import churn |
| `tests/unit/features/mission/**` | High impact from mission reorganization |
| `tests/unit/tui/**` | High impact from TUI state/app reorganization |
| `tests/unit/infra/usecases/manage-agents.usecase.test.ts` | Impact from infra agent split |
| `tests/unit/features/agent/usecases/generate-agent-prompt.usecase.test.ts` | Impact from agent prompt split |
| `tests/unit/scripts/check-feature-boundaries.test.ts` | May need expected path updates if public surface rules change |

Implementation should not rely on search-and-replace alone. Use compiler errors and targeted tests to catch import misses.

## Phased Implementation Plan

### Phase 0: Stabilize Public Barrels

Goal: reduce deep cross-feature imports before any physical move.

Tasks:

1. Export currently leaked task contract types/ports/usecases from a stable surface.
   - Option A: `src/features/task/index.ts`
   - Option B: new `src/features/task/contracts.ts`
   - Recommendation: start with `index.ts` for compatibility; add `contracts.ts` only if `index.ts` becomes too noisy.
2. Update cross-feature callers to import from public surfaces where possible.
3. Add or update boundary checker expectations if the allowed public surfaces change.

Verification:

```bash
bun run check:boundaries
bun run build
```

### Phase 1: Task Folder Reorganization Without Splitting Logic

Goal: move task files into subdomain folders, keeping file contents mostly intact.

Tasks:

1. Move contract domain, ports, adapters, usecases, and commands to `contracts` folders.
2. Move candidate, continuation, run-state, and NOW files to matching folders.
3. Move daily task CRUD/status/ownership usecases to `usecases/tasks`.
4. Update `src/features/task/index.ts` and `src/features/task/services.ts`.
5. Update all relative imports and tests.

Verification:

```bash
bun run check:boundaries
bun test tests/unit/features/task
bun test tests/integration/features/task
bun run build
```

### Phase 2: Split Task Large Files

Goal: improve locality after path moves are stable.

Tasks:

1. Split `commands/task.command.ts` into registration groups while preserving `registerTaskCommand(program)`.
2. Split `commands/contract/contract.command.ts` into draft parsing, formatting, actor/session resolution, and registration.
3. Split `usecases/contracts/contract-workflows.usecase.ts` into facade plus workflow helper modules.
4. Split `adapters/task-store/jsonl-task-store.adapter.ts` only for low-risk pure helpers.

Verification:

```bash
bun test tests/unit/features/task/commands
bun test tests/unit/features/task/contract
bun test tests/unit/features/task/adapters
bun test tests/unit/features/task/usecases
bun run build
```

### Phase 3: Mission Subdomain Reorganization

Goal: make mission lifecycle, milestones, assertions, checkpoints, principles, workflows, feature, and reply visually distinct.

Tasks:

1. Move mission aggregate files into `mission/`.
2. Move milestone, assertion, checkpoint, principle, and workflow files into named subfolders.
3. Preserve `feature/` and `reply/` subtrees.
4. Update `src/features/mission/index.ts`, `services.ts`, and `src/features/mission/AGENTS.md`.
5. Update agent prompt, handoff, bundle, TUI, infra init, and tests.

Verification:

```bash
bun test tests/unit/features/mission
bun test tests/integration/features/mission
bun test tests/unit/features/agent
bun test tests/unit/features/bundle
bun run check:boundaries
bun run build
```

### Phase 4: TUI Reorganization and Splits

Goal: make Mission Control snapshot, projections, reducer, modal builders, and renderer easier to navigate.

Tasks:

1. Move snapshot files into `state/snapshot/`.
2. Move projection files into `state/projections/`.
3. Move broad type files into `state/types/` if this does not create circular import friction.
4. Move `reducer.ts` to `state/reducer/reducer.ts` with temporary exports.
5. Split `app/modal-builders.ts` into `app/modals/*`.
6. Split `state/config-inspector.ts` into `state/config/*`.
7. Update `src/tui/AGENTS.md` and `src/tui/README.md` if paths change.

Verification:

```bash
bun test tests/unit/tui
bun run build
./dist/maestro mission-control --render-check --size 120x40
```

### Phase 5: Agent and Infra Large-File Splits

Goal: split high-friction large modules after the core feature/TUI paths settle.

Tasks:

1. Move agent prompt generation into `src/features/agent/prompt/*`.
2. Split infra manage-agents into `src/infra/usecases/agents/*`.
3. Preserve public exports and command behavior.

Verification:

```bash
bun test tests/unit/features/agent
bun test tests/unit/infra/usecases/manage-agents.usecase.test.ts
bun run build
```

### Phase 6: Final Whole-Repo Verification

Goal: prove the refactor did not break the CLI or Mission Control.

Verification:

```bash
bun run check:boundaries
bun run typecheck
bun run build
bun run test
./dist/maestro mission-control --render-check --size 120x40
```

## Commit Strategy

Use tiny commits in this order:

1. `refactor(task): expose contract internals through public surface`
2. `refactor(task): group task files by subdomain`
3. `refactor(task): split task command helpers`
4. `refactor(task): split contract command helpers`
5. `refactor(task): split contract workflow helpers`
6. `refactor(mission): group mission files by lifecycle area`
7. `refactor(tui): group snapshot and projection files`
8. `refactor(tui): split modal builders by modal family`
9. `refactor(tui): split config inspector helpers`
10. `refactor(agent): split prompt generation helpers`
11. `refactor(infra): split agent install management helpers`
12. `docs(src): update source tree guidance after reorganization`

Each commit should update imports, tests, and relevant `AGENTS.md` or README path references in the same commit.

## Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Import churn creates many compile failures | High | Do one folder family per commit, run `bun run build`, use compiler errors to finish import updates |
| Boundary checker misses non-feature deep imports | Medium | Add explicit `rg` checks for `@/features/*/(domain|ports|adapters|usecases|commands)` after each phase |
| Tests import internal paths heavily | High | Update tests in the same commit as moves, run targeted test directories before full suite |
| Mission Control read-only contract gets blurred | High | Keep snapshot/projection moves separate from interactive write-path changes, run render-check |
| `mission` reorganization conflicts with existing `AGENTS.md` guidance | Medium | Update `src/features/mission/AGENTS.md` in the mission move commit |
| Large-file splitting changes behavior accidentally | High | Move first, split second, and preserve public function signatures |
| Generated template files get touched by accident | Medium | Do not move generated template files; exclude `src/infra/domain/*skill-templates.ts` from refactor |

## Explicit Non-Goals

- Do not rename top-level `features`, `infra`, `shared`, or `tui`.
- Do not introduce a new dependency.
- Do not change CLI behavior while moving files.
- Do not move tests into a new structure in the same pass unless import updates require it.
- Do not rewrite business logic as part of the folder move.
- Do not flatten `mission/feature` or `mission/reply` without a separate decision.
- Do not change generated files or skill sync behavior.

## Recommended First Implementation Slice

The safest first slice is Phase 0 plus the smallest part of Phase 1:

1. Export task contract ports/types/usecases through the task public surface.
2. Update cross-feature imports in `verdict`, `tui`, `ci`, and task verify to use public surfaces.
3. Run `bun run check:boundaries` and `bun run build`.
4. Move only task contract files into `domain/contracts`, `ports/contracts`, `adapters/contracts`, `usecases/contracts`, and `commands/contract`.
5. Run task contract tests and build.

This gives immediate proof that the reorganization pattern works before touching the wider task, mission, or TUI surfaces.

## Review Questions

1. Should task contract internals be exported from `@/features/task`, or should we create a narrower `@/features/task/contracts` public surface?
2. Should mission keep `feature/` and `reply/` as direct child subtrees, or should the accepted target be a fully symmetrical subdomain layout?
3. Should TUI `state/types.ts` stay in place to reduce import churn, or move into `state/types/types.ts` for consistency?
4. Should file split phases be included in the same refactor effort, or should the first effort be folder moves only?
5. Should tests remain at their current paths, or should a later pass mirror the new source subdomain folders?

