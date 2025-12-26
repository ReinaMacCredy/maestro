# BMAD V6 Integration - Specification

## Overview

Replace the current 12-agent party mode system with full BMAD v6 integration, bringing 16 expert agents, 6 CIS creative workflows, and enhanced orchestration capabilities to the Maestro design skill.

**Strategy:** Conductor Absorbs BMAD (Hybrid)
- Conductor OWNS the flow (Double Diamond → spec → plan → implement)
- BMAD ENHANCES each stage with 16 expert agents
- Party Mode AVAILABLE everywhere (design, spec, plan, implement stages)
- CIS Workflows ON-DEMAND via `*` triggers

## Functional Requirements

### FR-1: Agent System (16 Agents)

**FR-1.1: Core Module (1 agent)**
- BMad Master (🧙) orchestrator that:
  - Loads manifest.yaml to discover all agents
  - Scores agents by topic/expertise match
  - Selects 2-3 relevant agents per Party Mode session
  - Coordinates cross-talk (1-2 rounds max)
  - Synthesizes insights before returning to A/P/C

**FR-1.2: BMM Module (9 agents)**
| ID | Name | Icon | Expertise |
|----|------|------|-----------|
| pm | John | 📋 | Product strategy, roadmaps, prioritization |
| analyst | Mary | 📊 | Data analysis, metrics, user research |
| architect | Winston | 🏗️ | System design, distributed systems, scalability |
| dev | Amelia | 💻 | Implementation, code quality, DX |
| sm | Sarah | 🏃 | Scrum, agile processes |
| tea | Murat | 🧪 | Testing strategy, quality gates |
| ux-designer | Sally | 🎨 | User experience, accessibility |
| tech-writer | Paige | 📝 | Documentation, API design |
| quick-flow-solo-dev | Barry | ⚡ | Rapid solo development |

**FR-1.3: CIS Module (6 agents)**
| ID | Name | Icon | Expertise |
|----|------|------|-----------|
| brainstorming-coach | Carson | 🧠 | Ideation facilitation, 36 techniques |
| creative-problem-solver | Dr. Quinn | 🔬 | Problem decomposition, root cause |
| design-thinking-coach | Maya | 🎯 | Design methodology, empathy mapping |
| innovation-strategist | Victor | 💡 | Strategic innovation |
| presentation-master | Leo | 🎤 | Presentations, pitch decks |
| storyteller | Sophia | 📖 | Narrative design (with sidecar knowledge) |

**FR-1.4: Agent Format**
```yaml
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
- System design, distributed systems
```

### FR-2: CIS Workflows (6 Workflows)

| Workflow | Trigger | Purpose | Steps |
|----------|---------|---------|-------|
| party-mode | [P] at A/P/C | Multi-agent collaboration | select → respond → crosstalk → synthesize |
| brainstorming | `*brainstorm` | 36 ideation techniques | method-selection → diverge → cluster → converge |
| design-thinking | `*design-thinking` | 5-phase design | empathize → define → ideate → prototype → test |
| innovation-strategy | `*innovate` | Strategic innovation | opportunity → strategy → roadmap |
| problem-solving | `*problem-solve` | Systematic resolution | define → decompose → solve → validate |
| storytelling | `*story` | Narrative frameworks | hero → structure → tell |

**FR-2.1: Workflow Structure**
- Each workflow in `references/bmad/workflows/{name}/`
- Contains: `workflow.md`, `steps/`, optional resources (csv, templates)
- Uses adapter.md for path transformations

**FR-2.2: Brainstorming Knowledge**
- `brain-methods.csv`: 36 ideation techniques
- Loaded by brainstorming-coach agent

**FR-2.3: Storyteller Sidecar**
- `storyteller/sidecar/`: Knowledge base files
- Story structures, narrative frameworks, examples

### FR-3: Party Mode Orchestration

**FR-3.1: Trigger Points**
- [P] at any A/P/C checkpoint during Double Diamond
- Available in ALL Conductor stages: design, spec, plan, implement

**FR-3.2: Selection Algorithm**
1. Analyze context: Extract key themes from discussion
2. Score agents: Match themes against expertise (+3 direct, +1 related, +1 diversity)
3. Select trio: Primary (best match), Secondary (complement), Tertiary (devil's advocate)

**FR-3.3: Response Format**
```
📍 **Party Mode Activated**
Current phase: [PHASE]
Topic: [Topic]
Consulting: [Icon] [Name], [Icon] [Name], [Icon] [Name]

[Icon] **[Name]**: [Response in character, Vietnamese]
...

📍 **Party Mode Synthesis**
Key insights: ...
Points of agreement: ...
Tensions to resolve: ...
```

**FR-3.4: Limits**
- Max 3 agents per session
- Max 2 cross-talk rounds
- Vietnamese communication

### FR-4: Integration Points

**FR-4.1: Double Diamond Preserved**
- DISCOVER → DEFINE → DEVELOP → DELIVER unchanged
- A/P/C checkpoints at each phase end

**FR-4.2: Conductor Flow Preserved**
- ds → design.md → /conductor-newtrack → spec.md + plan.md → fb → implement

**FR-4.3: Beads Integration Preserved**
- Zero manual bd commands
- TDD cycle unchanged

### FR-5: Configuration & Registry

**FR-5.1: manifest.yaml**
- Agent registry with id, name, icon, module, path, expertise
- Used by BMad Master for agent discovery

**FR-5.2: config.yaml**
- Maestro-specific configuration
- Language settings (Vietnamese default)
- Limits (max agents, max rounds)

**FR-5.3: adapter.md**
- Transform rules for v6 workflow paths
- Enables sync with upstream BMAD

## Non-Functional Requirements

### NFR-1: Sync-ability
- Agent format designed for easy manual sync with BMAD upstream
- Version pinned in agent frontmatter (source field)
- Adapter layer isolates v6 structure changes

### NFR-2: Resilience
| Scenario | Solution |
|----------|----------|
| Agent file not found | Skip with warning, use fallback agents |
| Step file missing | Skip step, continue workflow |
| CSV parse error | Fall back to inline techniques |
| Party mode loops | Max 2 rounds, then synthesize |

### NFR-3: Language
- All agent responses in Vietnamese
- UI prompts and synthesis in Vietnamese

### NFR-4: Token Efficiency
- External mode option for token-intensive sessions
- Copy-paste prompt for ChatGPT/Gemini

## Acceptance Criteria

| # | Criterion | Verification |
|---|-----------|--------------|
| AC-1 | 16 agents load correctly | Read each agent file, verify persona structure |
| AC-2 | Party mode selects 2-3 agents | Trigger [P], verify selection logged |
| AC-3 | Brainstorming loads techniques | Check brain-methods.csv accessible |
| AC-4 | Storyteller has knowledge | Verify sidecar/ files readable |
| AC-5 | 6 CIS workflows executable | Trigger each *workflow, verify steps run |
| AC-6 | Double Diamond still works | Run `ds`, verify 4 phases complete |
| AC-7 | A/P/C checkpoints functional | Verify [A], [P], [C] options present |
| AC-8 | Vietnamese communication | Agents respond in Vietnamese |
| AC-9 | Old party-mode archived | Verify party-mode-backup/ exists |
| AC-10 | manifest.yaml valid | Parse YAML, verify 16 agents listed |

## Out of Scope

- BMGD module (game dev agents)
- BMB module (builder agents)
- npx installer integration
- `_bmad/` folder structure
- Menu triggers system
- TTS integration
- agent-manifest.csv (use YAML instead)
