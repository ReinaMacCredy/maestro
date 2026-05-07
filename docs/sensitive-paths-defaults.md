# Sensitive Paths — Default Globs

Maestro's Trust Verifier runs a `checkSensitivePaths` check as one of its six
parallel checks during `maestro task verify`. The check reads globs from
`.maestro/policies/sensitive-paths.yaml` and emits a `warn` finding for every
diff path that matches.

`maestro init` bootstraps `.maestro/policies/sensitive-paths.yaml` with 8
default globs. This file documents each glob, explains why it is sensitive, and
describes how teams should extend or relax the list.

## Default Globs

### `src/auth/**`

Authentication logic: session handling, token issuance, credential validation,
and privilege escalation paths. Changes here can silently widen attack surface
or break existing auth flows. Any diff touching auth code should receive extra
review regardless of how small the change appears.

### `src/payments/**`

Payment processing, billing integrations, and financial data flows. Mistakes
in payment code can cause data loss, double-charges, or compliance violations.
Treat as sensitive even when the change is a refactor.

### `**/secrets/**`

Any directory named `secrets` in the tree. Directories with this name typically
hold credentials, API keys, or encryption material. Even a rename or a comment
change in this area is worth flagging so reviewers can confirm no key material
is being added to the diff.

### `package.json`

The dependency manifest for Node/Bun projects. A change to `package.json`
without a corresponding lockfile update can introduce phantom dependencies or
leave the lockfile out of sync with reality. The `checkLockfileParity` check
also fires on `package.json` diffs, but `sensitive-paths` provides an
independent advisory layer that fires even when parity is satisfied.

### `bun.lock`

The Bun lockfile. Lockfile changes are high-impact: they affect every developer
and every CI run. A `bun.lock` diff that appears in isolation (without
`package.json`) is unusual and worth reviewing. Teams that run automatic
lockfile updates via CI may want to relax this glob or limit it to production
dependency sections.

### `.github/workflows/**`

GitHub Actions workflow files. CI configuration controls what runs against
every commit and PR, and workflow files can be used to exfiltrate secrets or
bypass merge gates. Any change to a workflow file should be reviewed with the
same care as a code change.

### `**/migrations/**`

Database migration files. Migrations are append-only by convention and hard to
reverse in production. A change inside a committed migration file is almost
always a mistake; a new migration that modifies shared schema needs careful
review for compatibility with in-flight deployments.

### `**/permissions/**`

Permission and authorization configuration. Files under a `permissions/`
directory typically define who can do what. Silent changes here are high-risk
even when the diff looks trivial.

## Extending the Default List

Add additional globs to `.maestro/policies/sensitive-paths.yaml` under the
`paths` key:

```yaml
paths:
  - "src/auth/**"
  - "src/payments/**"
  - "**/secrets/**"
  - "package.json"
  - "bun.lock"
  - ".github/workflows/**"
  - "**/migrations/**"
  - "**/permissions/**"
  - "infra/terraform/**"   # example: add your own
```

Globs follow the same matching rules as Bun's `Glob` class (micromatch-compatible).
All paths in the diff are normalized to forward slashes before matching.

## Relaxing the Default List

Remove any glob that does not apply to your repository. For example, a pure
frontend project with no backend auth module can remove `src/auth/**`. Edit the
file directly — there is no CLI command for policy relaxation at L2.

Per Rule 9 (Asymmetric policy editing): tightening a policy (adding globs or
stricter rules) takes effect immediately. Loosening a policy (removing or
weakening globs) requires a soak period at L3 and above before the change gates
on CI. At L2 the file is advisory only — findings from `checkSensitivePaths`
are `warn` severity, not `error`, so they surface in `maestro task verify`
output without blocking completion.

## Sensitive-path Waivers (L5+)

At L5 and above, touching a path that matches a sensitive-path glob requires a
`sensitive_waiver` Evidence row signed by a principal in the `sensitive_waiver`
role from `.maestro/policies/owners.yaml`. At L2 the check is advisory; no
waiver is required, but the finding is written to the verifier output so
reviewers see which sensitive paths were touched.
