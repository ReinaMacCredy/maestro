# Plan: Doc-Sync Feature

## Epic 1: Skill Scaffold

Create basic skill structure following Maestro conventions.

### Task 1.1: Create SKILL.md
- **File:** `skills/doc-sync/SKILL.md`
- **Action:** Create skill definition with YAML frontmatter
- **Content:**
  - name: doc-sync
  - description: Auto-sync documentation with code changes
  - triggers: `/doc-sync`, after `/conductor-finish`
  - When to use section
- **Verify:** Skill appears in available skills list

### Task 1.2: Create references directory structure
- **Files:**
  - `skills/doc-sync/references/scanner.md`
  - `skills/doc-sync/references/detector.md`
  - `skills/doc-sync/references/updater.md`
  - `skills/doc-sync/references/integration.md`
- **Action:** Create empty reference files with headers
- **Verify:** Directory structure exists

---

## Epic 2: Doc Scanner

Implement document scanning to find markdown files with code references.

### Task 2.1: Define scanner logic in scanner.md
- **File:** `skills/doc-sync/references/scanner.md`
- **Content:**
  - Scan algorithm (find all .md, extract code refs)
  - Code reference patterns (file paths, imports, function names, code blocks)
  - Output format (dependency map JSON)
- **Verify:** Document complete with examples

### Task 2.2: Define code reference patterns
- **File:** `skills/doc-sync/references/scanner.md` (append)
- **Content:**
  - Regex patterns for:
    - File paths: `/src/`, `./lib/`, etc.
    - Imports: `import X from`, `require('X')`
    - Function names: backtick-wrapped code
    - Code blocks: triple-backtick with language
- **Verify:** Patterns documented with test cases

---

## Epic 3: Change Detector

Implement change detection from git diff and beads context.

### Task 3.1: Define git diff parsing in detector.md
- **File:** `skills/doc-sync/references/detector.md`
- **Content:**
  - Git diff command: `git diff --name-status HEAD~N`
  - Parse output: A (added), D (deleted), M (modified), R (renamed)
  - Detect function signature changes (optional, via diff content)
- **Verify:** Document complete with examples

### Task 3.2: Define beads context extraction
- **File:** `skills/doc-sync/references/detector.md` (append)
- **Content:**
  - Command: `bd list --status=closed --json`
  - Extract: issue titles, descriptions, affected files
  - Merge with git diff for complete picture
- **Verify:** Beads extraction documented

### Task 3.3: Define impact classification
- **File:** `skills/doc-sync/references/detector.md` (append)
- **Content:**
  - Minor: file renames, path changes
  - Major: new features, removed features, API changes
  - Classification algorithm
- **Verify:** Impact rules documented with examples

---

## Epic 4: Doc Updater

Implement update strategies for minor and major changes.

### Task 4.1: Define minor update strategies in updater.md
- **File:** `skills/doc-sync/references/updater.md`
- **Content:**
  - Path replacement algorithm
  - Function name replacement
  - Code block updates
- **Verify:** Strategies documented

### Task 4.2: Define major update prompts
- **File:** `skills/doc-sync/references/updater.md` (append)
- **Content:**
  - Prompt templates for:
    - New feature: "Add section for X?"
    - Removed feature: "Remove section for X?"
    - API change: "Update examples for X?"
  - User response handling
- **Verify:** Prompts documented

### Task 4.3: Define output format
- **File:** `skills/doc-sync/references/updater.md` (append)
- **Content:**
  - Summary table format
  - Diff preview format
  - --dry-run output format
- **Verify:** Output formats documented

---

## Epic 5: Conductor Integration

Integrate doc-sync into Conductor workflow.

### Task 5.1: Define integration points in integration.md
- **File:** `skills/doc-sync/references/integration.md`
- **Content:**
  - Phase 7 in /conductor-finish flow
  - Manual /doc-sync command
  - Flag handling (--dry-run, --force)
- **Verify:** Integration documented

### Task 5.2: Update finish-workflow.md
- **File:** `skills/conductor/references/finish-workflow.md`
- **Action:** Add Phase 7: Doc-Sync section
- **Content:**
  - Trigger conditions
  - Skip conditions
  - Error handling
- **Verify:** finish-workflow.md updated

### Task 5.3: Update SKILL.md with full workflow
- **File:** `skills/doc-sync/SKILL.md`
- **Action:** Add complete workflow instructions
- **Content:**
  - Trigger detection
  - Phase flow (scan → detect → update)
  - Examples
- **Verify:** SKILL.md complete and functional

---

## Epic 6: Testing & Documentation

Final testing and documentation updates.

### Task 6.1: Test skill on Maestro project itself
- **Action:** Run `/doc-sync --dry-run` on this project
- **Verify:** Skill detects docs and code refs correctly
- **Output:** Document any issues found

### Task 6.2: Update CODEMAPS
- **File:** `conductor/CODEMAPS/overview.md`
- **Action:** Add doc-sync to entry points table
- **Verify:** CODEMAPS reflects new skill

### Task 6.3: Update AGENTS.md
- **File:** `AGENTS.md`
- **Action:** Add doc-sync to skills table
- **Verify:** AGENTS.md lists new skill

---

## Summary

| Epic | Tasks | Focus |
|------|-------|-------|
| 1 | 2 | Skill scaffold |
| 2 | 2 | Doc scanner |
| 3 | 3 | Change detector |
| 4 | 3 | Doc updater |
| 5 | 3 | Conductor integration |
| 6 | 3 | Testing & docs |

**Total: 6 epics, 16 tasks**

---

*Plan version 1.0 | Created 2025-12-27*
