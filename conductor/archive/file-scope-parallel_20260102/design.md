# Design: File-Scope Parallel Detection

## Problem Statement

When `/conductor-implement` runs, it only triggers orchestrator (parallel execution) if:
1. `## Track Assignments` section exists in plan.md, OR
2. `metadata.json.orchestrated = true`

But many tasks that *could* run in parallel don't, because:
- Tasks are named sequentially ("Phase 1, 2, 3...")
- Beads are filed with implicit dependencies
- No automatic analysis of actual file scope

## User Expectation

"6 tasks exist → system should detect which can run parallel based on files touched"

## Solution: Two-Stage File Scope Analysis

### Stage 1: `/conductor-newtrack` (Plan Creation)

After plan.md is generated, analyze each task's file scope:

```python
for each task in plan:
    scope = extract_file_paths(task.description)

parallel_groups = group_non_overlapping(scopes)

if len(parallel_groups) >= 2:
    generate_track_assignments(parallel_groups)
    set metadata.json.orchestrated = true
```

Output: Auto-generated `## Track Assignments` section in plan.md

### Stage 2: `/conductor-implement` (Runtime)

When Track Assignments detected:
1. Show confirmation prompt: "Plan has N parallel tracks. Run parallel? [Y/n]"
2. If confirmed → spawn workers via orchestrator
3. If declined → sequential with TDD

## Key Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Detection location | Both newtrack + implement | Suggest at creation, confirm at runtime |
| Grouping strategy | File scope (directory-based) | Same directory = potential conflicts |
| Threshold | ≥2 non-overlapping groups | Matches existing bead threshold |
| User control | Confirmation prompt | User can decline parallel if preferred |

## Example Analysis

Given tasks:
- Phase 1: Add handoff to metadata.schema.json
- Phase 2: Create handoff.toml
- Phase 3: Create handoff.md
- Phase 4: Update implement.md (refs handoff.md)
- Phase 5: Delete old files, update SKILL.md
- Phase 6: Update AGENTS.md

File scope grouping:
| Phase | Files | Group |
|-------|-------|-------|
| 1 | schemas/metadata.schema.json | A |
| 2 | commands/handoff.toml | B |
| 3 | workflows/handoff.md | C |
| 4 | workflows/implement.md | D (after C) |
| 5 | various, SKILL.md | E (after C) |
| 6 | AGENTS.md | F |

Result:
- Track 1 (parallel): Phases 1, 2, 3
- Track 2 (sequential after Track 1): Phases 4, 5, 6

## Out of Scope

- ML-based dependency inference
- Cross-repository analysis
- Real-time file conflict detection during execution
