# Test Optimization Implementation Prepare Plan

Feature: optimize-test-runtime-and-architecture
SPEC: ./SPEC-test-optimization.md
VERIFY: ./VERIFY-test-optimization.md

## Task T1: Inventory CLI helper variants
check: Inventory covers local maestro/maestro_with*/Command::new(env!("CARGO_BIN_EXE_maestro")) helpers, recording cwd/env/stdin/status/stdout/stderr/timeout/return-type patterns and assigning D2 behavior/cost labels where evidence is available.
covers: ac-1
covers: ac-2

## Task T2: Design provisional CLI harness interface
after: T1
check: Harness interface proposal keeps generic process mechanics separate from domain fixtures, names conservative defaults, and records which existing helper variants it can migrate mechanically.
covers: ac-3
covers: ac-4

## Task T3: Implement minimal CLI process harness and migrate representative tests
after: T2
check: Minimal harness owns command construction plus cwd/env/stdin/status/stdout/stderr capture; migrated representative tests preserve existing assertions and do not move card/session fixture semantics into the harness.
covers: ac-3
covers: ac-4

## Task T4: Record runtime baseline or external blocker
after: T3
blocker: Current production compile errors in src/domain/feature/archive.rs and src/domain/card/store.rs are outside this feature; runtime listing/timing evidence must wait until those blockers clear or be recorded as blocked.
check: `cargo test --workspace -- --list` result is recorded; if it succeeds, runtime/timing baseline evidence is captured, otherwise exact compile blockers are recorded without fixing them in this feature.
covers: ac-2

## Task T5: Complete verification gate and rollout plan
after: T4
check: ./VERIFY-test-optimization.md is updated with evidence for AC1-AC4 and D1-D3; remaining rollout slices and non-goals are explicitly recorded without locking timing thresholds or CI sharding.
covers: ac-1
covers: ac-2
covers: ac-3
covers: ac-4
