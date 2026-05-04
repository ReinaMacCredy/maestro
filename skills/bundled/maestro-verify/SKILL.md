---
name: maestro-verify
description: The canonical verification protocol for any task in a maestro project. Documents witness levels, Trust Verifier scope, ProofMap, plan-check, verdict semantics, cost-budget monitoring, AI Reviewer protocol (Rule 1 veto-only), and threat-model production. Cross-referenced by maestro-task, maestro-plan, and maestro-handoff. Read this skill when starting a non-trivial task or before declaring completion.
---

# Maestro Verify

This skill documents the canonical verification protocol every agent follows before claiming a task is done. Read it when planning a non-trivial task or before declaring completion.

Cross-referenced by `maestro-task` (pre-claim ritual), `maestro-plan` (plan-check), and `maestro-handoff` (handoff gate). When those skills say "see maestro-verify", this is the document.

---

## Witness Levels

Every Evidence row carries a `witness_level` field. The Risk Engine uses this field when evaluating autopilot policy: a high-risk task may require `witnessed-by-maestro`, meaning evidence at a weaker level will not clear the auto-pass threshold and the Verdict downgrades from `PASS` to `HUMAN`.

The ladder from weakest to strongest:

### `agent-claimed-and-not-reproducible`

Manual notes — something happened that cannot be reproduced mechanically. Used exclusively for `--kind manual-note` evidence. Does not satisfy any witness-level gate beyond `low` in default autopilot policies.

```bash
maestro evidence record --task <id> --kind manual-note \
  --note "Verified UI on staging at 1280x800"
```

### `agent-claimed-locally`

The agent self-reported a local run. Maestro did not observe the execution. This is the **default** for newly recorded Evidence rows unless overridden with `--witness`.

```bash
maestro evidence record --task <id> --command "bun test" --exit 0
```

### `witnessed-by-ci`

A trusted CI gate ran the check and posted the result back to Maestro. Treated as authoritative for most `autopilot.yaml` policies at `medium` and `high` risk classes.

### `witnessed-by-maestro`

Maestro itself ran the command and captured the exit code. The most trustworthy level — Maestro controls the execution environment. Satisfies all autopilot policy thresholds, including `high` and `critical` when the policy permits auto-pass.

```bash
maestro evidence record --task <id> --command "bun test" --exit 0
# Maestro shells out to the command; the level is set automatically
```

The default autopilot thresholds (overridable in `policies/autopilot.yaml`):

```yaml
witness_thresholds:
  low:      agent-claimed-locally
  medium:   agent-claimed-locally
  high:     witnessed-by-maestro
  critical: witnessed-by-maestro
```

See `docs/witness-levels.md` for the full ladder, Risk Engine consumption rules, and policy interaction.

---

## Trust Verifier Scope

`maestro task verify` runs 6 deterministic checks against the current diff and the locked contract:

| Check | What it catches |
|---|---|
| `scope` | Changed paths outside `contract.scope.filesExpected` |
| `lockfile` | Lockfile edited when the contract does not permit it |
| `generated` | Generated files hand-edited |
| `sensitive-paths` | Paths matched by `policies/sensitive-paths.yaml` |
| `commit-metadata` | Commit messages not following Conventional Commits |
| `secrets` | Secret-like strings introduced in the diff |

Each finding carries severity `error`, `warn`, or `info`. Address every `error` finding before requesting a Verdict.

```bash
maestro task verify --task <id>
maestro task verify --task <id> --base <git-ref>   # explicit base
maestro task verify --task <id> --json             # machine-readable output
```

Example output:

```
Trust Verifier: 2 findings (1 error, 1 warning, 0 info)
  [error] scope: src/features/auth/secret.ts, src/features/auth/utils.ts
  [warn]  commit-metadata
    Commit "wip" does not match Conventional Commits format
```

Exit codes: `0` = no findings, `1` = at least one error, `2` = warnings or info only.

---

## ProofMap

Every acceptance criterion in the linked Spec needs at least one Evidence row covering it before the Verdict can pass.

```bash
maestro task proof --task <id>
maestro task proof --task <id> --json
```

The verb prints which criteria are covered, by how many rows, and at what witness levels. A criterion with zero rows is `uncovered`. A criterion with rows all below the autopilot threshold is `under-witnessed`.

Example output:

```
ProofMap for task tsk-aaa123:
  [covered]         ac-1  "API returns 200 for valid input"  (2 rows, witnessed-by-maestro)
  [under-witnessed] ac-2  "No secrets in response"  (1 row, agent-claimed-locally)
  [uncovered]       ac-3  "Error path returns 422"
```

Run `maestro task proof` after every significant Evidence record and before requesting a Verdict. Gaps here will cause a `FAIL` or `HUMAN` verdict.

---

## Plan-Check Protocol

At the start of a non-trivial task, write a plan file and run `maestro plan check` before writing any code. The verb records a `plan-check` Evidence row and surfaces three checks.

**Plan file format** (JSON or YAML):

```yaml
intendedFiles:
  - src/features/foo/**
  - tests/unit/features/foo/**
proofSet:
  - criterionId: ac-1
    evidenceKinds: [command]
  - criterionId: ac-2
    evidenceKinds: [command, ai-review]
riskClass: medium
notes: "Optional context"
```

```bash
maestro plan check --task <id> --plan-file ./plan.yaml
maestro plan check --task <id> --plan-file ./plan.yaml --json
```

The three checks:

- **`scope-widens`** — plan touches files not in `contract.scope.filesExpected`. Resolve by narrowing `intendedFiles` or amending the contract before coding.
- **`missing-proof`** — an acceptance criterion from the Spec has no entry in `proofSet`. Every criterion needs a planned proof strategy.
- **`risk-class-too-low`** — `intendedFiles` triggers a higher risk class than `riskClass` declares (Rule 1 plan-time gate). Raise `riskClass` to match.

The verb always exits 0. Agents read the findings and address them before implementing. A clean plan-check does not mean the task will pass — it means the plan is internally consistent.

---

## Verdict Semantics

`maestro verdict request` runs the full decision tree: Trust Verifier + ProofMap + autopilot policy + cost-budget check. It persists the Verdict to disk and exits with a code you branch on.

```bash
maestro verdict request --task <id>
maestro verdict request --task <id> --json
```

Exit codes:

| Code | Decision | Action |
|---|---|---|
| `0` | `PASS` | Claim the task done. All criteria met at required witness level. |
| `1` | `FAIL` | Fix the cited findings, then loop back to `maestro task verify`. |
| `2` | `HUMAN` | Run `maestro handoff create`. This risk class requires human review before the task can complete. |
| `3` | `BLOCK` | Stop. A blocker condition is active — cost budget exhausted, critical risk class with no human signoff, or a pending policy loosening still soaking. Surface the reason to the user; do not retry without guidance. |

Inspect an existing Verdict without re-running:

```bash
maestro verdict show --task <id>
maestro verdict show --task <id> --version <verdictId>
```

Example text output:

```
Decision:   FAIL
Risk:       medium (proposed: low)
ComputedAt: 2026-05-04T10:22:11Z
Task:       tsk-aaa123
ID:         vrd-bbb456
Reasons:
  [coverage] missing-criterion: ac-3 has no Evidence rows
Evidence consulted: 4
Trust verifier: 1 findings (1 errors, 0 warns, 0 infos)
```

---

## Cost-Budget Monitoring

When retries are accumulating, check the current budget consumption before the next loop:

```bash
maestro task budget --task <id>
maestro task budget --task <id> --json
```

Fields reported:

- `retryCount / maxRetries`
- `wallClockElapsedSeconds / maxWallClockSeconds`
- `tokensUsed / maxTokens` (when the contract sets a token cap)
- `exhausted: yes/no` and reason when exhausted

Once any limit is exceeded, the next `verdict request` returns `BLOCK`. At that point, stop and surface the reason to the user.

---

## AI Reviewer Protocol

When an agent runtime has reviewer integration, record findings via `maestro evidence record --kind ai-review`:

```bash
maestro evidence record --task <id> --kind ai-review \
  --reviewer <bug|security|architecture> \
  --findings '<inline-json-or-path>' \
  --confidence <0-1>
```

**Rule 1 (LLM veto-only)** governs how ai-review findings affect risk class:

- Any `error`-severity finding raises `effectiveRiskClass` by one notch.
- A `security`-reviewer `error` always lifts to `critical`.
- A clean ai-review (zero error findings) **never lowers** the deterministic baseline. The Risk Engine derives risk class from diff signals; the LLM can only raise it.

Example invocations:

```bash
# Security review, one error finding
maestro evidence record --task tsk-aaa --kind ai-review \
  --reviewer security \
  --findings '[{"severity":"error","message":"SQL injection risk in buildQuery"}]' \
  --confidence 0.9

# Architecture review, clean pass
maestro evidence record --task tsk-aaa --kind ai-review \
  --reviewer architecture \
  --findings '[]' \
  --confidence 0.8

# Bug review from a file
maestro evidence record --task tsk-aaa --kind ai-review \
  --reviewer bug \
  --findings ./ai-review-findings.json \
  --confidence 0.7
```

Confidence semantics and per-reviewer finding schemas are documented in `docs/ai-reviewer-protocol.md` (ships in L4.DOCS).

---

## Threat-Model Production

When the diff intersects security-relevant paths (anything matched by a `critical`-class signal in `policies/risk.yaml`), produce a threat-model document and record it before requesting a Verdict:

```bash
maestro evidence record --task <id> --kind threat-model \
  --threat-model-file ./threat-model.json
```

The file is JSON or YAML with this schema:

```yaml
assets:
  - session tokens
  - password hashes
threatCategories:
  - spoofing
  - tampering
  - info-disclosure
mitigations:
  - threat: session-fixation
    mitigation: rotate token on login
  - threat: weak-hashing
    mitigation: argon2id with workfactor 3
residualRisk: low          # low | medium | high
# optional:
criterion_id: ac-2
```

**Rule 1 applies here too.** Schema-valid presence clears the `threat-model-required` predicate, but it is not sufficient on its own. Other gates still apply: the autopilot required-witness threshold and the L6 path-touched gate (reviewed when the PR is opened). Substantive correctness — whether the threat model meaningfully covers the change — is reviewed by a human.

See `docs/threat-model-format.md` for the full schema, examples, and Risk Engine semantics.

---

## The Pre-Claim Ritual

Run this loop before marking any task done. Steps are ordered; do not skip.

1. **Plan** — write a plan file and run `maestro plan check --task <id> --plan-file <path>`. Address `scope-widens`, `missing-proof`, and `risk-class-too-low` before writing code.

2. **Implement** — write code. Record Evidence after each verification command:
   ```bash
   maestro evidence record --task <id> --command "bun test" --exit 0
   ```

3. **Verify** — `maestro task verify --task <id>`. Address every `error` finding.

4. **ProofMap** — `maestro task proof --task <id>`. Every criterion must be covered at the required witness level.

5. **Verdict** — `maestro verdict request --task <id>`.

6. **Branch on exit code:**
   - `0` PASS — claim the task done (`maestro task update <id> --status completed --reason "..."`)
   - `1` FAIL — fix the cited findings, loop back to step 3
   - `2` HUMAN — run `maestro handoff create` and stop
   - `3` BLOCK — stop, surface the reason to the user

If retries are accumulating before step 5, run `maestro task budget --task <id>` to check consumption.
