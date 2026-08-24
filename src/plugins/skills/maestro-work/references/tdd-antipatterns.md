# TDD anti-patterns

Each entry traces to one of the five root laws in SKILL.md and is checkable on
a concrete diff or test. Curation rule: an entry that cannot fail a review of
a real diff is a slogan and gets cut; the catalog stays at or under 20 entries.

| Anti-pattern | Smell on the diff | Fix |
|---|---|---|
| Contract minting (law 1) | Red test asserts behavior no decision or acceptance names | Return to design; decide the seam, then transcribe it |
| Spec by accident (law 1) | Test freezes an arbitrary implementation detail (exact wording, ordering) nobody chose | Assert only the decided contract; loosen the rest |
| Guessed seam (law 1) | Test drives an internal function while the CLI/API behavior is still undecided | Spike without tests, decide, then test the seam |
| Internals lock-in (law 2) | Test imports private modules or reaches into state instead of the public surface | Rewrite against the outermost stable seam |
| Mock theater (law 4) | Test asserts a mock was called with the mocked value | Assert observable output at the real seam |
| Tautology (law 4) | Expected value computed by the same code path that produces the actual | Hardcode the expected literal from the decision |
| Test-pleasing code (law 3) | Production code grows branches that only test inputs can reach | Fix or shift the test openly; delete the branch |
| Goodhart anchoring (law 3) | Later work optimizes for the existing test wording, not the behavior it stands for | Re-read the decision behind the test before extending it |
| Silent test shift (law 5) | Assertion changed in the same commit as the code it verifies, no reason recorded | Split the shift out; record it with a decision |
| Silent test delete (law 5) | Failing test removed instead of the defect fixed | Fix the behavior, or record the deliberate contract change |
| Junk evidence (laws 4, 5) | Claim/proof filled with placeholders ("test: a", "p1") to pass a gate | Name the real falsifier: the check that fails if the claim is wrong |
| Forced green (law 3) | Work marked done while its verification still fails | Report the failure; done only after the falsifier passes |
| Over-broad red (law 1) | One red test bundles several undecided behaviors | One behavior, one test; split the rest into their own decisions |
| Snapshot everything (law 4) | Golden-file snapshot of an entire output where one field was decided | Assert the decided field; keep snapshots for decided wholes |
| Flake tolerance (law 4) | Retries or sleeps added until the test passes | Remove the race at the seam or test a deterministic surface |
| Coverage chasing (law 1) | Tests added purely to raise a coverage number, asserting nothing decided | Delete or replace with a falsifiable behavior test |
| Fixture drift (law 2) | Test setup re-implements production logic and diverges from it | Build fixtures through the public seam the user would use |
| Assertion-free run (law 4) | Test executes code and asserts only that no exception was thrown | Assert the observable result the decision names |
| Test-driven API invention (law 1) | Red test names a production symbol that does not exist; the "fix" mints it to clear the compile error | Stabilize the minimum contract first (spec, existing code, or a decision — stable enough for this slice); red must fail on an assertion, not a missing symbol |
| Contract drift (law 1) | Spec says A, existing code does B, the test asserts a third shape C and implementation follows C | Reconcile with a recorded decision before writing the test; the test transcribes, never arbitrates |

Seed specimens from the live-agent loop on this repo: a junk claim/proof pair
("test: a" / "p1") passed the proof gate until the pair-check tightened; a
round-4 stop-condition forced a false "done" claim the adjudicator had to
revert. Both are the Junk evidence and Forced green rows above.

On API invention: red tests may discover implementation, but must not invent
an unstabilized contract. Agents are especially prone here — a human reading a
sketched API in a test knows it is a sketch; an agent treats the compile
failure as work and bridges it by creating whatever symbols the test names.
Each mint is a premature commitment later tasks start depending on, and every
task can stay green while the architecture drifts (local convergence, global
divergence). If the contract is unresolved, stop at the boundary rather than
letting the test define it.
