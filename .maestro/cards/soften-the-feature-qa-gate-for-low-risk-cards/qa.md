---
amend_log_position: 0
---

### QA Baseline Contract

- Scope: soften-the-feature-qa-gate-for-low-risk-cards — the feature ship gate's
  `qa_declared_none` freshness check in `src/domain/feature/registry.rs`, plus
  the maestro-card skill prose documenting the existing `--qa none --reason`
  no-behavioral-surface accept path.
- Critical workflow chains:
  - qa:none lifecycle: accept --qa none --reason -> start -> amend -> ship
    - Steps: accept with `--qa none --reason` (no qa.md) -> start -> add an
      amend -> `feature ship --dry-run` and read the gap list
    - Touched link: the ship gate reads the `qa: none` declaration freshness
      against the amend log
    - Minimal proof: scratch repo, real `maestro feature` verbs, inspect
      `ship --dry-run` gap output before/after the amend

- Scenario Matrix:
  - [bl-001] Non-behavioral amend does not re-block a qa:none feature at ship (covers: ac-2)
    - Dimensions: state/lifecycle; integration boundary (ship gate x amend log)
    - Setup: scratch repo; feature accepted with `--qa none --reason`,
      then `feature start`, then `feature amend --add-non-goal "<x>" --reason "<y>"`
      (a non-behavioral amend: `is_behavioral()` is false)
    - Action: `maestro feature ship <id> --outcome "<o>" --dry-run`
    - Oracle: the gap list contains NO `qa-baseline` / `qa-slice` line; the only
      remaining gaps are unrelated gates (e.g. the acceptance sweep)
    - Evidence to capture: the `ship --dry-run` gap output, showing no qa gap
    - Reproduction: run the chain above; before the fix the gap "qa-baseline
      missing" appears (proven live 2026-06-17), after the fix it does not
  - [bl-002] Behavioral amend still re-blocks a qa:none feature at ship (covers: ac-3)
    - Dimensions: state/lifecycle; trust/safety (no behavioral dodge)
    - Setup: scratch repo; feature accepted with `--qa none --reason`,
      then `feature start`, then `feature amend --add-acceptance "<new behavior>"
      --reason "<y>"` (a behavioral amend: `is_behavioral()` is true)
    - Action: `maestro feature ship <id> --outcome "<o>" --dry-run`
    - Oracle: the gap list DOES contain a `qa-baseline` line (the stale
      declaration re-arms the gate); shipping is blocked until a qa.md baseline
      is captured
    - Evidence to capture: the `ship --dry-run` gap output, showing the qa gap
    - Reproduction: run the chain above; the qa-baseline gap must appear

- Preserved behaviors:
  - A feature accepted normally (no `--qa none`) still requires a non-empty
    qa.md at accept and a counting slice per `[bl-NNN]` at ship -> Proof:
    `cargo test -p maestro feature::qa` (existing ship_qa_gaps tests stay green)
  - A fresh qa:none feature with no amends still ships with no qa gap -> Proof:
    `feature ship --dry-run` shows no qa line (the ac-1/ac-2 baseline behavior)
- Changed behaviors:
  - The ship gate's `qa_declared_none` freshness moves from amend-count equality
    (`amend_log_position == amends.len()`) to a behavioral-amend check
    (no `is_behavioral()` amend at/after the declaration position), matching
    `ship_qa_gaps` E.1.
- Critical probes before commit:
  - qa.rs / registry.rs unit + integration suite -> `cargo test`
  - the live scratch chain for [bl-001] and [bl-002] on the freshly built binary
- Required artifacts:
  - None (no new files; one-line gate change + skill prose)
- Baseline gaps:
  - None

```yaml
slices:
  - at: "2026-06-17T00:00:00Z"
    scenarios: ["bl-001", "bl-002"]
    probes:
      - "cargo test --test feature_qa_gate_integration"
      - "cargo test --lib feature::qa"
      - "cargo test (full suite)"
    result: pass
    evidence:
      - "bl-001 (real CLI): qa_none_survives_a_non_behavioral_amend_without_redeclaring PASSES -- accept --qa none --reason -> start -> amend --add-non-goal -> ship succeeds with no re-declaration (feature_qa_gate_integration: 5 passed, 0 failed)"
      - "bl-002 (real CLI): qa_none_accept_skips_gates_until_a_behavioral_amend_requires_a_fresh_declaration PASSES -- amend --add-area re-arms the gate, ship blocks on qa-baseline until re-declared (safety preserved)"
      - "unit: feature::qa 18 passed, 0 failed -- incl qa_none_survives_non_behavioral_amend (RED before fix), qa_none_rearms_on_behavioral_amend, qa_none_waives_ship_with_no_amends, qa_none_fresh_requires_surface_none"
      - "full suite: cargo test 936 passed, 0 failed, CARGO_EXIT=0"
```
