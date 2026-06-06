---
name: maestro-audit
version: 1.0.0
description: "Use for read-only Maestro repo audits that propose harness backlog improvements without implementing them."
---

# Maestro Audit

Use this for repo-wide improvement audits. The audit is agent work; Maestro only
stores, merges, and surfaces proposals.

Activate:
`maestro hook record --event skill_activation --skill maestro-audit`

## Stop

Do not implement, edit code, or change repo artifacts during this skill run.
Produce proposals only.

## Do

1. Read known state: `maestro status`, `maestro harness list --all`, active
   features, active tasks, decisions, and repo instructions.
2. Re-read the repo from scratch, including docs, code ownership boundaries,
   tests, scripts, and shipped embedded resources relevant to the finding.
3. Cross-check findings against Maestro state so you do not propose work already
   accepted, dismissed, measured, or covered by active tasks.
4. Re-propose every finding still seen. Use one stable topic per finding so the
   verb merges repeats:

```sh
maestro harness propose --title "<finding>" --evidence "<file:line evidence and why it matters>" --topic <stable-topic>
```

## Evidence

Each proposal needs concrete evidence: file paths, line numbers, command output,
or exact artifact names. Do not file style opinions without a repo-specific
impact and a way to verify the improvement.

## Hand-off

Pipeline: `[maestro-audit] -> maestro harness apply -> maestro-task`

Next: proposals filed -> inspect with `maestro harness list`; accepted proposals
spawn normal tasks through `maestro harness apply <id>`.
