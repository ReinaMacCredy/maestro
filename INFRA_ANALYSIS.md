# src/infra/ Structure Analysis

**Generated:** 2026-05-08  
**Purpose:** Deep exploration of the `src/infra/` directory structure, responsibilities, and organization

---

## Executive Summary

`src/infra/` owns **shared CLI plumbing, adapters, config/git surfaces, and sanctioned cross-feature command seams**. It is the infrastructure layer that sits between the CLI entry point (`src/index.ts`) and the feature-bounded contexts (`src/features/`).

**Key Responsibilities:**
- Top-level CLI commands (init, install, doctor, status, update, mission-control)
- Configuration management (YAML config loading, layering, editing)
- Git operations (state reading, worktree management)
- Agent skill management (installation, removal, manifest tracking)
- Bootstrap template generation
- Update checking and binary installation

**Does NOT contain:**
- Feature-specific business logic (belongs in `src/features/`)
- Generic utilities (belongs in `src/shared/`)
- Mission Control rendering (belongs in `src/tui/`)

---

## Directory Structure

```
src/infra/
├── AGENTS.md                          # Documentation for infra layer
├── services.ts                        # Composition root for infra services
├── commands/                          # Top-level CLI command handlers
│   ├── doctor.command.ts
│   ├── init.command.ts
│   ├── install.command.ts
│   ├── mission-control.command.ts
│   ├── providers.command.ts
│   ├── status.command.ts
│   ├── uninstall.command.ts
│   └── update.command.ts
├── usecases/                          # Shared operational flows
│   ├── check-for-update.usecase.ts
│   ├── check-status.usecase.ts
│   ├── config-edit.usecase.ts
│   ├── fetch-latest-version.usecase.ts
│   ├── init.usecase.ts
│   ├── install-release-binary.usecase.ts
│   ├── manage-agents.usecase.ts
│   └── run-doctor.usecase.ts
├── adapters/                          # Infrastructure adapters
│   ├── config.adapter.ts
│   ├── git.adapter.ts
│   └── update-check-cache.adapter.ts
├── ports/                             # Infrastructure ports
│   ├── config.port.ts
│   └── git.port.ts
├── domain/                            # Infra-only types and templates
│   ├── agents.ts
│   ├── bootstrap-templates.ts
│   ├── built-in-skill-templates.ts   # GENERATED - do not hand-edit
│   ├── bundled-skill-templates.ts    # GENERATED - do not hand-edit
│   ├── config-types.ts
│   ├── git-types.ts
│   ├── providers.ts
│   └── status-types.ts
└── lib/                               # Infra-specific utilities
    └── agent-block.ts
```

---

## File-by-File Breakdown

### Root Files

#### `services.ts`
**Purpose:** Composition root for infrastructure services  
**Exports:** `InfraServices` interface, `buildInfraServices()` function  
**Dependencies:** None (builds adapters directly)  
**Key Types:**
- `InfraServices`: `{ config: ConfigPort, git: GitPort }`

**Notes:**
- Currently ignores `projectDir` parameter (adapters resolve cwd per-call)
- Instantiates `YamlConfigAdapter` and `ShellGitAdapter`
- No feature dependencies

#### `AGENTS.md`
**Purpose:** Documentation for the infra layer  
**Content:**
- Structure overview
- Where to look for specific tasks
- Conventions and anti-patterns
- Links to parent `src/AGENTS.md`

---

### commands/

Top-level CLI command handlers. These are thin orchestration layers that delegate to use cases.

#### `init.command.ts`
**Command:** `maestro init [--global] [--json]`  
**Purpose:** Initialize maestro in current project or globally  
**Use Case:** `initMaestro()`  
**Features:**
- Interactive file replacement prompts (TTY only)
- Creates `.maestro/` structure
- Writes bootstrap templates
- Updates `.gitignore`
- Installs built-in skills to project agent directories

**Dependencies:**
- `@/services` → `getServices()`
- `../usecases/init.usecase` → `initMaestro()`
- `@/shared/lib/output` → `output()`

#### `install.command.ts`
**Command:** `maestro install [--json]`  
**Purpose:** Initialize global config and inject agent instructions  
**Use Cases:** `initMaestro()`, `injectAgentBlocks()`  
**Features:**
- Runs global init
- Installs bundled skills to `~/.claude/skills/`, `~/.codex/skills/`, etc.
- Windows PATH warning

**Dependencies:**
- `@/services` → `getServices()`
- `../usecases/init.usecase` → `initMaestro()`
- `../usecases/manage-agents.usecase` → `injectAgentBlocks()`
- `@/shared/lib/output` → `output()`, `formatAgentResults()`

#### `doctor.command.ts`
**Command:** `maestro doctor [--json]`  
**Purpose:** Verify maestro dependencies and configuration  
**Use Case:** `runDoctor()`  
**Features:**
- Checks git availability
- Validates config files
- Detects legacy handoff files
- Finds empty feature directories
- Detects oversized root docs
- Exit code 1 if any check fails

**Dependencies:**
- `@/services` → `getServices()`
- `../usecases/run-doctor.usecase` → `runDoctor()`
- `@/shared/lib/output` → `output()`
- `@/shared/lib/project-root` → `resolveMaestroProjectRoot()`

#### `status.command.ts`
**Command:** `maestro status [--json]`  
**Purpose:** Show current maestro state  
**Use Case:** `checkStatus()`  
**Features:**
- Reports initialization status
- Shows config source (global/project/none)
- Checks git availability
- Counts legacy handoff artifacts

**Dependencies:**
- `@/services` → `getServices()`
- `../usecases/check-status.usecase` → `checkStatus()`
- `@/shared/lib/output` → `output()`, `resolveJsonFlag()`

#### `mission-control.command.ts`
**Command:** `maestro mission-control [options]`  
**Purpose:** Interactive mission control dashboard  
**Options:**
- `--mission <id>` - Mission ID (auto-selects if omitted)
- `--json` - Output snapshot as JSON
- `--preview [screen]` - Render read-only preview frame
- `--feature <id>` - Select feature for preview
- `--size <WxH>` - Render dimensions
- `--format <type>` - Output format (plain/ansi)
- `--render-check` - Validate all preview screens

**Features:**
- Builds mission control snapshot
- JSON output for agent consumption
- Preview mode for non-interactive rendering
- Render check for validation
- Interactive TUI mode (requires TTY)
- Caching git/config ports for performance

**Dependencies:**
- `@/services` → `getServices()`
- `@/tui/state/snapshot` → `buildSnapshot()`, `buildHomeSnapshot()`
- `@/tui/opentui/index` → `renderDashboard()`, `renderPreviewFrame()`, `runRenderCheck()` (dynamic import)
- `@/tui/lib/snapshot-poll-cache` → `CachingGitPort`, `CachingConfigPort`

**Notes:**
- Dynamic imports for TUI rendering to avoid cold-start penalty in `--json` mode
- Snapshot loader pattern with caching for poll-based updates
- Redacts snapshot for read-only output

#### `update.command.ts`
**Command:** `maestro update [--json]`  
**Purpose:** Update maestro to the latest version  
**Use Cases:** `checkForUpdate()`, `installReleaseBinary()`  
**Features:**
- Checks for newer version
- Downloads and installs binary
- Platform-specific installation

#### `uninstall.command.ts`
**Command:** `maestro uninstall [--json]`  
**Purpose:** Remove agent instructions  
**Use Case:** `removeAgentBlocks()`  
**Features:**
- Removes bundled skills from agent directories
- Cleans up legacy MAESTRO.md references

#### `providers.command.ts`
**Command:** `maestro providers [--json]`  
**Purpose:** List available agent providers  
**Features:**
- Lists all supported providers (claude, codex, hermes, agentskills)
- Shows config paths and skill roots

---

### usecases/

Shared operational flows that orchestrate adapters and domain logic.

#### `init.usecase.ts`
**Function:** `initMaestro(config, opts)`  
**Purpose:** Initialize maestro project or global config  
**Responsibilities:**
- Create `.maestro/` directory structure
- Write bootstrap templates from `domain/bootstrap-templates.ts`
- Migrate legacy `.factory/` files to `.maestro/`
- Update `.gitignore` with runtime state patterns
- Sync built-in skills to project agent directories
- Handle file replacement prompts

**Key Logic:**
- `collectProjectBootstrapFiles()` - Merges default templates with legacy `.factory/` content
- `overlayLegacyFile()` / `overlayLegacyTree()` - Migration from old structure
- `ensureRuntimeGitignore()` - Adds maestro runtime patterns
- `syncProjectAgentBuiltInSkills()` - Installs built-in skills to `.claude/skills/`, `.codex/skills/`
- `assertProjectLocalPathSafe()` - Security check against symlink traversal

**Returns:** `InitResult` with created/skipped paths

#### `manage-agents.usecase.ts`
**Functions:** `injectAgentBlocks()`, `removeAgentBlocks()`  
**Purpose:** Install/remove bundled maestro skills to agent directories  
**Responsibilities:**
- Write bundled skills to `~/.maestro/skills/<skill>/` (source of truth)
- Create symlinks from agent skill roots to maestro skills
- Migrate pre-redesign real directories to symlinks
- Preserve user-edited skill files
- Remove stale skills
- Clean up legacy MAESTRO.md references

**Key Logic:**
- `writeBundledSkill()` - Writes skill with manifest-based user-edit detection
- `ensureSkillLink()` - Creates/repairs symlinks to maestro skill tree
- `migrateRealDirToSymlink()` - Converts legacy real dirs to symlinks, preserving edits
- `classifyAgentSkillEntry()` - Distinguishes maestro-managed from user-authored skills
- `cleanupLegacyMaestroMd()` - Removes old reference files and inline blocks

**Manifest System:**
- `.maestro-bundled.json` in each skill directory
- Tracks file hashes to detect user edits
- Preserves user edits across updates
- Enables safe stale-file cleanup

**Returns:** `InjectResult[]` or `RemoveResult[]` per agent

#### `check-for-update.usecase.ts`
**Function:** `checkForUpdate(deps)`  
**Purpose:** Check for newer maestro version (non-blocking)  
**Responsibilities:**
- Read cached update check result
- Kick off background refresh if stale (24h) or missing
- Respect cooldown period (15min) between attempts
- Never block current invocation

**Key Logic:**
- `isStale()` - Checks if cache is older than 24h
- `isAttemptCoolingDown()` - Prevents rapid retries
- `refreshCache()` - Background fetch with signal support
- `isNewerSemver()` - Semantic version comparison

**Returns:** `CheckForUpdateResult` with cached result and optional refresh promise

#### `fetch-latest-version.usecase.ts`
**Function:** `fetchLatestVersion(opts)`  
**Purpose:** Fetch latest maestro version from GitHub releases  
**Responsibilities:**
- Query GitHub API for latest release
- Parse version from tag name
- Handle network errors gracefully

**Returns:** `{ version: string, tag: string }`

#### `install-release-binary.usecase.ts`
**Function:** `installReleaseBinary(version, opts)`  
**Purpose:** Download and install maestro binary  
**Responsibilities:**
- Construct download URL for platform/arch
- Download binary to temp location
- Move to install directory
- Set executable permissions
- Platform-specific path handling

**Returns:** `{ installedPath: string }`

#### `run-doctor.usecase.ts`
**Function:** `runDoctor(git, config, dir, options)`  
**Purpose:** Run health checks on maestro installation  
**Checks:**
1. Git repository detection
2. Project config existence
3. Global config existence
4. Ignored project config keys (global-only settings)
5. Legacy handoff artifacts
6. Empty feature directories
7. Oversized root docs (>500 lines, not in allowlist)

**Returns:** `DoctorCheck[]` with status (ok/warn/fail), message, and optional fix

#### `check-status.usecase.ts`
**Function:** `checkStatus(config, git, dir, options)`  
**Purpose:** Gather current maestro state  
**Checks:**
- Project config existence
- Global config existence
- Git availability
- Legacy handoff count

**Returns:** `StatusReport` with initialization status and config source

#### `config-edit.usecase.ts`
**Functions:** `previewConfigEdit()`, `applyConfigEdit()`  
**Purpose:** Edit config values by key path  
**Responsibilities:**
- Load config layers
- Parse draft value (on/off → boolean, numbers, strings)
- Set nested value by dot-path
- Preview or apply changes
- Validate scope health (no YAML errors)

**Returns:** `ConfigEditPreview` or void

---

### adapters/

Infrastructure adapters implementing ports.

#### `config.adapter.ts`
**Class:** `YamlConfigAdapter implements ConfigPort`  
**Purpose:** YAML-based config file management  
**Methods:**
- `load(projectDir)` - Load effective config (merged layers)
- `loadLayers(projectDir)` - Load all layers with error tracking
- `write(scope, projectDir, config)` - Write config to file
- `exists(scope, projectDir)` - Check if config file exists

**Key Logic:**
- Reads global (`~/.maestro/config.yaml`) and project (`.maestro/config.yaml`)
- Deep merges layers: defaults → global → project
- Tracks YAML parse errors per scope
- Uses `@/shared/lib/yaml` for parsing/stringifying

**Dependencies:**
- `@/infra/domain/config-types` → `MaestroConfig`, `DEFAULT_CONFIG`
- `@/shared/lib/fs` → `ensureDir()`, `readText()`, `writeText()`
- `@/shared/lib/yaml` → `parseYaml()`, `stringifyYaml()`, `deepMerge()`

#### `git.adapter.ts`
**Class:** `ShellGitAdapter implements GitPort`  
**Purpose:** Git operations via shell commands  
**Methods:**
- `getState(cwd)` - Get branch, commits, changed files, diff stat
- `isRepo(cwd)` - Check if directory is a git repo
- `getCurrentBranch(cwd)` - Get current branch name
- `createWorktree(cwd, input)` - Create git worktree with unique slug

**Key Logic:**
- Uses `execArgv()` for parallel git commands
- Parses `git status --porcelain` for file changes
- Handles rename/copy detection
- Generates unique worktree slugs with collision avoidance
- Classifies file changes (added, modified, deleted, renamed, etc.)

**Dependencies:**
- `@/shared/lib/shell` → `execArgv()`, `execOrThrow()`
- `@/shared/lib/fs` → `dirExists()`

#### `update-check-cache.adapter.ts`
**Functions:** `readUpdateCheckCache()`, `writeUpdateCheckCache()`  
**Purpose:** Persistent cache for update check results  
**Storage:** `~/.maestro/update-check.json`  
**Schema:** `UpdateCheckCacheEntry`
- `checkedAt` - Last successful check timestamp
- `lastAttemptAt` - Last attempt timestamp (for cooldown)
- `currentVersion` - Version at check time
- `latestVersion` - Latest available version
- `latestTag` - Git tag for latest version

**Key Logic:**
- Swallows all read/parse errors (corrupted cache never crashes CLI)
- Validates entry shape before returning

---

### ports/

Infrastructure port interfaces.

#### `config.port.ts`
**Interface:** `ConfigPort`  
**Methods:**
- `load(projectDir): Promise<MaestroConfig>` - Load effective config
- `loadLayers(projectDir): Promise<ConfigLayers>` - Load all layers with errors
- `write(scope, projectDir, config): Promise<void>` - Write config
- `exists(scope, projectDir): Promise<boolean>` - Check existence

**Types:**
- `ConfigScope` - "global" | "project"
- `ConfigLoadError` - Scope, path, message
- `ConfigLayers` - Defaults, effective, global, project, errors, paths

#### `git.port.ts`
**Interface:** `GitPort`  
**Methods:**
- `getState(cwd): Promise<GitState>` - Get git state
- `isRepo(cwd): Promise<boolean>` - Check if git repo
- `getCurrentBranch(cwd): Promise<string>` - Get current branch
- `createWorktree(cwd, input): Promise<GitWorktree>` - Create worktree

---

### domain/

Infra-only types and templates.

#### `config-types.ts`
**Exports:** `MaestroConfig`, `DEFAULT_CONFIG`  
**Purpose:** Configuration schema and defaults  
**Key Fields:**
- `defaultAgent` - Default agent slug
- `sourceRepo` - Source repository URL
- `contracts` - Contract enforcement settings
- `sessionDetection` - Session detection config
- `defaultWorkflow` - Default workflow name
- `workflowTemplates` - Workflow template definitions
- `ui` - UI configuration
- `memory` - Memory system configuration

**Dependencies:**
- `@/features/session` → `AgentSlug`
- `@/features/memory` → `MemoryConfig`
- `@/features/mission` → `WorkflowTemplate`
- `@/shared/domain/ui-config` → `UiConfig`

#### `agents.ts`
**Exports:** Agent configuration types and utilities  
**Key Types:**
- `AgentConfigSpec` - Agent configuration specification
- `SUPPORTED_AGENTS` - Array of supported agents (claude-code, codex, hermes, agentskills)
- `RUNTIME_AGENTS` - Agents that can run missions
- `SKILL_TARGET_AGENTS` - Agents that can receive skills

**Key Functions:**
- `agentSkillsRoot(agent, projectDir, homeDir)` - Resolve skill directory
- `agentConfigPath(agent, projectDir, homeDir)` - Resolve config file path
- `agentConfigDirPath(agent, projectDir, homeDir)` - Resolve config directory
- `agentReferencePath(agent, projectDir, homeDir)` - Resolve reference file path
- `agentLegacyConfigPaths(agent, projectDir, homeDir)` - Legacy paths for migration

**Constants:**
- `BLOCK_START_MARKER` - `<!-- maestro:start -->`
- `BLOCK_END_MARKER` - `<!-- maestro:end -->`
- `REFERENCE_FILE` - `MAESTRO.md`

#### `providers.ts`
**Exports:** Provider descriptor types and utilities  
**Key Types:**
- `ProviderId` - "codex" | "claude" | "hermes" | "agentskills"
- `ProviderDescriptor` - Provider metadata with paths

**Key Functions:**
- `listProviders(projectDir, homeDir)` - List all providers
- `getProvider(idOrSlug, projectDir, homeDir)` - Get provider by ID or slug
- `listSkillTargetProviders(projectDir, homeDir)` - List skill-capable providers

#### `git-types.ts`
**Exports:** Git domain types  
**Key Types:**
- `GitState` - Branch, commits, changed files, working tree status, diff stat
- `GitFileChange` - Path and change kind (added, modified, deleted, etc.)
- `GitWorktree` - Slug, base branch, branch, path

#### `status-types.ts`
**Exports:** Status and doctor check types  
**Key Types:**
- `DoctorCheck` - Name, status (ok/warn/fail), message, optional fix
- `StatusReport` - Initialization status, config source, git availability, legacy handoff count

#### `bootstrap-templates.ts`
**Exports:** `PROJECT_BOOTSTRAP_TEMPLATES`  
**Purpose:** Default bootstrap files for project initialization  
**Templates:**
- `.maestro/AGENTS.md` - Project-level agent guidance
- `.maestro/MAESTRO.md` - Read order and daily commands
- `.maestro/specs/.gitkeep` - Spec directory placeholder
- `.maestro/tasks/contract-templates/default.md` - Default contract template
- `.maestro/bootstrap/init.sh` - Bootstrap init script
- `.maestro/bootstrap/services.yaml` - Commands and services manifest
- `.maestro/bootstrap/library/*.md` - Architecture, environment, user-testing docs
- `.maestro/bootstrap/validation/README.md` - Validation references
- `.maestro/policies/*.yaml` - Sensitive paths, owners, risk, autopilot, release policies

**Type:** `BootstrapTemplateFile` - Path, content, optional executable flag

#### `built-in-skill-templates.ts`
**Exports:** `BUILT_IN_SKILL_TEMPLATES`  
**Purpose:** Project-local built-in skills (synced to `.claude/skills/`, `.codex/skills/`)  
**Status:** GENERATED - do not hand-edit  
**Source:** `skills/built-in/`  
**Sync Command:** `bun scripts/sync-built-in-skills.ts`

**Skills:**
- `maestro:agent-base` - Base procedures for all mission agents
- `maestro:blueprint` - Generate visual HTML blueprints and structured plan specs

**Type:** `BuiltInSkillTemplate` - Name, files array

#### `bundled-skill-templates.ts`
**Exports:** `BUNDLED_SKILL_TEMPLATES`  
**Purpose:** Global bundled skills (installed to `~/.maestro/skills/`, symlinked from agent dirs)  
**Status:** GENERATED - do not hand-edit  
**Source:** `skills/bundled/`  
**Sync Command:** `bun run sync:bundled-skills`  
**Check Command:** `bun run check:bundled-skills`

**Skills:** (7 total as of L4.5)
- `maestro-brainstorm`
- `maestro-plan`
- `maestro-task`
- `maestro-mission`
- `maestro-handoff`
- `maestro-setup`
- `maestro-verify`

**Type:** `BundledSkillTemplate` - Name, files array

---

### lib/

Infra-specific utilities.

#### `agent-block.ts`
**Purpose:** Manage maestro instruction blocks in agent config files  
**Functions:**
- `hasReference(content)` - Check for `@MAESTRO.md` reference line
- `injectReference(content)` - Add reference line
- `removeReference(content)` - Remove reference line
- `wrapBlock(content)` - Wrap content in maestro markers
- `hasBlock(content)` - Check for maestro block
- `extractBlock(content)` - Extract block content
- `injectBlock(content, block)` - Add new block
- `replaceBlock(content, newBlock)` - Replace existing block
- `removeBlock(content)` - Remove block
- `removeLegacyBlock(content)` - Remove unmarked legacy section

**Markers:**
- `BLOCK_START_MARKER` - `<!-- maestro:start -->`
- `BLOCK_END_MARKER` - `<!-- maestro:end -->`
- `REFERENCE_LINE` - `@MAESTRO.md`

---

## Import/Export Relationships

### Imports FROM Features

`src/infra/` imports from features for:
- **Session detection:** `@/features/session` → `AgentSlug`
- **Memory config:** `@/features/memory` → `MemoryConfig`
- **Mission workflows:** `@/features/mission` → `WorkflowTemplate`, `DEFAULT_PRINCIPLES`
- **Handoff counting:** `@/features/handoff` → `countLegacyHandoffFiles()`
- **Mission Control snapshot:** All feature stores for snapshot building

### Imports FROM Shared

`src/infra/` heavily uses shared utilities:
- **Filesystem:** `@/shared/lib/fs` → `ensureDir()`, `readText()`, `writeText()`, `dirExists()`, etc.
- **Shell:** `@/shared/lib/shell` → `execArgv()`, `execOrThrow()`
- **YAML:** `@/shared/lib/yaml` → `parseYaml()`, `stringifyYaml()`, `deepMerge()`
- **Output:** `@/shared/lib/output` → `output()`, `resolveJsonFlag()`, `formatAgentResults()`
- **Defaults:** `@/shared/domain/defaults` → `MAESTRO_DIR`, `resolveMaestroSkillsRoot()`, etc.
- **Errors:** `@/shared/errors` → `MaestroError`
- **Version:** `@/shared/version` → `VERSION`

### Exports TO Root

`src/infra/` exports to root composition:
- `services.ts` → `buildInfraServices()` used by `src/services.ts`
- Command registration functions used by `src/index.ts`

### Exports TO Features

Features may import from infra for:
- Config types: `@/infra/domain/config-types` → `MaestroConfig`
- Git types: `@/infra/domain/git-types` → `GitState`, `GitFileChange`

---

## Patterns and Conventions

### Hexagonal Architecture

Infra follows ports-and-adapters:
- **Ports:** `ports/config.port.ts`, `ports/git.port.ts` - Interfaces
- **Adapters:** `adapters/config.adapter.ts`, `adapters/git.adapter.ts` - Implementations
- **Use Cases:** `usecases/*.usecase.ts` - Orchestration logic
- **Commands:** `commands/*.command.ts` - CLI entry points

### Composition Root

`services.ts` is the composition root for infra services:
```typescript
export function buildInfraServices(_projectDir: string): InfraServices {
  return {
    config: new YamlConfigAdapter(),
    git: new ShellGitAdapter(),
  };
}
```

### Command Pattern

All commands follow the same structure:
1. Parse options
2. Get services via `getServices()`
3. Call use case(s)
4. Format output via `output()` helper
5. Exit with appropriate code

### Error Handling

- Config adapter swallows YAML errors and tracks them in `ConfigLayers.errors`
- Update check cache swallows all read errors (never crashes CLI)
- Git adapter uses `execArgv()` which returns exit codes without throwing
- Use cases throw `MaestroError` for user-facing errors

### Generated Files

Two domain files are generated and must not be hand-edited:
- `domain/built-in-skill-templates.ts` - Synced from `skills/built-in/`
- `domain/bundled-skill-templates.ts` - Synced from `skills/bundled/`

Both have header comments warning against manual edits.

---

## Observations and Potential Improvements

### Organization

**Strengths:**
- Clear separation of concerns (commands, use cases, adapters, ports, domain)
- Consistent naming conventions
- Well-documented with AGENTS.md
- Follows hexagonal architecture

**Potential Issues:**
- `manage-agents.usecase.ts` is 800+ lines - could be split into smaller modules
- `init.usecase.ts` is 400+ lines - bootstrap template handling could be extracted
- `mission-control.command.ts` has complex option parsing - could benefit from a dedicated options parser

### Naming Consistency

**Consistent:**
- All commands end with `.command.ts`
- All use cases end with `.usecase.ts`
- All adapters end with `.adapter.ts`
- All ports end with `.port.ts`

**Inconsistent:**
- `agent-block.ts` in `lib/` doesn't follow a naming pattern (could be `agent-block.lib.ts`)

### Dependencies

**Good:**
- No circular dependencies detected
- Clear dependency flow: commands → use cases → adapters → ports
- Minimal feature dependencies (only for types and utilities)

**Potential Issues:**
- `mission-control.command.ts` imports from `@/tui/` - this is a sanctioned cross-cutting seam but creates coupling
- Heavy reliance on `@/shared/` - if shared grows too large, it becomes a dumping ground

### Testing Surface

**Well-Suited for Testing:**
- Ports provide clear test boundaries
- Use cases are pure orchestration (easy to mock adapters)
- Adapters have clear responsibilities

**Testing Gaps (based on structure):**
- No test files visible in this directory
- Complex logic in `manage-agents.usecase.ts` (manifest tracking, symlink migration) needs coverage
- Git parsing logic in `git.adapter.ts` needs edge case coverage

### Security

**Good Practices:**
- `init.usecase.ts` has `assertProjectLocalPathSafe()` to prevent symlink traversal
- `manage-agents.usecase.ts` validates paths before operations
- Config adapter validates scope before writes

**Potential Concerns:**
- Shell command execution in `git.adapter.ts` - uses `execArgv()` which is safer than string interpolation
- File operations in `manage-agents.usecase.ts` - extensive but appears safe

### Performance

**Optimizations:**
- `mission-control.command.ts` uses dynamic imports to avoid TUI cold-start penalty in `--json` mode
- `check-for-update.usecase.ts` never blocks current invocation
- Parallel git commands in `git.adapter.ts`

**Potential Issues:**
- `manage-agents.usecase.ts` does extensive file I/O - could benefit from batching
- `init.usecase.ts` writes many files sequentially - could parallelize

---

## Files That May Belong Elsewhere

### None Identified

All files in `src/infra/` appear to be correctly placed:
- Commands are top-level CLI concerns (not feature-specific)
- Use cases orchestrate infrastructure operations
- Adapters implement infrastructure ports
- Domain types are infra-specific

The only cross-cutting concern is `mission-control.command.ts` importing from `@/tui/`, but this is explicitly documented as a "sanctioned cross-feature read-only seam" in AGENTS.md.

---

## Summary

`src/infra/` is well-organized and follows clear architectural patterns. It serves as the infrastructure layer between the CLI entry point and feature-bounded contexts. The hexagonal architecture (ports/adapters/use cases/commands) is consistently applied, and the separation of concerns is clear.

**Key Strengths:**
- Clear responsibility boundaries
- Consistent naming and structure
- Good documentation
- Follows hexagonal architecture
- Minimal feature coupling

**Areas for Improvement:**
- Some use cases are large and could be split
- Testing coverage appears to be missing
- Performance optimizations possible in file I/O heavy operations

**No files appear to be misplaced.** The organization is logical and follows the documented conventions in AGENTS.md.
