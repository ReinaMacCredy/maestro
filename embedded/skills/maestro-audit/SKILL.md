---
name: maestro-audit
version: 1.4.0
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
   Sweep every lens so coverage is checkable, not just whatever surfaced
   first: correctness, security, performance, test coverage, tech debt,
   dependencies, developer experience, docs. The tech-debt lens includes the
   reach-ladder (HARNESS Code style): code a lower rung -- stdlib, native
   platform, an installed dependency, a one-liner -- already covers. The
   session lean mode tunes how strictly to propose these (`maestro lean`):
   `ultra` proposes replacing such code, `full`/`lite` propose the cheaper
   form, `off` skips the reach-ladder lens. `maestro lean audit` runs the
   focused, mode-adjusted reach-ladder pass; this skill still only proposes
   (no edits, no markers).
3. Vet each finding before filing: try to refute it against the live repo
   (re-read the code, re-run the command). Drop findings that do not survive.
4. Cross-check findings against Maestro state so you do not propose work already
   accepted, dismissed, measured, or covered by active tasks.
5. Re-propose every finding still seen with `maestro harness propose`
   (signatures: [reference/cli.md](reference/cli.md)). Use one stable
   `--topic` per finding so the verb merges repeats, and end the `--evidence`
   text with a leverage estimate:
   `impact/effort/confidence: <H|M|L>/<H|M|L>/<H|M|L>`.

## Evidence

Each proposal needs concrete evidence: file paths, line numbers, command output,
or exact artifact names, plus the closing `impact/effort/confidence` estimate
(`H`, `M`, or `L` each) so the backlog ranks without re-deriving it. Do not
file style opinions without a repo-specific impact and a way to verify the
improvement.

## Hand-off

Pipeline: `[maestro-audit] -> maestro harness apply -> maestro-card`

Next: proposals filed -> inspect with `maestro harness list`; accepted proposals
spawn normal tasks through `maestro harness apply <id>`.
