---
name: council
description: Lead-only council on a hard-to-reverse fork (Hub w32, d92-d95) - seats by tier, sealed reports, a decision model, a premise verifier on unanimity or bounded verifiers on dispute, one cross-examination round, the Lead's draft, an auditor by tier, the Lead's verdict.
input:
  brief: {required: true, description: "the neutral brief from the maestro-council skill (references/brief.md), verbatim, with the case output contract"}
  tier: {default: debate, description: "lens | debate | debate-with-proof | high-risk"}
limits: {nodes: 16, loops: 1, fanout: 4}
verdict: verdict
nodes:
  seats: {kind: router}
  independent:
    kind: agent
    profile: independent
    schema: &report
      type: object
      required: [position, claims, falsifier]
      properties:
        position: {type: string}
        recommendation: {type: string}
        claims:
          type: array
          items:
            type: object
            required: [id, type, claim, evidence]
            properties:
              id: {type: string}
              type: {type: string, enum: [FACT, INFERENCE, CAUSAL CLAIM, FORECAST, VALUE / PREFERENCE, AUTHORITATIVE CONSTRAINT]}
              claim: {type: string}
              evidence: {type: string}
        alternative: {type: string}
        counterargument: {type: string}
        failure_mode: {type: string}
        falsifier: {type: string}
        unknowns: {type: array, items: {type: string}}
        confidence: {type: string}
  challenger: {kind: agent, profile: challenger, schema: *report}
  specialist: {kind: agent, profile: specialist, schema: *report}
  seal: {kind: join}
  model:
    kind: agent
    profile: classifier
    schema:
      type: object
      required: [unanimous, premise, disputed]
      properties:
        unanimous: {type: boolean}
        premise: {type: string}
        disputed:
          type: array
          items:
            type: object
            required: [id, proposition, mandate]
            properties:
              id: {type: string}
              proposition: {type: string}
              mandate: {type: string, enum: [supporting evidence, disconfirming evidence, coverage audit]}
  verify-gate: {kind: router}
  premise:
    kind: agent
    profile: verifier
    schema: &verification
      type: object
      required: [proposition, mandate, result, observations]
      properties:
        proposition: {type: string}
        mandate: {type: string}
        sources: {type: array, items: {type: string}}
        observations: {type: string}
        result: {type: string, enum: [verified, falsified, partial, insufficient coverage, snapshot mismatch]}
        limitations: {type: string}
  disputed: {kind: foreach, over: model.disputed, key: id}
  verify: {kind: agent, profile: verifier, schema: *verification}
  verified: {kind: join}
  cross:
    kind: agent
    profile: challenger
    schema: &responses
      type: object
      required: [responses]
      properties:
        responses:
          type: array
          items:
            type: object
            required: [id, response, reason]
            properties:
              id: {type: string}
              response: {type: string, enum: [CONCEDE, MAINTAIN, NARROW, REVERSE]}
              reason: {type: string}
              evidence: {type: string}
              falsifier: {type: string}
  response: {kind: agent, profile: independent, schema: *responses}
  draft: {kind: human}
  audit-gate: {kind: router}
  auditor:
    kind: agent
    profile: auditor
    schema:
      type: object
      required: [result, findings]
      properties:
        result: {type: string, enum: [CLEAR, REVISE, STOP]}
        findings: {type: array, items: {type: string}}
  verdict: {kind: human}
edges:
  - {from: seats, to: independent, when: tier}
  - {from: seats, to: challenger, when: {path: tier, ne: lens}}
  - {from: seats, to: specialist, when: {path: tier, eq: high-risk}}
  - {from: independent, to: seal}
  - {from: challenger, to: seal}
  - {from: specialist, to: seal}
  - {from: seal, to: model}
  - {from: model, to: verify-gate}
  - {from: verify-gate, to: premise, when: model.unanimous}
  - {from: verify-gate, to: disputed, when: {not: model.unanimous}}
  - {from: disputed, to: verify}
  - {from: verify, to: verified}
  - {from: verified, to: cross}
  - {from: cross, to: response}
  - {from: premise, to: draft}
  - {from: response, to: draft}
  - {from: draft, to: audit-gate}
  - {from: audit-gate, to: auditor, when: {any: [{path: tier, eq: debate-with-proof}, {path: tier, eq: high-risk}]}}
  - {from: audit-gate, to: verdict}
  - {from: auditor, to: verdict}
---

## independent

seat execution mode: work as a fully autonomous reviewer with independent
judgment inside the authorized scope. This assignment asks for your own
analysis, not orchestration: do not load the council skill, open work, spawn
or contact agents, or read other seats' work items. Begin directly.

{brief}

Role line: reason from first principles, recommend the strongest answer,
expose the decision-critical assumptions. Type each material claim (FACT,
INFERENCE, CAUSAL CLAIM, FORECAST, VALUE / PREFERENCE, AUTHORITATIVE
CONSTRAINT).

This is analysis only. Do not edit, create, rename, or delete files. Do not
write code. Do not spawn or contact agents. Do not optimize for agreement.
Distinguish direct observations from inference and state what evidence would
prove your position wrong. Answer with one JSON object matching the schema
and nothing else.

## challenger

seat execution mode: work as a fully autonomous reviewer with independent
judgment inside the authorized scope. This assignment asks for your own
analysis, not orchestration: do not load the council skill, open work, spawn
or contact agents, or read other seats' work items. Begin directly.

{brief}

Role line: test the framing and the shared premises, build at least one
viable counterfactual, say what it makes unnecessary; do not manufacture
disagreement. Type each material claim (FACT, INFERENCE, CAUSAL CLAIM,
FORECAST, VALUE / PREFERENCE, AUTHORITATIVE CONSTRAINT).

This is analysis only. Do not edit, create, rename, or delete files. Do not
write code. Do not spawn or contact agents. Do not optimize for agreement.
Distinguish direct observations from inference and state what evidence would
prove your position wrong. Answer with one JSON object matching the schema
and nothing else.

## specialist

seat execution mode: work as a fully autonomous reviewer with independent
judgment inside the authorized scope. This assignment asks for your own
analysis, not orchestration: do not load the council skill, open work, spawn
or contact agents, or read other seats' work items. Begin directly.

{brief}

Role line: apply only the requested domain semantics; expertise does not
override stronger evidence or product authority. Type each material claim
(FACT, INFERENCE, CAUSAL CLAIM, FORECAST, VALUE / PREFERENCE, AUTHORITATIVE
CONSTRAINT).

This is analysis only. Do not edit, create, rename, or delete files. Do not
write code. Do not spawn or contact agents. Do not optimize for agreement.
Distinguish direct observations from inference and state what evidence would
prove your position wrong. Answer with one JSON object matching the schema
and nothing else.

## model

reduce the sealed seat reports below into the smallest decision model that
keeps every natural unit; attribute by role only. Tier {tier}.

Brief:

{brief}

Sealed reports (producer = seat role):

```json
{seal.items}
```

unanimous: true only when every valid seat reaches the same position.
premise: when unanimous, the one shared premise in the brief that drives the
common conclusion (Hub d94); otherwise the sentence that names the fork.
disputed: when not unanimous, one entry per material factual dispute with a
precise proposition and the single mandate a verifier needs (supporting
evidence, disconfirming evidence, or coverage audit); one to three entries,
never an ensemble of identical prompts (Hub d95). Only facts and direct
observations are eligible; a value or preference is never disputed here.
Answer with one JSON object matching the schema and nothing else.

## premise

every seat agreed, which is not a skip (Hub d94). Check exactly one
proposition under one mandate and nothing else.

PROPOSITION: {model.premise}
MANDATE: disconfirming evidence, against the authorized sources in the brief.

{brief}

You are analysis only: no edits, no spawning, no contact with other seats.
Answer with one JSON object {proposition, mandate, sources, observations,
result, limitations} where result is verified, falsified, partial,
insufficient coverage or snapshot mismatch, and nothing else.

## verify

check exactly one proposition under one mandate and nothing else.

PROPOSITION ({item.id}): {item.proposition}
MANDATE: {item.mandate}, against the authorized sources in the brief.

{brief}

You are analysis only: no edits, no spawning, no contact with other seats.
Answer with one JSON object {proposition, mandate, sources, observations,
result, limitations} where result is verified, falsified, partial,
insufficient coverage or snapshot mismatch, and nothing else.

## cross

one challenge per disputed unit, never free-form debate. You receive the
sealed reports (by role) and the verifier results for tier {tier}.

Brief:

{brief}

Reports:

```json
{seal.items}
```

Verifications (instance = the proposition id):

```json
{verified.items}
```

For each disputed proposition, put one targeted question to the position
that the verification weakens most, and answer for the challenging side:
response is CONCEDE, MAINTAIN, NARROW or REVERSE with the reason, the direct
evidence, and the falsifier. New material factual claims go to a verifier,
never back into debate. Answer with one JSON object {"responses": [...]}
matching the schema and nothing else.

## response

one response per disputed unit, never free-form debate. You receive the
sealed reports (by role), the verifier results and the challenge for tier
{tier}.

Brief:

{brief}

Reports:

```json
{seal.items}
```

Verifications:

```json
{verified.items}
```

Challenge:

```json
{cross.responses}
```

Answer each challenged proposition for the first-principles side: response
is CONCEDE, MAINTAIN, NARROW or REVERSE with the reason, the direct
evidence, and the falsifier. Answer with one JSON object {"responses":
[...]} matching the schema and nothing else.

## draft

You are the Lead. Draft the verdict alone from the material below; no vote,
no averaged confidence, seat count never creates authority. Decide the
authoritative outcome and hard constraints; the options verified
constraints exclude; verified, falsified and unresolved premises; fit under
realistic failure modes; robustness if an assumption is wrong;
reversibility; whether serious dissent has stronger evidence or a decisive
falsifier. Reconcile through premise, mechanism, boundary, failure,
reversibility, evidence, authority and proof.

Tier {tier}. Brief:

{brief}

Decision model:

```json
{model}
```

Sealed reports:

```json
{seal.items}
```

Premise verification (unanimity path):

```json
{premise}
```

Bounded verifications and the cross-examination round (dispute path):

```json
{verified}
```

```json
{cross}
```

```json
{response}
```

Reply with the draft verdict and its dissent as text.

## auditor

audit the Lead's draft verdict against the reports and the evidence; you
never replace the verdict. You receive the brief, every valid report by
role only, the decision model, the verified evidence, the draft and the
dissent; never seat identities or transcripts. Find where the draft claims
more than the evidence supports, drops a material dissent, or rests on a
fragile chain.

Brief:

{brief}

Decision model:

```json
{model}
```

Reports:

```json
{seal.items}
```

Evidence:

```json
{premise}
```

```json
{verified}
```

Draft verdict and dissent:

{draft}

Answer with one JSON object {"result": "CLEAR" | "REVISE" | "STOP",
"findings": [...]} and nothing else.

## verdict

You are the Lead. Record the binding verdict. Tier {tier}; audit result:

```json
{auditor}
```

Your draft:

{draft}

Resolve every material audit finding by revising the draft, dropping the
unsupported claim, or naming the proposition that returns to a verifier; at
most one audit round. Reply with the final verdict text, the dissent and the
Lead's answer to it, then record it: maestro decision draft "<decision>"
--rationale "<why; accepted and rejected claims; the dissent and the answer>"
--dissent "<losing view>" --work <id>, maestro decision lock <id>, and note
the handoff contract on the work item.
