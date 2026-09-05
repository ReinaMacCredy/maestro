---
name: fix-loop
description: Run the check, review the scope with two lenses, let a writing fixer resolve what they found, and loop until the check passes with no findings (three rounds at most); a human confirms the close or decides the escalation.
input:
  scope: {required: true, description: "what to fix: a failing test, a finding list, or a bug in one sentence"}
  check: {default: "bun test", description: "the shell command that must pass before the loop closes"}
limits: {nodes: 24, loops: 3, fanout: 4}
nodes:
  verify: {kind: function, command: "if sh -c {check} > /dev/null 2>&1; then printf '{\"passed\": true}'; else printf '{\"passed\": false}'; fi"}
  review-bugs:
    kind: agent
    profile: reviewer-correctness
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
  review-regressions: {kind: agent, profile: reviewer-regression, schema: *findings}
  findings: {kind: join, collect: findings, key: [file, line], window: 0}
  gate: {kind: router}
  fix:
    kind: agent
    profile: fixer
    writes: true
    schema:
      type: object
      required: [files, summary, unresolved]
      properties:
        files: {type: array, items: {type: string}}
        summary: {type: string}
        unresolved: {type: array, items: {type: string}}
  confirm: {kind: human}
  escalate: {kind: human}
edges:
  - {from: verify, to: review-bugs}
  - {from: verify, to: review-regressions}
  - {from: review-bugs, to: findings}
  - {from: review-regressions, to: findings}
  - {from: findings, to: gate}
  - {from: gate, to: fix, when: {any: [{path: verify.passed, eq: false}, findings.items]}}
  - {from: gate, to: confirm}
  - {from: fix, to: verify, max_rounds: 3}
  - {from: fix, to: escalate}
---

## review-bugs

Repository: the current working directory. Scope under repair: {scope}. Round
{round}; the check `{check}` currently reports passed={verify.passed}. Read
the code the scope names and its callers. Do not edit any file.

Report behavioral bugs only, inside the scope or introduced by the last fix:
wrong output, crash, unhandled state, broken invariant. Each finding gives
the concrete inputs or state that produce the wrong result. Answer with one
JSON object {"findings": [...]} matching the schema and nothing else; an
empty list is a valid answer and closes the loop.

## review-regressions

Repository: the current working directory. Scope under repair: {scope}. Round
{round}; the check `{check}` currently reports passed={verify.passed}. Read
the code the scope names and its callers. Do not edit any file.

Report callers, data, or flows outside the scope that the last fix breaks or
silently changes. Each finding names the caller or data path and how it
observes the change. Answer with one JSON object {"findings": [...]}
matching the schema and nothing else; an empty list is a valid answer.

## fix

Repository: the current working directory. Scope under repair: {scope}. Round
{round}; the check `{check}` reports passed={verify.passed}.

Findings to resolve (deduplicated; provenance lists the other lens that
raised the same file and line):

```json
{findings.items}
```

Apply the smallest change that resolves each finding where every caller
routes through, and make `{check}` pass. No cleanups outside the findings,
no new abstractions, no commit: the graph records the files you name and the
next round reads the working tree. Run the narrowest check that can falsify
each fix. Answer with one JSON object {"files": [...], "summary": "...",
"unresolved": [...]} and nothing else; unresolved names each finding you
could not fix and why.

## confirm

Round {round}: the check `{check}` reports passed={verify.passed} and the
reviewers report no findings for {scope}. Reply "approved" to close the
loop, or state what still blocks it.

## escalate

Three fix rounds are exhausted for {scope}. The check reports
passed={verify.passed}; the last fix reported {fix}; findings still open:

```json
{findings.items}
```

Decide: describe the next action for the handback (a new card, a design
question, or a manual fix), or reply "abandon" with the reason.
