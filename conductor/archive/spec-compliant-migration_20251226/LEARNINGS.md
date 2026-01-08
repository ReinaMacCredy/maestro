# LEARNINGS - spec-compliant-migration_20251226

## Summary

Migrated Maestro from 3-layer architecture (commands/, workflows/, skills/) to spec-compliant 2-layer architecture (skills/, AGENTS.md).

**Duration:** 1 session
**Commits:** 4

## Commands

- `./scripts/validate-links.sh .` - Validate markdown links
- `./scripts/validate-anchors.sh .` - Validate anchor references
- `sed -i '' 's|old|new|g' file` - macOS in-place sed replacement
- `rg "pattern" --type md -l` - Find files containing pattern

## Gotchas

- Relative paths change when files move - `references/X.md` becomes `./X.md` when you're already in references/
- Archive files contain historical references - don't fix them (they're snapshots)
- Template example files need reference stubs created
- Some broken links are intentional (external docs like Anthropic's)
- Party-mode agent paths need updating when workflow moves

## Patterns

- **Two-commit strategy**: git mv first (preserves blame), then content updates
- **Validation before commit**: Run link validation before each commit
- **Progressive fixing**: Fix critical paths first, archive paths last
- **Exclude patterns**: Use grep -v for archive/, MIGRATION, CHANGELOG when validating

## Key Decisions

1. **Pure alias commands deleted**: ds, fb, rb, ci, cn, ct were just triggers - skill handles directly
2. **compact.md → session-compaction skill**: Full skill with triggers, not just a command
3. **workflows/README.md → pipeline.md**: More descriptive name for the workflow pipeline doc
4. **Archive files not updated**: They represent historical state, updating would be incorrect

## Files Changed

### Created
- `skills/session-compaction/SKILL.md`
- `skills/design/references/execution-routing.md`
- `skills/conductor/references/revisions.md`
- `skills/dispatching-parallel-agents/references/agent-coordination/patterns/*.md`
- `templates/claude-code-setup/skills/example-skill/references/detailed-guide.md`
- `docs/MIGRATION_V2.md`

### Major Path Fixes
- `skills/beads/references/workflow.md` - 25+ path fixes
- `skills/conductor/references/workflows/*.md` - 15+ path fixes
- `skills/conductor/references/conductor/*.md` - 10+ path fixes
- `skills/design/references/party-mode/workflow.md` - party-mode agent paths

## Metrics

- **Files moved:** 65+ (via git mv)
- **Links fixed:** 146 → 0 (active files)
- **Commands removed:** 16 files
- **New skill created:** 1 (session-compaction)
