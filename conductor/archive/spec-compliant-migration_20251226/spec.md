# Spec: Spec-Compliant Skills-Only Architecture Migration

## Overview

Migrate Maestro from 3-layer architecture (commands/, workflows/, skills/) to spec-compliant 2-layer architecture (skills/, AGENTS.md).

**Version:** 2.0.0 (breaking change)

## Requirements

### R1: Remove workflows/ Directory
- [ ] Move all 56 workflow files to appropriate skills/*/references/
- [ ] Preserve directory structure where meaningful
- [ ] Update all internal references

### R2: Remove commands/ Directory  
- [ ] Merge 10 command files into skills/*/references/
- [ ] Delete 6 pure alias commands (ds, fb, rb, ci, cn, ct)
- [ ] Create new session-compaction skill from compact.md
- [ ] Merge ground.md into skills/design/references/grounding.md

### R3: Update All References (~95 references)
- [ ] Update paths in all 16 skill SKILL.md files
- [ ] Update AGENTS.md architecture section
- [ ] Update README.md structure diagram
- [ ] Update CLAUDE.md workflow triggers
- [ ] Update TUTORIAL.md examples
- [ ] Update docs/PIPELINE_ARCHITECTURE.md
- [ ] Update templates/claude-code-setup/

### R4: Create New Skill
- [ ] Create skills/session-compaction/SKILL.md
- [ ] Move compact.md logic into it
- [ ] Add to skill registry in AGENTS.md

### R5: Update Templates
- [ ] Remove templates/claude-code-setup/.claude/commands/
- [ ] Convert audit.md to skills/audit/SKILL.md pattern
- [ ] Update SETUP.md to reflect skills-only pattern

### R6: Documentation
- [ ] Complete docs/MIGRATION_V2.md (created)
- [ ] Complete docs/MIGRATION_PATH_MAP.md (created)
- [ ] Add validation scripts to docs/

### R7: Version Bump
- [ ] Use `feat!:` commit message for CI auto-bump to 2.0.0
- [ ] Do NOT manually edit plugin.json version
- [ ] Feature freeze during migration

## Acceptance Criteria

### AC1: Zero Old Path References
```bash
rg "workflows/" --type md  # Returns 0 (except migration docs)
rg "commands/" --type md   # Returns 0 (except migration docs)
```

### AC2: All Skills Load Successfully
Each trigger works without "file not found":
- `ds` → loads design skill
- `fb` → loads beads skill (FILE_BEADS)
- `rb` → loads beads skill (REVIEW_BEADS)
- `/conductor-implement` → works correctly

### AC3: Git History Preserved
2-commit strategy:
1. Commit 1: `git mv` operations only (preserves blame)
2. Commit 2: Reference updates

### AC4: CI Passes
- Plugin manifest validates: `cat .claude-plugin/plugin.json | jq .`
- No broken relative links
- Version correctly bumped to 2.0.0

### AC5: Documentation Complete
- MIGRATION_V2.md explains all breaking changes
- MIGRATION_PATH_MAP.md has complete old→new mapping
- README.md reflects new architecture

## Out of Scope

- Functionality changes to skills
- New features beyond architecture migration
- Performance optimization
- Test coverage additions
