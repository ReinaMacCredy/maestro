# tdd skill adoption

2026-06-12  dec-tdd-content-placement-fold-into-maestro-660c locked -- TDD content placement: fold into maestro-card reference
2026-06-12  dec-tdd-trigger-default-test-first-for-b8df locked -- TDD trigger: default test-first for implement cards
2026-06-12  dec-tdd-fidelity-full-6-file-verbatim-port-d59f locked -- TDD fidelity: full 6-file verbatim port
2026-06-12  Loose question (methodology vs harness-only bundle) resolved by dec-tdd-content-placement / dec-tdd-fidelity: maestro carries methodology only as a maestro-card reference, never as standalone bundled skills. Implementation defaults noted: MIT attribution as a header line in tdd.md; maestro-card SKILL.md version bump (1.0.0 -> 1.1.0) + extraction guard re-record are part of the change, per established skill-edit constraint.
2026-06-12  2026-06-12 authorization: user said 'ship it' — implement and ship this feature on branch card-model with local commits (no push). Constraint: skip qa-baseline ('skip qa baseline in this') — accept with --qa none; feature ships static skill reference content, runtime extraction behavior is covered by existing extraction guard tests.
2026-06-12  Regression caught by full suite: tests/skills_extract.rs:178 pins the maestro-card SKILL.md version literal (known gotcha alongside resources_version_guard). Fixed by updating the pin 1.0.0 -> 1.1.0 — that is the pin's documented purpose (acknowledge user-visible skill changes), not a softened assertion. skills_extract now 21/21.
