# Architecture-lint and Trust Verifier

Two distinct check sets: `maestro task verify` runs the lightweight
architecture-lint corpus; `maestro verdict request` runs the heavier Trust
Verifier (8 deterministic checks) plus ProofMap and policy.

## Architecture-lint (run by `task verify`)

Enforces the layered architecture and the passive-harness invariant
against `src/{config,providers,repo,runtime,service,types,ui}/**`. Findings
are written as `lint-violation` evidence rows.

Rules currently shipped:

- `layer-order` (error) — imports must flow forward along the layer chain
  `types -> config -> repo -> service -> runtime -> ui`. `providers` is
  cross-cutting and may import from any layer.
- `passive-harness` (error) — no scheduler / background-work tokens
  (`setInterval`, `setTimeout`, `cron`, `daemon`, …) in the layered tree.

See `docs/architecture.yaml` for the configuration source, and
`docs/architecture-lints.md` for adding new rules. `bun run lint:arch`
invokes the same library standalone.

### Invocation

```bash
maestro task verify <id>
maestro task verify <id> --json
```

Exit codes: `0` PASS (auto-advanced to `ready`), `1` FAIL (one or more
`error` findings), `2` HUMAN, `3` BLOCK. `task verify` does not run Trust
Verifier — request a Verdict when you are ready for the full check.

## Trust Verifier (run by `verdict request`)

8 deterministic checks against the current diff and the locked contract:

| Check | What it catches |
|---|---|
| `non-empty-diff` | Empty diff against the resolved base |
| `scope` | Changed paths outside `contract.scope.filesExpected` |
| `lockfile` | Lockfile edited when the contract does not permit it |
| `generated` | Generated files hand-edited |
| `sensitive-paths` | Paths matched by `policies/sensitive-paths.yaml` |
| `commit-metadata` | Commit messages not following Conventional Commits |
| `secrets` | Secret-like strings introduced in the diff |
| `architecture-lints` | Re-runs the lint set above so verdict consolidates the full picture |

Each finding carries severity `error`, `warn`, or `info`. Trust Verifier
results feed the verdict's `reasons[]` array — see `reference/verdict.md`
for the decision tree.
