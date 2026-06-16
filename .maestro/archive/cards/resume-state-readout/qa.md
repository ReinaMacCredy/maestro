---
amend_log_position: 0
---

### QA Baseline Contract

- Scope: resume-state-readout -- `maestro resume` (default, no --full), `maestro status`, and `maestro feature ship --dry-run` output surfaces. No new command; extends existing read/preview output.
- Critical workflow chains:
  - Resume-then-act (the trunk journey this feature serves)
    - Steps: open a repo with live work + a dirty tree -> `maestro resume` -> read state -> run the named next/repair command
    - Touched link: resume/status read model gains git working-tree state + a concern-only proof line; the proof-recovery pointer changes verb
    - Minimal proof: `maestro resume` and `maestro status` output captured against a known repo state, compared line-by-line to the oracle below
- Scenario Matrix:
  - [bl-001] resume default renders the git line on a dirty repo (covers: ac-1, ac-2)
    - Dimensions: entrypoint (CLI read), state (dirty working tree), data (paths under .maestro/ vs elsewhere)
    - Setup: a repo with both a `.maestro/` change and a non-`.maestro/` change uncommitted, on a named branch
    - Action: `maestro resume`
    - Oracle: output contains a git line naming the current branch and two uncommitted counts -- one labelled for `.maestro/` cards, one for code/other
    - Evidence to capture: the resume stdout block showing the git line
    - Reproduction: edit one src file + one `.maestro/` file, do not commit, run `maestro resume`
  - [bl-002] status renders the same git line (covers: ac-1)
    - Dimensions: entrypoint (second read surface), consistency
    - Setup: same dirty repo as bl-001
    - Action: `maestro status`
    - Oracle: status output contains the same branch + split-count git line
    - Evidence to capture: status stdout showing the git line
    - Reproduction: as bl-001 with `maestro status`
  - [bl-003] clean-worktree note is contextual, not always shown (covers: ac-3)
    - Dimensions: state/lifecycle (next verb shape), data (code/other dirty count)
    - Setup A: next valid verb is ship/verify-shaped AND code/other dirty > 0
    - Setup B: next verb is not ship/verify-shaped, OR code/other dirty == 0
    - Action: `maestro resume` (and `maestro status`)
    - Oracle: clean-worktree note present in A; absent in B
    - Evidence to capture: both outputs, showing the note only in A
    - Reproduction: contrive each state and run resume
  - [bl-004] proof line appears only on concern (covers: ac-4)
    - Dimensions: state/lifecycle (task proof state), failure surfacing
    - Setup: (i) task in needs_verification with no/failed/stale proof; (ii) task verified with fresh proof; (iii) ready/unclaimed task
    - Action: `maestro resume` / `maestro status` (default, no --full)
    - Oracle: proof line present for Stale/Failed/Missing-when-needs_verification (i); absent for accepted+fresh (ii) and ready/unclaimed (iii)
    - Evidence to capture: outputs for each state
    - Reproduction: drive a task through each proof state and run resume
  - [bl-005] each proof concern names the exact repair (covers: ac-5)
    - Dimensions: actionability, failure recovery
    - Setup: a task with stale proof (HEAD moved after a passing verification)
    - Action: `maestro resume`
    - Oracle: the proof line names `maestro task verify <id>`, framed as a refresh (HEAD moved; likely no code change), with the stale reason; Failed names fix-then-verify; Missing(expected) names the lifecycle verb
    - Evidence to capture: the proof line text with the named command
    - Reproduction: verify a task, advance HEAD, run resume
  - [bl-006] resume proof-recovery pointer names task verify, not query proof (covers: ac-6)
    - Dimensions: actionability (the resume.rs:226 pointer)
    - Setup: the resume state that previously emitted `maestro query proof <id>`
    - Action: `maestro resume`
    - Oracle: the pointer reads `maestro task verify <id>`; the string `query proof` no longer appears there
    - Evidence to capture: the pointer line
    - Reproduction: reach that resume state and read the pointer
  - [bl-007] ship --dry-run prints a non-blocking proof advisory (covers: ac-7)
    - Dimensions: integration boundary (ship gate), non-functional (advisory must not block)
    - Setup: a feature with >=1 verified child task whose proof commit differs from current HEAD
    - Action: `maestro feature ship --dry-run <id>`
    - Oracle: ship preview lists those child tasks as a "verified at older commits; re-verify if their code changed" advisory; the dry-run still passes (no new blocking gap); a feature with no stale-commit child shows no advisory
    - Evidence to capture: the ship --dry-run preview block
    - Reproduction: verify a task, advance HEAD, run `feature ship --dry-run`
- Preserved behaviors:
  - resume default still prints objective/state/blockers/next/required-reads/guardrails/memory -> Proof: `maestro resume` against current repo
  - status still prints repo/resume/tasks/features/run/ACTIONS/ACTIVE FEATURES -> Proof: `maestro status`
  - `--full` resume still carries the existing full context (proof block remains in --full) -> Proof: `maestro resume --full`
  - ship gate still blocks on live child tasks + QA baseline/slice coverage; ac-7 adds NO blocking gap -> Proof: `maestro feature ship --dry-run <id>` on a feature with a live child still reports the live-child gap
- Changed behaviors:
  - resume default + status gain a git line and a concern-only proof line (previously absent)
  - resume.rs proof-recovery pointer verb changes `query proof` -> `task verify`
  - ship --dry-run gains a non-blocking proof advisory
- Critical probes before commit:
  - GitSnapshot categorization on a fixture repo (.maestro/ vs code/other) -> `cargo test` (foundation/core/git unit test)
  - resume/status render under each proof state -> targeted unit/integration tests on the read model
- Required artifacts:
  - None beyond the card store
- Baseline gaps:
  - Clean-worktree "next verb is ship/verify-shaped" detection reuses resume's existing next_action_for; if that classifier mislabels a state the note could mis-fire -> Proposed probe: unit test the ship/verify-shaped predicate against representative next-verb states

```yaml
slices:
  - at: "2026-06-16T12:10:00Z"
    scenarios: ["bl-001", "bl-002", "bl-003"]
    probes:
      - "real binary g6bebcfa5: maestro resume / maestro status on /tmp/maestro-qaslice-rsr (dirty tree, named branch)"
      - "cargo test --test core_backup_diff_git git_snapshot_splits_dirty_counts_by_maestro_prefix"
    result: pass
    evidence:
      - "bl-001 (ac-1,ac-2): resume git line 'git: main, 1 code/other + 8 maestro-card uncommitted' -- branch + split counts both present (/tmp/ev_resume_bl001.txt)"
      - "bl-002 (ac-1): status prints the identical 'git: main, 1 code/other + 8 maestro-card uncommitted' line (/tmp/ev_status_bl002.txt)"
      - "bl-003A (ac-3): clean-worktree note present when next verb verify-shaped (in_progress) AND code/other>0 (/tmp/ev_resume_bl001.txt note line); bl-003B: note ABSENT when code/other==0, git line still shown (/tmp/ev_resume_bl003B.txt)"
  - at: "2026-06-16T12:10:00Z"
    scenarios: ["bl-004", "bl-005", "bl-006"]
    probes:
      - "real binary g6bebcfa5: maestro resume / maestro status on /tmp/maestro-qaslice-{failed,stale,fresh}"
      - "cargo test --test status_next_integration needs_verification_failed_proof_surfaces_verify_repair_on_resume_and_status verified_stale_proof_surfaces_refresh_repair_via_resume_task"
    result: pass
    evidence:
      - "bl-004 concern present: failed proof -> 'proof: failed; fix, then re-verify: maestro task verify <id>' on resume AND status (/tmp/ev_resume_bl004_failed.txt, /tmp/ev_status_bl004_failed.txt); concern ABSENT on fresh verified task via resume --task (/tmp/ev_resume_bl004_fresh.txt)"
      - "bl-005 (ac-5): stale -> 'proof: stale (ebfd200->f7fa3bc); refresh (HEAD moved, likely no code change): maestro task verify <id>' (/tmp/ev_resume_bl005_stale.txt); Failed names fix-then-verify (bl-004 evidence)"
      - "bl-006 (ac-6): resume next-action 'recover proof with maestro task verify <id>' and status proof_recovery 'run: maestro task verify <id>' -- not 'query proof'; query proof remains only as the read-only inspector under required reads (/tmp/ev_resume_bl004_failed.txt, /tmp/ev_status_bl004_failed.txt)"
  - at: "2026-06-16T12:10:00Z"
    scenarios: ["bl-007"]
    probes:
      - "real binary g6bebcfa5: maestro feature ship ship-advisory --dry-run on /tmp/maestro-qaslice-ship (verified child at older commit)"
      - "cargo test --test status_next_integration feature_ship_dry_run_flags_verified_children_at_older_commits_without_blocking"
    result: pass
    evidence:
      - "bl-007 (ac-7): dry-run prints 'would ship' (non-blocking) and INSIDE the ship preview block, under 'feature:', the line 'note: 1 child task(s) verified at older commits (HEAD moved); re-verify if their code changed: task-child-of-advisory-dbb4 (advisory; does not block ship)' (/tmp/ev_ship_bl007_drift.txt)"
      - "no-advisory branch (verified children all match HEAD -> no note line) proven by the integration test's pre-HEAD-move assertion !contains('verified at older commits')"
```
