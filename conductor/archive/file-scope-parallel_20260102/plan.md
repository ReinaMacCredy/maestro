# Plan: File-Scope Parallel Detection

## Track Assignments

| Track | Tasks | Files | Depends On |
|-------|-------|-------|------------|
| A | 1.1, 1.2 | newtrack.md, metadata.schema.json | - |
| B | 2.1, 2.2 | implement.md, auto-routing.md | - |
| C | 3.1, 3.2, 3.3 | (after A+B) | Track A, Track B |

---

## Phase 1: Newtrack Enhancement (Track A)

### 1.1 Add File Scope Extraction Logic
- [ ] Create `references/file-scope-extractor.md` with extraction algorithm
- [ ] Define regex patterns for common file path formats
- [ ] Handle edge cases: relative paths, glob patterns, directory references
- [ ] **Files:** `conductor/references/file-scope-extractor.md` (new)

### 1.2 Add fileScopes to metadata.schema.json
- [ ] Add `beads.fileScopes` object to schema
- [ ] Format: `{ "taskId": ["path1", "path2"] }`
- [ ] Add to examples section
- [ ] **Files:** `conductor/references/schemas/metadata.schema.json`

---

## Phase 2: Implement Enhancement (Track B)

### 2.1 Add Parallel Grouping Algorithm
- [ ] Create `references/parallel-grouping.md` with grouping logic
- [ ] Define overlap detection (file-level, directory-level)
- [ ] Define threshold rules (≥2 groups → parallel)
- [ ] **Files:** `conductor/references/parallel-grouping.md` (new)

### 2.2 Add Confirmation Prompt
- [ ] Update `implement.md` Phase 2 (Execution Routing)
- [ ] Add prompt before orchestrator dispatch
- [ ] Handle Y/n response → route accordingly
- [ ] **Files:** `conductor/references/workflows/implement.md`

---

## Phase 3: Integration (Track C) - Sequential after A+B

### 3.1 Update newtrack.md Workflow
- [ ] Add Phase 4.5: File Scope Analysis (after plan generation)
- [ ] Call file-scope-extractor
- [ ] Call parallel-grouping
- [ ] If groups ≥2: generate Track Assignments section
- [ ] Update metadata.json with fileScopes + orchestrated flag
- [ ] **Files:** `conductor/references/workflows/newtrack.md`

### 3.2 Update auto-routing.md
- [ ] Add fileScopes-based routing as Priority 1.5
- [ ] Between Track Assignments check and beads auto-detect
- [ ] **Files:** `orchestrator/references/auto-routing.md`

### 3.3 Update SKILL.md Quick References
- [ ] Add file-scope detection to conductor SKILL.md
- [ ] Add confirmation prompt to orchestrator SKILL.md
- [ ] **Files:** `conductor/SKILL.md`, `orchestrator/SKILL.md`

---

## Automated Verification

```bash
# Verify new files created
ls -la skills/conductor/references/file-scope-extractor.md
ls -la skills/conductor/references/parallel-grouping.md

# Verify schema updated
jq '.properties.beads.properties.fileScopes' skills/conductor/references/schemas/metadata.schema.json

# Verify workflow updated
grep -n "File Scope Analysis" skills/conductor/references/workflows/newtrack.md
grep -n "confirmation prompt" skills/conductor/references/workflows/implement.md
```
