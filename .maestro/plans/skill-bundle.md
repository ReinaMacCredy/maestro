# Plan: Ship maestro 5-skill bundle, replace AGENT_INSTRUCTION_BLOCK

## Context

Today `maestro agent inject` writes a 245-line static string (`AGENT_INSTRUCTION_BLOCK` in `src/infra/domain/bootstrap-templates.ts`) into each supported agent's home directory as `MAESTRO.md`, and injects a `@MAESTRO.md` reference into the agent's main config file. This turns into ambient prose that every Claude/Codex session reads on startup.

Two problems with that model:

1. It is a single opaque block. Agents cannot "load just the handoff guidance" or "load just the task workflow". Every session pays for the full block.
2. It drifts from the CLI. The block documents `handoff list` and `handoff show` subcommands that only exist in `./dist/maestro` (0.56.0), not in the installed binary (0.55.10). Content lives far from the commands it describes.

The user also wants to ship two existing skills they authored (`~/.claude/skills/preplan-brainstorm/` and `~/.claude/skills/execution-plan/`) under the maestro umbrella so the full workflow chains: brainstorm → plan → task → implementation → handoff.

The change ships a 5-skill bundle:

1. `maestro-brainstorm` (renamed from `preplan-brainstorm`, chain points to `maestro-plan`)
2. `maestro-plan` (renamed from `execution-plan`, chain points to `maestro-task` and `maestro-handoff`)
3. `maestro-task` (new, auto-trigger when `.maestro/` is detected, converts plans to `maestro task plan --file -` batches)
4. `maestro-mission` (new, mission/feature/memory layer plus mission-control TUI reference)
5. `maestro-handoff` (new, paseo-handoff style, user-invoked, uses `maestro handoff --prompt-file`)

Source of truth moves to `skills/bundled/` in the maestro repo. `maestro agent inject` installs the 5 skills into `~/.claude/skills/maestro-<name>/` and `~/.codex/skills/maestro-<name>/`, and cleans up the legacy `MAESTRO.md` + `@MAESTRO.md` reference. `AGENT_INSTRUCTION_BLOCK` is deleted. `droid` and `gemini` are dropped from `SUPPORTED_AGENTS` (re-add later).

CLI adapts to serve the skill workflow: `maestro handoff` gains `--prompt-file <path>` so `maestro-handoff` can write a hand-crafted brief and launch with it; the auto-generated brief stays as a fallback.

## Target (definition of done)

- `skills/bundled/{maestro-brainstorm,maestro-plan,maestro-task,maestro-mission,maestro-handoff}/` exist with valid `SKILL.md` and reference files.
- `maestro agent inject` against `./dist/maestro` writes all 5 skills to `~/.claude/skills/maestro-<name>/` and `~/.codex/skills/maestro-<name>/`, and removes `~/.claude/MAESTRO.md`, `~/.codex/MAESTRO.md`, and any `@MAESTRO.md` reference line from the main config file.
- `maestro agent remove` deletes the skill directories it installed (inverse parity).
- `maestro handoff --prompt-file /tmp/brief.md --agent codex --json` launches using the supplied brief and persists it at `.maestro/launches/<id>/prompt.md`.
- `SUPPORTED_AGENTS` contains only `claude-code` and `codex`.
- `AGENT_INSTRUCTION_BLOCK` and `renderBlock` are deleted from the codebase.
- `PROJECT_BOOTSTRAP_TEMPLATES` untouched — it is per-project, not part of this change.
- `bun test` passes. `bun run check:boundaries` passes. `bun run check:skills` passes. New `bun run check:bundled-skills` passes.

## Constraints, assumptions, and decisions

- **Same install command.** No new user-facing command. `maestro agent inject` is the entry point for both install and upgrade.
- **Silent legacy cleanup.** `MAESTRO.md` and `@MAESTRO.md` removal happens silently on next `agent inject`, matching current silent inject behavior. Released notes will call this out for users who may have hand-edited `MAESTRO.md`.
- **Global install only.** Skills land in home dirs, not per-project. Project-level installs for bundled skills are out of scope for this change.
- **Claude + Codex only.** `droid` and `gemini` are removed entirely. Their orphan files (`~/.droid/MAESTRO.md`, `~/.gemini/MAESTRO.md`) are not cleaned up by this change; release notes tell users to delete them manually.
- **`PROJECT_BOOTSTRAP_TEMPLATES` stays unchanged.** Its tests must be preserved when rewriting `bootstrap-templates.test.ts`.
- **`--prompt-file` writes into `launchStore`.** The early-return bypass in `buildHandoffPrompt` produces a final prompt string; `launchStore.create` still persists it to `.maestro/launches/<id>/prompt.md`. The flag changes the *source* of the prompt, not the *destination*.
- **`ackage.json` bug is an investigation, not a pre-diagnosed fix.** The Plan agent noted that `git.adapter.ts:120` rename parsing looks correct. The actual root cause must be found before coding a fix. Scope: if the cause is a small local bug, fix in branch; if it turns out to be a refactor, defer to follow-up.
- **Artifact non-persistence is an investigation.** `handoff list` showed `crisp-swan-1` without `.maestro/launches/crisp-swan-1/` on disk during manual probe. Confirm repro, find root cause, decide fix scope.
- **Skill content is authored against planned post-Phase-3 CLI.** `maestro-handoff/SKILL.md` will reference `--prompt-file` even though the flag does not exist until Phase 3. A brief comment in the skill body notes "requires maestro >= 0.57" (or whichever version ships this change) so users upgrading see the reason for any "unknown option" error.
- **`maestro-plan` persists final plans to `.maestro/plans/<slug>.md`.** The existing `.maestro/plans/` directory is a convention in this repo (8 existing plan files with topic-slug names and consistent markdown structure). `maestro-plan` extends its output step so approved plans become durable, searchable references. Gate: only when a maestro project is detected. Collision handling: numeric suffix, never overwrite.

## Phased plan

### Phase 1 — Author 5 skills (the contract frame)

**Purpose:** Produce the skill content that defines the maestro workflow. No install wiring, no CLI changes. Content is the spec.

**Tasks:**

1. Create `skills/bundled/` at repo root.
2. Write `skills/bundled/maestro-handoff/SKILL.md` (frontmatter + workflow + parsing + brief template + launching + after-launch + pickup + reference pointers). Add `skills/bundled/maestro-handoff/reference/brief-template.md` and `reference/pickup.md`.
3. Write `skills/bundled/maestro-task/SKILL.md` (when-to-activate, hard rules, plan-to-batch conversion, claim-and-start, contract flow, continuation state, completion, discovery, recovery). Add `reference/plan-conversion.md`, `reference/commands.md`, `reference/contracts.md`, `reference/recovery.md`.
4. Write `skills/bundled/maestro-mission/SKILL.md` (when-to-activate, mission/feature/memory commands, mission-control TUI reference). Add `reference/mission-lifecycle.md`, `reference/mission-control-tui.md`, `reference/assertions.md`.
5. Copy `~/.claude/skills/preplan-brainstorm/` into `skills/bundled/maestro-brainstorm/` (includes `SKILL.md`, `visual-companion.md`, `scripts/`, `spec-document-reviewer-prompt.md`).
6. Apply edits to `skills/bundled/maestro-brainstorm/SKILL.md`:
   - Frontmatter: `name: maestro-brainstorm`.
   - Hard rule 7: change `execution-plan` to `maestro-plan`.
   - "Hand off cleanly" section: change `execution-plan` to `maestro-plan`.
   - All absolute paths pointing at `/Users/reinamaccredy/.claude/skills/preplan-brainstorm/*` become relative (`./visual-companion.md`, `./scripts/start-server.sh`, etc.). Lines 34, 35, 107, 140, 141 per the Read snapshot.
7. Copy `~/.claude/skills/execution-plan/` into `skills/bundled/maestro-plan/`.
8. Apply edits to `skills/bundled/maestro-plan/SKILL.md`:
   - Frontmatter: `name: maestro-plan`.
   - "Start from the brainstorm handoff" section: change `preplan-brainstorm` to `maestro-brainstorm`.
   - "Hand off cleanly" section: replace with the maestro-aware chain (emit task-batch JSON matching `maestro task plan --schema`; hand off to `maestro-task` when `.maestro/` detected; hand off to `maestro-handoff` for cross-session transfer; generic fallback otherwise).
   - **Add new section "Persist the plan"** (after "Return an execution-ready answer"): when `.maestro/plans/` exists in the cwd or an ancestor, write the final approved plan to `.maestro/plans/<slug>.md` before handing off. Slug is derived from the plan subject (kebab-case, short, unique). Follow the existing convention in that directory: top-level `# <Title>`, `## Objective`, `## Scope` (In/Out), `## Research Findings`, `## Tasks` (checkbox list), `## Verification`, `## Notes`. Persisted plans are durable references future sessions can read with `maestro-mission` or any agent. If the file already exists at that slug, append a numeric suffix (`<slug>-2.md`) rather than overwrite. This section is skipped if no maestro project is detected (same gate as the `maestro-task` chain step).
9. Add a comment near `--prompt-file` in `maestro-handoff/SKILL.md` noting "requires maestro >= <next-version>".

**Dependencies:** None. Can start immediately.

**Verification:**
- Every `SKILL.md` has valid YAML frontmatter with `name:` matching the directory basename.
- No absolute paths starting with `/Users/` anywhere under `skills/bundled/`.
- Chain references are consistent: `maestro-brainstorm` references `maestro-plan`; `maestro-plan` references `maestro-task` and `maestro-handoff`. No references to `preplan-brainstorm` or `execution-plan`.
- `maestro-plan/SKILL.md` contains a "Persist the plan" section referencing `.maestro/plans/<slug>.md` and the collision suffix rule.
- Manual review of each skill against the brainstorm-approved content.

### Phase 2 — Rewire install pipeline

**Purpose:** Make `maestro agent inject` install the 5 skills globally and clean up legacy state. The pipeline is the only thing that changes in `src/`; skill content from Phase 1 is its input.

**Tasks:**

1. Create `scripts/sync-bundled-skills.ts`, patterned after `scripts/sync-built-in-skills.ts` but reading `skills/bundled/` and emitting a new `src/infra/domain/bundled-skill-templates.ts` export (`BUNDLED_SKILL_TEMPLATES: readonly BundledSkillTemplate[]`). Include a `--check` mode.
2. Add `package.json` scripts: `sync:bundled-skills` and `check:bundled-skills`. Wire `check:bundled-skills` into `scripts/ci.ts` the same way `check:skills` is wired.
3. Extend `src/features/agent/usecases/manage-agents.usecase.ts`:
   - Rewrite `processInject` to iterate `BUNDLED_SKILL_TEMPLATES` and write each skill's files under `join(homedir, agent.configDir, "skills", skillName, <file>)`. Use `homedir()`, not `process.cwd()` or `opts.dir`. Write all files per skill atomically.
   - Legacy cleanup step inside `processInject`:
     - Delete `join(homedir, agent.configDir, "MAESTRO.md")` if it exists (use `removeIfExists`).
     - Remove `@MAESTRO.md` reference from the main config file via existing `removeReference` helper.
     - Still clean up any old inline block via `removeBlock` / `removeLegacyBlock`.
   - Introduce a new action value `"migrated-to-skills"` in `InjectResult` to distinguish this from the existing `"migrated"`. Tests use this to assert the new code path.
   - Rewrite `processRemove` (counterpart) to delete the 5 installed skill directories (not just `MAESTRO.md`).
   - Add stale-skill cleanup: when `processInject` runs, delete any `~/.claude/skills/maestro-*/` directories that correspond to skills no longer in `BUNDLED_SKILL_TEMPLATES`. Mirror `removeStaleManagedSkillDirs` in `src/infra/usecases/init.usecase.ts`.
4. Prune `src/features/agent/domain/agents.ts`:
   - Remove `droid` and `gemini` entries from `SUPPORTED_AGENTS`.
   - Remove their legacy config paths from `agentLegacyConfigPaths`.
   - Add a single-line comment: `// droid and gemini will be re-added once skill support lands in those CLIs`.
5. Delete `AGENT_INSTRUCTION_BLOCK` and `renderBlock` from `src/infra/domain/bootstrap-templates.ts`. Keep `PROJECT_BOOTSTRAP_TEMPLATES` and `BootstrapTemplateFile` untouched. Remove the `AGENT_INSTRUCTION_BLOCK` import in `manage-agents.usecase.ts`.
6. Rewrite `tests/unit/infra/domain/bootstrap-templates.test.ts`:
   - Delete the 8 `it` blocks that assert on `AGENT_INSTRUCTION_BLOCK` (lines 9–72, approximately).
   - **Preserve** the `PROJECT_BOOTSTRAP_TEMPLATES` assertions (contract guidance mirror at line 73, shared task loop mirror at line 92, contract draft template at line 112, handoff launcher mention at line 121, primary agent verbs at line 126). These assertions guard `.maestro/AGENTS.md` bootstrap template content and must not be lost.
7. Extend `tests/unit/features/agent/usecases/manage-agents.usecase.test.ts`:
   - New test group: "skill install". Uses existing `mkdtemp` + `fakeHome` pattern. Asserts `injectAgentBlocks(tmpDir, "all", fakeHome)` writes all 5 skill `SKILL.md` files under `<fakeHome>/.claude/skills/maestro-<name>/` and `<fakeHome>/.codex/skills/maestro-<name>/`, and returns `action: "migrated-to-skills"` when legacy `MAESTRO.md` is present.
   - New test: legacy cleanup. Pre-populate `<fakeHome>/.claude/MAESTRO.md` and `<fakeHome>/.claude/CLAUDE.md` with `@MAESTRO.md`. Run `injectAgentBlocks`. Assert both are gone after.
   - New test: stale-skill cleanup. Pre-create `<fakeHome>/.claude/skills/maestro-obsolete/`. Run `injectAgentBlocks`. Assert the obsolete dir is removed.
   - New test: `removeAgentBlocks` deletes the 5 installed skill dirs.
8. Add `tests/unit/infra/domain/bundled-skill-templates.test.ts`: mirrors the built-in drift test. Reads `skills/bundled/` on disk and asserts the generated `BUNDLED_SKILL_TEMPLATES` matches.

**Dependencies:** Phase 1 (skill source files must exist for sync to produce non-empty output).

**Verification:**
- `bun run sync:bundled-skills` runs cleanly and updates `src/infra/domain/bundled-skill-templates.ts`.
- `bun run check:bundled-skills` exits 0 when in sync.
- `bun test tests/unit/features/agent/usecases/manage-agents.usecase.test.ts` passes.
- `bun test tests/unit/infra/domain/bootstrap-templates.test.ts` passes.
- `bun test tests/unit/infra/domain/bundled-skill-templates.test.ts` passes.
- `bun run build` succeeds.
- Manual smoke: `./dist/maestro agent inject` in a tmp `$HOME` writes the 5 skills and removes legacy state.

### Phase 3 — CLI adjustments to serve the skill workflow

**Purpose:** Add `--prompt-file` to `maestro handoff` and investigate the two bugs surfaced during probe. The skills from Phase 1 reference `--prompt-file`; Phase 3 makes that reference correct.

**Tasks:**

1. Add `--prompt-file <path>` option to `src/features/handoff/commands/handoff.command.ts` next to the existing flag definitions.
2. Plumb `promptFile` through the usecase input down to `src/features/handoff/usecases/build-handoff-prompt.usecase.ts:buildHandoffPrompt`.
3. In `buildHandoffPrompt`, early-return when `promptFile` is set: read the file, return `{ prompt: fileContents, context: <minimal context still built from git/mission/task inputs so launch record is well-formed> }`. Continuation summary and refs still populate from task-id / mission context when those inputs are also provided.
4. Validation before early-return:
   - Resolve relative paths against `process.cwd()` to absolute. Use an existing path helper if one is in `src/shared/lib/path-safety.ts`.
   - Throw `MaestroError` with hint if the file does not exist.
   - Throw `MaestroError` with hint if the file is empty.
   - Warn if the file is over 500KB but do not hard-fail.
   - Do not accept `-` (stdin) in this iteration. Document as a follow-up.
5. `launchStore.create` still writes the content to `.maestro/launches/<id>/prompt.md`. The bypass changes the *source* of the prompt, not the destination.
6. Investigate `package.json` → `ackage.json`. Steps:
   - Reproduce: create a fresh tmp git repo, make `package.json` dirty, run `maestro handoff "probe"`, inspect the generated `Relevant Files` list.
   - Search the rendering path: `src/features/handoff/usecases/build-handoff-prompt.usecase.ts:renderRelevantFiles` (line ~401) → `renderInlineCodeSpan` → `sanitizeInlineCodeContent` in `src/shared/lib/sanitize.ts`. Also audit `src/infra/adapters/git.adapter.ts:parseGitFileChanges` for rename-line handling.
   - Fix the root cause once located. Add a targeted unit test for the specific input that produced `ackage.json`.
7. Investigate artifact non-persistence. Steps:
   - Reproduce: run `./dist/maestro handoff "probe" --agent claude --name probe --json` in a clean repo, inspect `.maestro/launches/`.
   - If the dir is absent but `handoff list --json` shows the packet, the registry and artifact store are out of sync. Trace `launchStore.create` (`src/features/handoff/adapters/launch-store.adapter.ts:17`) and any downstream cleanup.
   - If the cause is a small local bug, fix. If it requires refactoring, defer to follow-up and document in the release notes.
8. Add tests:
   - `tests/unit/features/handoff/usecases/build-handoff-prompt.usecase.test.ts`: `--prompt-file` bypasses synthesis and returns file contents as the prompt.
   - Path validation tests: non-existent, empty, large, relative-vs-absolute.
   - Integration test: `maestro handoff --prompt-file <file>` produces a launch record whose `promptPath` points at `.maestro/launches/<id>/prompt.md` and that file contains the supplied content (not the auto-generated brief).
   - Regression test for `ackage.json` once the fix lands.

**Dependencies:** Phase 2 complete (install pipeline shipped so manual end-to-end tests against `./dist/maestro` reflect the new reality).

**Verification:**
- `./dist/maestro handoff --help` shows `--prompt-file <path>`.
- `./dist/maestro handoff --prompt-file /tmp/brief.md --agent claude --name probe --json` returns a JSON launch descriptor with `promptPath: ".maestro/launches/<id>/prompt.md"` and that file on disk matches `/tmp/brief.md`.
- `./dist/maestro handoff --prompt-file /nonexistent.md ...` fails with a clean error and a hint.
- `ackage.json` does not appear in the auto-generated Relevant Files for `package.json` (if fix in scope).
- `bun test` passes.

### Phase 4 — End-to-end verification

**Purpose:** Prove the full chain works in a clean environment before docs and release.

**Tasks:**

1. Rebuild: `bun run build && ./dist/maestro --version`.
2. In a tmp `$HOME` (`HOME=$(mktemp -d)`), preload fake Claude and Codex config dirs:
   ```
   mkdir -p "$HOME/.claude" "$HOME/.codex"
   echo "# claude config" > "$HOME/.claude/CLAUDE.md"
   echo "# codex config" > "$HOME/.codex/AGENTS.md"
   ```
3. Run `./dist/maestro agent inject`. Assert:
   - `$HOME/.claude/skills/maestro-brainstorm/SKILL.md` exists and has `name: maestro-brainstorm` in frontmatter.
   - Same for `maestro-plan`, `maestro-task`, `maestro-mission`, `maestro-handoff`.
   - Same for `$HOME/.codex/skills/maestro-*/SKILL.md`.
   - `$HOME/.claude/MAESTRO.md` does not exist.
   - `$HOME/.claude/CLAUDE.md` does not contain `@MAESTRO.md`.
   - Same for Codex.
4. Run `./dist/maestro agent inject` again. Assert second run is idempotent (no changes, no errors).
5. Pre-seed a stale skill dir and confirm cleanup: `mkdir $HOME/.claude/skills/maestro-obsolete`, re-run inject, assert dir is gone.
6. Run `./dist/maestro agent remove`. Assert all 5 skill dirs are deleted under both Claude and Codex home dirs.
7. Handoff smoke:
   - Write `/tmp/brief.md` with known content.
   - Run `./dist/maestro handoff --prompt-file /tmp/brief.md --agent claude --name smoke --json`.
   - Capture handoff id. Kill the spawned process immediately.
   - Assert `.maestro/launches/<id>/prompt.md` exists and its content matches `/tmp/brief.md`.
   - Run `./dist/maestro handoff show <id> --json`. Assert `promptPath` is correct.
8. Full suite: `bun test`. `bun run check:boundaries`. `bun run check:skills`. `bun run check:bundled-skills`.
9. Manual skill-chain sanity: start a fresh Claude session in the maestro repo. Invoke `/maestro-brainstorm`, confirm it loads. Invoke `/maestro-handoff`, confirm it loads. Confirm `maestro-task`'s description matches against multi-step work requests.

**Dependencies:** Phases 1, 2, 3 complete.

**Verification:** All assertions above. If any fail, fix forward before proceeding to Phase 5. Do not release with known regressions.

### Phase 5 — Docs, version, release-prep

**Purpose:** Communicate the change and ship.

**Tasks:**

1. Update `AGENTS.md` (repo root): the agent-instruction section (if it references `MAESTRO.md` or `AGENT_INSTRUCTION_BLOCK`) should point at the bundled skill set instead. Per the project convention noted in `AGENTS.md`, changes to agent-facing surfaces require updating the instruction block — this change is the update.
2. Update `README.md` if it references `MAESTRO.md` injection. The two-working-loops section stays accurate; skill-install mechanics may warrant a short new subsection.
3. Note for the user personally: `~/.claude/skills/preplan-brainstorm/` and `~/.claude/skills/execution-plan/` are safe to delete once `./dist/maestro agent inject` has run and written the renamed versions.
4. Release notes draft covering:
   - 5-skill bundle replaces `MAESTRO.md` injection. Users who hand-edited `MAESTRO.md` should copy their customizations elsewhere before upgrading.
   - `droid` and `gemini` no longer supported by `maestro agent inject`. Users should manually delete `~/.droid/MAESTRO.md` and `~/.gemini/MAESTRO.md`.
   - `maestro handoff --prompt-file` is new.
   - Any bug fixes from Phase 3.
5. Version bump (minor). Update `src/shared/version.ts` (or wherever the version is sourced — follow repo convention).
6. Conventional Commit message: `feat(skills): ship 5-skill maestro bundle, replace AGENT_INSTRUCTION_BLOCK with global skill install`.

**Dependencies:** Phase 4 verified clean.

**Verification:**
- `./dist/maestro --version` reports the new minor version.
- `git log -1` shows the Conventional Commit.
- Docs updated.

## Critical files

- `src/features/agent/usecases/manage-agents.usecase.ts` — major rewrite of `processInject` and `processRemove`.
- `src/features/agent/domain/agents.ts` — prune `SUPPORTED_AGENTS`.
- `src/infra/domain/bootstrap-templates.ts` — delete `AGENT_INSTRUCTION_BLOCK` and `renderBlock`, keep `PROJECT_BOOTSTRAP_TEMPLATES`.
- `src/features/handoff/commands/handoff.command.ts` — add `--prompt-file`.
- `src/features/handoff/usecases/build-handoff-prompt.usecase.ts` — early-return bypass for `promptFile`.
- `src/infra/adapters/git.adapter.ts` and `src/shared/lib/sanitize.ts` — investigate `ackage.json` root cause.
- `src/features/handoff/adapters/launch-store.adapter.ts` — investigate artifact non-persistence root cause.
- `scripts/sync-bundled-skills.ts` — new, patterned on `scripts/sync-built-in-skills.ts`.
- `scripts/ci.ts` — wire `check:bundled-skills`.
- `tests/unit/features/agent/usecases/manage-agents.usecase.test.ts` — new test groups.
- `tests/unit/infra/domain/bootstrap-templates.test.ts` — delete block tests, preserve `PROJECT_BOOTSTRAP_TEMPLATES` tests.
- `tests/unit/infra/domain/bundled-skill-templates.test.ts` — new.
- `tests/unit/features/handoff/usecases/build-handoff-prompt.usecase.test.ts` — new `--prompt-file` cases.
- `skills/bundled/*` — all new/migrated.
- `package.json` — add `sync:bundled-skills`, `check:bundled-skills`.

## Existing utilities to reuse

- `scripts/sync-built-in-skills.ts` — pattern for `scripts/sync-bundled-skills.ts`.
- `src/shared/lib/skill-path.ts:decodeSkillDirectoryName` — URL-decode `%3A` in skill dir names if needed (bundled skills use plain names like `maestro-task`, no encoding needed).
- `src/features/agent/lib/agent-block.ts:removeReference` — strips `@MAESTRO.md` from main config.
- `src/features/agent/lib/agent-block.ts:removeBlock` and `removeLegacyBlock` — strip inline legacy blocks.
- `src/shared/lib/fs.ts:writeText`, `writeJson`, `ensureDir`, `removeIfExists` — already used by the inject flow.
- `src/infra/usecases/init.usecase.ts:removeStaleManagedSkillDirs` — mirror for stale-skill cleanup in `processInject`.
- `tests/unit/features/agent/usecases/manage-agents.usecase.test.ts:mkdtemp` pattern — extend for home-dir tests.

## Risks and cut lines

**Top 3 risks (priority order):**

1. **Install path rewrite scope.** `processInject` today returns one `InjectResult` per agent (one file written). The new version writes N files per agent (5 skills × multiple files each). The return type and callers may need to change. If the rewrite is done minimally instead of with a coherent new contract, tests and callers silently drift. Mitigation: design the new `InjectResult` shape deliberately; update call sites explicitly; new test group covers the shape.

2. **Losing `PROJECT_BOOTSTRAP_TEMPLATES` test coverage.** `bootstrap-templates.test.ts` mixes `AGENT_INSTRUCTION_BLOCK` tests and `PROJECT_BOOTSTRAP_TEMPLATES` tests. Deleting the former is easy; accidentally deleting the latter is a silent regression on `.maestro/AGENTS.md` bootstrap content. Mitigation: explicit preservation checklist during the rewrite (lines 73, 92, 112, 121, 126 cited above), verified by running the remaining tests.

3. **`--prompt-file` bypass breaks launch record persistence.** If the bypass skips `launchStore.create`, `handoff list` shows packets with `promptPath` pointing at files that never got written. This is exactly the artifact non-persistence bug already observed. Mitigation: the bypass replaces only the *synthesis* step; `launchStore.create` still runs and still writes the prompt to `.maestro/launches/<id>/prompt.md`. Covered by integration test.

**Cut lines (what can defer if time pressure):**

- Artifact non-persistence fix — can ship skills and `--prompt-file` without this fix; document as a known issue and follow up.
- `ackage.json` fix — same, document and follow up.
- Stale-skill cleanup (removing obsolete `maestro-*` dirs) — nice-to-have; can defer without blocking the bundle.
- Codex install — if Codex skill directory conventions turn out to differ from `.codex/skills/`, ship Claude-only first and add Codex in a follow-up.

**Must-have (no cut):**

- 5 skill files authored and correct.
- `agent inject` writes skills globally.
- `MAESTRO.md` legacy cleanup.
- `AGENT_INSTRUCTION_BLOCK` deleted (or no-op) so there is one source of truth.
- Tests cover the new install path.

## First execution step

1. Create a feature branch off `main` for this refactor. Proposed name: `feat/skill-bundle`. `git checkout -b feat/skill-bundle`. All Phase 1 through Phase 5 work happens on this branch.
2. Then start Phase 1 task 1: create `skills/bundled/` directory and draft `skills/bundled/maestro-handoff/SKILL.md` using the content already approved in the preplan-brainstorm phase.

## State

`ready-for-implementation`
