---
name: maestro-intake
description: Use BEFORE writing code in any project that uses maestro. Run `maestro intake --paths <paths>` to classify the intended change into a lane (tiny / normal / high-risk) and get a recommended next step. Auto-invokes when the user prompts a change and you have not yet run an intake for it.
---

# Maestro Intake

Plan-time risk classifier. Runs before code is written; returns a lane and a recommended next step. Reuses the same Risk Engine that `maestro verdict request` runs after the diff exists, so the pre-flight class matches the post-diff class for the paths you name.

---

## When to activate

Auto-activate when:

1. The user asks for a non-trivial implementation and no `maestro intake` has been run for it yet.
2. You are about to claim or create a `maestro task` for a multi-step change.
3. You are about to call `/maestro-plan` or convert a plan into a task batch.

Do not activate for:

- One-line typo fixes the user explicitly scoped to a single file.
- Read-only questions or explanations.

## Hard rules

1. **Run intake before `task plan` / `task create`.** The lane it returns is the input to which task ceremony you use.
2. **Pass real intended paths.** The output is path-derived; vague summaries without paths produce vague lanes.
3. **Treat hard gates as load-bearing.** If `hardGatesTriggered` is non-empty, do not silently lower the lane. Either accept the high-risk ceremony or stop and ask the user to narrow scope.

## Verb

```bash
maestro intake --paths <comma-list> [--flag <flag> ...] [--summary "<text>"] [--json]
```

`--paths` (required): the files you intend to change. Use forward slashes, paths relative to the repo root.
`--flag` (repeatable): declare a risk flag explicitly. Auto-detection is conservative; declared flags always count.
`--summary` (optional): one-liner; not consumed by the classifier today, but persisted for future use.

Exit code is always `0`. React to the lane in the output, not the exit code.

## Lanes and what they mean

| Lane | When | Next step |
|---|---|---|
| `tiny` | 0–1 flags, no hard gate | Patch directly; run validation; close with reason. |
| `normal` | 2–3 flags, no hard gate | `maestro task plan --file -` then `maestro plan check`. |
| `high-risk` | any hard gate, OR 4+ flags | High-risk task with Spec acceptance criteria and `threat-model` evidence. |

## Risk flags

| Flag | Auto-detected when path matches | Hard gate? |
|---|---|---|
| `auth` | `**/auth/**`, `**/session/**`, `**/jwt*`, `**/login*`, `**/logout*` | yes |
| `authz` | (declare) | yes |
| `data-model` | `**/migrations/**`, `**/db/migrations/**`, `**/schema/**` | yes |
| `audit-security` | matches `policies/sensitive-paths.yaml` | yes |
| `external-systems` | `package.json`, lockfiles, `Cargo.toml`, `pyproject.toml`, etc. | yes |
| `public-contracts` | (declare) | no |
| `cross-platform` | (declare) | no |
| `existing-behavior` | (declare) | no |
| `weak-proof` | (declare) | no |
| `multi-domain` | (declare) | no |

Auto-detection is intentionally narrow: declared flags are the primary input. When in doubt, declare the flag.

## JSON shape

```json
{
  "lane": "normal",
  "derivedRiskClass": "medium",
  "derivedRiskSignal": "diff-source-only",
  "autoDetectedFlags": [],
  "declaredFlags": ["existing-behavior", "weak-proof"],
  "hardGatesTriggered": [],
  "recommendedNextStep": "create a task via `maestro task plan` and run `maestro plan check`"
}
```

## Examples

Tiny lane (single docs path, no flags):
```bash
maestro intake --paths README.md --json
# lane: tiny  -> patch directly
```

High-risk via auto-detected hard gate (auth path):
```bash
maestro intake --paths src/auth/session.ts --json
# autoDetectedFlags: ["auth"]
# hardGatesTriggered: ["auth"]
# lane: high-risk
```

Normal lane via two declared flags:
```bash
maestro intake --paths src/foo.ts \
  --flag existing-behavior --flag weak-proof --json
# lane: normal -> maestro task plan + maestro plan check
```

## How this fits with the rest of maestro

- Before code: `maestro intake` (this skill) sets the lane.
- During plan-conversion: `maestro task plan --file -` creates the task batch (`maestro-task` skill).
- Before claiming complete: `maestro plan check` and `maestro task verify` (`maestro-verify` skill).
- Verdict pipeline reuses the same Risk Engine the intake calls, so the derived class is consistent before and after the diff exists.
