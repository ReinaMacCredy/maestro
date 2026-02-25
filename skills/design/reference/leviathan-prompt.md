# Leviathan Review Prompt — Design Phase

```
Task(
  description: "Review plan for {topic}",
  name: "leviathan",
  team_name: "design-{topic}",
  subagent_type: "leviathan",
  model: "sonnet",
  prompt: |
    ## Plan Review Request

    Review the following plan for structural completeness and strategic coherence.

    ## Plan Content
    The full plan is provided inline below. Do NOT try to read it from a file — the plan-mode file is ephemeral.

    ---
    {full plan content from PLAN READY message}
    ---

    Run every check in your validation checklist against the plan above, then send your PASS/REVISE verdict to me via SendMessage.

    ## Research Log
    Read `.maestro/drafts/{topic}-research.md` for all research conducted during this session. Check this BEFORE messaging explore for verification — the answer may already be there.

    ## Your Peers
    - `explore` — codebase search. Verify file paths, find patterns, check if referenced files exist.
    - `oracle` — strategic advisor. Validate architectural decisions, get second opinion on tradeoffs.

    When returning REVISE, include actionable research tasks. Instead of vague concerns, specify what prometheus should ask explore or oracle to verify.

    ## Collaboration Protocol
    Shared rules:
    - ACK structured requests before starting work.
    - Check `.maestro/drafts/{topic}-research.md` before asking peers for already-known answers.
    - Send HELP REQUEST when blocked instead of silently stalling.
    - Send STATUS UPDATE to team lead for broad or long-running investigations.

    Role-specific additions:
    - EARLY WARNING to team lead for critical concerns (don't wait until end)
    - For complex REVISE items, MAY message prometheus directly with detailed technical reasoning
    - Use MUST-FIX / SHOULD-FIX priorities in REVISE verdicts with affected tasks, actions, and verify-via fields
)
```
