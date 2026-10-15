---
name: review-gate
description: Route a git range through the WORKFLOW.md review gate - classify, router, lens reviewers judged against the stated intent (a Claude and Codex correctness pair and a structure lens at tier full), join, one refuter per finding, a synthesizer that sorts Act on, Consider, Noted and Dismissed.
input:
  range: {required: true, description: "git range under review, for example main..HEAD or abc123^..abc123"}
  intent: {default: "none stated; infer it from the commit messages in the range and say so", description: "what the change is for, in one paragraph taken from the work item's acceptance or the dispatch brief; reviewers judge the diff against it and never question it"}
  tier: {default: light, description: "light (simplify lens, after green before commit) or full (Claude and Codex correctness pair plus the structure lens, frozen diff after verify)"}
  justified: {default: none, description: "the author's written justification for any presumptive blocker, one per line as file:line and the reason, quoted from the work note or commit that records it"}
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
        findings: &findinglist
          type: array
          items:
            type: object
            required: [file, line, severity, summary, evidence]
            properties:
              file: {type: string}
              line: {type: integer}
              severity: {type: string, enum: [critical, warning, nit]}
              summary: {type: string}
              evidence: {type: string}
  review-correctness: {kind: agent, profile: reviewer-correctness, schema: *findings}
  review-correctness-codex: {kind: agent, profile: reviewer-correctness-codex, schema: *findings}
  review-structure:
    kind: agent
    profile: reviewer-structure
    schema:
      type: object
      required: [findings]
      properties:
        findings:
          type: array
          items:
            type: object
            required: [file, line, severity, summary, evidence, beyondDiff]
            properties:
              file: {type: string}
              line: {type: integer}
              severity: {type: string, enum: [critical, warning, nit]}
              summary: {type: string}
              evidence: {type: string}
              beyondDiff: {type: boolean}
  review-regression:
    kind: agent
    profile: reviewer-regression
    schema:
      type: object
      required: [findings, safetyFact, rung, proof, cleared]
      properties:
        findings: *findinglist
        safetyFact: {type: string}
        rung: {type: integer, enum: [1, 2, 3, 4, 5]}
        proof: {type: string}
        cleared: {type: array, items: {type: string}}
  review-contracts: {kind: agent, profile: reviewer-contracts, schema: *findings}
  review-security: {kind: agent, profile: reviewer-security, schema: *findings}
  findings: {kind: join, collect: findings, key: [file, line], window: 0}
  each-finding: {kind: foreach, over: findings.items}
  refute:
    kind: agent
    profile: refuter
    schema:
      type: object
      required: [outcome, reason]
      properties:
        outcome: {type: string, enum: [confirmed, refuted, unconfirmed]}
        reason: {type: string}
  verdicts: {kind: join}
  verdict:
    kind: agent
    profile: synthesizer
    schema:
      type: object
      required: [verdict, actOn, consider, noted, dismissed, agreement, summary]
      properties:
        verdict: {type: string, enum: [pass, fail]}
        actOn:
          type: array
          items:
            type: object
            required: [file, line, severity, summary, lenses, reason]
            properties:
              file: {type: string}
              line: {type: integer}
              severity: {type: string, enum: [critical, warning, nit]}
              summary: {type: string}
              lenses: {type: array, items: {type: string}}
              reason: {type: string}
              justification: {type: string}
        consider: &bucket
          type: array
          items:
            type: object
            required: [file, line, severity, summary, lenses, reason]
            properties:
              file: {type: string}
              line: {type: integer}
              severity: {type: string, enum: [critical, warning, nit]}
              summary: {type: string}
              lenses: {type: array, items: {type: string}}
              reason: {type: string}
        noted: *bucket
        dismissed: *bucket
        agreement: {type: string}
        safety: {type: string}
        summary: {type: string}
edges:
  - {from: diffstat, to: classify}
  - {from: classify, to: route}
  - {from: route, to: review-simplify, when: {path: tier, eq: light}}
  - {from: route, to: review-correctness, when: {path: tier, eq: full}}
  - {from: route, to: review-correctness-codex, when: {path: tier, eq: full}}
  - {from: route, to: review-structure, when: {path: tier, eq: full}}
  - {from: route, to: review-regression, when: {any: [classify.touchesTrustBoundary, classify.touchesSchemaOrMigration, {path: classify.subsystems.length, gt: 1}]}}
  - {from: route, to: review-contracts, when: {any: [classify.touchesTrustBoundary, classify.touchesSchemaOrMigration, {path: classify.subsystems.length, gt: 1}]}}
  - {from: route, to: review-security, when: classify.touchesAuthSecretsOrInput}
  - {from: review-simplify, to: findings}
  - {from: review-correctness, to: findings}
  - {from: review-correctness-codex, to: findings}
  - {from: review-structure, to: findings}
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

The intent below comes from the work item's acceptance or the brief. Judge
whether the diff achieves it well and never question the intent itself.

```text
{intent}
```

Simplification pass (tier light, after green, before commit). Report only
cleanups the diff itself introduced: duplicated logic that an existing helper
covers, dead branches, speculative options with one caller, over-abstraction.
Code judo beyond the diff belongs to the structure lens. Not bugs. Each finding
names the file and line and the smaller form. Give every finding a severity:
critical when it would cause a bug, data loss, a security hole or broken
behavior; warning for a design or maintainability risk that will cause pain;
nit for style, naming or a minor improvement. Answer with one JSON object
{"findings": [...]} matching the schema and nothing else; an empty list is a
valid answer.

## review-correctness

Repository: the current working directory. Diff under review: run `git diff {range}`.
Read surrounding code when the diff alone is not enough. Do not edit any file.

The intent below comes from the work item's acceptance or the brief. Judge
whether the diff achieves it well and never question the intent itself.

```text
{intent}
```

Correctness review (tier full, frozen diff after verify). Report behavioral
bugs only: wrong output, crash, unhandled state, broken invariant, an
operation that is not safe to run twice, a race on shared state. Each finding
gives concrete inputs or state that produce the wrong result. No style, no
cleanups. Give every finding a severity: critical when it would cause a bug,
data loss, a security hole or broken behavior; warning for a correctness risk
that is not broken yet but will cause pain; nit for a minor improvement.
Answer with one JSON object {"findings": [...]} matching the schema and nothing
else; an empty list is a valid answer.

## review-correctness-codex

Repository: the current working directory. Diff under review: run `git diff {range}`.
Read surrounding code when the diff alone is not enough. Do not edit any file.

The intent below comes from the work item's acceptance or the brief. Judge
whether the diff achieves it well and never question the intent itself.

```text
{intent}
```

Correctness review (tier full, frozen diff after verify). Report behavioral
bugs only: wrong output, crash, unhandled state, broken invariant, an
operation that is not safe to run twice, a race on shared state. Each finding
gives concrete inputs or state that produce the wrong result. No style, no
cleanups. Give every finding a severity: critical when it would cause a bug,
data loss, a security hole or broken behavior; warning for a correctness risk
that is not broken yet but will cause pain; nit for a minor improvement.
Answer with one JSON object {"findings": [...]} matching the schema and nothing
else; an empty list is a valid answer.

## review-structure

Repository: the current working directory. Diff under review: run `git diff {range}`.
Read surrounding code when the diff alone is not enough. Do not edit any file.

The intent below comes from the work item's acceptance or the brief. Judge
whether the diff achieves it well and never question the intent itself.

```text
{intent}
```

Structure lens (tier full, the structural approval bar). Hold the diff to the
presumptive blockers in your role and look for the code-judo move. For the
1000-line rule, compare each changed file's line count before and after the
range. Each finding names the file and line and the smaller structure. Set
beyondDiff to true when the proposal restructures code the diff did not touch;
such a proposal goes to the owner and this gate never applies it. Give every
finding a severity: critical when it would cause a bug, data loss, a security
hole or broken behavior; warning for a design or maintainability risk that will
cause pain; nit for style, naming or a minor improvement. Answer with one JSON
object {"findings": [...]} matching the schema and nothing else; an empty list
is a valid answer.

## review-regression

Repository: the current working directory. Diff under review: run `git diff {range}`
and `git log {range}` for the commits behind it. Read surrounding code when the
diff alone is not enough. Do not edit any file.

The intent below comes from the work item's acceptance or the brief. Judge
whether the diff achieves it well and never question the intent itself.

```text
{intent}
```

Regression lens for a diff that spans a trust boundary, a schema or migration,
or several subsystems ({classify.summary}). Report callers, data, or flows
outside the changed lines that the change breaks or silently changes. Each
finding names the caller or data path, how it observes the change, how likely
and how costly that is, and the cheapest check that catches it. Put the one
fact the change is safe because of in safetyFact, how far down your proof
ladder it got (1 to 5) in rung, the command you ran and what it printed (or
unproven) in proof, and each risk you checked and cleared, with its file:line,
in cleared. Give every finding a severity: critical when it would cause a bug,
data loss, a security hole or broken behavior; warning for a risk that is not
broken yet but will cause pain; nit for a minor improvement. Answer with one
JSON object {"findings": [...], "safetyFact": "...", "rung": ..., "proof": "...",
"cleared": [...]} matching the schema and nothing else; an empty findings list
is a valid answer.

## review-contracts

Repository: the current working directory. Diff under review: run `git diff {range}`.
Read surrounding code when the diff alone is not enough. Do not edit any file.

The intent below comes from the work item's acceptance or the brief. Judge
whether the diff achieves it well and never question the intent itself.

```text
{intent}
```

Contract and test lens for a diff that spans a trust boundary, a schema or
migration, or several subsystems ({classify.summary}). Report public contracts
the diff changes without a matching test or doc update, tests that no longer
falsify the behavior they name, and checks that trust a proxy or a self-report
instead of the real thing. Each finding names the contract and the missing
check. Give every finding a severity: critical when it would cause a bug, data
loss, a security hole or broken behavior; warning for a verification gap that
will cause pain; nit for a minor improvement. Answer with one JSON object
{"findings": [...]} matching the schema and nothing else; an empty list is a
valid answer.

## review-security

Repository: the current working directory. Diff under review: run `git diff {range}`.
Read surrounding code when the diff alone is not enough. Do not edit any file.

The intent below comes from the work item's acceptance or the brief. Judge
whether the diff achieves it well and never question the intent itself.

```text
{intent}
```

Security review for a diff that touches auth, secrets, or input handling.
Report injection, missing validation at a trust boundary, secret exposure,
privilege or auth bypass, unsafe path or command construction. Each finding
names the sink and the untrusted source that reaches it. Give every finding a
severity: critical when it would cause a bug, data loss, a security hole or
broken behavior; warning for a risk that is not exploitable yet but will cause
pain; nit for a minor hardening. Answer with one JSON object {"findings": [...]}
matching the schema and nothing else; an empty list is a valid answer.

## refute

Repository: the current working directory. Diff under review: run `git diff {range}`.
Read surrounding code when the diff alone is not enough. Do not edit any file.

The change is meant to do this:

```text
{intent}
```

Try to refute this review finding.
It came from the {item.producer} lens with severity {item.severity}.
File: {item.file} line {item.line}
Claim: {item.summary}
Evidence offered: {item.evidence}
Check the actual code and diff. The outcome is refuted when the claim is wrong,
already handled, a hypothetical no real caller or input reaches, or not caused
by this change. A review-structure finding may reach beyond the diff by design
(d870), so that alone never refutes it. The outcome is confirmed only when you
can point at the line and the input or state that makes it real. The outcome
is unconfirmed when you can do neither; never round it to refuted or
confirmed. Answer with one JSON object {"outcome": "...", "reason": "..."} and
nothing else.

## verdict

Repository: the current working directory. Diff under review: `git diff {range}`
at tier {tier}. Do not edit any file.

Intent:

```text
{intent}
```

The author's written justifications (none when the text is none):

```text
{justified}
```

Classification:

```json
{classify}
```

Deduplicated findings (each carries its severity and producer lens; provenance
lists the other lenses that raised the same file and line):

```json
{findings}
```

Refuter outcomes, one per finding (instance = the finding's index above):

```json
{verdicts}
```

The regression lens's safety fact, its proof rung (1 to 5), the proof, and the
risks it cleared (all empty when that lens did not run):

```text
safety fact: {review-regression.safetyFact}
rung: {review-regression.rung}
proof: {review-regression.proof}
cleared: {review-regression.cleared}
```

Sort every finding into actOn, consider, noted or dismissed with its lenses
(the producer plus its provenance), its severity and a one-line reason. These
rules come first, and your lead judgment sorts the rest.

- A refuted finding is dismissed with the refuter's reason.
- An unconfirmed finding from review-correctness, review-correctness-codex,
  review-regression or review-security goes to consider for the owner (d869).
  An unconfirmed finding from any other lens is dismissed as an unconfirmed
  cleanup.
- A review-structure finding with beyondDiff true goes to consider at most.
  The gate never applies it, and work beyond the task's path is a finding for
  the owner (d870, d864).
- Every confirmed presumptive blocker the diff introduced goes to actOn
  (d871), and so does every confirmed critical finding.

An actOn item is justified when one of the author's justifications names it
with a reason that holds against the finding; for the 1000-line rule that is a
compelling structural reason with the file still clearly organized. Copy an
accepted reason into the item's justification field, and say in its reason
when you reject one.

The agreement field says where lenses and model families agreed or diverged
and what that pattern says. review-correctness runs on Claude and
review-correctness-codex on Codex with the identical prompt; every other lens
runs on Claude. The safety field restates the regression safety fact with its
rung and proof, or says unproven; leave it out when that lens did not run.
The verdict is fail while any actOn item has no accepted justification, and
pass otherwise; a fix lands as a new commit and a new gate run. The summary
states the verdict and what decided it, briefly. Answer with one JSON object
matching the schema and nothing else.
