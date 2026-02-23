# Security Review

**Trigger**: The plan has a `## Security` section (added by Prometheus during /design).
**Skip**: Plans without a `## Security` section skip this step entirely.

---

## When triggered

Spawn a `security-reviewer` worker with this prompt:

```
Review the git diff for this execution. Check for:
{concerns from plan's ## Security section}

Report findings with severity (Critical / High / Medium / Low) and file:line evidence.

Also run ecosystem audit if applicable:
- JS/TS: `bun audit` or `npm audit`
- Python: `pip-audit` (if available)
- Go: `govulncheck` (if available)

Report your findings when done.
```

## Processing the report

1. **Critical / High findings** → send details to the responsible worker(s) to fix. Re-run security review after fixes.
2. **Medium / Low findings** → log in the wisdom file as security notes. Do not block completion.
3. Proceed to critic review (if applicable) or wrap-up.
