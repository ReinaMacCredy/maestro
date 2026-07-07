---
name: maestro-witness
version: 1.0.2
description: "Witness feature close: use after Maestro feature proof and QA pass, before feature close, to write current witness.md/advisor.md receipts, auto-invoke an independent advisor when allowed, apply risk-tier and human-demo policy, and emit Gate: APPROVED."
---

# Maestro Witness

Use this after implementation, task proof, `maestro feature verify`, and QA
evidence are complete, before `maestro feature close`. This skill is the witness
conductor: it turns accepted evidence into two close receipts and emits
`Gate: APPROVED` only when those receipts are current, independent, and complete.
It does not implement code, replace proof or QA, or turn broad audit into a
blocking close gate.

Exact command signatures live in [reference/cli.md](reference/cli.md),
generated from the binary. A verb or flag not listed there does not exist.

## Close Inputs

Read current evidence before writing receipts:

- `maestro feature show <id>`
- `maestro feature design <id>` or `.maestro/cards/<id>/handoff.md`
- `.maestro/cards/<id>/qa.md` or the accepted `qa: none` declaration
- feature proof and sweep output from `maestro feature verify <id>`
- task proof state for the accepted work
- current git state for the target checkout

Do not paste large code dumps into the receipts. Cite commands, file paths,
hash refs, acceptance ids, proof ids, QA scenario ids, and short observed facts.
Complete this intake only when every accepted `ac-N` has proof/QA evidence and
the current handoff, QA, proof, sweep, and tree anchors are known.

## Receipt Contract

Normal approval writes exactly two sidecars:

```text
.maestro/cards/<id>/witness.md
.maestro/cards/<id>/advisor.md
```

`witness.md` is the worker-side close brief. It must map every accepted
`ac-N` item to proof and QA evidence and must anchor to the current close
inputs:

```yaml
gate: APPROVED
contract_ref: handoff:<sha256 of handoff.md>
proof_ref: proof:<sha256 of serialized acceptance evidence and sweeps>
qa_ref: qa:<sha256 of qa.md>
tree_ref: git:<current HEAD, or none outside git>
risk_tier: T1
acceptance_mapping_complete: true
proof_matrix_complete: true
ac-1: PASS
```

For accepted `qa: none`, use `qa_ref: qa-declaration:<sha256 of the QA
declaration>` instead of a `qa.md` hash. Refresh the witness if the handoff,
proof, QA, acceptance sweep, or git head changes. `witness.md` is complete only
when all accepted `ac-N` rows are present, all anchors are current, and the risk
tier matches the changed surface.

`advisor.md` is the independent review receipt. It must be written by a
different fresh-context session, subagent, or human reviewer from the worker:

```yaml
verdict: APPROVE
reviewed_witness_ref: witness:<sha256 of witness.md>
worker_ref: session:<worker>
advisor_ref: subagent:<advisor>
independent_session: true
acceptance_audit_complete: true
proof_spot_check_result: pass
confidence: H
```

`advisor independence` is a hard close condition: `worker_ref` and `advisor_ref`
must both be present, must be distinct, and must show separate review authority.
`advisor.md` is complete only when the advisor independently checked the
acceptance mapping, proof matrix, proof spot-check, risk tier, and human-demo
policy and returned `verdict: APPROVE`.

Default advisor shape: the conductor may automatically invoke a fresh-context
subagent and pass it the evidence packet. That is independent when the subagent
performs its own review and returns the receipt. Human review is required only
when the risk tier, user instruction, policy, or tool boundary explicitly
requires a human demo, human reviewer, or expert escalation. If the subagent
cannot write `advisor.md` in the owning checkout, the conductor may transcribe
the returned receipt verbatim and record `advisor_ref: subagent:<advisor>` plus
the source output or run ref. The conductor must not invent approval or reuse
the worker session as advisor.

## Risk Tiers

- `risk_tier: T0`: no material implementation risk. Skip normal witness/advisor
  only with explicit user authorization in `witness.md`.
- `risk_tier: T1`: routine low-risk change. Normal witness/advisor receipts are
  enough.
- `risk_tier: T2`: human-demo surface. Include demo evidence, or an advisor
  waiver with `demo_waived: true` and `demo_waiver_reason: <why safe>`.
- `risk_tier: T3`: high-risk, ambiguous, security-sensitive, release-sensitive,
  or outside-advisor-lens surface. Include demo evidence. If `confidence: L` or
  `advisor_lens_exceeded: true`, also include
  `expert_escalation: satisfied`.

T0 skip format:

```yaml
skipped: true
tier: T0
skipped_by: user
user_authorization_ref: <message, ticket, or run ref>
skip_reason: <why normal witness/advisor is unnecessary>
changed_surface: <bounded surface>
```

T1 and above never skip the witness/advisor pair.

## Steps

1. Confirm close state. Read the feature, handoff, QA, proof, task state, and
   git state. Stop if proof, QA, or accepted scope is stale or incomplete.
2. Refresh `witness.md`. Include the current anchors, risk tier, and one PASS
   row per accepted `ac-N`. The receipt is not current after any handoff, proof,
   QA, sweep, or git-head change.
3. Get independent advisor review. Auto-invoke a fresh-context advisor subagent
   for routine T1 close unless the risk tier, policy, or user requires human
   review. The advisor checks acceptance mapping, proof matrix,
   proof spot-check, risk tier, and human-demo policy, then writes or returns
   `advisor.md` only for `verdict: APPROVE`.
4. Approve and close. Emit the conductor line `Gate: APPROVED` only after both
   receipts are current and complete, then run
   `maestro feature close <id> --outcome "<outcome>"`.

If the advisor finds a defect in code, tests, QA, proof, or scope, do not force
close. Return to `maestro-card` work/proof/QA and refresh the witness afterward.

## Audit Boundary

During witness sign-off, audit is backlog-only. A broad review may produce
follow-up findings with `maestro harness propose`, but audit findings do not
become close blockers unless they invalidate the accepted contract, proof, QA,
or risk-tier policy. Do not widen sign-off into a new audit implementation
batch.

## Stop

- Do not close without `witness.md` and `advisor.md`, except an explicit T0 user
  skip receipt.
- Do not accept `Gate: APPROVED` from the worker alone.
- Do not let the advisor be the same session as the worker.
- Do not require a human for routine T1 when an independent fresh-context
  subagent can complete the advisor receipt.
- Do not paste large code dumps into either receipt.
- Do not treat backlog audit ideas as failed witness gates.
