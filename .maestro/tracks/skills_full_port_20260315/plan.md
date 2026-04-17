# Implementation Plan: Skills System Full Port

> Track: skills_full_port_20260315
> Type: feature
> Created: 2026-03-15

## Context

maestroCLI's built-in skills system currently supports 12 flat-file skills (single SKILL.md each, embedded at build time). A richer reference implementation exists at `/Users/reinamaccredy/Code/maestro/skills` with reference subdirectories, step-file protocols, progressive disclosure, and more comprehensive skill content. This track ports that architecture and content into maestroCLI, merging overlapping skills and adding new ones, resulting in 18 built-in skills with colon-prefixed naming.

## Critical Files

| File | Role |
|------|------|
| `src/skills/registry.ts` | Core API: `loadSkill()`, `listSkills()`, `discoverInternal()` -- add `loadSkillReference()`, extend `InternalSkill` with `dirPath` |
| `src/skills/generate.ts` | Build-time embedding -- extend to embed `reference/` dirs, `argumentHint`. Directory name IS the skill slug (not frontmatter `name`). |
| `src/skills/registry.generated.ts` | Auto-generated -- will be regenerated |
| `src/commands/skill.ts` | CLI `skill` command -- add `--ref` flag |
| `src/commands/skill-list.ts` | CLI `skill-list` -- add argument-hint column |
| `src/server/skill.ts` | MCP tool `maestro_skill` -- add reference param, update example names in schema description |
| `src/server/status.ts` | Status recommendations -- update skill names |
| `src/hooks/sessionstart.ts` | Session start hook -- hardcodes `writing-plans` in recommendations |
| `src/types.ts` | `AgentModelConfig` + `DEFAULT_HIVE_CONFIG` -- 14 hardcoded old skill names (10 in `skills` + 4 in `autoLoadSkills`) across 6 agent configs |
| `src/adapters/fs/config.ts` | `getAgentConfig()` filters `disableSkills` against `autoLoadSkills` -- needs alias resolution before rename |
| `src/utils/frontmatter.ts` | Returns kebab-case keys; consumers must use `fm['argument-hint']` bracket notation |
| `skills/built-in/` | Skill directories -- restructure with colon prefix + reference dirs |
| `src/__tests__/unit/skills-registry.test.ts` | Tests -- extend for new capabilities |
| `CLAUDE.md` (maestroCLI) | References old skill names |
| `../CLAUDE.md` (parent repo) | References old skill names -- SHARED repo, update carefully |
| `README.md` | References old skill names |

## Design Decisions

### Colon in directory names
Colons are legal on macOS (HFS+/APFS) and Linux (ext4) but illegal on Windows (NTFS). Since this is a CLI tool for AI agents running on macOS/Linux and the reference implementation already uses this convention, we accept this limitation. Add a note in README.md that Windows is not supported for development.

### Directory name IS the skill slug
`generate.ts` derives the `BuiltinSkillName` type-safe tuple and `BUILTIN_SKILLS` record keys from directory names, not from the `name:` field in SKILL.md frontmatter. The frontmatter `name` must exist (validation) but has no functional effect on lookup. The reference implementation uses hyphens in `name:` (e.g., `name: maestro-design`) while directories use colons (`maestro:design/`). During migration, update all frontmatter `name:` fields to match the directory slug exactly (colons included) for consistency, even though it is not functionally required.

### Backward compatibility aliases
Add a `SKILL_ALIASES` map in `registry.ts` that translates old names to new names with a deprecation warning. This handles scripts, saved prompts, and user muscle memory. Example: `loadSkill('writing-plans')` warns and loads `maestro:design`.

### Canonical reference directory name: `reference/` (singular)
The build system embeds only `reference/` (singular) subdirectories. The existing `prompt-leverage` skill has `references/` (plural) -- rename it to `reference/` during migration (Task 2.1) and update any paths in its SKILL.md content (`references/framework.md` -> `reference/framework.md`). Other non-reference subdirectories (`agents/`, `scripts/`) are runtime filesystem resources only -- they work for internal skills but not for built-in embedded skills. Document this distinction.

### Frontmatter key: `argument-hint` (kebab-case)
`parseFrontmatter` returns keys exactly as written in YAML. The frontmatter uses `argument-hint` (kebab-case) but TypeScript properties use `argumentHint` (camelCase). All code accessing this field must use bracket notation: `fm['argument-hint']`, not `fm.argumentHint`. Both `generate.ts` and `discoverInternal()` must handle this mapping.

### Internal skill dirPath
`discoverInternal()` currently returns only `{ slug, description, content, source }`. Extend it to also return `dirPath` so `loadSkillReference()` can locate reference files on the filesystem for internal skills.

### Four-source discovery (not three-tier)
The registry has four skill sources scanned in priority order: `skills/internal/` (source: `internal`), `.maestro/skills/` (source: `maestro`), `.claude/skills/` (source: `claude`), and built-in (source: `builtin`).

### Parent CLAUDE.md is shared
The parent `CLAUDE.md` at `/Users/reinamaccredy/Code/agent-hive-cc/CLAUDE.md` is shared by claude-hive and opencode-hive. Update skill name references there, but verify no other package depends on the old names.

## Phase 1: Registry Architecture

### Task 1.1: Extend types and registry API
- [ ] Add `argumentHint?: string` to `SkillEntry` type in `registry.ts`
- [ ] Add `references?: Record<string, string>` to the built-in skill type (maps relative path -> content)
- [ ] Extend `discoverInternal()` to return `dirPath: string` (the filesystem path of the skill directory)
- [ ] Update `listSkills()` to populate `argumentHint` from: (a) built-in generated data, (b) internal skill frontmatter via `fm['argument-hint']` (bracket notation -- see Design Decisions)
- [ ] Add `loadSkillReference(name: string, refPath: string, basePath?: string): { content: string } | { error: string }`:
  - For built-in skills: look up `references[refPath]` from embedded data
  - For internal skills: read from `{dirPath}/reference/{refPath}` on filesystem
  - Returns descriptive error if reference not found, listing available references
- [ ] Add `SKILL_ALIASES: Record<string, string>` mapping old names to new:
  - `writing-plans` -> `maestro:design`
  - `executing-plans` -> `maestro:implement`
  - `code-reviewer` -> `maestro:review`
  - `agents-md-mastery` -> `maestro:agents-md`
  - `brainstorming` -> `maestro:brainstorming`
  - `dispatching-parallel-agents` -> `maestro:dispatching`
  - `docker-mastery` -> `maestro:docker`
  - `parallel-exploration` -> `maestro:parallel-exploration`
  - `prompt-leverage` -> `maestro:prompt-leverage`
  - `systematic-debugging` -> `maestro:debugging`
  - `test-driven-development` -> `maestro:tdd`
  - `verification-before-completion` -> `maestro:verification`
- [ ] In `loadSkill()`: if name matches an alias, resolve to new name + emit deprecation warning to stderr
- [ ] Add alias resolution to `getAgentConfig()` in `src/adapters/fs/config.ts`: resolve old names in `disableSkills` before comparing against `autoLoadSkills`. Note: `skills` array is a recommendation list not filtered by `disableSkills`, so only `autoLoadSkills` filtering needs alias resolution.
- [ ] Write failing tests first, then implement

### Task 1.2: Extend build-time generation
- [ ] Update `generate.ts` to scan `reference/` subdirectories recursively within each skill dir
- [ ] Embed each reference file as a key-value pair in the generated record: `references: { 'steps/step-01.md': '...' }`
- [ ] Parse `argument-hint` from YAML frontmatter using bracket notation: `fm['argument-hint']`
- [ ] Include `argumentHint` (camelCase) in the generated output
- [ ] Handle colon in directory names correctly (directory `maestro:design/` yields slug `maestro:design`)
- [ ] Only embed `reference/` subdirectories -- skip other subdirectories (`agents/`, `scripts/`, etc.)
- [ ] Embed ALL file types under `reference/` regardless of extension (`.md`, `.py`, `.template`, etc.) -- the consumer decides what to request via `--ref`
- [ ] The recursive reference scanner must NOT call `parseFrontmatter()` on files inside `reference/`. Nested `SKILL.md` files (e.g., `reference/codex-skills/land/SKILL.md`) are opaque content to embed, not skill definitions. Use a separate code path from skill discovery.
- [ ] Note: generated file will grow from ~85KB to ~320-350KB due to embedded references. This is within Bun/V8 limits but is a 4x increase.
- [ ] Write tests, verify `bun run build` succeeds

### Task 1.3: Update CLI commands
- [ ] Add `--ref <path>` flag to `skill` command for reference file loading via `loadSkillReference()`
- [ ] Update `skill-list` to display `argument-hint` column when present
- [ ] Wire alias resolution into CLI commands (show deprecation warning)
- [ ] Update error messages to suggest colon-prefixed names when skill not found
- [ ] Verify with `bun src/cli.ts skill --help` and `bun src/cli.ts skill-list`

### Task 1.4: Update MCP server tool
- [ ] Add optional `reference` string parameter to `maestro_skill` tool schema
- [ ] When `reference` is provided, call `loadSkillReference()` instead of `loadSkill()`
- [ ] Update schema description examples from `'writing-plans, executing-plans'` to `'maestro:design, maestro:implement'`
- [ ] Update error messages with available reference files list

### Phase 1 Verification
- [ ] `bun run build` succeeds
- [ ] `bun test` passes
- [ ] Manual: create a test skill with `reference/` dir, verify `--ref` flag works
- [ ] Unit test: `SKILL_ALIASES` map is exported and contains all 12 mappings
- [ ] Note: alias resolution is wired in `loadSkill()` but targets don't exist until Phase 2 renames directories. Manual alias loading verification is deferred to Phase 2.

## Phase 2: Skill Content Migration + Reference Updates (atomic)

> Phase 2 renames skills AND updates all code references atomically. This avoids the ordering problem where renamed directories break tests/status/hooks that still reference old names.

### Task 2.1: Rename 8 non-overlapping skills + update all references
- [ ] Rename directories under `skills/built-in/`:
  - `brainstorming/` -> `maestro:brainstorming/`
  - `dispatching-parallel-agents/` -> `maestro:dispatching/`
  - `docker-mastery/` -> `maestro:docker/`
  - `parallel-exploration/` -> `maestro:parallel-exploration/`
  - `prompt-leverage/` -> `maestro:prompt-leverage/`
  - `systematic-debugging/` -> `maestro:debugging/`
  - `test-driven-development/` -> `maestro:tdd/`
  - `verification-before-completion/` -> `maestro:verification/`
- [ ] Update `name:` in each SKILL.md frontmatter to match directory slug (required for validation, not for lookup)
- [ ] Rename `prompt-leverage/references/` (plural) to `prompt-leverage/reference/` (singular) and update paths in its SKILL.md content (`references/framework.md` -> `reference/framework.md`, etc.)
- [ ] Update cross-skill references inside SKILL.md prose (at least 11 occurrences across 6 skills). Two changes needed: (1) old skill name -> new name, AND (2) `hive_skill:<name>` prefix -> `maestro skill maestro:<name>` (or slash-command `/maestro:<name>`):
  - `brainstorming/SKILL.md`: `hive_skill:writing-plans` -> `maestro skill maestro:design`
  - `parallel-exploration/SKILL.md`: `maestro skill dispatching-parallel-agents` -> `maestro skill maestro:dispatching`
  - `writing-plans/SKILL.md`: references to `executing-plans` -> `maestro:implement`
  - `executing-plans/SKILL.md`: `hive_skill:verification-before-completion` -> `maestro skill maestro:verification`
  - `systematic-debugging/SKILL.md` (3 occurrences): `hive_skill:test-driven-development` -> `maestro skill maestro:tdd`, `hive_skill:verification-before-completion` -> `maestro skill maestro:verification`
  - `docker-mastery/SKILL.md` (3 occurrences): `hive_skill:systematic-debugging` -> `maestro skill maestro:debugging`, `hive_skill:test-driven-development` -> `maestro skill maestro:tdd`, `hive_skill:verification-before-completion` -> `maestro skill maestro:verification`
- [ ] **Atomically** update all code referencing old names:
  - `src/types.ts`: `DEFAULT_HIVE_CONFIG` skill arrays (14 entries: 10 in `skills` + 4 in `autoLoadSkills` across 6 agent configs)
  - `src/server/status.ts`: skill recommendation arrays (4 references across 2 push sites) AND the `hint:` format string that renders `maestro_skill('writing-plans')` examples
  - `src/hooks/sessionstart.ts`: `writing-plans` reference
  - `src/__tests__/unit/skills-registry.test.ts`: test assertions
  - `CLAUDE.md` (maestroCLI): skill name references
  - `../CLAUDE.md` (parent repo -- shared, verify no cross-package breakage)
  - `README.md`: skill name references
- [ ] Rebuild and verify `bun run build` + `bun test` pass

### Task 2.2: Merge 4 overlapping skills
- [ ] Remove old directories: `agents-md-mastery/`, `writing-plans/`, `executing-plans/`, `code-reviewer/`
- [ ] **maestro:agents-md**: Create new directory `maestro:agents-md/` (NOT copying reference `maestro:AGENTS.md/` directly -- different target name). Use current quality content (iron law, signal/noise, red flags, verification) as base. Fold in reference generation workflow as a "Generation Workflow" section. Create `reference/` if needed. Update frontmatter `name:` to `maestro:agents-md`.
- [ ] **maestro:design**: Reference `maestro:design` as base (16 step files in `reference/steps/`). Fold in current `writing-plans` task structure (bite-sized granularity, task dependencies, acceptance criteria) and execution handoff protocol.
- [ ] **maestro:implement**: Reference as base (mode detection, context loading, parallel/team modes). Fold in current `executing-plans` post-batch hygienic review, when-to-stop-and-ask, and re-planning signals.
- [ ] **maestro:review**: Reference as orchestration base (track selection, automated checks, auto-fix). Fold in current `code-reviewer` iron laws, de-slop pass, review layers, confidence filter, common review smells.
- [ ] Update all code references for the 4 merged skills (same files as Task 2.1)
- [ ] Rebuild and verify all 4 load correctly with references

### Task 2.3: Port 6 new skills from reference
- [ ] Copy from `/Users/reinamaccredy/Code/maestro/skills`, adapting to maestroCLI conventions:
  - `maestro:new-track/` (with `reference/` containing 4 files: `interview-questions.md`, `plan-template.md`, `spec-template.md`, `metadata-and-registry.md`)
  - `maestro:note/`
  - `maestro:revert/`
  - `maestro:setup/` (with `reference/` containing interviews, templates, workflow template, and nested `styleguides/` subdirectory with 9 language-specific files -- verify recursive embedding handles 2+ levels)
  - `maestro:status/`
  - `maestro:symphony-setup/` (with `reference/codex-skills/` nested inside `reference/`)
- [ ] Fix known frontmatter issues before verification:
  - `maestro:symphony-setup/SKILL.md` is missing `---` delimiters -- add them
  - All `name:` fields must use colons to match directory slugs (reference uses hyphens -- update)
- [ ] Verify each SKILL.md has valid frontmatter (name, description, argument-hint where applicable)
- [ ] Adapt any references to `maestro skill` (the old tool) to work with maestroCLI's CLI/MCP interface
- [ ] Rebuild and verify all 6 load correctly with their reference files

### Phase 2 Verification
- [ ] `bun run build` succeeds
- [ ] `bun test` passes
- [ ] `maestro skill-list` shows all 18 skills, all colon-prefixed, with descriptions and argument-hints
- [ ] Spot-check: `maestro skill maestro:design --ref steps/step-01.md` returns step 1 content
- [ ] Spot-check: `maestro skill maestro:setup --ref interviews.md` returns interview content
- [ ] Alias check: `maestro skill writing-plans` warns and loads `maestro:design`

## Phase 3: Final Verification & Cleanup

### Task 3.1: Comprehensive test coverage
- [ ] Add/update tests for:
  - `loadSkillReference()` with built-in skill (embedded references)
  - `loadSkillReference()` with internal skill (filesystem references)
  - `loadSkillReference()` with nonexistent reference (error path)
  - `argument-hint` parsing and display
  - `SKILL_ALIASES` resolution with deprecation warning
  - Internal skill override with colon-prefixed names
  - All 18 skills load without error
- [ ] Run full test suite: `bun test`

### Task 3.2: Documentation and config migration notes
- [ ] Document in README.md:
  - New colon-prefixed naming convention
  - Reference file support (`--ref` flag)
  - Windows not supported for development (colon in directory names)
  - Old skill names are aliased with deprecation warnings
- [ ] Note for users: if `disableSkills` in `~/.config/maestro/config.json` references old names, update to new names. The alias layer in `getAgentConfig()` (added in Task 1.1) handles this automatically, but users should migrate to new names.

### Phase 3 Verification
- [ ] `bun run build` succeeds
- [ ] `bun test` passes (all new + existing tests)
- [ ] `bun src/cli.ts skill maestro:design` returns merged design content
- [ ] `bun src/cli.ts skill maestro:design --ref steps/step-01.md` returns step 1
- [ ] `bun src/cli.ts skill-list` shows 18 skills with descriptions and argument hints
- [ ] `bun src/cli.ts status` shows updated recommendations with colon-prefixed names
- [ ] `bun src/cli.ts skill writing-plans` shows deprecation warning + loads maestro:design
- [ ] Internal skill override: create `.maestro/skills/maestro%3Abrainstorming/SKILL.md`, verify it shadows built-in

## Non-Goals
- Skill packaging/distribution (.skill ZIP format)
- System-enforced step tracking
- Skill versioning or dependency resolution
- Remote skill fetching
- skills-lock.json integrity hashing
- Embedding non-`reference/` subdirectories (agents/, scripts/) at build time
- `skill-creator` from reference implementation (meta-tool for the reference repo's packaging system, not applicable to maestroCLI's embedded skills)

## Ghost Diffs
- `registry.generated.ts` will be completely regenerated (expected, auto-generated)
- Old skill directories under `skills/built-in/` will be removed (replaced by colon-prefixed versions)
- `DEFAULT_HIVE_CONFIG` in `types.ts` will change all skill name strings
- Both CLAUDE.md files will have skill name updates

## Discovery

### Codebase exploration findings
- **Registry architecture**: `generate.ts` scans `skills/built-in/`, reads `SKILL.md`, derives slug from directory name (not frontmatter). Embeds content as strings into `registry.generated.ts`. `registry.ts` provides `loadSkill()` and `listSkills()` with four-source discovery (internal > maestro > claude > builtin).
- **Clean architecture**: Commands (`src/commands/`) call registry functions. MCP server (`src/server/skill.ts`) wraps the same registry. Types in `src/types.ts`.
- **Internal override**: Internal skills from `skills/internal/`, `.maestro/skills/`, `.claude/skills/` shadow built-ins by name. `discoverInternal()` returns `{ slug, description, content, source }` but NOT `dirPath` -- needs extension for reference file loading.
- **Existing subdirectories**: `prompt-leverage` already has `references/`, `agents/`, `scripts/` subdirectories that `generate.ts` currently ignores. Only `reference/` will be embedded.
- **Hardcoded skill names found in**: `types.ts` (14 entries: 10 `skills` + 4 `autoLoadSkills`), `status.ts` (4 entries across 2 push sites), `sessionstart.ts` (1 entry), `server/skill.ts` (schema examples), both CLAUDE.md files, README.md. Cross-skill references inside SKILL.md prose: at least 10 occurrences across 6 skills.
- **Config interaction**: `getAgentConfig()` in `fs/config.ts` filters `disableSkills` -- needs alias resolution for backward compat.
- **Reference implementation**: Uses `reference/` subdirectories, step-file protocol with content gates, `argument-hint` frontmatter, colon-prefixed naming. Proven to work in practice.
- **Overlap analysis**: 4 overlapping pairs identified. Each pair has complementary strengths -- current skills have quality discipline, reference skills have orchestration workflows. All 4 merge (richer reference as base, fold in current unique content).

### Source reference
- Reference skills location: `/Users/reinamaccredy/Code/maestro/skills`
- Current skills location: `skills/built-in/` (12 skills)
- Result: 18 skills (4 merged + 8 renamed + 6 new)
