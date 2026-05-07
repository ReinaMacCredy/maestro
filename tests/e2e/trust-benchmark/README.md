# Trust Benchmark — Edge-Case Mitigation Corpus

Seed of 9 of 32 edge cases. The corpus grows demand-driven (one PR per new scenario). Not a complete benchmark — the full "20+ scenarios" vision is intentionally not blocked on this slice. Stability matters more than breadth: nine well-tested scenarios beat twenty hand-waved ones.

## Scenarios

| File | Edge case | Mitigation under test |
|------|-----------|----------------------|
| `ec05-out-of-scope.test.ts` | EC 5 (out-of-scope harmless) | Trust Verifier scope check at L2.3 |
| `ec06-generated-drift.test.ts` | EC 6 (generated drift) | Generated-file parity at L2.3 |
| `ec09-sensitive-path.test.ts` | EC 9 (sensitive path) | `forbidden_paths` + `sensitive-paths.yaml` |
| `ec12-security-thin.test.ts` | EC 12 (security thin) | Threat-model required predicate at L4 |
| `ec22-amendment-creep.test.ts` | EC 22 (amendments hide creep) | Amendment-budget rules 3–7 at L2 |
| `ec23-proof-not-tied.test.ts` | EC 23 (proof not tied) | ProofMap at L3.5 |
| `ec27-rebase-squash.test.ts` | EC 27 (rebase/squash) | Tree-SHA verdict identity at L5.3 |
| `ec31-decision-authority.test.ts` | EC 31 (decision authority) | `owners.yaml.deploy_approver` at L7.9 |
| `ec32-self-weakening.test.ts` | EC 32 (PR self-weakening) | Rule 12 base-branch reading at L5.2 |

## How to run

```bash
bun test tests/e2e/trust-benchmark/
```

Each test file calls `buildCompiledCli` in `beforeAll`, so the first run also builds `dist/maestro`. Subsequent runs in the same process reuse the cached binary.

## How to add a scenario

1. Name the file `ec<NN>-<slug>.test.ts` where `<NN>` is the edge-case number from the master list.
2. Every file **must** include both a positive assertion (mitigation fires when the trigger is present) and a negative assertion (mitigation does not fire when the trigger is absent). Both can be separate `it(...)` blocks or two assertions inside one block.
3. Follow the existing fixture pattern: `mkdtemp` → `initGitRepo` → `runCompiled(["init"])` → write `.maestro/` fixtures → drive the binary → assert exit code + JSON output.
4. Use helpers from `../../helpers/run-compiled-cli.js` and `../../helpers/command-runner.js`.

## EC 26

EC 26 (cross-task conflict) lives in `tests/e2e/l8-cross-task-conflict.test.ts`, not here. It depends on the `cross-task-conflict` Evidence kind and related infrastructure shipped in L8.1.
