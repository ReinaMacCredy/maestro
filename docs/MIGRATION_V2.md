# Migration Guide: v1.x to v2.0

## Overview

Maestro 2.0 migrates from a 3-layer architecture (commands/, workflows/, skills/) to a spec-compliant 2-layer architecture (skills/, AGENTS.md).

**Breaking Change**: The `commands/` and `workflows/` directories no longer exist.

## What Changed

### Directory Structure

**Before (v1.x):**
```
commands/           # Slash command definitions
workflows/          # Detailed workflow logic
  beads/           
  conductor/       
  party-mode/      
  agent-coordination/
skills/             # Thin skill stubs
```

**After (v2.0):**
```
skills/             # Complete skills with references/
  beads/
    SKILL.md
    references/     # Workflow docs moved here
  conductor/
    SKILL.md
    references/
      conductor/    # Old workflows/conductor/
      workflows/    # Old workflows/*.md
      schemas/      # Old workflows/schemas/
  design/
    SKILL.md
    references/
      party-mode/   # Old workflows/party-mode/
  dispatching-parallel-agents/
    references/
      agent-coordination/  # Old workflows/agent-coordination/
  session-compaction/     # NEW (was commands/compact.md)
```

### Removed Directories

| Old Path | New Path |
|----------|----------|
| `commands/` | Deleted - logic merged into skills |
| `workflows/` | Deleted - moved to `skills/*/references/` |

### Path Mapping

#### workflows/ → skills/*/references/

| Old Path | New Path |
|----------|----------|
| `workflows/beads/workflow.md` | `skills/beads/references/workflow.md` |
| `workflows/beads/references/*` | `skills/beads/references/*` |
| `workflows/conductor/*` | `skills/conductor/references/conductor/*` |
| `workflows/*.md` (setup, implement, etc.) | `skills/conductor/references/workflows/*.md` |
| `workflows/schemas/*` | `skills/conductor/references/schemas/*` |
| `workflows/party-mode/*` | `skills/design/references/party-mode/*` |
| `workflows/agent-coordination/*` | `skills/dispatching-parallel-agents/references/agent-coordination/*` |
| `workflows/context-engineering/*` | `skills/design/references/*` |

#### commands/ → Deleted or Merged

| Old Command | Status |
|-------------|--------|
| `commands/ds.md` | Deleted (pure alias - skill triggers directly) |
| `commands/fb.md` | Deleted (pure alias) |
| `commands/rb.md` | Deleted (pure alias) |
| `commands/ci.md` | Deleted (pure alias) |
| `commands/cn.md` | Deleted (pure alias) |
| `commands/ct.md` | Deleted (pure alias) |
| `commands/compact.md` | → `skills/session-compaction/SKILL.md` |
| `commands/ground.md` | → `skills/design/references/grounding.md` |
| `commands/decompose-task.md` | → `skills/conductor/references/decompose-task.md` |
| `commands/conductor-*.md` | → `skills/conductor/references/workflows/*.md` |

## New Skill: session-compaction

The `compact` command is now a full skill:

**Triggers:** `compact`, `/compact`, `session compact`, `compress context`

**Location:** `skills/session-compaction/SKILL.md`

## Migration Steps

### For Plugin Users

1. Update to v2.0
2. Clear any cached skill paths
3. Skills continue to work with same triggers (`ds`, `fb`, `rb`, `/conductor-implement`, etc.)

### For Custom Integrations

If you have custom tooling referencing old paths:

1. Update `workflows/` references to `skills/*/references/`
2. Remove `commands/` references - use skill triggers directly
3. Run validation: `./scripts/validate-links.sh .`

## Validation

After migration, verify:

```bash
# Check active paths only (archive and CHANGELOG may contain historical references)
rg "workflows/" skills docs README.md AGENTS.md --type md | grep -v "MIGRATION"
rg "commands/" skills docs README.md AGENTS.md --type md | grep -v "MIGRATION"

# Validate plugin
cat .claude-plugin/plugin.json | jq .

# Check links
./scripts/validate-links.sh .
```

> **Note:** References in `conductor/archive/`, `CHANGELOG.md`, and architecture samples are expected—these document historical states and don't affect runtime.

## Rollback

If issues occur, revert to v1.x:

```bash
git checkout v1.11.0
```

## Questions

Open an issue at https://github.com/ReinaMacCredy/maestro/issues
