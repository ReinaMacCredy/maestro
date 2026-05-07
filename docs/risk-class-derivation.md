# Risk Class Derivation

The Risk Engine derives a risk class from deterministic diff signals and then
takes the **higher** of the agent-proposed class and the Maestro-derived class.
Per Rule 1, an LLM-proposed class can never lower the Maestro-derived class.
This closes the path where an agent could propose `medium` to qualify for
auto-merge despite touching auth code.

The derived class drives auto-pass eligibility, required reviewer set, required
witness levels, and rollback requirements.

## Risk Class Enumeration

`Contract.risk_class` and `Verdict.risk_class` use a four-level scale. The
agent proposes a class during planning; Maestro can raise it (per Rule 1).

```text
low      : isolated change, no sensitive paths, no production behavior shift, easily reversible.
           Eligible for L6 auto-merge when L6 is shipped.

medium   : touches non-trivial logic, reversible, no sensitive paths.
           Default for most tasks. L5 default; L6 only if all gating evidence is
           witnessed-by-maestro or witnessed-by-ci.

high     : touches sensitive paths, alters production behavior, or has
           hard-to-reverse data effects. L5 with required human review;
           L6 ineligible by default.

critical : touches auth, payments, secrets, migrations, dependency manifests,
           CI workflows, or permission model. Always human review at L5;
           L6 ineligible regardless of evidence; L7 requires witnessed
           rollback per Rule 10.
```

## Signal-to-class mapping

The following table is the normative implementation spec for
`deriveRiskClassFromDiff`. Signals are evaluated in order; the first match wins.
The table is configurable via `.maestro/policies/risk.yaml` so teams can extend
or tighten the defaults; absent `risk.yaml` means this table applies as-is.

```text
Signal                                                              Derived class
---                                                                 ---
Diff intersects sensitive_paths.security set                        critical
  (auth/**, secrets/**, permissions/**, payments/**)
Diff modifies dependency manifests                                  high
  (package.json, bun.lock, Cargo.toml, requirements.txt, etc.)
Diff includes database migration files                              high
  (paths matching policies/migration_paths)
Diff modifies CI workflow files                                      high
  (.github/workflows/**, .circleci/**, .gitlab-ci.yml)
Diff modifies policies/, ratchets/, or owners.yaml in .maestro/    high
Diff modifies build configuration                                   medium
  (tsconfig.json, bunfig.toml, vite.config.*, etc.)
Any source code change not matched by the above rows              medium  (default)
Diff is docs-only, comment-only, or formatting-only                low
```

> The deriver does not heuristically classify changes as "trivial" or
> "non-trivial" — that requires LLM judgment and would violate Rule 1. The
> default is `medium` for any source change not matched by a prior row; `low` is
> reserved for docs/comments/formatting-only diffs (deterministically detectable
> via file extension and/or AST-vs-comment-only diff).

## Implementation

The Risk Engine lives at `src/features/risk/`:

- `usecases/derive-risk-class.ts` — applies the signal-to-class table above.
- `usecases/compute-risk.ts` — orchestrates full risk computation including
  the comparison between agent-proposed class and Maestro-derived class.
- `usecases/risk-class-order.ts` — utility for comparing and taking the higher
  of two risk class values.

## Extending the table

Teams extend the signal-to-class mapping by editing
`.maestro/policies/risk.yaml`, not this doc. Rules added in `risk.yaml` can
only raise the derived class for a signal match; they cannot lower it below the
ROADMAP defaults (Rule 12). See `docs/policy-format.md` for the `risk.yaml`
schema.

## See also

- `docs/witness-levels.md` — how the derived risk class gates required witness levels.
- `docs/policy-format.md` — schema for `risk.yaml` and `autopilot.yaml`.
