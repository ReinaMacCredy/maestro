# owners.yaml — Schema Reference

`.maestro/policies/owners.yaml` defines decision-authority roles for Maestro
policy enforcement. It is a committed, repo-tracked file bootstrapped by
`maestro init`.

## Location

```
.maestro/policies/owners.yaml
```

## Schema

The file is a YAML object with three optional top-level keys. Each key maps to
a list of GitHub usernames or team handles (e.g., `@org/team`). All keys are
optional; an absent or empty list defaults to "any maintainer."

```yaml
policy_approver: []      # required to approve changes to .maestro/policies/ (L3+)
ratchet_approver: []     # required to approve ratchet promotions (L7+)
sensitive_waiver: []     # required to sign off on sensitive-path changes (L5+)
```

### `policy_approver`

Users or teams authorized to approve changes to policy files under
`.maestro/policies/`. Enforced at L3 and above. A PR that modifies a policy
file without an approving review from this list is blocked until one is
provided.

### `ratchet_approver`

Users or teams authorized to approve ratchet promotions — moving a ratchet rule
from narrow scope to broad scope, or from advisory to gating. Enforced at L7
and above. Ratchet promotions that widen scope can affect every future PR in
the repository, so the approval bar is intentionally higher.

### `sensitive_waiver`

Users or teams authorized to sign off on changes to paths that match
`.maestro/policies/sensitive-paths.yaml` globs. Enforced at L5 and above. At
L2 the `sensitive-paths` check is advisory (`warn` severity) and no waiver is
required; the finding appears in `maestro task verify` output but does not gate
completion.

## Example

```yaml
policy_approver:
  - "@acme/platform-owners"
  - alice

ratchet_approver:
  - "@acme/security"

sensitive_waiver:
  - "@acme/security"
  - bob
```

## Bootstrap Exemption

When `owners.yaml` contains only empty lists (as bootstrapped by `maestro init`),
Maestro treats role membership as open — any maintainer is implicitly authorized.
This is the expected state at L2, where role enforcement is not yet active.

At higher levels (L3 for `policy_approver`, L5 for `sensitive_waiver`, L7 for
`ratchet_approver`), empty lists mean the gate can never be satisfied by a named
individual. Fill in the lists before enabling the corresponding enforcement level.

## `gh` CLI Resolution

At L3 and above, when a role list contains a GitHub team handle
(e.g., `@org/team`), Maestro attempts to resolve team membership via the `gh`
CLI. If `gh` is not installed or not authenticated, team-handle resolution falls
back to raw string comparison. Individual usernames are always compared as
plain strings. At L2, resolution is raw string comparison only — `gh` is never
invoked.

## Validation

The `loadOwners` use-case (`src/features/policy/usecases/load-owners.usecase.ts`)
validates the file on every read:

- The file must exist at `.maestro/policies/owners.yaml`.
- The file must be valid YAML.
- The top-level value must be an object (not an array or scalar).
- Each present role key must be a list of strings.

Malformed files cause `maestro task verify` and other policy-dependent commands
to fail with a descriptive error pointing at the offending field.
