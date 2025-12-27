# Design: Doc-Sync Feature

## Problem Statement

Khi developers thêm/xóa/sửa/refactor features trong code, documentation (README.md, SETUP.md, TUTORIAL.md, API docs) thường bị outdated. Cần một hệ thống tự động detect code changes và update docs tương ứng.

## Solution Overview

Tạo **doc-sync skill** cho Maestro plugin:
- Auto-scan `.md` files có chứa code references
- Detect code changes qua git diff + beads context
- Auto-update minor changes, review prompt cho major changes
- Tích hợp vào Conductor flow

## Triggers

| Trigger | When |
|---------|------|
| Auto | Sau `/conductor-finish` |
| Manual | `/doc-sync` command |

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                     doc-sync skill                       │
├─────────────────────────────────────────────────────────┤
│                                                          │
│  1. SCAN          2. DETECT         3. UPDATE           │
│  ┌──────────┐    ┌──────────┐     ┌──────────┐         │
│  │ Find .md │───▶│ Git diff │────▶│ Minor:   │         │
│  │ with code│    │ + Beads  │     │ Auto-edit│         │
│  │ refs     │    │ context  │     │          │         │
│  └──────────┘    └──────────┘     │ Major:   │         │
│                                   │ Review   │         │
│                                   └──────────┘         │
└─────────────────────────────────────────────────────────┘
```

## Components

### 1. Doc Scanner

Tìm tất cả `.md` files có code references:
- File paths (`src/`, `lib/`, etc.)
- Import statements
- Function/class names
- Code blocks with language tags

**Output:** List of docs + their code dependencies

### 2. Change Detector

Detect code changes từ 2 sources:

**Git Diff:**
- Files added/removed/modified
- Function signatures changed
- File paths renamed

**Beads Context:**
- Closed issues trong track
- Summary của changes từ beads

**Output:** Change manifest (what changed, impact level)

### 3. Doc Updater

Phân loại và xử lý:

| Change Type | Impact | Action |
|-------------|--------|--------|
| File path changed | Minor | Auto-update refs |
| Function renamed | Minor | Auto-update examples |
| New feature added | Major | Prompt: add section? |
| Feature removed | Major | Prompt: remove section? |
| API signature changed | Major | Prompt: update examples? |

### 4. Integration Points

**Với `/conductor-finish`:**
```
Phase 6 (CODEMAPS) → Phase 7 (Doc-Sync) → Archive
```

**Manual command:**
```
/doc-sync [--dry-run] [--force]
  --dry-run  Show changes without applying
  --force    Auto-apply all changes (skip review)
```

## Data Flow

```
Code changes (git/beads)
        ↓
   Change Detector
        ↓
   Impact Analysis (minor/major)
        ↓
   ┌────┴────┐
   ↓         ↓
Minor     Major
   ↓         ↓
Auto     Prompt User
Edit     "Update section X?"
   ↓         ↓
   └────┬────┘
        ↓
   Updated Docs
        ↓
   Show Summary
```

## File Structure

```
skills/
└── doc-sync/
    ├── SKILL.md           # Skill definition
    └── references/
        ├── scanner.md     # Doc scanning logic
        ├── detector.md    # Change detection logic
        ├── updater.md     # Update strategies
        └── integration.md # Conductor integration
```

## Success Criteria

1. ✅ Auto-detect `.md` files với code references
2. ✅ Detect add/remove/modify/refactor changes
3. ✅ Auto-update minor changes (paths, names)
4. ✅ Prompt for major changes (new/removed sections)
5. ✅ Integrate với `/conductor-finish`
6. ✅ Manual `/doc-sync` command works
7. ✅ Works với any project using Conductor

## Out of Scope

- Real-time file watching (chỉ trigger-based)
- Non-markdown docs (HTML, PDF, etc.)
- External documentation sites
- Translation/i18n sync

## Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| False positives (detect wrong refs) | Conservative matching, require explicit code blocks |
| Over-aggressive auto-edit | Minor = only path/name changes, everything else = review |
| Large docs slow to scan | Cache scan results, incremental updates |

## Open Questions

1. Config file format? (`.doc-sync.yaml` vs convention-only)
2. Should track doc-sync history in beads?
3. Support custom templates for new sections?

---

*Design approved: Ready for `/conductor-newtrack doc-sync_20251227`*
