---
name: review-gate
description: Route a git range through the WORKFLOW.md review gate - classify, router, lens reviewers, join, one refuter per finding, synthesis.
input:
  range: {required: true, description: "git range under review, for example main..HEAD or abc123^..abc123"}
  tier: {default: light, description: "light (simplify lens, after green before commit) or full (correctness lens, frozen diff after verify)"}
limits: {nodes: 60, loops: 1, fanout: 20}
verdict: verdict
nodes:
  diffstat: {kind: function, command: "git diff --stat {range}"}
  classify:
    kind: agent
    profile: classifier
    schema:
      type: object
      required: [files, subsystems, touchesTrustBoundary, touchesSchemaOrMigration, touchesAuthSecretsOrInput, summary]
      properties:
        files: {type: array, items: {type: string}}
        subsystems: {type: array, items: {type: string}}
        touchesTrustBoundary: {type: boolean}
        touchesSchemaOrMigration: {type: boolean}
        touchesAuthSecretsOrInput: {type: boolean}
        summary: {type: string}
  route: {kind: router}
  review-simplify:
    kind: agent
    profile: reviewer-simplify
    schema: &findings
      type: object
      required: [findings]
      properties:
        findings:
          type: array
          items:
            type: object
            required: [file, line, summary, evidence]
            properties:
              file: {type: string}
              line: {type: integer}
              summary: {type: string}
              evidence: {type: string}
  review-correctness: {kind: agent, profile: reviewer-correctness, schema: *findings}
  review-regression: {kind: agent, profile: reviewer-regression, schema: *findings}
  review-contracts: {kind: agent, profile: reviewer-contracts, schema: *findings}
  review-security: {kind: agent, profile: reviewer-security, schema: *findings}
  findings: {kind: join, collect: findings, key: [file, line], window: 0}
  each-finding: {kind: foreach, over: findings.items}
  refute:
    kind: agent
    profile: refuter
    schema:
      type: object
      required: [refuted, reason]
      properties:
        refuted: {type: boolean}
        reason: {type: string}
  verdicts: {kind: join}
  verdict:
    kind: agent
    profile: synthesizer
    schema:
      type: object
      required: [verdict, confirmed, refuted, summary]
      properties:
        verdict: {type: string, enum: [pass, fail]}
        confirmed:
          type: array
          items:
            type: object
            required: [file, line, summary]
            properties:
              file: {type: string}
              line: {type: integer}
              summary: {type: string}
              lens: {type: string}
        refuted:
          type: array
          items:
            type: object
            required: [file, line, summary, reason]
            properties:
              file: {type: string}
              line: {type: integer}
              summary: {type: string}
              reason: {type: string}
        summary: {type: string}
edges:
  - {from: diffstat, to: classify}
  - {from: classify, to: route}
  - {from: route, to: review-simplify, when: {path: tier, eq: light}}
  - {from: route, to: review-correctness, when: {path: tier, eq: full}}
  - {from: route, to: review-regression, when: {any: [classify.touchesTrustBoundary, classify.touchesSchemaOrMigration, {path: classify.subsystems.length, gt: 1}]}}
  - {from: route, to: review-contracts, when: {any: [classify.touchesTrustBoundary, classify.touchesSchemaOrMigration, {path: classify.subsystems.length, gt: 1}]}}
  - {from: route, to: review-security, when: classify.touchesAuthSecretsOrInput}
  - {from: review-simplify, to: findings}
  - {from: review-correctness, to: findings}
  - {from: review-regression, to: findings}
  - {from: review-contracts, to: findings}
  - {from: review-security, to: findings}
  - {from: findings, to: each-finding}
  - {from: each-finding, to: refute}
  - {from: refute, to: verdicts}
  - {from: verdicts, to: verdict}
---

## classify

Repository: the current working directory. Diff under review: run `git diff {range}`
(and `git diff --stat {range}`, shown below). Read surrounding code when the diff
alone is not enough. Do not edit any file.

```text
{diffstat}
```

Report routing facts for this diff. files: every changed path. subsystems: the
distinct top-level packages, services, or modules touched (one entry per
subsystem, not per file). touchesTrustBoundary: true if the diff reads user
input, external APIs, untrusted data, or IPC. touchesSchemaOrMigration: true if
it changes a persisted schema, a migration, or a stored data format.
touchesAuthSecretsOrInput: true if it changes authentication, authorization,
secret handling, or parsing of external input. summary: one sentence of what the
diff does. Answer with one JSON object matching the schema and nothing else.

## review-simplify

Repository: the current working directory. Diff under review: run `git diff {range}`.
Read surrounding code when the diff alone is not enough. Do not edit any file.

Simplification pass (tier light, after green, before commit). Report only
cleanups the diff itself introduced: duplicated logic that an existing helper
covers, dead branches, speculative options with one caller, over-abstraction.
Not bugs. Each finding names the file and line and the smaller form. Answer with
one JSON object {"findings": [...]} matching the schema and nothing else; an
empty list is a valid answer.

## review-correctness

Repository: the current working directory. Diff under review: run `git diff {range}`.
Read surrounding code when the diff alone is not enough. Do not edit any file.

Correctness review (tier full, frozen diff after verify). Report behavioral
bugs only: wrong output, crash, unhandled state, broken invariant. Each finding
gives concrete inputs or state that produce the wrong result. No style, no
cleanups. Answer with one JSON object {"findings": [...]} matching the schema
and nothing else; an empty list is a valid answer.

## review-regression

Repository: the current working directory. Diff under review: run `git diff {range}`.
Read surrounding code when the diff alone is not enough. Do not edit any file.

Regression lens for a diff that spans a trust boundary, a schema or migration,
or several subsystems ({classify.summary}). Report callers, data, or flows
outside the changed lines that the change breaks or silently changes. Each
finding names the caller or data path and how it observes the change. Answer
with one JSON object {"findings": [...]} matching the schema and nothing else;
an empty list is a valid answer.

## review-contracts

Repository: the current working directory. Diff under review: run `git diff {range}`.
Read surrounding code when the diff alone is not enough. Do not edit any file.

Contract and test lens for a diff that spans a trust boundary, a schema or
migration, or several subsystems ({classify.summary}). Report public contracts
the diff changes without a matching test or doc update, and tests that no
longer falsify the behavior they name. Each finding names the contract and the
missing check. Answer with one JSON object {"findings": [...]} matching the
schema and nothing else; an empty list is a valid answer.

## review-security

Repository: the current working directory. Diff under review: run `git diff {range}`.
Read surrounding code when the diff alone is not enough. Do not edit any file.

Security review for a diff that touches auth, secrets, or input handling.
Report injection, missing validation at a trust boundary, secret exposure,
privilege or auth bypass, unsafe path or command construction. Each finding
names the sink and the untrusted source that reaches it. Answer with one JSON
object {"findings": [...]} matching the schema and nothing else; an empty list
is a valid answer.

## refute

Repository: the current working directory. Diff under review: run `git diff {range}`.
Read surrounding code when the diff alone is not enough. Do not edit any file.

Try to refute this review finding. It came from the {item.producer} lens.
File: {item.file} line {item.line}
Claim: {item.summary}
Evidence offered: {item.evidence}
Check the actual code and diff. refuted=true if the claim is wrong, already
handled, outside the diff, or you cannot confirm it; default to refuted=true
when uncertain. Answer with one JSON object {"refuted": ..., "reason": "..."}
and nothing else.

## verdict

Repository: the current working directory. Diff under review: `git diff {range}`
at tier {tier}. Do not edit any file.

Classification:

```json
{classify}
```

Deduplicated findings (each carries its producer lens; provenance lists the
other lenses that raised the same file and line):

```json
{findings}
```

Refuter verdicts, one per finding (instance = the finding's index above):

```json
{verdicts}
```

Compose the gate verdict. confirmed: every finding whose refuter answered
refuted=false, with its lens. refuted: the rest with the refuter's reason.
verdict: "fail" when any confirmed finding is a behavioral bug or a security
issue, otherwise "pass". summary: the verdict and what decided it, briefly.
Answer with one JSON object matching the schema and nothing else.
