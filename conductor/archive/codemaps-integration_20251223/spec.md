# Spec: Codemaps Integration into Conductor

## Overview

Integrate the standalone `codemaps` skill into the Conductor workflow, providing automatic codebase architecture documentation that helps agents and humans orient quickly when starting work.

## Requirements

### Functional Requirements

#### FR-1: Generate CODEMAPS during Setup
- `/conductor-setup` MUST generate `conductor/CODEMAPS/` directory
- MUST always create `overview.md` with project summary, directory tree, key files, and data flow diagram
- SHOULD create additional module codemaps for significant areas (skills, api, database, etc.)
- MUST create `.meta.json` with generation metadata
- If CODEMAPS/ exists, MUST prompt: "CODEMAPS exists. Regenerate? [Y/n]"

#### FR-2: Auto-regenerate CODEMAPS during Finish
- `/conductor-finish` MUST regenerate CODEMAPS as the final step
- MUST support `--skip-codemaps` flag to skip regeneration
- MUST check `.meta.json` for user-modified files and warn before overwriting
- If finish is aborted, CODEMAPS regeneration (being last step) should not have run yet

#### FR-3: Manual Regenerate via Refresh
- `/conductor-refresh` MUST support `codemaps` as a scope option
- If no CODEMAPS/ exists, MUST generate fresh
- MUST warn before overwriting user-modified files
- If no `conductor/` directory exists, MUST error: "Run /conductor-setup first"

#### FR-4: Load CODEMAPS during Design Session
- `ds` MUST check for `conductor/CODEMAPS/` at session start
- If exists, MUST load `overview.md` and relevant module maps into context
- MUST display: "📚 Loaded CODEMAPS for context"
- If missing, MUST warn: "⚠️ No CODEMAPS found. Consider running /conductor-refresh first."
- MUST continue session even if CODEMAPS missing (optional but recommended)

#### FR-5: Merge Codemaps Skill into Conductor
- MUST move `skills/codemaps/references/CODEMAPS_TEMPLATE.md` to `skills/conductor/references/`
- MUST delete `skills/codemaps/` directory after merge
- MUST update documentation (README.md, AGENTS.md, TUTORIAL.md)

### Non-Functional Requirements

#### NFR-1: Scale Limits
- Directory scan depth: Top 2 levels only
- Key files per codemap: Max 50 files
- Module codemaps: Max 10 files

#### NFR-2: Monorepo Support
- MUST detect monorepo patterns (`packages/`, `apps/`, workspaces)
- If monorepo, SHOULD generate per-package codemaps

#### NFR-3: User Modification Tracking
- `.meta.json` MUST track which files were generated vs user-modified
- Modification detection: timestamp-based (file.mtime > meta.generated)

## Acceptance Criteria

### AC-1: Setup Generation
```
GIVEN a project without conductor/CODEMAPS/
WHEN user runs /conductor-setup
THEN conductor/CODEMAPS/ is created with at least overview.md
AND .meta.json contains valid generation metadata
AND user sees "✅ Generated CODEMAPS: overview.md, ..."
```

### AC-2: Setup with Existing CODEMAPS
```
GIVEN a project with existing conductor/CODEMAPS/
WHEN user runs /conductor-setup
THEN user is prompted "CODEMAPS exists. Regenerate? [Y/n]"
AND user choice is respected
```

### AC-3: Finish Auto-regeneration
```
GIVEN a completed track
WHEN user runs /conductor-finish
THEN CODEMAPS is regenerated as the final step
AND .meta.json is updated with new timestamp
```

### AC-4: Finish with Skip Flag
```
GIVEN a completed track
WHEN user runs /conductor-finish --skip-codemaps
THEN CODEMAPS regeneration is skipped
```

### AC-5: Refresh with Codemaps Scope
```
GIVEN a project with conductor/
WHEN user runs /conductor-refresh with scope codemaps
THEN conductor/CODEMAPS/ is regenerated
```

### AC-6: Design Session Context Loading
```
GIVEN a project with conductor/CODEMAPS/
WHEN user starts a design session (ds)
THEN CODEMAPS is loaded into context
AND user sees "📚 Loaded CODEMAPS for context"
```

### AC-7: Design Session without CODEMAPS
```
GIVEN a project without conductor/CODEMAPS/
WHEN user starts a design session (ds)
THEN user sees warning about missing CODEMAPS
AND session continues normally
```

### AC-8: Skill Deletion
```
GIVEN the integration is complete
WHEN user lists available skills
THEN skills/codemaps/ no longer exists
AND codemaps functionality is documented in conductor skill
```

## Out of Scope

- Hash-based modification detection (v2)
- Auto-trigger staleness detection (v2)
- `/codemap` standalone command (v2)
- Complex pattern matching for file detection

## Dependencies

- Existing conductor skill and workflows
- Existing design skill
- beads CLI (for track management)

## Risks

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Break existing conductor flow | Medium | High | Test setup/refresh after changes |
| Mermaid diagrams too complex | Low | Low | Keep to 2 simple diagrams |
| Users miss standalone skill | Low | Low | Not heavily used |
