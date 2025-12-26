# BMAD V6 Integration - README

Integration of BMAD v6 agent system with Maestro's design skill.

## Overview

This directory contains 16 expert agents and 6 CIS workflows integrated from BMAD v6.

## Directory Structure

```
bmad/
├── config.yaml           # Maestro-specific configuration
├── manifest.yaml         # Agent registry (16 agents)
├── adapter.md            # Path transformation rules for v6 sync
├── agents/
│   ├── core/             # Orchestrator (1 agent)
│   │   └── bmad-master.md
│   ├── bmm/              # Business/Management (9 agents)
│   │   ├── pm.md, analyst.md, architect.md, dev.md
│   │   ├── sm.md, tea.md, ux-designer.md
│   │   ├── tech-writer.md, quick-flow-solo-dev.md
│   └── cis/              # Creative/Ideation (6 agents)
│       ├── brainstorming-coach.md, creative-problem-solver.md
│       ├── design-thinking-coach.md, innovation-strategist.md
│       ├── presentation-master.md
│       └── storyteller/
│           ├── storyteller.md
│           └── sidecar/  # Knowledge base
├── workflows/
│   ├── party-mode/       # Multi-agent collaboration
│   ├── brainstorming/    # 36 ideation techniques
│   ├── design-thinking/  # 5-phase design process
│   ├── innovation-strategy/
│   ├── problem-solving/
│   └── storytelling/
└── teams/
    └── default-party.csv # Pre-configured agent teams
```

## Agent Modules

| Module | Count | Purpose |
|--------|-------|---------|
| Core | 1 | BMad Master orchestrates Party Mode sessions |
| BMM | 9 | Business, Management, Methodology experts |
| CIS | 6 | Creative, Ideation, Storytelling experts |

## Workflow Triggers

| Workflow | Trigger | Purpose |
|----------|---------|---------|
| Party Mode | [P] at A/P/C | Multi-agent collaboration |
| Brainstorming | `*brainstorm` | 36 ideation techniques |
| Design Thinking | `*design-thinking` | 5-phase human-centered design |
| Innovation Strategy | `*innovate` | Strategic innovation planning |
| Problem Solving | `*problem-solve` | Systematic problem resolution |
| Storytelling | `*story` | Narrative design |

## Language

**All agent responses are in English.**

This is configured in `config.yaml`:
```yaml
language:
  default: en
  agent_responses: en
```

## Sync with BMAD Upstream

To sync with the official BMAD v6 repository:

1. Check BMAD releases: https://github.com/bmad-code-org/BMAD-METHOD
2. Review breaking changes in release notes
3. Update version in `config.yaml` and agent `source` fields
4. Use `adapter.md` for path transformations
5. Run verification tests

## Rollback

If integration fails:
1. Delete `skills/design/references/bmad/`
2. Restore from `skills/design/references/party-mode-backup/`
3. Revert SKILL.md changes via git

## Source

Based on BMAD v6.0.0-alpha.21 from https://github.com/bmad-code-org/BMAD-METHOD
