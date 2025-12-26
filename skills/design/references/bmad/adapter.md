# BMAD Adapter for Maestro

Path transformation rules for BMAD v6 compatibility.

## Purpose

This adapter enables Maestro to use BMAD v6 workflow structures while maintaining
compatibility with upstream BMAD updates. When BMAD v6 changes paths or structures,
update this adapter rather than refactoring all files.

## Path Transformations

### Agent Paths

| BMAD v6 Path | Maestro Path |
|--------------|--------------|
| `_bmad/personas/{id}.md` | `agents/{module}/{id}.md` |
| `_bmad/personas/bmad-master.md` | `agents/core/bmad-master.md` |
| `agent-manifest.csv` | `manifest.yaml` |

### Workflow Paths

| BMAD v6 Path | Maestro Path |
|--------------|--------------|
| `_bmad/workflows/{name}/` | `workflows/{name}/` |
| `_bmad/workflows/{name}/workflow.md` | `workflows/{name}/workflow.md` |
| `_bmad/checklists/*.md` | `workflows/{name}/steps/*.md` |

### Resource Paths

| BMAD v6 Path | Maestro Path |
|--------------|--------------|
| `_bmad/data/brain-methods.csv` | `workflows/brainstorming/brain-methods.csv` |
| `_bmad/data/story-*.md` | `agents/cis/storyteller/sidecar/*.md` |
| `_bmad/teams/*.csv` | `teams/*.csv` |

## Format Transformations

### Agent Format

BMAD v6 uses YAML-based agent definitions. Maestro uses native MD with YAML frontmatter:

**BMAD v6:**
```yaml
# agent.yaml
id: architect
name: Winston
persona: |
  Role, identity, communication style...
principles:
  - User journeys drive technical decisions
```

**Maestro:**
```markdown
---
id: architect
name: Winston
title: Architect
icon: 🏗️
module: bmm
source: bmad-v6.0.0-alpha.21
---

# Winston - System Architect

## Persona
Role, Identity, Communication Style

## Principles
- User journeys drive technical decisions

## Expertise
- System design
- Distributed systems
```

### Workflow Format

Workflow structure is preserved. Steps are split into separate files in `steps/` directory.

## Sync Workflow

When syncing with upstream BMAD:

1. Check BMAD release notes for breaking changes
2. Update version in `config.yaml` source field
3. For agent changes:
   - Compare persona content
   - Merge into Maestro MD format
   - Update source field in agent frontmatter
4. For workflow changes:
   - Update workflow.md
   - Update/add step files
5. For new resources (CSV, sidecar):
   - Copy directly, no transformation needed
6. Run verification tests

## Version Tracking

Each agent's frontmatter includes a `source` field:
```yaml
source: bmad-v6.0.0-alpha.21
```

This tracks which BMAD version the agent was last synced from.

## Fallback Behavior

If a path transform fails:
1. Log warning with original path
2. Try alternative paths in order
3. Fall back to inline content if all paths fail
4. Continue workflow (don't halt)
