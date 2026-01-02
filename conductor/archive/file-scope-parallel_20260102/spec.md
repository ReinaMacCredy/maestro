# Spec: File-Scope Parallel Detection

## Overview

Enhance `/conductor-newtrack` and `/conductor-implement` to automatically detect parallelization opportunities based on file scope analysis. Tasks touching different files can run in parallel; tasks touching same files must run sequentially.

## Functional Requirements

### FR1: File Scope Extraction (newtrack)

- Parse each task description in plan.md
- Extract file paths mentioned (explicit or inferred from task context)
- Store file scope in `metadata.json.beads.fileScopes`

### FR2: Parallel Grouping Algorithm (newtrack)

- Group tasks by file overlap:
  - Same file → same group (sequential)
  - Different files → can be in different groups (parallel)
- Use directory-level grouping as fallback when file paths unclear
- Threshold: ≥2 non-overlapping groups → generate Track Assignments

### FR3: Track Assignments Generation (newtrack)

- Auto-generate `## Track Assignments` section in plan.md
- Format:
  ```markdown
  ## Track Assignments
  
  | Track | Tasks | Files | Depends On |
  |-------|-------|-------|------------|
  | 1 | 1.1, 1.2, 1.3 | schemas/, commands/, workflows/ | - |
  | 2 | 2.1, 2.2, 2.3 | workflows/implement.md, SKILL.md, AGENTS.md | Track 1 |
  ```
- Set `metadata.json.orchestrated = true`

### FR4: Confirmation Prompt (implement)

- When Track Assignments detected, show:
  ```
  📊 Parallel execution detected:
  - Track 1: 3 tasks (schemas/, commands/, workflows/)
  - Track 2: 3 tasks (depends on Track 1)
  
  Run parallel? [Y/n]:
  ```
- Y → route to orchestrator
- n → sequential with TDD

### FR5: Dependency Detection

- Detect implicit dependencies:
  - Task mentions "after X" or "once Y is done"
  - Task references file created by earlier task
  - Task is in later "Phase" than another
- Wire dependencies in Track Assignments

## Non-Functional Requirements

### NFR1: Performance

- File scope extraction must complete in <2 seconds for plans with ≤50 tasks

### NFR2: Accuracy

- File path extraction should be ≥80% accurate for explicit file mentions
- False positives (claiming parallel when not safe) should be <5%

### NFR3: User Override

- User can always decline parallel execution
- User can manually edit Track Assignments before implementation

## Acceptance Criteria

- [ ] `/conductor-newtrack` generates Track Assignments for plans with ≥2 independent file scopes
- [ ] File scopes stored in `metadata.json.beads.fileScopes`
- [ ] `/conductor-implement` shows confirmation prompt when Track Assignments exist
- [ ] Declining parallel routes to sequential TDD mode
- [ ] Dependencies correctly detected from "Phase X" naming convention

## Out of Scope

- ML-based dependency inference
- Cross-repository file analysis
- Real-time conflict detection during parallel execution
- Automatic conflict resolution
