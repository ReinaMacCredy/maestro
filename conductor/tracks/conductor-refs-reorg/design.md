# Design: Conductor References Reorganization

## Problem

The `skills/conductor/references/` directory has 20 loose markdown files (~6,200 lines) at root level, making navigation difficult and defeating organizational clarity.

## Solution

**Index Pattern** — Group files into thematic subfolders with README indexes, preserving lazy loading while improving organization.

### Structure

```
references/
├── beads/
│   ├── README.md        ← index (which file for what)
│   ├── facade.md
│   ├── integration.md
│   ├── session.md
│   ├── lifecycle.md     ← renamed from track-init-beads.md
│   ├── status-sync.md
│   ├── migrate.md
│   ├── preflight.md
│   ├── tdd-checkpoints.md
│   └── revise-reopen.md
├── execution/
│   ├── README.md        ← index
│   ├── pipeline.md
│   ├── parallel-grouping.md
│   ├── decompose-task.md
│   └── file-scope-extractor.md
├── workflows.md         ← absorbs 4 small files (with anchors)
├── structure.md
└── CODEMAPS_TEMPLATE.md
```

### Key Decisions

| Decision | Rationale |
|----------|-----------|
| Index pattern over merge | Preserves lazy loading - AI reads small README, loads only needed files |
| Beads subfolder | 9 files share "beads" theme (~4,700 lines) |
| Execution subfolder | 4 files about task execution (~800 lines) |
| Absorb 4 tiny files | finish-workflow (15), revisions (18), checkpoint (41), remember (45) → workflows.md with anchors |
| Stub files for migration | Temporary redirects prevent broken refs |

### Result

- **Before:** 20 loose files at root
- **After:** 3 root files + 2 indexed subfolders (15 files total, organized)

## Oracle Audit

✅ Approved with recommendations:
- Add stable anchors in workflows.md for absorbed content
- Temporary stub files during migration
- Update cross-references in SKILL.md and other skills
