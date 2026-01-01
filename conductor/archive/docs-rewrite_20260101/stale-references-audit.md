# Stale References Audit

Generated: 2026-01-01

## Summary

Found **13 stale references** that need correction in Phase 2.

## Stale References by File

### TUTORIAL.md (11 references)

| Line | Issue | Context |
|------|-------|---------|
| 162 | Removed skill `file-beads` | Historical example mentions "beads, file-beads, review-beads" |
| 163 | Removed skill `file-beads` | "I then ran file-beads (fb)" |
| 164 | Removed skill `review-beads` | "I ran review-beads (rb)" |
| 172 | Removed skills | "Delete skills/file-beads/, skills/review-beads/" |
| 208 | Non-existent file | "@skills/file-beads/SKILL.md" |
| 209 | Non-existent file | "@skills/review-beads/SKILL.md" |
| 212 | Removed skills | "(beads, file-beads, review-beads)" |
| 217-220 | Outdated migration notes | References to old skill names |
| 239 | Outdated command | "rb (review-beads)" |
| 647 | Removed skill | "systematic-debugging" |
| 1298 | Removed skill | "condition-based-waiting" |

**Action**: These are historical handoff examples. In the rewrite, either:
- Remove these examples entirely (recommended - they're verbose)
- Replace with current workflow examples

### README.md

No stale references to deprecated skills found.

### SETUP_GUIDE.md

No stale references to deprecated skills found.

### AGENTS.md

No stale references to deprecated skills found.

### docs/*.md

No stale references to deprecated skills found.

## Links Validation

All internal markdown links verified as valid:
- ✓ skills/beads/references/workflow-integration.md
- ✓ skills/orchestrator/references/agent-coordination.md
- ✓ skills/orchestrator/references/router.md
- ✓ skills/design/references/bmad/workflows/party-mode/workflow.md
- ✓ All conductor/references/handoff/*.md files

## Recommended Actions for Phase 2

1. **TUTORIAL.md rewrite**: Remove or replace the historical handoff examples (lines 147-250) with concise, current examples
2. **TUTORIAL.md Quick Reference**: Update skill references table (line 647, 1298) - these skills are now external (superpowers plugin)
3. **Consider**: The historical examples take ~100 lines but add little value - recommend removing them entirely per the design goal of "~500 lines max"
