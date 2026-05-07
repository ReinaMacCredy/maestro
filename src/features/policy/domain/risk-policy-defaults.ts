import type { RiskPolicy } from "./policy-types.js";

/**
 * Default risk policy from ROADMAP §"Risk Class Enumeration".
 * Rows are evaluated in order; first match wins.
 */
export const DEFAULT_RISK_POLICY: RiskPolicy = {
  kind: "risk",
  id: "risk-policy-default",
  description: 'Default risk policy from ROADMAP §"Risk Class Enumeration"',
  version: "1",
  rows: [
    {
      signal: "diff-intersects-sensitive-security",
      derivedClass: "critical",
      description:
        "Diff intersects sensitive_paths.security set (auth/**, secrets/**, permissions/**, payments/**)",
    },
    {
      signal: "diff-modifies-dependency-manifests",
      derivedClass: "high",
      description:
        "Diff modifies dependency manifests (package.json, bun.lock, Cargo.toml, requirements.txt, etc.)",
    },
    {
      signal: "diff-modifies-migrations",
      derivedClass: "high",
      description: "Diff includes database migration files (paths matching policies/migration_paths)",
    },
    {
      signal: "diff-modifies-ci-workflows",
      derivedClass: "high",
      description:
        "Diff modifies CI workflow files (.github/workflows/**, .circleci/**, .gitlab-ci.yml)",
    },
    {
      signal: "diff-modifies-policy-files",
      derivedClass: "high",
      description: "Diff modifies policies/, ratchets/, or owners.yaml in .maestro/",
    },
    {
      signal: "diff-modifies-build-config",
      derivedClass: "medium",
      description:
        "Diff modifies build configuration (tsconfig.json, bunfig.toml, vite.config.*, etc.)",
    },
    {
      signal: "diff-source-only",
      derivedClass: "medium",
      description: "Any source code change not matched by the above rows (default for source changes)",
    },
    {
      signal: "diff-docs-only",
      derivedClass: "low",
      description: "Diff is docs-only, comment-only, or formatting-only",
    },
  ],
};
