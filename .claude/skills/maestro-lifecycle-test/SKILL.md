---
name: maestro-lifecycle-test
version: 1.0.0
description: Validate a built maestro binary end to end (install/init completeness + the feature->task->QA lifecycle + standalone tasks + real-use edge cases + agent-UX) in an isolated throwaway repo, emitting a machine-aggregatable NDJSON report. Run as one sub-agent of a swarm fix-loop, or standalone to smoke-test a change.
---

# maestro-lifecycle-test

Drive the real maestro CLI through its whole lifecycle in a disposable repo and report what
held and what broke, as structured findings the orchestrator can merge across many runs.

This is a **single-agent, single-pass** procedure. It never spawns sub-agents. The swarm fan-out
(below) is the caller's job, not this skill's.

## When to use

- A maestro source change touched install/init, the feature 5-state machine, the task lifecycle,
  the QA gates (qa-baseline/qa-slice), or proof/verify, and you want an integrated smoke test.
- You are one worker in a swarm fix-loop validating maestro (see "Swarm usage" below).

It is NOT a replacement for `cargo test` -- the Rust suite already proves the mechanics
(extraction rollback, symlink wiring, gate logic) at the unit/contract layer. This skill's value
is the **integrated journey + observable CLI output + the loop-ready report**.

## How to run

1. Ensure the binary under test is built. The harness uses a PREBUILT binary and never builds
   itself (so a swarm of workers does not trigger N concurrent `cargo build`s):

   ```sh
   cargo build            # the orchestrator does this once per round, in the source checkout
   ```

2. Run the harness from anywhere inside the maestro source checkout:

   ```sh
   bash .claude/skills/maestro-lifecycle-test/harness.sh
   ```

   Optional env:
   - `MAESTRO_BIN` -- path to the binary (default `target/debug/maestro` under the source repo).
   - `MAESTRO_SRC` -- source checkout that holds `embedded/` (default: git toplevel of the script).
   - `KEEP=1` -- keep the throwaway target repo for inspection instead of deleting it.

3. Read the report. The harness prints NDJSON to stdout (one finding per line, then a summary
   object), also written to `<target-repo>/maestro-lifecycle-test.report.json`, and a one-line
   human summary to stderr. Exit code is `0` iff there are zero **blocking** failures.

## What it checks (phases, stable step IDs)

- **P0 install+init completeness.** `init` scaffolds every dir/file + the 6 skill dirs; `install`
  populates every `SKILL.md` (frontmatter parses, body non-empty, byte-identical to `embedded/`,
  no drift); `record.sh` present (it is `-rw-r--r--`, invoked via `sh` -- not asserted +x);
  `events.yaml` is source-only (NOT extracted); the hook is **wired** -- `.claude/settings.local.json`
  registers `record.sh` against a hook event, and the managed instruction blocks land in
  CLAUDE.md/AGENTS.md/.gitignore; `doctor` green; bare re-init errors with the `--merge`/`--force`
  recovery, `init --merge` and re-install are clean no-ops. If the binary predates `embedded/`,
  drift findings are reclassified `test_or_env` (rebuild) instead of `product_bug`.
- **P1 feature -> task -> QA happy path.** new -> set -> accept gate (blocks + dry-run preview) ->
  freeze -> start -> child task -> ship gate aggregates live-child + coverage -> drive the child
  claim/complete/verify (real proof) -> fresh baseline + counting slice -> ship -> idempotent re-ship.
- **P2 standalone task.** zero-check guard fires at `claim`; add a `--check`, then the full
  draft -> verified chain.
- **P3 edge cases.** C5 (verified child survives a behavioral amend); cancel cascade (live->abandoned,
  verified stays); illegal-source cells (amend-on-proposed, start-on-terminal, accept-from-draft);
  clap misuse (cancel without --reason); evidence-less slice does not count; QA-C zero-`bl` baseline
  ships clean; task archive/unarchive round-trip.
- **P4 agent-UX.** distinct exit codes (no-op 0 / gate 1 / misuse 2); `--dry-run` mutates nothing;
  errors on stderr with stdout clean; actionable errors carry a runnable `maestro ...` fix; and the
  keystone **self-recovery** check: following the printed fixes actually unblocks the gate.
- **Appendix.** spec-future verbs (`feature new --from-task`, feature archive/unarchive,
  `show --archived`) are emitted as `skip` / `category:not_built` so the suite extends when they land.

Assertions are grounded in real captured CLI output (`SPEC-test-scenarios.md` §5), including
maestro's unicode em dash / arrow / ellipsis -- not the spec's illustrative wording.

## Report schema

Each finding (one NDJSON line):

```json
{"step":"P1.15","phase":"P1 feature->task->QA happy","title":"ship blocks on a live child",
 "status":"pass|fail|skip","severity":"blocking|non_blocking",
 "category":"product_bug|not_built|test_or_env",
 "expected":"...","actual":"...","evidence":"...","fix_area":"src/domain/feature/registry"}
```

Then a final summary object: `{"summary":{"total":N,"pass":N,"fail":N,"skip":N,"blocking_fail":N,...}}`.

- `severity: blocking` -- must be fixed before the next swarm round. `non_blocking` -- UX/cosmetic.
- `category: product_bug` -- a real maestro defect (the loop fixes these). `not_built` -- spec-future,
  skip. `test_or_env` -- a harness or host problem (e.g. missing `jq`), not a maestro bug.
- `fix_area` -- the maestro source area the orchestrator should look at first.

## Swarm usage (the caller's loop, NOT this skill)

```
main session ── cargo build (once per round) ── spawn N sub-agents
     ▲                                               │ each runs this skill in its OWN mktemp repo
     │              NDJSON findings (also written to the target repo file)
     └── fix maestro source for blocking product_bugs ──◄── main aggregates all reports
            then REBUILD and re-swarm ── loop until blocking_fail == 0 across all workers
```

- Workers are isolated: each gets a fresh `mktemp` target repo; the source checkout is read-only
  to the test (only `embedded/` is read).
- Merge by `step` id: the same step failing across workers is one bug. Fix only `product_bug`
  findings; `not_built` and `test_or_env` never gate the loop.
- The report is also a file in each target repo, so a crashed worker still leaves evidence
  (run with `KEEP=1` when investigating).
- Between rounds the orchestrator rebuilds the binary; within a round all workers share the one
  prebuilt binary path.
