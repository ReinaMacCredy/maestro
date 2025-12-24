---
track_id: double-diamond-party-mode_20251223
created: 2025-12-23T00:00:00Z
status: approved
---

# Double Diamond + Party Mode

## Problem Statement

The current `/conductor:design` command uses a linear brainstorming flow that lacks explicit phase structure, user control over exploration depth, and multi-perspective validation. Users cannot easily loop back to earlier phases, and grounding happens only at the end.

**We are solving** limited design exploration **for** developers using Maestro **because** better designs come from structured diverge/converge cycles and diverse perspectives.

## Success Criteria

- Design sessions follow explicit DISCOVER → DEFINE → DEVELOP → DELIVER phases
- Users control exploration depth via A/P/C menu at checkpoints
- Party Mode provides 2-3 relevant agent perspectives on demand
- Loop-back allows revisiting earlier phases without restarting
- Grounding happens incrementally, not just at the end
- Output remains compatible with `/conductor:newTrack`

## Chosen Approach

**Double Diamond + BMAD v6 Party Mode (Simplified)**

Adopt the Double Diamond methodology with 4 phases, integrate A/P/C checkpoints from BMAD, and implement a simplified Party Mode with 12 agents using the LLM-as-orchestrator pattern (no external orchestration engine).

### Why This Approach

- **Double Diamond**: Proven design methodology with explicit diverge/converge cycles
- **A/P/C Menu**: Gives users control without forcing overhead
- **Simplified Party Mode**: All orchestration via prompt instructions (matches Maestro's no-runtime-code philosophy)
- **v6 Patterns**: Agent selection, cross-talk, synthesis are battle-tested

## Design

### Architecture Overview

```
workflows/
└── party-mode/
    ├── workflow.md          # All-in-one: manifest + rules + orchestration
    ├── manifest.yaml        # Agent registry
    ├── agents/
    │   ├── product/         # PM, Analyst, UX
    │   ├── technical/       # Architect, Developer, QA, Docs
    │   └── creative/        # Storyteller, Brainstorm, Design, Strategist, Solver
    └── custom/
        ├── README.md
        └── _template.md

commands/conductor/design.toml  # Updated with Double Diamond + Party Mode
skills/design/SKILL.md          # References new workflow
skills/conductor/SKILL.md       # Maps DD to Conductor phases
```

### Components

| Component       | Responsibility                                                            |
| --------------- | ------------------------------------------------------------------------- |
| `workflow.md`   | Orchestration rules, agent manifest, selection logic, cross-talk patterns |
| `manifest.yaml` | Agent registry with metadata (name, icon, module, expertise)              |
| Agent files     | Individual persona definitions (frontmatter + markdown)                   |
| `design.toml`   | Double Diamond phases, A/P/C checkpoints, Party Mode protocol             |
| `custom/`       | User-defined agents with template                                         |

### Data Model & Interfaces

**Agent Format (Hybrid MD + YAML):**

```yaml
---
name: Winston
title: Architect
icon: 🏗️
module: technical
role: System Architect + Technical Design Leader
identity: Senior architect with expertise in distributed systems
communication_style: "Calm, pragmatic. Champions boring technology."
principles:
  - User journeys drive technical decisions
  - Design simple solutions that scale
---
# Winston - System Architect
[Extended instructions...]
```

**Agent Manifest (manifest.yaml):**

```yaml
agents:
  - id: pm
    name: John
    icon: 📋
    module: product
    path: agents/product/pm.md
  # ... 12 agents total
```

### User Flow

1. User runs `/conductor:design [description]`
2. Setup check (verify conductor/ exists)
3. **DISCOVER**: Explore problem space → CHECKPOINT 1 [A/P/C]
4. **DEFINE**: Synthesize problem statement → CHECKPOINT 2 [A/P/C]
5. **DEVELOP**: Generate solution options → CHECKPOINT 3 [A/P/C]
6. **DELIVER**: Detail chosen approach → CHECKPOINT 4 [A/P/C]
7. Save design.md → Handoff to `fb` or `/conductor:newTrack`

**Party Mode Flow (when [P] selected):**

1. Load workflow.md + relevant agent files
2. Select 2-3 agents based on topic
3. Each agent responds in character
4. Cross-talk allowed
5. Synthesize insights
6. Return to main flow

### Error Handling

| Situation                 | Behavior                           |
| ------------------------- | ---------------------------------- |
| Setup missing             | Halt, prompt `/conductor:setup`    |
| User exits mid-session    | Offer to save draft design.md      |
| Grounding finds conflicts | Surface issue, let user decide     |
| Party Mode loops          | Limit to 2 rounds, then synthesize |
| track_id collision        | Append timestamp suffix            |

### Testing Strategy

1. **Integration Test**: Run full design → newTrack → implement cycle
2. **Party Mode Test**: Trigger [P] at each checkpoint, verify agent selection
3. **Loop-back Test**: "revisit discover" mid-session
4. **Compatibility Test**: Verify design.md works with existing newTrack

## Grounding Notes

- [x] Codebase patterns checked: workflows/\*.md use Phase/Steps structure
- [x] No existing party-mode (net-new addition)
- [x] Tech stack confirms MD + YAML frontmatter is correct format
- [x] BMAD v6 patterns verified via librarian queries

## Out of Scope

- Text-to-Speech integration
- External GPT/Gem API hooks (future)
- Persistent Party Mode session state
- Game Development agents (BMGD module)
