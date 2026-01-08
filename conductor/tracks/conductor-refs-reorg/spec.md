# Spec: Conductor References Reorganization

## Overview

Reorganize `skills/conductor/references/` from 20 loose files to indexed subfolders while preserving lazy loading.

## Requirements

### Functional

1. **Create `beads/` subfolder** with 9 files + README index
2. **Create `execution/` subfolder** with 4 files + README index
3. **Consolidate workflows.md** absorbing 4 small utility files with section anchors
4. **Update all cross-references** in SKILL.md and related files
5. **Create temporary stub files** at old paths with redirect notes

### Non-Functional

- Lazy loading preserved (AI reads README first)
- No content loss during reorganization
- Git history preserved via `git mv`

## File Mapping

### beads/ subfolder

| Old Path | New Path |
|----------|----------|
| beads-facade.md | beads/facade.md |
| beads-integration.md | beads/integration.md |
| beads-session.md | beads/session.md |
| track-init-beads.md | beads/lifecycle.md |
| status-sync-beads.md | beads/status-sync.md |
| migrate-beads.md | beads/migrate.md |
| preflight-beads.md | beads/preflight.md |
| tdd-checkpoints-beads.md | beads/tdd-checkpoints.md |
| revise-reopen-beads.md | beads/revise-reopen.md |

### execution/ subfolder

| Old Path | New Path |
|----------|----------|
| pipeline.md | execution/pipeline.md |
| parallel-grouping.md | execution/parallel-grouping.md |
| decompose-task.md | execution/decompose-task.md |
| file-scope-extractor.md | execution/file-scope-extractor.md |

### Absorbed into workflows.md

| Old File | New Anchor |
|----------|------------|
| finish-workflow.md | workflows.md#finish-workflow |
| checkpoint.md | workflows.md#checkpoint |
| remember.md | workflows.md#remember |
| revisions.md | workflows.md#revisions |

### Kept at root

- CODEMAPS_TEMPLATE.md
- structure.md
- workflows.md (expanded)

## Success Criteria

- [ ] All 9 beads files moved to beads/ with README
- [ ] All 4 execution files moved to execution/ with README
- [ ] 4 small files absorbed into workflows.md
- [ ] All cross-references updated
- [ ] Stub files created for migration safety
- [ ] No broken links after reorganization
