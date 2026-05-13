# Trust Verifier

`maestro task verify` runs 8 deterministic checks against the current diff
and the locked contract.

## Checks

| Check | What it catches |
|---|---|
| `non-empty-diff` | Empty diff against the resolved base |
| `scope` | Changed paths outside `contract.scope.filesExpected` |
| `lockfile` | Lockfile edited when the contract does not permit it |
| `generated` | Generated files hand-edited |
| `sensitive-paths` | Paths matched by `policies/sensitive-paths.yaml` |
| `commit-metadata` | Commit messages not following Conventional Commits |
| `secrets` | Secret-like strings introduced in the diff |
| `architecture-lints` | Repo-shape invariants (no-runner-inversion, single-opentui-render, mission-control-readonly, no-hand-edit-generated) |

Each finding carries severity `error`, `warn`, or `info`. Address every
`error` finding before requesting a Verdict.

## Architecture-lint rules

The `architecture-lints` group enforces:

- `no-runner-inversion` (error): Maestro must not spawn Claude or Codex CLIs as subprocesses.
- `single-opentui-render` (error): `root.render()` may be called at most once per process.
- `mission-control-readonly` (warn): Mission Control snapshot/preview/render-check paths must not write.
- `no-hand-edit-generated` (error): generated template files require a matching edit under `skills/built-in/**` or `skills/bundled/**`.

See `docs/architecture-lints.md` for full rule semantics, escape-hatch
syntax, and instructions for adding new rules. Run `bun run lint:arch` to
invoke the same library standalone.

## Invocation

```bash
maestro task verify --task <id>
maestro task verify --task <id> --base <git-ref>   # explicit base
maestro task verify --task <id> --json             # machine-readable output
```

Example output:

```
Trust Verifier: 2 findings (1 error, 1 warning, 0 info)
  [error] scope: src/features/auth/secret.ts, src/features/auth/utils.ts
  [warn]  commit-metadata
    Commit "wip" does not match Conventional Commits format
```

Exit codes: `0` = no findings, `1` = at least one error, `2` = warnings or
info only.
