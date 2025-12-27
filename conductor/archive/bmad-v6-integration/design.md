---
track_id: bmad-v6-integration
created: 2025-12-27
status: approved
approved_by:
  - BMad Master (Orchestrator)
  - John (PM)
  - Winston (Architect)
  - Amelia (DEV)
  - Murat (QA)
language: English
---

# BMAD V6 Integration

## Problem Statement

Maestro's design skill only has a simple party mode with 12 agents, lacks creative workflows, and is difficult to sync with BMAD upstream.

**We are solving:** Limited design exploration and party mode capabilities

**For:** Developers using Maestro workflow

**Because:** Full BMAD v6 integration will bring 16+ agents, 6 CIS workflows, and easy syncing of updates from upstream.

## Success Criteria

| # | Criterion | Measurement |
|---|-----------|-------------|
| 1 | 16 agents load correctly | Read each agent file, verify persona |
| 2 | Party mode selects 2-3 agents | Trigger [P], verify selection |
| 3 | Brainstorming loads techniques | Check brain-methods.csv readable |
| 4 | Storyteller has knowledge | Verify sidecar/ accessible |
| 5 | CIS workflows executable | Trigger each *workflow |
| 6 | Double Diamond still works | Run `ds`, verify 4 phases |
| 7 | A/P/C checkpoints functional | Verify [A], [P], [C] options |
| 8 | English communication | Agents respond in English |

## Chosen Approach

**Conductor Absorbs BMAD (Hybrid Strategy)**

- Conductor OWNS the flow
- BMAD ENHANCES each stage
- Party Mode AVAILABLE everywhere
- CIS Workflows ON-DEMAND

### Why This Approach

- Keeps Conductor flow (ds → spec → plan → implement)
- Keeps Beads integration
- Keeps TDD cycle
- Adds 16 agents with deep expertise
- Adds 6 CIS workflows for creative deep dives
- Easy to sync with BMAD upstream

## Design

### Architecture Overview

```
skills/design/
├── SKILL.md                           # Updated with BMAD integration
├── references/
│   ├── bmad/
│   │   ├── config.yaml                # Maestro-specific config
│   │   ├── agents/                    # 🔄 FORK & REWRITE (Native MD)
│   │   │   ├── core/
│   │   │   │   └── bmad-master.md     # 🧙 Orchestrator
│   │   │   ├── bmm/                   # 9 agents
│   │   │   │   ├── analyst.md
│   │   │   │   ├── architect.md
│   │   │   │   ├── dev.md
│   │   │   │   ├── pm.md
│   │   │   │   ├── sm.md
│   │   │   │   ├── tea.md
│   │   │   │   ├── tech-writer.md
│   │   │   │   ├── ux-designer.md
│   │   │   │   └── quick-flow-solo-dev.md
│   │   │   └── cis/                   # 6 agents
│   │   │       ├── brainstorming-coach.md
│   │   │       ├── creative-problem-solver.md
│   │   │       ├── design-thinking-coach.md
│   │   │       ├── innovation-strategist.md
│   │   │       ├── presentation-master.md
│   │   │       └── storyteller/
│   │   │           ├── storyteller.md
│   │   │           └── sidecar/       # Knowledge base
│   │   ├── workflows/                 # 🔌 ADAPTER (Keep v6 structure)
│   │   │   ├── party-mode/
│   │   │   │   ├── workflow.md
│   │   │   │   └── steps/
│   │   │   ├── brainstorming/
│   │   │   │   ├── workflow.md
│   │   │   │   ├── steps/
│   │   │   │   ├── brain-methods.csv  # 62 techniques
│   │   │   │   └── template.md
│   │   │   ├── design-thinking/
│   │   │   ├── innovation-strategy/
│   │   │   ├── problem-solving/
│   │   │   └── storytelling/
│   │   ├── teams/
│   │   │   └── default-party.csv
│   │   ├── manifest.yaml              # Agent registry
│   │   └── adapter.md                 # Transform rules
│   ├── double-diamond.md              # Keep as-is
│   ├── grounding.md
│   └── design-routing-heuristics.md
└── DELETE: party-mode/                # Archive old folder
```

### Hybrid Strategy

| Component | Strategy | Format | Sync Effort |
|-----------|----------|--------|-------------|
| 16 Agents | Fork & Rewrite | Native MD with YAML frontmatter | Manual review |
| brain-methods.csv | Copy | CSV kept as-is | Copy paste |
| storyteller-sidecar/ | Copy | MD files kept as-is | Copy paste |
| 6 Workflows | Adapter | Keep v6 structure + adapter.md | Copy + update adapter |
| manifest.yaml | Native | Maestro YAML format | Regenerate from agents |
| config.yaml | New | Maestro-specific | N/A |

### Agent Roster (16 Total)

| Module | Agent | Icon | Role |
|--------|-------|------|------|
| **Core** | BMad Master | 🧙 | Orchestrator, coordinates party mode |
| **BMM** | PM (John) | 📋 | Product Manager |
| | Analyst (Mary) | 📊 | Business Analyst |
| | Architect (Winston) | 🏗️ | System Architect |
| | DEV (Amelia) | 💻 | Developer |
| | SM (Bob) | 🏃 | Scrum Master |
| | TEA (Murat) | 🧪 | Test Engineer |
| | UX Designer (Sally) | 🎨 | UX/UI Designer |
| | Tech Writer (Paige) | 📝 | Documentation |
| | Quick Flow Solo Dev (Barry) | ⚡ | Rapid solo development |
| **CIS** | Brainstorming Coach (Carson) | 🧠 | Ideation facilitation |
| | Creative Problem Solver (Dr. Quinn) | 🔬 | Problem decomposition |
| | Design Thinking Coach (Maya) | 🎯 | Design methodology |
| | Innovation Strategist (Victor) | 💡 | Strategic innovation |
| | Presentation Master (Caravaggio) | 🎨 | Visual Communication |
| | Storyteller (Sophia) | 📖 | Narrative design |

### Agent Format (Native MD)

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

**Role:** System Architect + Technical Design Leader

**Identity:** 
Senior architect with expertise in distributed systems...

**Communication Style:** 
Speaks in calm, pragmatic tones...

## Principles

- User journeys drive technical decisions
- Embrace boring technology for stability

## Expertise

- System design
- Distributed systems
- Scalability
```

### CIS Workflows (6)

| Workflow | Trigger | Purpose |
|----------|---------|---------|
| party-mode | [P] at A/P/C | Multi-agent collaboration |
| brainstorming | *brainstorm | 36 ideation techniques |
| design-thinking | *design-thinking | 5-phase design process |
| innovation-strategy | *innovate | Strategic innovation |
| problem-solving | *problem-solve | Systematic problem resolution |
| storytelling | *story | Narrative frameworks |

### Integration with Conductor Flow

```
┌─────────────────────────────────────────────────────────────┐
│                    UNIFIED FLOW                             │
├─────────────────────────────────────────────────────────────┤
│ PHASE 1: DESIGN (ds)                                        │
│ ─────────────────                                           │
│ Double Diamond + Party Mode (16 agents)                     │
│ CIS: *brainstorm, *design-thinking available                │
│ Output: design.md                                           │
│ ≈ BMAD Analysis + Early Planning                            │
├─────────────────────────────────────────────────────────────┤
│ PHASE 2: SPECIFICATION (/conductor-newtrack)                │
│ ─────────────────────────────────────                       │
│ Generate: spec.md                                           │
│ Party Mode: PM, Analyst, Architect review                   │
│ ≈ BMAD Planning                                             │
├─────────────────────────────────────────────────────────────┤
│ PHASE 3: PLANNING (plan.md + fb)                            │
│ ───────────────────────────                                 │
│ Generate: plan.md, File Beads                               │
│ Party Mode: Architect, DEV review                           │
│ ≈ BMAD Solutioning                                          │
├─────────────────────────────────────────────────────────────┤
│ PHASE 4: IMPLEMENTATION (/conductor-implement)              │
│ ─────────────────────────────────────────                   │
│ TDD Cycle: RED → GREEN → REFACTOR                           │
│ Party Mode: DEV, TEA, QA assist                             │
│ ≈ BMAD Implementation                                       │
└─────────────────────────────────────────────────────────────┘
```

### BMad Master Orchestration

```
[P] selected at A/P/C checkpoint
    ↓
BMad Master activated
    ↓
Load manifest.yaml (16 agents)
    ↓
Score agents by topic/expertise
    ↓
Select 2-3 relevant agents
    ↓
Agent responses (in character, English)
    ↓
Cross-talk (1-2 rounds max)
    ↓
User asks for deep dive?
    ├─ Yes → Trigger CIS workflow
    └─ No → Synthesize insights
    ↓
Return to A/P/C menu
```

### Error Handling

| Scenario | Solution |
|----------|----------|
| Agent file not found | Skip with warning, use fallback agents |
| Step file missing | Skip step, continue workflow |
| CSV parse error | Fall back to inline techniques |
| Party mode loops | Max 2 rounds, then synthesize |
| User exits mid-workflow | Save draft, offer resume |

### Estimates

| Task | Effort |
|------|--------|
| Agent files (16) | 3-4 hours |
| Workflows (6) | 4-6 hours |
| Integration | 2-3 hours |
| Testing | 2-3 hours |
| **Total** | **12-16 hours** |

### Test Plan (32 Cases)

| Category | Count | Tests |
|----------|-------|-------|
| Unit: Agent loading | 16 | Each agent loads correctly |
| Integration: Party Mode | 5 | Selection scenarios |
| Integration: CIS workflows | 6 | Each workflow triggers |
| E2E: Full cycle | 1 | ds → design.md |
| Regression: Double Diamond | 4 | 4 phases work |

## Scope

### In Scope ✅

- 16 agents (Core: 1, BMM: 9, CIS: 6)
- 6 CIS workflows
- Party Mode with BMad Master orchestration
- brain-methods.csv (62 techniques)
- storyteller-sidecar knowledge
- Integration at ALL Conductor stages
- Adapter layer for v6 compatibility
- Archive old party-mode folder

### Out of Scope ❌

- BMGD module (game dev)
- BMB module (builder)
- npx installer integration
- `_bmad/` folder structure
- Menu triggers system
- TTS integration
- agent-manifest.csv (use YAML instead)

## Grounding Notes

- [x] BMAD v6 repo structure verified
- [x] 16 agents identified with full personas
- [x] 6 CIS workflows documented
- [x] Integration points with Conductor verified
- [x] No conflicts with existing Beads/TDD flow
- [x] Sync workflow documented

## Risks & Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| v6 breaking changes | Medium | High | Pin version, review before sync |
| Workflow step loading fails | Low | High | Fallback: inline steps |
| Agent personality drift | Low | Medium | Regular sync reviews |
| Large implementation effort | Medium | Medium | Phased rollout |

## Rollback Plan

1. Archive old party-mode/ to party-mode-backup/
2. If integration fails, restore from backup
3. Git history preserves all changes

## Next Steps

1. Run `/conductor-newtrack` to generate spec.md + plan.md
2. File beads (`fb`) to create epic + issues
3. Implement in phases:
   - Phase 1: Core + BMM agents
   - Phase 2: CIS agents + sidecar
   - Phase 3: Workflows + adapter
   - Phase 4: Integration + testing
