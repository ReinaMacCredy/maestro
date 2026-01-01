# Design: Documentation Rewrite

## Problem Statement

Maestro's documentation is out of sync with v3.1+ structure, contains stale references, is too verbose for quick reference, and lacks coverage for recent features (auto-orchestration, validation, handoff system). Both human users and AI agents struggle to find accurate, actionable information.

## Success Criteria

- [ ] All docs reflect current skill structure (6 consolidated skills)
- [ ] No references to removed skills/features
- [ ] Human docs: scannable, conceptual, "why" focused
- [ ] Agent docs: precise triggers, decision trees, "how" focused
- [ ] Complete coverage of v3.1 features (orchestrator, validation, handoff)

## Design Decision

**Approach: Streamlined Single-Source**

Fewer files, clear purpose per file, minimal duplication.

## Final Structure

```
README.md           # Landing page (~100 lines max)
SETUP_GUIDE.md      # Installation (~150 lines)
TUTORIAL.md         # Human guide - concepts + workflow (~500 lines)
REFERENCE.md        # Commands/triggers/quick-ref (NEW - ~300 lines)
AGENTS.md           # Agent instructions only
docs/
├── ARCHITECTURE.md # Pipeline diagrams (consolidate from PIPELINE_ARCHITECTURE)
└── CHANGELOG.md    # Version history
```

## Files to DELETE/ARCHIVE

| File | Action |
|------|--------|
| `docs/GLOBAL_CONFIG.md` | Merge into SETUP_GUIDE.md |
| `docs/manual-workflow-guide.md` | Merge into TUTORIAL.md |
| `docs/handoff-system.md` | Merge into TUTORIAL.md |
| `docs/MIGRATION_V2.md` | Archive (historical) |
| `docs/PIPELINE_ARCHITECTURE.md` | Rename to ARCHITECTURE.md |

## Content Plan

| File | Purpose | Key Sections | Max Lines |
|------|---------|--------------|-----------|
| **README.md** | First impression, quick install, links | Install, Quick Start, Links | 100 |
| **SETUP_GUIDE.md** | Get running | Plugin install, CLI install, Global config, Verify | 150 |
| **TUTORIAL.md** | Learn the workflow | Why Maestro, Core concepts, Workflow walkthrough, Scenarios | 500 |
| **REFERENCE.md** | Quick lookup | Commands table, Triggers table, Skill reference, Troubleshooting | 300 |
| **AGENTS.md** | Agent behavior | Workflow rules, Decision trees, Session protocol | 200 |
| **docs/ARCHITECTURE.md** | Deep dive diagrams | Mermaid diagrams, Pipeline flow | 300 |

## Key Principles

1. **Audience separation**: Humans get concepts/why, agents get triggers/how
2. **Single source of truth**: No duplicated content across files
3. **Scannable**: Tables, bullet points, minimal prose
4. **Current**: All references verified against v3.1 codebase
5. **Concise**: Strict line limits per file

## Out of Scope

- Skill SKILL.md files (maintain separately)
- Code comments
- Inline documentation in scripts
