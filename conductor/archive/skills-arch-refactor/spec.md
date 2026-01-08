# Skills Architecture Refactor - Specification

**Track ID:** skills-arch-refactor  
**Version:** 1.0  
**Created:** 2026-01-08

---

## Overview

Refactor the Maestro workflow skills to establish clear ownership boundaries, eliminate overlap, and align with Anthropic's official skill best practices.

## Goals

1. **Clear Ownership** - Each skill owns specific artifacts with no overlap
2. **Gerund Naming** - Align with Anthropic naming conventions where appropriate
3. **Flattened References** - Max 1-level deep (except bmad/, agents/)
4. **Reduced Command Surface** - Commands owned by appropriate skills

## Non-Goals

- Agent Mail coordination fixes (follow-up track)
- Validation gate simplification (follow-up track)
- New feature development

---

## Functional Requirements

### FR-1: Skill Renaming
- `design/` → `designing/`
- `beads/` → `tracking/`
- `skill-creator/` + `writing-skills/` → `creating-skills/`

### FR-2: Ownership Boundaries
- `designing` owns: phases 1-8, Oracle, research, `ds`, `cn`
- `conductor` owns: `ci`, TDD, validation gates
- `orchestrator` owns: `co`, workers, agents, file reservations
- `tracking` owns: `bd`, beads CLI, dependencies, memory
- `handoff` owns: `ho`, session context, archive

### FR-3: Command Migration
- Move `/conductor-newtrack` → `designing`
- Move `/conductor-design` → `designing`
- Move `/conductor-orchestrate` → `orchestrator`
- Move `/conductor-finish` → `handoff`
- Move `/conductor-handoff` → `handoff`

### FR-4: Reference Flattening
- Eliminate cross-skill references (e.g., `../conductor/references/`)
- Flatten nested directories to single level
- Exception: Keep `bmad/` and `agents/` as subdirectories

### FR-5: Documentation Updates
- Update `maestro-core` routing table and hierarchy
- Update CODEMAPS to reflect new structure
- Update AGENTS.md with new mappings

---

## Technical Requirements

### TR-1: Backward Compatibility
- Keep CLI triggers stable (`ds`, `ci`, `co`, `bd`, `ho`)
- Add deprecation notices for moved commands

### TR-2: File Operations
- Use `git mv` for renames to preserve history
- Update all internal references before deleting old paths

### TR-3: Validation
- All SKILL.md files must be ≤500 lines
- All references must be 1-level deep (with exceptions)
- No broken cross-skill links

---

## Constraints

- No changes to `using-git-worktrees` or `sharing-skills`
- Keep `maestro-core` as noun (router, not action)
- Keep `conductor`, `orchestrator`, `handoff` names per user preference

---

## Acceptance Criteria

| ID | Criterion | Verification |
|----|-----------|--------------|
| AC-1 | All renames complete | `ls .claude/skills/` shows new names |
| AC-2 | No cross-skill refs | `grep -r "../" .claude/skills/` returns empty |
| AC-3 | Triggers work | Manual test: `ds`, `ci`, `co`, `bd`, `ho` |
| AC-4 | SKILL.md ≤500 lines | `wc -l .claude/skills/*/SKILL.md` |
| AC-5 | CODEMAPS updated | Review `conductor/CODEMAPS/skills.md` |
| AC-6 | AGENTS.md updated | Review root + conductor AGENTS.md |
