# Design: Spec-Compliant Skills-Only Architecture

## Problem Statement

Maestro's 3-layer architecture (commands/, workflows/, skills/) violates Anthropic's skills specification. Commands are not thin stubs, and workflows contain detailed references that should reside within skills.

## Decision

Adopt 100% spec-compliant architecture:
- All detailed logic and references reside within `skills/*/references/`
- Remove `commands/` and `workflows/` directories entirely
- AGENTS.md serves as high-level pipeline map and trigger table

---

## Architecture Change

### Before (v1.x)
```
maestro/
├── skills/           # Skill definitions
├── commands/         # Slash command files
└── workflows/        # Workflow definitions
```

### After (v2.0.0)
```
maestro/
├── skills/           # Skills + all references
│   └── */references/ # Detailed logic lives here
└── AGENTS.md         # Pipeline map + triggers
```

**Why:** Anthropic's skills specification recommends all detailed logic reside within `skills/*/references/`. This enables better tooling integration and clearer mental model.

---

## Key Decisions

### Architecture
1. **Single container pattern**: All logic in `skills/*/references/`
2. **No commands/**: Slash commands become skill triggers or merge into references
3. **No workflows/**: All workflow docs move to appropriate skill's references/

### Decisions from Party Mode Review
1. **Keep 7 files** in agent-coordination (don't flatten)
2. **Convert templates** to skill pattern (remove templates/commands/)
3. **Feature freeze** during migration to avoid CI version bump conflicts
4. **Let CI handle** version bump via `feat!:` commit

### Cross-Skill References
- `subagent-driven-development` → `dispatching-parallel-agents/references/`
- `beads` → `conductor/references/beads-integration.md`
- Document coupling in SKILL headers

---

## Breaking Changes

### 1. Directories Removed

| Directory | Replacement |
|-----------|-------------|
| `commands/` | Merged into `skills/*/references/` |
| `workflows/` | Merged into `skills/*/references/` |

### 2. Slash Command Aliases Removed

| Removed | Use Instead |
|---------|-------------|
| `/ds` | `ds` (no slash, trigger phrase) |
| `/fb` | `fb` (no slash, trigger phrase) |
| `/rb` | `rb` (no slash, trigger phrase) |
| `/ci` | `/conductor-implement` |
| `/cn` | `/conductor-newtrack` |
| `/ct` | `/conductor-status` |

---

## Migration Summary

| Category | Count | Action |
|----------|-------|--------|
| workflows/ files | 56 | Move to skills/*/references/ |
| workflows/ schemas | 6 | Move to skills/conductor/references/schemas/ |
| commands/ files | 18 | 10 merge, 6 delete, 2 new skills |
| **Total files affected** | **80** | |

---

## Path Mappings

### workflows/beads/ → skills/beads/references/

| Old Path | New Path |
|----------|----------|
| `workflows/beads/workflow.md` | `skills/beads/references/workflow.md` |
| `workflows/beads/references/AGENTS.md` | `skills/beads/references/AGENTS.md` |
| `workflows/beads/references/BOUNDARIES.md` | `skills/beads/references/BOUNDARIES.md` |
| `workflows/beads/references/CLI_REFERENCE.md` | `skills/beads/references/CLI_REFERENCE.md` |
| `workflows/beads/references/CONFIG.md` | `skills/beads/references/CONFIG.md` |
| `workflows/beads/references/DAEMON.md` | `skills/beads/references/DAEMON.md` |
| `workflows/beads/references/DEPENDENCIES.md` | `skills/beads/references/DEPENDENCIES.md` |
| `workflows/beads/references/FILE_BEADS.md` | `skills/beads/references/FILE_BEADS.md` |
| `workflows/beads/references/GIT_INTEGRATION.md` | `skills/beads/references/GIT_INTEGRATION.md` |
| `workflows/beads/references/ISSUE_CREATION.md` | `skills/beads/references/ISSUE_CREATION.md` |
| `workflows/beads/references/LABELS.md` | `skills/beads/references/LABELS.md` |
| `workflows/beads/references/RESUMABILITY.md` | `skills/beads/references/RESUMABILITY.md` |
| `workflows/beads/references/REVIEW_BEADS.md` | `skills/beads/references/REVIEW_BEADS.md` |
| `workflows/beads/references/STATIC_DATA.md` | `skills/beads/references/STATIC_DATA.md` |
| `workflows/beads/references/TROUBLESHOOTING.md` | `skills/beads/references/TROUBLESHOOTING.md` |
| `workflows/beads/references/VILLAGE.md` | `skills/beads/references/VILLAGE.md` |
| `workflows/beads/references/WORKFLOWS.md` | `skills/beads/references/WORKFLOWS.md` |

**Count: 17 files**

### workflows/conductor/ → skills/conductor/references/conductor/

| Old Path | New Path |
|----------|----------|
| `workflows/conductor/beads-session.md` | `skills/conductor/references/conductor/beads-session.md` |
| `workflows/conductor/checkpoint.md` | `skills/conductor/references/conductor/checkpoint.md` |
| `workflows/conductor/preflight-beads.md` | `skills/conductor/references/conductor/preflight-beads.md` |
| `workflows/conductor/remember.md` | `skills/conductor/references/conductor/remember.md` |
| `workflows/conductor/revise-reopen-beads.md` | `skills/conductor/references/conductor/revise-reopen-beads.md` |
| `workflows/conductor/status-sync-beads.md` | `skills/conductor/references/conductor/status-sync-beads.md` |
| `workflows/conductor/tdd-checkpoints-beads.md` | `skills/conductor/references/conductor/tdd-checkpoints-beads.md` |
| `workflows/conductor/track-init-beads.md` | `skills/conductor/references/conductor/track-init-beads.md` |

**Count: 8 files**

### Root workflows/*.md → skills/conductor/references/workflows/

| Old Path | New Path |
|----------|----------|
| `workflows/README.md` | `skills/conductor/references/pipeline.md` |
| `workflows/setup.md` | `skills/conductor/references/workflows/setup.md` |
| `workflows/newtrack.md` | `skills/conductor/references/workflows/newtrack.md` |
| `workflows/implement.md` | `skills/conductor/references/workflows/implement.md` |
| `workflows/status.md` | `skills/conductor/references/workflows/status.md` |
| `workflows/revert.md` | `skills/conductor/references/workflows/revert.md` |
| `workflows/revise.md` | `skills/conductor/references/workflows/revise.md` |
| `workflows/finish.md` | `skills/conductor/references/workflows/finish.md` |
| `workflows/validate.md` | `skills/conductor/references/workflows/validate.md` |

**Count: 9 files**

### workflows/schemas/ → skills/conductor/references/schemas/

| Old Path | New Path |
|----------|----------|
| `workflows/schemas/fb_progress.schema.json` | `skills/conductor/references/schemas/fb_progress.schema.json` |
| `workflows/schemas/finish_state.schema.json` | `skills/conductor/references/schemas/finish_state.schema.json` |
| `workflows/schemas/implement_state.schema.json` | `skills/conductor/references/schemas/implement_state.schema.json` |
| `workflows/schemas/metadata.schema.json` | `skills/conductor/references/schemas/metadata.schema.json` |
| `workflows/schemas/setup_state.schema.json` | `skills/conductor/references/schemas/setup_state.schema.json` |
| `workflows/schemas/track_progress.schema.json` | `skills/conductor/references/schemas/track_progress.schema.json` |

**Count: 6 files**

### workflows/party-mode/ → skills/design/references/party-mode/

| Old Path | New Path |
|----------|----------|
| `workflows/party-mode/workflow.md` | `skills/design/references/party-mode/workflow.md` |
| `workflows/party-mode/agents/creative/brainstorm.md` | `skills/design/references/party-mode/agents/creative/brainstorm.md` |
| `workflows/party-mode/agents/creative/design-thinking.md` | `skills/design/references/party-mode/agents/creative/design-thinking.md` |
| `workflows/party-mode/agents/creative/solver.md` | `skills/design/references/party-mode/agents/creative/solver.md` |
| `workflows/party-mode/agents/creative/storyteller.md` | `skills/design/references/party-mode/agents/creative/storyteller.md` |
| `workflows/party-mode/agents/creative/strategist.md` | `skills/design/references/party-mode/agents/creative/strategist.md` |
| `workflows/party-mode/agents/product/analyst.md` | `skills/design/references/party-mode/agents/product/analyst.md` |
| `workflows/party-mode/agents/product/pm.md` | `skills/design/references/party-mode/agents/product/pm.md` |
| `workflows/party-mode/agents/product/ux.md` | `skills/design/references/party-mode/agents/product/ux.md` |
| `workflows/party-mode/agents/technical/architect.md` | `skills/design/references/party-mode/agents/technical/architect.md` |
| `workflows/party-mode/agents/technical/developer.md` | `skills/design/references/party-mode/agents/technical/developer.md` |
| `workflows/party-mode/agents/technical/docs.md` | `skills/design/references/party-mode/agents/technical/docs.md` |
| `workflows/party-mode/agents/technical/qa.md` | `skills/design/references/party-mode/agents/technical/qa.md` |
| `workflows/party-mode/custom/_template.md` | `skills/design/references/party-mode/custom/_template.md` |
| `workflows/party-mode/custom/README.md` | `skills/design/references/party-mode/custom/README.md` |

**Count: 15 files**

### workflows/context-engineering/ → skills/design/references/

| Old Path | New Path |
|----------|----------|
| `workflows/context-engineering/session-lifecycle.md` | `skills/design/references/session-lifecycle.md` |
| `workflows/context-engineering/references/anchored-state-format.md` | `skills/design/references/anchored-state-format.md` |
| `workflows/context-engineering/references/design-routing-heuristics.md` | `skills/design/references/design-routing-heuristics.md` |

**Count: 3 files**

### workflows/agent-coordination/ → skills/dispatching-parallel-agents/references/agent-coordination/

| Old Path | New Path |
|----------|----------|
| `workflows/agent-coordination/workflow.md` | `skills/dispatching-parallel-agents/references/agent-coordination/workflow.md` |
| `workflows/agent-coordination/examples/dispatch-three-agents.md` | `skills/dispatching-parallel-agents/references/agent-coordination/examples/dispatch-three-agents.md` |
| `workflows/agent-coordination/patterns/execution-routing.md` | `skills/dispatching-parallel-agents/references/agent-coordination/patterns/execution-routing.md` |
| `workflows/agent-coordination/patterns/graceful-fallback.md` | `skills/dispatching-parallel-agents/references/agent-coordination/patterns/graceful-fallback.md` |
| `workflows/agent-coordination/patterns/parallel-dispatch.md` | `skills/dispatching-parallel-agents/references/agent-coordination/patterns/parallel-dispatch.md` |
| `workflows/agent-coordination/patterns/session-lifecycle.md` | `skills/dispatching-parallel-agents/references/agent-coordination/patterns/session-lifecycle.md` |
| `workflows/agent-coordination/patterns/subagent-prompt.md` | `skills/dispatching-parallel-agents/references/agent-coordination/patterns/subagent-prompt.md` |

**Count: 7 files**

---

## Commands Mapping

### Commands to MERGE into skills/*/references/

| Old Path | New Path | Target Skill |
|----------|----------|--------------|
| `commands/conductor-design.md` | `skills/design/references/conductor-design-workflow.md` | design |
| `commands/conductor-setup.md` | `skills/conductor/references/workflows/setup.md` | conductor (merge) |
| `commands/conductor-newtrack.md` | `skills/conductor/references/workflows/newtrack.md` | conductor (merge) |
| `commands/conductor-implement.md` | `skills/conductor/references/workflows/implement.md` | conductor (merge) |
| `commands/conductor-status.md` | `skills/conductor/references/workflows/status.md` | conductor (merge) |
| `commands/conductor-revert.md` | `skills/conductor/references/workflows/revert.md` | conductor (merge) |
| `commands/conductor-revise.md` | `skills/conductor/references/workflows/revise.md` | conductor (merge) |
| `commands/conductor-finish.md` | `skills/conductor/references/finish-workflow.md` | conductor (merge) |
| `commands/conductor-migrate-beads.md` | `skills/conductor/references/migrate-beads.md` | conductor |
| `commands/decompose-task.md` | `skills/conductor/references/decompose-task.md` | conductor |

**Count: 10 files**

### Commands to DELETE (pure aliases)

| Old Path | Reason |
|----------|--------|
| `commands/ds.md` | Alias for design skill trigger |
| `commands/fb.md` | Alias for beads FILE_BEADS trigger |
| `commands/rb.md` | Alias for beads REVIEW_BEADS trigger |
| `commands/ci.md` | Alias for /conductor-implement |
| `commands/cn.md` | Alias for /conductor-newtrack |
| `commands/ct.md` | Alias for /conductor-status |

**Count: 6 files**

### Commands to NEW SKILLS

| Old Path | New Skill | New Path |
|----------|-----------|----------|
| `commands/compact.md` | session-compaction | `skills/session-compaction/SKILL.md` |
| `commands/ground.md` | (merge into design) | `skills/design/references/grounding.md` |

**Count: 2 files**

---

## Cross-Skill References

After migration, these cross-skill references must be updated:

| From Skill | References To | New Path (relative from source) |
|------------|---------------|--------------------------------|
| `skills/beads/SKILL.md` | conductor/references/beads-integration.md | `../conductor/references/beads-integration.md` |
| `skills/conductor/SKILL.md` | design/references/party-mode/ | `../design/references/party-mode/` |
| `skills/subagent-driven-development/SKILL.md` | dispatching-parallel-agents/references/agent-coordination/ | `../dispatching-parallel-agents/references/agent-coordination/` |
| `skills/design/SKILL.md` | conductor/SKILL.md | `../conductor/SKILL.md` |

---

## Skill Entry Point Updates

### skills/beads/SKILL.md Entry Points Table

**Before:**
```markdown
| Trigger | Workflow | Action |
|---------|----------|--------|
| `bd`, `beads` | `workflows/beads/workflow.md` | Core CLI operations |
| `fb`, `file-beads` | `workflows/beads/references/FILE_BEADS.md` | File beads from plan |
| `rb`, `review-beads` | `workflows/beads/references/REVIEW_BEADS.md` | Review filed beads |
```

**After:**
```markdown
| Trigger | Reference | Action |
|---------|-----------|--------|
| `bd`, `beads` | `references/workflow.md` | Core CLI operations |
| `fb`, `file-beads` | `references/FILE_BEADS.md` | File beads from plan |
| `rb`, `review-beads` | `references/REVIEW_BEADS.md` | Review filed beads |
```

---

## New Directories to Create

```bash
mkdir -p skills/beads/references
mkdir -p skills/design/references/party-mode/agents/creative
mkdir -p skills/design/references/party-mode/agents/product
mkdir -p skills/design/references/party-mode/agents/technical
mkdir -p skills/design/references/party-mode/custom
mkdir -p skills/dispatching-parallel-agents/references/agent-coordination/examples
mkdir -p skills/dispatching-parallel-agents/references/agent-coordination/patterns
mkdir -p skills/conductor/references/conductor
mkdir -p skills/conductor/references/workflows
mkdir -p skills/conductor/references/schemas
mkdir -p skills/session-compaction
```

---

## Git Commands (Commit 1: Moves)

```bash
# 1. workflows/beads/ → skills/beads/references/
git mv workflows/beads/workflow.md skills/beads/references/
git mv workflows/beads/references/* skills/beads/references/

# 2. workflows/conductor/ → skills/conductor/references/conductor/
git mv workflows/conductor/* skills/conductor/references/conductor/

# 3. Root workflows/*.md → skills/conductor/references/workflows/
git mv workflows/setup.md skills/conductor/references/workflows/
git mv workflows/newtrack.md skills/conductor/references/workflows/
git mv workflows/implement.md skills/conductor/references/workflows/
git mv workflows/status.md skills/conductor/references/workflows/
git mv workflows/revert.md skills/conductor/references/workflows/
git mv workflows/revise.md skills/conductor/references/workflows/
git mv workflows/finish.md skills/conductor/references/workflows/
git mv workflows/validate.md skills/conductor/references/workflows/
git mv workflows/README.md skills/conductor/references/pipeline.md

# 4. workflows/schemas/ → skills/conductor/references/schemas/
git mv workflows/schemas/* skills/conductor/references/schemas/

# 5. workflows/party-mode/ → skills/design/references/party-mode/
git mv workflows/party-mode/* skills/design/references/party-mode/

# 6. workflows/context-engineering/ → skills/design/references/
git mv workflows/context-engineering/session-lifecycle.md skills/design/references/
git mv workflows/context-engineering/references/* skills/design/references/

# 7. workflows/agent-coordination/ → skills/dispatching-parallel-agents/references/agent-coordination/
git mv workflows/agent-coordination/* skills/dispatching-parallel-agents/references/agent-coordination/

# 8. Remove empty workflows/
rmdir workflows/context-engineering/references workflows/context-engineering
rmdir workflows/beads/references workflows/beads
rmdir workflows/schemas workflows/conductor
rmdir workflows/party-mode/agents/creative workflows/party-mode/agents/product workflows/party-mode/agents/technical workflows/party-mode/agents workflows/party-mode/custom workflows/party-mode
rmdir workflows/agent-coordination/examples workflows/agent-coordination/patterns workflows/agent-coordination
rmdir workflows

# 9. Delete alias commands
git rm commands/ds.md commands/fb.md commands/rb.md commands/ci.md commands/cn.md commands/ct.md
```

---

## Verification Checklist

```bash
# 1. Find any remaining references to old paths
rg "workflows/" --type md
rg "commands/" --type md

# 2. Find broken relative links
rg "\.\./workflows/" --type md
rg "\.\./commands/" --type md

# 3. Verify all new paths exist
find skills/*/references -type f | wc -l  # Should be ~65

# 4. Validate JSON schemas still work
cat skills/conductor/references/schemas/*.json | jq .

# 5. Check for orphaned anchor links
rg "#[a-z-]+" --type md -o | sort | uniq -c | sort -rn
```

---

## Version Bump

**Strategy:** Use `feat!:` commit message to trigger CI auto-bump to 2.0.0

```bash
git commit -m "feat!: migrate to spec-compliant skills-only architecture

BREAKING CHANGE: Remove commands/ and workflows/ directories.
All logic now lives in skills/*/references/.

Migration guide: docs/MIGRATION_V2.md"
```

**Do NOT manually edit plugin.json version** - let CI handle it.

---

## Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| Broken links after move | Validation script + 2-commit strategy |
| Users confused by removed /ds | Document in migration section above |
| CI double-bump | Feature freeze during migration |
| Anchor links silently broken | Manual spot-check top 20 |

---

## Who Needs to Act

### Plugin Users (just update)
```bash
# Claude Code
/plugin update maestro

# Manual
git -C ~/.claude/plugins/maestro pull
```

No action needed — paths are internal.

### Skill Authors Extending Maestro
1. Search your skills for `workflows/` and `commands/`
2. Update paths per the mapping tables above
3. Test that references resolve

### Forks and Derivatives
1. Review path mappings above
2. Apply equivalent moves to your fork
3. Update all internal references

---

## Grounding

- Verified Anthropic skills spec recommends references/ pattern
- Confirmed 80 files need moving via find commands
- Oracle analysis validated completeness
- Party Mode review (Winston, Murat, Paige) approved with conditions
