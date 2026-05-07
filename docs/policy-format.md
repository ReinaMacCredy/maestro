# Policy File Format

Maestro uses five policy files committed under `.maestro/policies/`. All are
YAML; all are repo-tracked except `.pending-loosenings.json` (a derived cache).
Absent files fall back to built-in defaults where stated.

## `risk.yaml`

**Purpose:** Extends or replaces the ROADMAP-default signal-to-class mapping
used by the Risk Engine. Rows are evaluated in order; first match wins.
Absent means the ROADMAP defaults apply unchanged.

**Loading code:** `src/features/policy/usecases/load-risk-policy.usecase.ts`

**Schema:**

```typescript
interface RiskPolicyRow {
  signal: string;       // signal identifier matched by the Risk Engine
  derivedClass: "low" | "medium" | "high" | "critical";
  description?: string; // human note only
}

interface RiskPolicy {
  kind: "risk";
  id: string;
  description?: string;
  version: string;
  rows: RiskPolicyRow[];
}
```

**Example `risk.yaml`:**

```yaml
kind: risk
id: risk-policy-custom
version: "1"
rows:
  - signal: diff-intersects-sensitive-security
    derivedClass: critical
    description: "auth/**, secrets/**, permissions/**, payments/**"
  - signal: diff-modifies-dependency-manifests
    derivedClass: high
  - signal: diff-modifies-migrations
    derivedClass: high
  - signal: diff-modifies-ci-workflows
    derivedClass: high
  - signal: diff-modifies-policy-files
    derivedClass: high
  - signal: diff-modifies-build-config
    derivedClass: medium
  - signal: diff-source-only
    derivedClass: medium
  - signal: diff-docs-only
    derivedClass: low
```

**Notes:**
- Custom rows can only raise the derived class for a signal match; they cannot
  lower it below the ROADMAP defaults (Rule 12).
- See `docs/risk-class-derivation.md` for the normative signal-to-class table.

---

## `autopilot.yaml`

**Purpose:** Per-risk-class configuration for auto-pass eligibility and the
minimum witness level required before the verdict engine will issue a `PASS`
without human review.

**Loading code:** `src/features/policy/usecases/load-autopilot-policy.usecase.ts`

**Schema:**

```typescript
interface AutopilotPolicy {
  kind: "autopilot";
  id: string;
  description?: string;
  version: string;
  autoMergeAllowed: {
    low: boolean;
    medium: boolean;
    high: boolean;
    critical: boolean;
  };
  requiredWitnessLevel: {
    low: WitnessLevel;
    medium: WitnessLevel;
    high: WitnessLevel;
    critical: WitnessLevel;
  };
}
// WitnessLevel = "witnessed-by-maestro" | "witnessed-by-ci"
//              | "agent-claimed-locally" | "agent-claimed-and-not-reproducible"
```

**Example `autopilot.yaml`:**

```yaml
kind: autopilot
id: autopilot-policy-default
version: "1"
autoMergeAllowed:
  low: true
  medium: true
  high: false
  critical: false
requiredWitnessLevel:
  low: agent-claimed-locally
  medium: agent-claimed-locally
  high: witnessed-by-maestro
  critical: witnessed-by-maestro
```

**Notes:**
- If any evidence row for an acceptance criterion is below `requiredWitnessLevel`
  for the effective risk class, the verdict downgrades from `PASS` to `HUMAN`.
- `autoMergeAllowed: false` for a risk class means the verdict always returns
  `HUMAN` regardless of evidence witness level.
- See `docs/witness-levels.md` for witness level semantics.

---

## `release.yaml`

**Purpose:** Release-gate rules that apply when a release commit is stamped.
Requires a complete ProofMap and/or signed commits at configurable risk classes.

**Loading code:** `src/features/policy/usecases/load-release-policy.usecase.ts`

**Schema:**

```typescript
interface ReleasePolicy {
  kind: "release";
  id: string;
  description?: string;
  version: string;
  requireSignedCommits: boolean;
  requireProofMapComplete: boolean;
}
```

**Example `release.yaml`:**

```yaml
kind: release
id: release-policy-default
version: "1"
requireSignedCommits: false
requireProofMapComplete: true
```

---

## `sensitive-paths.yaml`

**Purpose:** Glob list of paths that trigger the `checkSensitivePaths` Trust
Verifier finding when a diff touches them.

**Loading code:** `src/features/verify/` (loaded as part of the Trust Verifier;
not a standalone policy loader in `src/features/policy/`).

**Schema:** A flat list of glob strings under the `paths` key.

**Example `sensitive-paths.yaml`:**

```yaml
paths:
  - "auth/**"
  - "secrets/**"
  - "permissions/**"
  - "payments/**"
  - ".env*"
  - "**/*.pem"
  - "**/*.key"
  - "**/credentials*"
```

For the 8 default globs and guidance on extending or relaxing them, see
`docs/sensitive-paths-defaults.md`.

---

## `owners.yaml`

**Purpose:** Decision-authority roles. Role lists contain GitHub usernames or
team handles. Empty lists default to "any maintainer".

**Loading code:** `src/features/policy/usecases/load-owners.usecase.ts`

**Schema:**

```typescript
interface OwnersYaml {
  policy_approver?: string[];   // approves changes to .maestro/policies/ (L3+)
  ratchet_approver?: string[];  // approves ratchet promotions (L7+)
  sensitive_waiver?: string[];  // signs off on changes to sensitive paths (L5+)
}
```

**Example `owners.yaml`:**

```yaml
policy_approver:
  - octocat
  - "@myorg/platform-team"
ratchet_approver: []
sensitive_waiver:
  - octocat
```

For the full schema reference and role semantics, see `docs/owners-yaml-format.md`.

---

## Asymmetric editing and the soak window

Policy tightenings (stricter rules, lower budgets, narrower auto-merge
eligibility) take effect immediately. Policy loosenings soak for 30 days before
becoming effective.

The classifier lives at
`src/features/policy/usecases/classify-policy-edit.usecase.ts`. Pending
loosenings are cached in `.maestro/policies/.pending-loosenings.json`
(gitignored). Use `maestro policy pending` to inspect the soak queue, and
`maestro policy check --task <id>` to see which rules apply to the current
contract and diff.

## See also

- `docs/risk-class-derivation.md` — signal-to-class mapping table.
- `docs/witness-levels.md` — witness level ladder and autopilot interaction.
- `docs/sensitive-paths-defaults.md` — default sensitive-path globs.
- `docs/owners-yaml-format.md` — owners schema and role semantics.
