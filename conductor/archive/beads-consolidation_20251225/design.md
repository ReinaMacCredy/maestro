# Design: Beads Consolidation

## Problem Statement

Consolidate 3 beads-related skills (beads, file-beads, review-beads) into a unified architecture with thin skill entry point and centralized workflow logic, reducing maintenance overhead while preserving all trigger patterns (bd, fb, rb).

## Success Criteria

| # | Criterion | Verification |
|---|-----------|--------------|
| 1 | `fb` trigger loads beads skill | Manual test |
| 2 | `rb` trigger loads beads skill | Manual test |
| 3 | `bd` commands work unchanged | Manual test |
| 4 | 1 skill folder for beads | `ls skills/ \| grep beads` |
| 5 | Logic in workflows/beads/ | Files exist |
| 6 | No old refs in active docs | Grep returns 0 |
| 7 | CLI_REFERENCE.md clean | No broken links |

## Architecture

### Before

```
skills/
├── beads/
│   ├── SKILL.md              # ~775 lines
│   └── references/           # 8 files
├── file-beads/
│   └── SKILL.md              # ~394 lines
└── review-beads/
    └── SKILL.md              # ~417 lines
```

### After

```
skills/beads/
└── SKILL.md                    # Thin stub (~50 lines)

workflows/beads/
├── workflow.md                 # Main entry (bd) - lowercase
└── references/                 # 10 files total - UPPERCASE
    ├── BOUNDARIES.md
    ├── CLI_REFERENCE.md        # + cleanup broken links
    ├── DEPENDENCIES.md
    ├── FILE_BEADS.md           # fb logic (renamed)
    ├── ISSUE_CREATION.md
    ├── RESUMABILITY.md
    ├── REVIEW_BEADS.md         # rb logic (renamed)
    ├── STATIC_DATA.md
    ├── VILLAGE.md
    └── WORKFLOWS.md

# DELETED
skills/file-beads/              ❌
skills/review-beads/            ❌
skills/file-beads/      ❌
skills/review-beads/    ❌
```

## Thin Skill Content

```yaml
---
name: beads
version: "2.0.0"
description: "Beads issue tracking (bd, fb, rb). Use for multi-session work, file-beads, review-beads, filing beads from plan, reviewing beads."
---

# Beads

Issue tracking for multi-session work with dependency graphs.

## Entry Points

| Trigger | Workflow | Action |
|---------|----------|--------|
| `bd`, `beads` | `workflows/beads/workflow.md` | Core CLI operations |
| `fb`, `file-beads` | `workflows/beads/references/FILE_BEADS.md` | File beads from plan |
| `rb`, `review-beads` | `workflows/beads/references/REVIEW_BEADS.md` | Review filed beads |

## Load Workflow

1. Identify trigger from user input
2. Load corresponding workflow file (see table above)
3. Follow instructions in loaded file

## Quick Decision

- **Multi-session work?** → Use beads
- **Single-session linear task?** → Use TodoWrite
```

## Naming Convention

- `workflow.md` - lowercase (main entry point)
- `references/*.md` - UPPERCASE (supporting docs)
- Renamed files: `file-beads.md` → `FILE_BEADS.md`, `review-beads.md` → `REVIEW_BEADS.md`

## Documentation Updates (14 files)

| File | Update Type |
|------|-------------|
| `commands/fb.md` | `file-beads skill` → `beads skill` |
| `commands/rb.md` | `review-beads skill` → `beads skill` |
| `commands/conductor/newTrack.toml` | Update prompts |
| `workflows/newtrack.md` | Update prompts |
| `workflows/README.md` | Mermaid nodes |
| `skills/conductor/SKILL.md` | Text refs |
| `skills/design/SKILL.md` | Mermaid nodes |
| `AGENTS.md` | Merge table rows |
| `README.md` | Multiple updates |
| `CLAUDE.md` | Table + tree |
| `TUTORIAL.md` | Comprehensive (14 lines) |
| `docs/PIPELINE_ARCHITECTURE.md` | Mermaid nodes |
| `conductor/CODEMAPS/overview.md` | Table rows |
| `conductor/CODEMAPS/skills.md` | Tree entries |

## CLI_REFERENCE.md Cleanup

Remove broken links (12 lines):
- Lines 281-282: Broken "See also" block
- Line 370: Broken sentence with CONFIG.md, TROUBLESHOOTING.md
- Line 403: Broken DAEMON.md reference
- Lines 556-561: Entire "See Also" section (all links broken)

## Hard Links

Both `skills/` and `skills/` directories are hard-linked (same inode). Updating one updates both.

## Rollback Plan

```bash
git checkout main -- skills/file-beads skills/review-beads
git checkout main -- skills/file-beads skills/review-beads
git checkout main -- skills/beads
rm -rf workflows/beads/
```

## Estimated Effort

~1.5 hours total
