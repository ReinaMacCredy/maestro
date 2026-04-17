# AGENTS.md
# TypeScript Style Guide

## Types
- Prefer `interface` for object shapes and `type` for unions or intersections
- Avoid `any`; use `unknown` and narrow with type guards
- Use `readonly` for immutable data
- Prefer `const` assertions for literal types
- Use discriminated unions over optional fields for variant types

## Naming
- Types and interfaces: PascalCase
- Variables and functions: camelCase
- Constants: UPPER_SNAKE_CASE
- Enums: PascalCase for both enum names and members
- Files: kebab-case

## Functions
- Prefer arrow functions for callbacks and short expressions
- Use named functions for top-level declarations
- Add explicit return types for public API functions
- Use function overloads sparingly; prefer union types

## Async
- Always `await` promises; avoid fire-and-forget flows
- Use `Promise.all()` for parallel independent operations
- Handle errors with `try/catch` at the boundary rather than every call site
- Prefer `async/await` over `.then()` chains

## Imports
- Group imports by built-in, external, internal, then relative
- Use named imports instead of `import *`
- Avoid circular dependencies

## Nullability
- Prefer `undefined` over `null`
- Use optional chaining (`?.`) and nullish coalescing (`??`)
- Avoid non-null assertions except in tests or tightly constrained cases

## Testing
- Use `describe` and `it` for structure
- Mock external dependencies, not internal modules
- Test error paths in addition to happy paths

## Compiled Binary Verification
- After `bun run build`, verify CLI changes against the fresh repo build first: `./dist/maestro --version` and then `./dist/maestro <command-under-test>`
- Do not assume `maestro` on `PATH` is the fresh build; treat `./dist/maestro` and `/Users/reinamaccredy/.local/bin/maestro` as separate artifacts
- For user-facing CLI or TUI work, finish by running `bun run release:local` so the local `maestro` command on `PATH` is refreshed to the newest compiled build before sign-off
- When reviewing the Maestro TUI, start with `./dist/maestro mission-control --preview` to smoke-test a single read-only frame before doing interactive TTY validation
- If you need to verify the installed `maestro` command, run `command -v maestro` first and record the resolved path in your notes
- Before testing the installed `maestro` command, refresh it from `./dist/maestro` using atomic replacement with a temp file plus `mv`; do not rely on a plain in-place overwrite
- After `bun run release:local`, verify both `maestro --version` and `./dist/maestro --version`, and record the installed path from `command -v maestro`
- For Mission Control or other TTY smoke tests, prefer `./dist/maestro mission-control ...` unless the goal is specifically to validate the installed command on `PATH`
- Every verification summary must state which binary was exercised: `./dist/maestro` or installed `maestro` on `PATH`

## Mission Control Contracts
- Keep `buildSnapshot()` and `buildHomeSnapshot()` read-only; do not perform runtime recovery, feature updates, or other state mutation inside snapshot projection
- Reply ingest is the sole sanctioned side effect and is strictly gated by `SnapshotBuildOptions.includeReplies` defaulting to false
- `mission-control --json` and `mission-control --preview` must remain read-only inspection paths; recovery or supervision belongs only in explicit orchestration/supervised runtime paths. Those paths never set `includeReplies: true`
- Interactive Mission Control may set `includeReplies: true` when the `[R]` Principles modal is open. This is the only path where reply YAMLs can advance feature state or append principle outcomes to `.maestro/principles/outcomes.jsonl`
- When adding Mission Control tests, cover both source-run and compiled `./dist/maestro` behavior if the change affects interactive flow, polling, or TTY handling

## Agent-Optimized TUI Preview
- Use `--size WxH` (e.g. `--size 120x40`) for deterministic render dimensions; do not rely on terminal auto-detection in agent or CI contexts
- Use `--format plain` to force stripped text output; use `--format ansi` to force ANSI-styled output; omit for TTY auto-detect
- Use `--preview all` to render every applicable screen in one pass with labeled `--- <screen> ---` separators
- Use `--render-check` to validate all screens and get machine-parseable JSON with pass/fail per screen, warnings for `undefined`, `NaN`, empty body, and missing box corners
- `--render-check` and `--preview all` automatically skip screens that require a mission when in home mode
- After TUI code changes, validate with: `bun run build && ./dist/maestro mission-control --render-check --size 120x40`
- For live iteration during TUI development, use `bun tui:dev` (watches `src/tui/**`, re-renders on save); supports `--screen`, `--size`, `--check`, `--mission`, `--compiled` flags
- Available preview screens: `dashboard`, `features`, `dependencies`, `handoffs`, `config`, `memory`, `graph`, `agents`, `dispatch`, `events`, `tasks`, `timeline`, `principles`, `help` (aliases: `feat`, `deps`, `cfg`, `mem`, `agent`, `event`, `task`, `principle`)
- Mission-only screens: `dependencies`, `dispatch`, `timeline`. Home+mission screens: all others (including `principles`)

## Shell Gotchas
- When running `git commit -m ...` through `zsh -lc`, do not put Markdown backticks inside double-quoted commit messages; use single-quoted heredocs, a temp file, or escaped backticks to avoid accidental command substitution

## Environment-Sensitive Tests
- Treat `tests/integration/session-sourcepath.test.ts` as environment-dependent: if `sourcePath` existence assertions fail, verify the expected local Claude session artifact exists before blaming unrelated code changes

## Release and Commit Conventions
- Bump the Maestro CLI version for every repo-tracked code change that affects runtime behavior, CLI output, TUI behavior, storage behavior, or user-visible workflows so the running binary can be identified exactly after each change
- Treat documentation-only or comment-only changes as exempt from version bumps unless they ship alongside behavior changes
- Make the version bump part of the same working increment and commit as the behavior change; do not leave version updates for a later cleanup commit
- `bun scripts/auto-bump.ts` computes the next version from conventional commits and updates tracked version files, but does not build, tag, install, or publish by itself
- `bun scripts/ci.ts` is the full local release-prep flow: auto-bump, test, build, commit the release metadata, and install the local binary. It does not push tags
- `bun run release:local` only rebuilds and reinstalls the local `maestro` binary; it does not bump the version or create a release commit
- `bun run deploy` currently uses the manual bump flow (`bun run bump`) rather than `auto-bump`; do not assume `deploy` applies conventional-commit versioning unless it is updated explicitly
- Version scheme is `0.x.y` where `x` bumps on features or breaking changes (`feat`, `feat!`, `BREAKING CHANGE`) and `y` bumps on everything else (`fix`, `refactor`, `docs`, `chore`, `test`)
- `bun scripts/bump.ts feature` or `bun scripts/bump.ts patch` for manual bumps
- Keep commit messages in Conventional Commits format, e.g. `feat(mission): add retry reason support`
- Prefer `feat` for user-visible functionality, `fix` for bug fixes, `refactor` for internal restructuring, and `test` for test-only changes
- Automatic GitHub releases only publish when `main` receives a dedicated release commit whose subject exactly matches `chore(release): v<package.json version>`
- Ordinary commits on `main`, including README or docs updates, must not publish releases; only the dedicated `chore(release): v...` commit is release-eligible
- Normal release flow is: run `bun scripts/ci.ts`, then push or merge that dedicated release commit to `main`; GitHub Actions creates the remote tag and GitHub Release automatically
- Release trigger matching now uses the checked-out commit subject (`git log -1 --pretty=%s`), not the full multiline commit message; release commits may include a body, but the first line must still be exactly `chore(release): v<package.json version>`
- Do not treat a green `Release` workflow as proof that a GitHub Release exists; confirm the `Publish Release` job ran and verify the tag with `gh release list` or the Releases page

## Task vs Mission

Maestro has two ways to track work. They coexist and never overlap.

- Use **`maestro task`** (Claude-style blocker graph) when the unit of work is small, mutable, and the plan comes from outside maestro. Tasks are meant for the daily loop: inline capture, blocker-aware queries, explicit ownership, and lightweight completion metadata. Storage is a single `.maestro/tasks/tasks.jsonl` file, one JSON object per line
- Use **`maestro mission`** (contract + gates) when the work is a sprint with a formal plan file, milestones, worker reports, assertions, and an approval step. Missions are heavier and immutable after approval; they are the right tool when you want a verified plan on disk, not a queue

One-line summary: **mission answers "what are we building?", task answers "what do I do next?"**

### Daily agent loop with tasks

```bash
maestro task ready --json --limit 5               # find actionable work
maestro task claim tsk-abc                        # take ownership
maestro task update tsk-abc --status in_progress  # explicitly start work
# ... do the work, commit ...
maestro task update tsk-abc --status completed --reason "shipped"  # finish
# or: maestro task unclaim tsk-abc                # release without closing
# blockers: maestro task block tsk-a tsk-b        # tsk-a blocks tsk-b
```

`task claim` is ownership-only: it sets `assignee` and `claimedAt`, but status stays separate. Use `maestro task update <id> --status in_progress` when work actually starts, and `maestro task update <id> --status completed --reason "..."` when it is done. Blocked tasks cannot be claimed until every task in `blockedBy` is completed. When session auto-detection is unavailable, use `maestro task claim <id> --force --session <id>` or `maestro task unclaim <id> --force --session <id>` as the explicit operator recovery path.

### Storage policy

`.maestro/tasks/**` is intentionally repo-tracked: tasks, ready-state metadata, and completion-derived candidate hints are part of the durable execution trail for day-to-day work, so they should be reviewed and committed deliberately when they matter. `.maestro/missions/**` and `.maestro/handoffs/**` remain ignored runtime data because they are heavier local orchestration artifacts rather than the lightweight shared queue. Task completion reasons and candidate hints are persisted verbatim, so treat them with the same care you would use for commit messages or `maestro note --content`: useful context is good, secrets and throwaway venting are not.

### When both make sense

Tasks can live inside or alongside a mission. You can use a mission to hold the big plan, then create tasks for the smaller units under each milestone — the two systems do not cross-reference today, but tasks carry labels and a `parentId` so you can group them however you want.

## Feature-Folder Layout

`src/` is organized by feature, not by architectural layer. The migration landed in Phase 8 (2026-04-09) and the layout is now stable.

### Structure
- `src/features/<name>/` -- each is a bounded context with `commands/ usecases/ domain/ ports/ adapters/ services.ts index.ts`. Some features nest further (e.g. `features/mission/{feature,validation,checkpoint}/`). A feature's `services.ts` exposes a single `build<Name>Services(projectDir)` factory; a feature's `index.ts` is the public surface
- `src/infra/` -- plumbing: commands (init, doctor, status, install, update, uninstall, mission-control), usecases, config and git ports/adapters, shared infra domain (bootstrap-templates, config-types, git-types, status-types). Not a feature; is consumed by every feature through the infra public surface
- `src/shared/` -- truly generic utilities with no domain knowledge: `lib/` (fs, yaml, shell, sanitize, path-safety, template, output, output-capture), `domain/` (id, ui-config, defaults), plus top-level `errors.ts`, `version.ts`, `version-format.ts`
- `src/tui/` -- rendering and input. NOT a feature; legitimately reaches into features through their public surfaces via `tui/state/snapshot.ts`
- `src/services.ts` -- composition root; one line per feature via the feature's service factory
- `src/index.ts` -- commander root; one `register<Feature>Command` call per feature

### Current features (10)
- `ratchet` -- memory quality assertions and promotions
- `handoff` -- UKI-format structured session handoffs between agents
- `notes` -- project notes captured to a local file
- `graph` -- project dependency graph linking across repos
- `session` -- agent session identity detection
- `memory` -- corrections, learnings, recall for agent guidance
- `mission` -- mission/feature/milestone/checkpoint/validation/principle lifecycle; includes behavioral principles (`.maestro/principles.jsonl`) that inject into worker prompts (score) and gate handoff creation
- `agent` -- worker prompt generation, agent management, fit recommendation, prior handoff replay (legitimately imports from `mission`, `memory`, and `handoff`; see Feature-specific exceptions)
- `task` -- br-style issue graph for the daily loop (create, ready, claim, close); JSONL storage at `.maestro/tasks/tasks.jsonl`
- `bundle` -- package a mission + its artifacts (plan, features, workers, replies, handoffs, principles, memory) as a portable `.mission.tar.gz` bundle; read-only aggregator (see Feature-specific exceptions)

### Public-surface rule
Cross-feature imports MUST go through `@/features/<name>`, which resolves to that feature's `index.ts`. Deep paths across features are FORBIDDEN in both forms:
- Alias form: `@/features/memory/usecases/recall` is a violation
- Relative form: `../../memory/usecases/recall` is a violation

Enforced by `bun run check:boundaries`, which walks `src/features/*/**/*.ts` and flags any import matching `features/<other>/(adapters|usecases|domain|ports|lib|commands)/...` where `<other>` is not the importing feature.

### Exempt files (may legitimately cross feature boundaries)
- `src/services.ts` -- composition root; wires all feature adapters into the shared `Services` interface
- `src/index.ts` -- commander root; registers commands from every feature
- `src/tui/state/snapshot.ts` -- read-only aggregation pipeline that assembles mission-control dashboard state from every feature
- `src/infra/commands/mission-control.command.ts` -- cross-feature read-only dashboard view

All four live outside `src/features/*`, so the boundary-check glob never walks them; the exemption list is preserved defensively so the contract survives any future relocation.

### Feature-specific exceptions
- `agent` may import from `mission`, `memory`, and `handoff` through their public surfaces. Rationale: agent composes prompts from mission context, memory hints, and prior handoff replay, so the cross-feature dependencies are essential, not incidental
- `bundle` may import from `mission`, `reply`, `handoff`, and `session` through their public surfaces. Rationale: bundle is a read-only aggregator that snapshots every mission artifact into a portable archive; the cross-feature reads are the whole point of the feature
- No other feature has exceptions. If a future feature needs an enforcement exception, add it explicitly in [scripts/check-feature-boundaries-lib.ts](/Users/reinamaccredy/Code/maestro/scripts/check-feature-boundaries-lib.ts) with matching tests and update this section in the same change

### Enforcement workflow
- `scripts/check-feature-boundaries.ts` is the CLI entry point; `scripts/check-feature-boundaries-lib.ts` performs the scan and canonicalization for feature-source imports
- Runs automatically in `scripts/ci.ts` before `bun test`
- Manual invocation: `bun run check:boundaries`

### Adding a new feature
1. Create `src/features/<name>/` with `commands/ usecases/ domain/ ports/ adapters/`
2. Create `services.ts` exposing `build<Name>Services(projectDir)` -- the feature-local composition factory
3. Create `index.ts` exposing the minimal public surface (types, commands, use-case functions that siblings or `services.ts` need)
4. Wire into `src/services.ts`: add the feature's service interface to the `Services` intersection and spread the factory result into `initServices`
5. Register commands in `src/index.ts` via the feature's `register<Name>Command` export

### When NOT to add a feature
- Plumbing (init, doctor, config, git adapters) -- put it in `src/infra/`
- Truly generic primitives with no domain knowledge (file I/O, YAML parsing, shell exec, sanitization) -- put it in `src/shared/lib/`
- Cross-cutting type primitives (IDs, UI config) -- put it in `src/shared/domain/`
