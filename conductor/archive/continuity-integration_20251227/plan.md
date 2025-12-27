# Continuity Integration Implementation Plan

## Overview

Phased implementation of Continuous-Claude patterns into Maestro workflow.

**Total Phases:** 4
**Estimated Effort:** ~8 hours

---

## Epic 1: Skill & Directory Structure (P1)

**Goal:** Create continuity skill and data directories
**Priority:** P0 - Foundation for all other work
**Estimated:** 2 hours

### Tasks

#### 1.1 Create sessions directory structure
- [ ] Create `conductor/sessions/active/`
- [ ] Create `conductor/sessions/archive/`
- [ ] Create `conductor/.cache/`
- [ ] Add `.gitkeep` files
- [ ] Create `conductor/sessions/.gitignore` (ignore LEDGER.md)
- [ ] Create `conductor/.cache/.gitignore` (ignore all)

**Verification:** `ls conductor/sessions/` shows active/ and archive/

#### 1.2 Create continuity skill
- [ ] Create `skills/continuity/SKILL.md` with frontmatter
- [ ] Define triggers: load, save, handoff, status, search
- [ ] Document automatic behavior (Claude Code)
- [ ] Document manual commands
- [ ] Create `skills/continuity/references/ledger-format.md`
- [ ] Create `skills/continuity/references/handoff-format.md`
- [ ] Create `skills/continuity/references/amp-setup.md`

**Verification:** Skill loads when saying "continuity load"

#### 1.3 Delete session-compaction skill
- [ ] Remove `skills/session-compaction/SKILL.md`
- [ ] Remove `skills/session-compaction/` directory
- [ ] Update any references in AGENTS.md

**Verification:** `ls skills/session-compaction` returns not found

#### 1.4 Update tracks.md
- [ ] Add continuity-integration track entry

**Verification:** Track appears in conductor/tracks.md

---

## Epic 2: Claude Code Hooks (P2)

**Goal:** Implement TypeScript hooks for Claude Code
**Priority:** P0 - Core automation
**Estimated:** 3 hours
**Depends on:** Epic 1

### Tasks

#### 2.1 Create hook infrastructure
- [ ] Create `.claude/hooks/src/` directory
- [ ] Create `.claude/hooks/package.json` with TypeScript deps
- [ ] Create `.claude/hooks/tsconfig.json`
- [ ] Create `.claude/hooks/settings-hooks.json` template

**Verification:** `npm install` succeeds in .claude/hooks/

#### 2.2 Implement continuity.ts
- [ ] Create main entry point with command switch
- [ ] Implement `handleSessionStart()` with ledger loading
- [ ] Implement `handlePreCompact()` with auto-handoff
- [ ] Implement `handlePostToolUse()` with file tracking
- [ ] Implement `handleStop()` with session archive
- [ ] Add `ensureDirectories()` fallback
- [ ] Add `--version` flag
- [ ] Add try/catch to all handlers

**Verification:** `node dist/continuity.js --version` outputs version

#### 2.3 Create install script
- [ ] Create `scripts/install-global-hooks.sh`
- [ ] Add Node.js/npm dependency check
- [ ] Add copy logic for src files
- [ ] Add build step
- [ ] Add settings.json merge logic
- [ ] Make script executable

**Verification:** Run script, verify `~/.claude/hooks/dist/continuity.js` exists

#### 2.4 Create smoke test
- [ ] Create `scripts/test-hooks.sh`
- [ ] Test version command
- [ ] Test SessionStart without ledger
- [ ] Test SessionStart with ledger
- [ ] Test PreCompact creates handoff
- [ ] Make script executable

**Verification:** `./scripts/test-hooks.sh` passes all tests

---

## Epic 3: Artifact Index (P3)

**Goal:** Implement Python scripts for searchable history
**Priority:** P1 - Enhancement
**Estimated:** 2 hours
**Depends on:** Epic 1

### Tasks

#### 3.1 Create artifact-index.py
- [ ] Implement SQLite database initialization
- [ ] Implement FTS5 virtual table creation
- [ ] Implement handoff parsing (YAML frontmatter)
- [ ] Implement index_handoffs() function
- [ ] Add `--verify` flag for integrity check
- [ ] Add main entry point

**Verification:** `uv run scripts/artifact-index.py --verify` shows count

#### 3.2 Create artifact-query.py
- [ ] Implement search() function with FTS5
- [ ] Add snippet highlighting
- [ ] Add limit parameter
- [ ] Add dependency check for uv
- [ ] Add main entry point

**Verification:** `uv run scripts/artifact-query.py test` shows results or "no results"

#### 3.3 Create artifact-cleanup.py
- [ ] Implement cleanup() with age threshold
- [ ] Add SQLite index sync after deletion
- [ ] Add `--dry-run` flag
- [ ] Add `--max-age` parameter
- [ ] Add main entry point

**Verification:** `uv run scripts/artifact-cleanup.py --dry-run` shows what would be deleted

---

## Epic 4: Amp & Setup Integration (P4)

**Goal:** Amp Code support and /conductor-setup integration
**Priority:** P1 - Full platform support
**Estimated:** 1 hour
**Depends on:** Epic 1, Epic 2

### Tasks

#### 4.1 Add Amp hooks configuration
- [ ] Create amp.hooks JSON for tool:post-execute
- [ ] Add session protocol instructions
- [ ] Update `skills/continuity/references/amp-setup.md` with full config

**Verification:** amp.hooks JSON is valid

#### 4.2 Update /conductor-setup
- [ ] Read `skills/conductor/references/workflows/setup.md`
- [ ] Add Phase 8: Continuity Setup section
- [ ] Add platform detection logic
- [ ] Add sessions structure creation
- [ ] Add platform-specific hook setup
- [ ] Add state update for continuity_configured

**Verification:** Run `/conductor-setup` on test project, verify Phase 8 runs

#### 4.3 Update documentation
- [ ] Update AGENTS.md with continuity section
- [ ] Update README.md with feature mention (optional)

**Verification:** AGENTS.md contains continuity instructions

---

## Verification Checkpoints

### After Epic 1
- [ ] `skills/continuity/SKILL.md` exists and loads
- [ ] `conductor/sessions/` structure exists
- [ ] `skills/session-compaction/` deleted

### After Epic 2
- [ ] Hooks installed at `~/.claude/hooks/`
- [ ] SessionStart injects context
- [ ] PreCompact creates handoff
- [ ] Smoke tests pass

### After Epic 3
- [ ] `uv run scripts/artifact-index.py` builds index
- [ ] `uv run scripts/artifact-query.py` searches
- [ ] `uv run scripts/artifact-cleanup.py` cleans

### After Epic 4
- [ ] Amp hooks documented
- [ ] `/conductor-setup` includes Phase 8
- [ ] All acceptance criteria pass

---

## Dependencies Graph

```
Epic 1 (Skill & Dirs)
    │
    ├──► Epic 2 (Claude Hooks)
    │        │
    │        └──► Epic 4 (Amp & Setup) ◄──┐
    │                                     │
    └──► Epic 3 (Artifact Index) ─────────┘
```

## Risk Mitigation Tasks

- [ ] Add concurrent session limitation to SKILL.md documentation
- [ ] Add Python/uv installation instructions to SKILL.md
- [ ] Test on fresh repo without any conductor/ setup
