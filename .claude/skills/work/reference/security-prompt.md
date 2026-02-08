# Security Review Prompt — Work Phase

**Trigger**: The plan has a `## Security` section (added by Prometheus during /design).
**Skip**: Plans without a `## Security` section skip this step entirely.

When triggered:

```
Task(
  description: "Security review of implementation",
  name: "sec-reviewer",
  team_name: "work-{plan-slug}",
  subagent_type: "security-reviewer",
  model: "opus",
  prompt: |
    Review the git diff for this execution. Check for:
    {concerns from plan's ## Security section}

    Report findings with severity (Critical/High/Medium/Low) and file:line evidence.

    Also run ecosystem audit if applicable:
    - JS/TS: `bun audit` or `npm audit`
    - Python: `pip-audit` (if available)
    - Go: `govulncheck` (if available)

    Send your report via SendMessage.
)
```

**Processing the report:**

1. Wait for security-reviewer's report
2. **Critical/High findings**: Message the responsible worker(s) to fix. Re-run security review after fixes.
3. **Medium/Low findings**: Log in the wisdom file as security notes. Do not block completion.
4. Proceed to Step 6.7 (Critic Review) or Step 7 (Extract Wisdom)
