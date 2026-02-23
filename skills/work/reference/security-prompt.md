# Security Review Prompt — Work Phase

**Trigger**: The plan has a `## Security` section (added by Prometheus during /design).
**Skip**: Plans without a `## Security` section skip this step entirely.

When triggered:

```
agent.spawn(
  role: "security-reviewer",
  model: "sonnet",
  prompt: |
    Review the git diff for this execution. Check for:
    {concerns from plan's ## Security section}

    Report findings with severity (Critical/High/Medium/Low) and file:line evidence.

    Also run ecosystem audit if applicable:
    - JS/TS: exec.command("bun audit") or exec.command("npm audit")
    - Python: exec.command("pip-audit") (if available)
    - Go: exec.command("govulncheck") (if available)

    Send your report via agent.message to the orchestrator.
)
```

**Processing the report:**

1. Wait for security-reviewer's report
2. **Critical/High findings**: `agent.message` the responsible worker(s) to fix. Re-run security review after fixes.
3. **Medium/Low findings**: Log in the wisdom file as security notes. Do not block completion.
4. Proceed to Step 6e (Critic Review) or Step 7 (Extract Wisdom)
