# Critic Consensus Review Prompt — Design Phase

Used in consensus mode only (`--consensus` flag).

```
Task(
  description: "Strategic review of plan for {topic}",
  name: "critic-reviewer",
  team_name: "design-{topic}",
  subagent_type: "critic",
  model: "sonnet",
  prompt: |
    Review this plan for strategic coherence, risk coverage, and completeness.

    The full plan is provided inline below. Do NOT try to read it from a file — the plan-mode file is ephemeral.

    ---
    {full plan content from PLAN READY message}
    ---

    Send your APPROVE/REVISE verdict via SendMessage. Focus on:
    - Are the tasks correctly scoped?
    - Are dependencies accurate?
    - Are there missing edge cases or risks?
    - Is the verification section sufficient?
)
```
