# TDD anti-patterns

Apply [Testing discipline](~/maestro/WORKFLOW.md#testing-discipline) before
using this catalog. These are diagnostic examples for a concrete concern,
not a checklist requiring new tests or decision records for every task.

| Anti-pattern | Smell on the diff | Fix |
|---|---|---|
| Contract minting | Test asserts behavior outside acceptance | Resolve the material contract gap; do not let the test authorize scope |
| Spec by accident | Test freezes an arbitrary internal detail (exact wording, ordering) nobody chose | Assert the accepted contract rather than incidental structure |
| Guessed seam | Test drives an internal function while CLI/API behavior is undecided | Resolve the observable outcome first |
| Internals lock-in | Test reaches into private state instead of exercising the contract | Prefer a stable consumer seam |
| Mock theater | Test asserts a mock was called with the mocked value | Assert observable output at the real seam |
| Tautology | Expected value computed by the same code path as the actual | Derive the expected result independently |
| Test-pleasing code | Production code grows branches solely to satisfy a wrong expectation | Correct the test openly against acceptance |
| Goodhart anchoring | Work optimizes for existing test wording, not the behavior it represents | Re-read the contract before extending the check |
| Silent test shift | Assertion changes without an explanation of the contract or measurement correction | Record the reason; a decision is needed only for a material choice |
| Silent test delete | Failing test removed instead of the defect fixed | Fix the behavior or obtain approval for a contract change |
| Junk evidence | Claim/proof uses placeholders ("test: a", "p1") | Name the check that would fail if the claim were wrong |
| Forced green | Work marked done while acceptance still fails | Report the failure and keep the acceptance open |
| Over-broad red | One test bundles several undecided behaviors | Settle the blocking behavior; defer unrelated questions |
| Snapshot everything | Entire-output snapshot where only one field is contractual | Assert the relevant field; use snapshots for meaningful wholes |
| Flake tolerance | Retries or sleeps added until the test happens to pass | Investigate the race or environment with new evidence |
| Coverage chasing | Tests added solely to raise a number | Stop unless there is an uncovered in-scope behavior or risk |
| Fixture drift | Setup re-implements production logic and diverges | Reuse fixtures or exercise the existing consumer path |
| Assertion-free | Check cannot distinguish a plausible incorrect result | Assert the observable result and demonstrate sensitivity to the defect |
| Test-driven API invention | Missing symbol in a test becomes an unapproved public API | Stabilize the minimum contract before implementing the symbol |
| Contract drift | Spec says A, code does B, test asserts C | Resolve the contradiction rather than treating current output as authority |

On API invention: red tests may discover implementation, but must not invent
an unstabilized contract. Agents are especially prone here — a human reading a
sketched API in a test knows it is a sketch; an agent treats the compile
failure as work and bridges it by creating whatever symbols the test names.
Each mint is a premature commitment later tasks start depending on, and every
task can stay green while the architecture drifts (local convergence, global
divergence). If the contract is unresolved, stop at the boundary rather than
letting the test define it.
