# A/P/C Checkpoints

At the end of each phase, present the checkpoint menu:

```
📍 End of [PHASE] phase.

Choose:
[A] Advanced - deeper analysis, assumption audit
[P] Party - multi-perspective feedback from expert agents
[C] Continue - proceed to next phase
[↩ Back] - return to previous phase
```

## [A] Advanced Mode

Phase-specific deep dives:

| Phase | Focus Areas |
|-------|-------------|
| **DISCOVER** | Challenge assumptions, explore biases, consider alternative users |
| **DEFINE** | Stress-test scope, challenge metrics, identify hidden dependencies |
| **DEVELOP** | Deep-dive components, explore alternatives, security/performance review |
| **DELIVER** | Edge case audit, security check, documentation completeness |

## [P] Party Mode

Invokes multi-agent collaborative review using BMAD v6 integration. See [bmad/workflows/party-mode/workflow.md](bmad/workflows/party-mode/workflow.md).

### 25 Agents Available

- **Core (1):** BMad Master (🧙) - Orchestrator
- **BMM (9):** PM, Analyst, Architect, Developer, Scrum Master, Test Architect, UX Designer, Tech Writer, Quick Flow Solo Dev
- **CIS (6):** Brainstorming Coach, Problem Solver, Design Thinking Coach, Innovation Strategist, Presentation Master, Storyteller
- **BMB (3):** Agent Builder, Module Builder, Workflow Builder
- **BMGD (6):** Game Architect, Game Designer, Game Dev, Game QA, Game Scrum Master, Game Solo Dev

### Agent Selection

BMad Master selects 2-3 agents based on topic relevance:
- **Primary:** Best expertise match
- **Secondary:** Complementary perspective
- **Tertiary:** Devil's advocate

### CIS Workflow Triggers

| Trigger | Purpose |
|---------|---------|
| `*brainstorm` | 36 ideation techniques |
| `*design-thinking` | 5-phase human-centered design |
| `*innovate` | Strategic innovation planning |
| `*problem-solve` | Systematic problem resolution |
| `*story` | Narrative design |

Agents respond in English, cross-talk (max 2 rounds), then synthesize insights.

## [C] Continue

Proceed to the next phase. Research verification runs automatically at phase transitions.

## [↩ Back]

Return to previous phase. When looping back:
1. Summarize what was established
2. Ask what to reconsider
3. Update subsequent phases if decisions change
