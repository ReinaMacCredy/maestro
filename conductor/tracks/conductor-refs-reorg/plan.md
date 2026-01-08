# Plan: Conductor References Reorganization

## Tasks

### Wave 1: Create Structure

| ID | Task | Files | Est |
|----|------|-------|-----|
| T1 | Create beads/ subfolder + README index | beads/README.md | 10m |
| T2 | Create execution/ subfolder + README index | execution/README.md | 10m |

### Wave 2: Move Files (parallel)

| ID | Task | Files | Est |
|----|------|-------|-----|
| T3 | Move 9 beads files to beads/ | beads/*.md | 15m |
| T4 | Move 4 execution files to execution/ | execution/*.md | 10m |

### Wave 3: Consolidate & Update

| ID | Task | Files | Est |
|----|------|-------|-----|
| T5 | Absorb 4 small files into workflows.md with anchors | workflows.md | 15m |
| T6 | Update cross-references in conductor SKILL.md | SKILL.md | 15m |
| T7 | Update cross-references in other skills | designing/, tracking/, etc. | 20m |

### Wave 4: Migration Safety & Cleanup

| ID | Task | Files | Est |
|----|------|-------|-----|
| T8 | Create stub files at old paths | 13 stub files | 15m |
| T9 | Validate no broken links | all | 10m |
| T10 | Delete stub files after verification | 13 stub files | 5m |

## Dependencies

```
T1, T2 (parallel)
    ↓
T3, T4 (parallel, depend on T1/T2)
    ↓
T5 (depends on T3, T4)
    ↓
T6, T7 (parallel, depend on T5)
    ↓
T8 (depends on T6, T7)
    ↓
T9 (depends on T8)
    ↓
T10 (depends on T9)
```

## Track Assignments

Single agent track - sequential execution recommended due to file move dependencies.

## Estimated Total

~2 hours
