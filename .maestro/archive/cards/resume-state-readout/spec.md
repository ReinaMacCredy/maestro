# resume state readout

## Current state

resume (src/interfaces/cli/resume.rs, 561 lines) reads card/decisions/feature/proof/task and never touches git: no branch or dirty state in any mode (bare, --full, --handoff).

Proof line is gated behind resume --full; bare resume and status omit it. resume.rs:226 points proof recovery at 'maestro query proof' (a read) rather than 'maestro task verify' (the actual repair).

Proof states (src/domain/proof/proof_status.rs:16-21): Missing/Failed/Accepted/Stale. Stale = was Passed then HEAD or contract_hash changed after proof (classify_binding :304-309); stale_reasons carries verified_commit expected/found.

src/foundation/core/git.rs already exposes GitSnapshot{head, dirty:bool} via git2; is_dirty counts tracked+untracked with recurse. No branch name, no count, no .maestro/-vs-code categorization. This repo: 0 tracked-modified, 15 untracked (14 under .maestro/, 1 .claude/workflows/).

Binary freshness is already surfaced passively: run_auto_check (src/main.rs:32) once/day plus maestro upgrade --check. Locked decision card-36759d: resume enhancements fold into resume, not a new feature.

## Problem

Resume threads reconstruct repo+proof state from transcripts. Stale proof reads as unfinished work though the fix is usually re-verify, not implementation. The dimensions resume/status omit today: git working-tree state, proof-in-the-default-view, and a named repair/next command.
