# Validation Ladder

The harness-experimental project models verification as a 7-rung ladder. Maestro's canonical verification protocol (`maestro-verify`) covers all 7 rungs but groups them under 6 steps. This doc shows the mapping.

## The 7-Rung Ladder

1. **Format** — code formatting checks (prettier, etc.)
2. **Lint** — static analysis (eslint, architecture lint, etc.)
3. **Type** — type checking (`tsc --noEmit`, etc.)
4. **Integration** — integration tests
5. **E2E** — end-to-end tests, compiled-binary tests
6. **Platform** — platform-specific tests, deploy readiness
7. **Release** — final verdict, release checks

## Mapping to `maestro-verify`

The 6-step ritual covers all 7 rungs:

- **Plan** → Pre-validation (read spec, contracts, prior evidence)
- **Implement** → Code changes
- **Verify** → Rungs 1–5 (format / lint / type / integration / e2e)
- **ProofMap** → Evidence coverage check
- **Verdict** → Rungs 6–7 (platform / release)
- **Branch** → Action based on verdict (merge, rollback, retry)

## Harness-Specific Validation

For `harness-improvement` work types, additional checks apply:

- **Policy schema validation** — `maestro policy check` against `policies/risk.yaml`, `autopilot.yaml`, owners file
- **Skill self-tests** — `bun run check:bundled-skills`, `bun run check:skills`
- **Contract amendment checks** — when a contract changes, ensure the `contract-amendment` evidence is recorded
- **Harness-delta evidence** — record one row per task that touched `.maestro/`, `policies/`, `skills/`, or `hooks/`

## When to use which rung

| Rung | When | Tooling |
|---|---|---|
| Format | Always | `prettier`, project's formatter |
| Lint | Always | `bun run lint:arch`, ESLint where present |
| Type | Always for TS/JS | `tsc --noEmit` (advisory in CI) |
| Integration | Behavior crossing module boundaries | `bun test tests/integration` |
| E2E | User-facing flows, CLI behavior | `bun test tests/e2e` |
| Platform | Cross-platform or deploy-sensitive | OS matrix in CI |
| Release | Before publishing | `bun run ci` (release-prep) |

## Cross-references

- `maestro-verify` skill (canonical protocol)
- `docs/witness-levels.md` (evidence trust ladder)
- `HARNESS.md` (product vs harness deltas)
- `FEATURE_INTAKE.md` (work-type classification)
