# Critic Consensus Review Prompt — Design Phase

Used in consensus mode only (`--consensus` flag).

```
Task(
  description: "Strategic review of plan for {topic}",
  name: "critic-reviewer",
  team_name: "design-{topic}",
  subagent_type: "critic",
  model: "opus",
  prompt: |
    Review this plan for strategic coherence, risk coverage, and completeness.

    Plan file: {path to plan file}

    Read the plan, then send your APPROVE/REVISE verdict via SendMessage. Focus on:
    - Are the tasks correctly scoped?
    - Are dependencies accurate?
    - Are there missing edge cases or risks?
    - Is the verification section sufficient?
)
```
