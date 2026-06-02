# decision-001: No proof journal to extract

## Status
Accepted

## Context
An `improve-codebase-architecture` deepening brainstorm (2026-05-29) proposed extracting a
"proof journal" as a candidate. Investigation of `src/domain/proof/` found no such construct:

- The only journal-named type is `CanonicalReportRestoreJournal` (`verify_task.rs:161`), a
  single-prior-report crash-recovery write-ahead log. It never accumulates and is already
  deep and private.
- The proof-result persistence surface is already **deep**: `task_verify` and the CLI read
  paths see only DTOs (`TaskVerification`, `ProofStatus`, `VerificationCommandRead`); the path
  literals (`verification.json`, the bounded `verification.attempts/` ring, the restore file),
  schema stamping (`VERIFICATION_SCHEMA_VERSION`), atomic writes, and the restore journal are
  all private / `pub(crate)` inside `verify_task.rs`. No caller rebuilds them. The deletion
  test passes.
- The shallow surface that does exist is the untyped proof-**event** writer/reader, which is
  already owned by the separate "Run event seam" deepening design (Candidate 2 in
  `DEEPENING-feature-and-run-seam.md`).

## Decision
Do not extract a proof journal. The proof-result surface stays as-is. Proof-event
writer/reader work is tracked under the Run event seam design, not as a proof-journal extract.

## Alternatives considered
- **Extract a proof-results ledger/journal.** Rejected: no accumulating ledger exists. Results
  live in the `task.yaml` verification receipt, `<task_dir>/verification.json`, and the bounded
  attempts ring, all already concentrated behind the proof facade.
- **Add a `PROOF_EVENT_KIND` constant for the `task_proof` literal.** Rejected: after the Run
  event seam change, `record_claim` takes no kind parameter and orphans the
  `event create --kind` arg (`cli/mod.rs:219`), leaving `task_proof` with a single consumer
  (the reader-match tolerance, `verify_task.rs:1239`). A shared constant for one consumer is a
  speculative abstraction.
- **Reopen the `task.yaml`-receipt vs `verification.json` coherence split.** Rejected: this is
  a deliberate Task/Proof seam reconciled by `report_reflected_in_task` (`proof_status.rs:485`)
  and governed by an architecture guardrail. It is not duplication; reopening it is ADR
  territory, not a deepening.

## Consequences
Future architecture reviews should not re-suggest extracting a proof journal. The genuine
proof-event seam work lives in Candidate 2 of the deepening doc. The cross-cutting
`PostToolUse` literal leak (four sites, three outside the proof domain) remains tracked
separately as general hygiene.

## Linked tasks

