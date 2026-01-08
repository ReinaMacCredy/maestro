# Plan: Ralph Integration into Maestro

## Epic 1: Schema & Foundation

### 1.1 Extend metadata.schema.json with ralph section
- Add `ralph` object definition to schema
- Fields: `enabled`, `active`, `maxIterations`, `currentIteration`, `progressFile`, `stories`
- Add `"autonomous"` to `workflow.history.command` enum
- Add `"autonomous"` to `threads[].action` enum
- Files: `skills/conductor/references/schemas/metadata.schema.json`

### 1.2 Create toolboxes/ralph directory structure
- Copy `ralph.sh` from `tmp/ralph/` to `toolboxes/ralph/`
- Copy `prompt.md` from `tmp/ralph/` to `toolboxes/ralph/`
- Create `README.md` with usage docs
- Files: `toolboxes/ralph/ralph.sh`, `toolboxes/ralph/prompt.md`, `toolboxes/ralph/README.md`

## Epic 2: Ralph Adaptation

### 2.1 Modify ralph.sh to read metadata.json
- Accept track path as argument
- Read `metadata.json` instead of `prd.json`
- Extract stories from `metadata.ralph.stories`
- Use `workflow.branch` for branch name
- Write progress to track's `progress.txt`
- Update `metadata.ralph.stories[id].passes` on completion
- Set `metadata.ralph.active = true/false` for locking
- Files: `toolboxes/ralph/ralph.sh`

### 2.2 Adapt prompt.md for Maestro conventions
- Reference Maestro track structure
- Read from `metadata.json` path
- Update AGENTS.md references
- Use bead IDs in commit messages
- Files: `toolboxes/ralph/prompt.md`

## Epic 3: Routing & Commands

### 3.1 Add ca trigger to maestro-core routing table
- Add `ca` → conductor mapping
- Add `/conductor-autonomous` → conductor mapping
- Add phrase triggers: "run autonomous", "ralph mode"
- Files: `skills/maestro-core/references/routing-table.md`

### 3.2 Add /conductor-autonomous command to conductor skill
- Add entry point in SKILL.md
- Implement preflight: check metadata.ralph.enabled
- Set metadata.ralph.active = true
- Invoke `toolboxes/ralph/ralph.sh <track-path>`
- Handle completion: set active = false, update workflow.state
- Files: `skills/conductor/SKILL.md`, `skills/conductor/references/workflows/autonomous.md`

## Epic 4: Designing Integration

### 4.1 Update ds to populate metadata.ralph.stories
- After plan.md generation, extract tasks
- Populate `metadata.ralph.stories` with title, acceptanceCriteria
- Set `enabled = true` when stories populated
- Files: `skills/designing/SKILL.md`, `skills/designing/references/pipeline.md`

### 4.2 Add ca option to Phase 8 READY menu
- Update "Ready to execute" menu
- Add `[A] Autonomous (ca)` option
- Auto-select based on user preference
- Files: `skills/designing/SKILL.md`, `skills/designing/references/pipeline.md`

## Epic 5: Documentation & Polish

### 5.1 Update AGENTS.md with Ralph commands
- Add `ca` to Commands Quick Reference
- Document Ralph workflow
- Add gotchas (active lock, progress.txt)
- Files: `AGENTS.md`

### 5.2 Update conductor/AGENTS.md with learnings
- Add Ralph-specific commands
- Document gotchas discovered during implementation
- Files: `conductor/AGENTS.md`

## Track Assignments

| Track | Tasks | File Scope | Dependencies |
|-------|-------|------------|--------------|
| Schema | 1.1 | `skills/conductor/references/schemas/metadata.schema.json` | None (Wave 1) |
| Toolbox | 1.2, 2.1, 2.2 | `toolboxes/ralph/*` | 1.2 → 2.1, 2.2 |
| Routing | 3.1, 3.2 | `skills/maestro-core/*`, `skills/conductor/*` | 3.1 → 3.2 |
| Designing | 4.1, 4.2 | `skills/designing/*` | 4.1 → 4.2 |
| Docs | 5.1, 5.2 | `AGENTS.md`, `conductor/AGENTS.md` | 5.1 → 5.2 |

## Execution Waves

### Wave 1 (Parallel - No Dependencies)
- 1.1: Extend metadata.schema.json
- 1.2: Create toolboxes/ralph structure
- 3.1: Add ca trigger to routing
- 4.1: Update ds for ralph.stories
- 5.1: Update AGENTS.md

### Wave 2 (Depends on Wave 1)
- 2.1: Modify ralph.sh (needs 1.1, 1.2)
- 2.2: Adapt prompt.md (needs 1.2)
- 3.2: Add /conductor-autonomous (needs 3.1)
- 4.2: Add ca to Phase 8 menu (needs 4.1)
- 5.2: Update conductor/AGENTS.md (needs 5.1)

## Acceptance Criteria

- [ ] `ca` command invokes Ralph loop successfully
- [ ] Ralph reads/writes `metadata.json.ralph` correctly
- [ ] `ds` populates ralph.stories from plan
- [ ] Phase 8 menu offers Autonomous option
- [ ] Exclusive lock prevents ci/co during ca
- [ ] Documentation updated
