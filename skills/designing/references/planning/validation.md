# Phase 8: Validation Reference

> **Purpose:** Validate beads completeness and dependency graph integrity before exiting the planning phase.

## bv Validation Commands

The `bv` (beads validate) command provides AI-powered analysis of your beads.

| Command | Purpose | Output |
|---------|---------|--------|
| `bv --robot-suggest` | Suggest missing dependencies | List of `bd dep add` commands |
| `bv --robot-insights` | Analyze bead quality | Quality scores and improvement suggestions |
| `bv --robot-priority` | Suggest priority order | Ordered list by dependency depth + impact |

### Usage Examples

```bash
# Suggest missing dependencies between beads
bv --robot-suggest
# Output: Suggests: bd dep add my-workflow:3-abc.2 my-workflow:3-abc.1

# Analyze bead quality and completeness
bv --robot-insights
# Output: Bead quality report with scores

# Get recommended implementation priority
bv --robot-priority
# Output: Priority-ordered list of beads
```

> **Note:** Always use `--robot-*` flags. Bare `bv` without flags will hang waiting for input.

## bd dep Commands for Fixes

When validation identifies missing or incorrect dependencies, use these commands:

```bash
# Add a missing dependency (child depends on parent)
bd dep add <child-id> <parent-id>

# Remove an incorrect dependency
bd dep remove <child-id> <parent-id>
```

### Examples

```bash
# Make bead .2 depend on bead .1
bd dep add my-workflow:3-qrdw.2 my-workflow:3-qrdw.1

# Remove an incorrect dependency
bd dep remove my-workflow:3-qrdw.3 my-workflow:3-qrdw.5

# View current dependencies for a bead
bd show my-workflow:3-qrdw.2 --deps
```

## Oracle Final Review

Before exiting Phase 8, invoke the Oracle for a completeness check.

### Oracle Prompt Template

```markdown
## Beads Completeness Review

**Track:** [track-id]
**Plan:** conductor/tracks/[track-id]/plan.md

### Review Criteria

1. **File Scope:** Every bead has explicit file(s) listed
2. **Dependencies:** Dependency graph is complete and acyclic
3. **Risks:** All HIGH risks have been resolved or explicitly accepted
4. **Actionability:** Each bead describes testable acceptance criteria
5. **Coverage:** Plan covers all requirements from spec.md

### Beads Summary
[paste `bd list --track [track-id]` output here]

### Dependency Graph
[paste `bd deps --track [track-id] --graph` output here]

### Request
Review the above beads for completeness. Return one of:
- **APPROVED** - Ready for implementation
- **NEEDS_REVISION** - With specific issues to address
```

### Oracle Response Format

**APPROVED Response:**
```
✅ APPROVED

Beads are complete and ready for implementation.
- Total: X beads
- Dependency depth: Y levels
- Estimated effort: Z story points
```

**NEEDS_REVISION Response:**
```
⚠️ NEEDS_REVISION

Issues found:
1. Bead .3 missing file scope
2. Circular dependency: .5 → .7 → .5
3. HIGH risk "API breaking change" not addressed

Actions required:
- bd update my-workflow:3-qrdw.3 --file "src/api/handler.ts"
- bd dep remove my-workflow:3-qrdw.7 my-workflow:3-qrdw.5
- Add mitigation for API breaking change risk
```

## Validation Checklist

Complete this checklist before exiting Phase 8 to Phase 9 (exit):

### Required

- [ ] **All beads have file scope** - Every bead specifies which file(s) it touches
- [ ] **No circular dependencies** - Run `bv --robot-suggest` and verify no cycles
- [ ] **HIGH risks resolved/accepted** - Each HIGH risk has mitigation or explicit acceptance
- [ ] **Oracle APPROVED** - Oracle review returned APPROVED status

### Recommended

- [ ] **Dependency depth ≤ 5** - Deep chains indicate poor decomposition
- [ ] **No orphan beads** - All beads connected to dependency graph
- [ ] **Clear acceptance criteria** - Each bead has testable definition of done
- [ ] **Effort estimates** - Story points assigned for planning

## Quick Validation Flow

```
┌─────────────────────────────────────────────────────────────┐
│  1. Run bv --robot-suggest                                  │
│     └─ Fix missing deps with bd dep add                     │
│                                                             │
│  2. Run bv --robot-insights                                 │
│     └─ Address quality issues                               │
│                                                             │
│  3. Check for circular dependencies                         │
│     └─ Fix with bd dep remove                               │
│                                                             │
│  4. Review HIGH risks                                       │
│     └─ Add mitigations or explicit acceptance               │
│                                                             │
│  5. Oracle final review                                     │
│     └─ APPROVED → Exit to Phase 9                           │
│     └─ NEEDS_REVISION → Fix and re-review                   │
└─────────────────────────────────────────────────────────────┘
```

## Common Validation Issues

| Issue | Detection | Fix |
|-------|-----------|-----|
| Missing file scope | `bv --robot-insights` | `bd update <id> --file "path"` |
| Circular dependency | `bv --robot-suggest` | `bd dep remove <child> <parent>` |
| Orphan bead | `bd deps --graph` | `bd dep add <child> <parent>` |
| Missing dependency | `bv --robot-suggest` | `bd dep add <child> <parent>` |
| Unaddressed HIGH risk | Manual review | Add mitigation to plan.md |

## Related References

- [filing.md](filing.md) - Phase 7 filing workflow
- [../phases/phase-8.md](../phases/phase-8.md) - Phase 8 details
- [../../tracking/references/workflow-integration.md](../../tracking/references/workflow-integration.md) - Beads workflow
