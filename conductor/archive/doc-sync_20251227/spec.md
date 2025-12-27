# Spec: Doc-Sync Feature

## Overview

Doc-sync skill cho Maestro plugin tự động sync documentation với code changes trong bất kỳ dự án nào sử dụng Conductor workflow.

## Problem

Documentation (README.md, SETUP.md, TUTORIAL.md, API docs) thường bị outdated khi code thay đổi. Developers phải manually track và update docs, dẫn đến:
- Docs không reflect reality
- New contributors confused
- Maintenance burden tăng

## Solution

Tạo skill với 3 components chính:
1. **Doc Scanner** - Tìm `.md` files có code references
2. **Change Detector** - Detect code changes từ git diff + beads
3. **Doc Updater** - Auto-update minor, review major changes

## Functional Requirements

### FR1: Doc Scanning
- Scan tất cả `.md` files trong project
- Detect code references: file paths, imports, function names, code blocks
- Output dependency map: doc → [code files it references]

### FR2: Change Detection
- Parse git diff để detect: added, removed, modified, renamed files
- Parse beads context từ closed issues trong track
- Output change manifest với impact classification (minor/major)

### FR3: Doc Updates
- **Minor changes** (auto-apply):
  - File path changes
  - Function/class name renames
  - Simple text replacements
- **Major changes** (prompt user):
  - New feature → add section?
  - Removed feature → remove section?
  - API signature → update examples?

### FR4: Conductor Integration
- Auto-trigger sau `/conductor-finish` (as Phase 7)
- Manual trigger via `/doc-sync` command
- Flags: `--dry-run`, `--force`

## Non-Functional Requirements

### NFR1: Performance
- Scan phải hoàn thành trong <5s cho project 1000 files
- Cache scan results để incremental updates

### NFR2: Accuracy
- False positive rate <5% cho code reference detection
- Conservative matching: prefer missing over wrong

### NFR3: User Experience
- Clear diff output cho proposed changes
- Easy rollback via git (no custom undo)

## Acceptance Criteria

### AC1: Scanner Works
```
Given a project with README.md containing `src/utils.ts` reference
When scanner runs
Then README.md appears in dependency map with src/utils.ts as dependency
```

### AC2: Minor Auto-Update
```
Given file renamed from `src/old.ts` to `src/new.ts`
And README.md contains reference to `src/old.ts`
When doc-sync runs
Then README.md is updated to reference `src/new.ts`
And no user prompt is shown
```

### AC3: Major Prompts User
```
Given a new feature added (detected via beads)
When doc-sync runs
Then user is prompted: "New feature X detected. Add section to README.md?"
```

### AC4: Conductor Integration
```
Given track completed via /conductor-finish
When Phase 6 (CODEMAPS) completes
Then Phase 7 (Doc-Sync) runs automatically
```

### AC5: Manual Command
```
Given user runs `/doc-sync --dry-run`
Then changes are shown but not applied
```

## Out of Scope

- Real-time file watching
- Non-markdown docs (HTML, PDF)
- External documentation sites
- Translation/i18n sync
- Custom templates cho new sections (future)

## Dependencies

- Git CLI (for git diff)
- Beads CLI (for context)
- Conductor skill (for integration)

## Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| False positives | Low | Conservative matching |
| Over-aggressive edits | Medium | Minor = path/name only |
| Performance on large projects | Low | Caching, incremental |

---

*Spec version 1.0 | Created 2025-12-27*
