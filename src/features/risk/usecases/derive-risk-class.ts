import { matchesAnyGlob } from "@/shared/lib/glob-match.js";
import { DEFAULT_RISK_POLICY } from "@/features/policy/index.js";
import type { RiskPolicy } from "@/features/policy/index.js";
import type { DerivedRiskInput, DerivedRiskResult } from "../domain/types.js";

// Dependency manifests — exhaustive list of well-known lock/manifest filenames.
const MANIFEST_GLOBS: readonly string[] = [
  "package.json",
  "bun.lock",
  "bun.lockb",
  "pnpm-lock.yaml",
  "yarn.lock",
  "package-lock.json",
  "Cargo.toml",
  "Cargo.lock",
  "pyproject.toml",
  "requirements*.txt",
  "Pipfile",
  "Pipfile.lock",
  "Gemfile",
  "Gemfile.lock",
  "go.mod",
  "go.sum",
];

// CI workflow directories (standard platforms).
const CI_WORKFLOW_GLOBS: readonly string[] = [
  ".github/workflows/**",
  ".circleci/**",
  ".gitlab-ci.yml",
];

// Policy / ratchet paths inside .maestro/.
const POLICY_GLOBS: readonly string[] = [
  ".maestro/policies/**",
  ".maestro/ratchets/**",
  ".maestro/owners.yaml",
];

// Build config files.
const BUILD_CONFIG_GLOBS: readonly string[] = [
  "tsconfig*.json",
  "jsconfig*.json",
  "bunfig.toml",
  "vite.config.*",
  "vitest.config.*",
  ".eslintrc*",
  ".eslintrc.js",
  ".eslintrc.cjs",
  ".eslintrc.mjs",
  ".eslintrc.json",
  ".eslintrc.yaml",
  ".eslintrc.yml",
  "eslint.config.*",
  "babel.config.*",
  ".babelrc*",
  "webpack.config.*",
  "rollup.config.*",
  "esbuild.config.*",
  "jest.config.*",
];

// Docs-only extensions and patterns.
const DOCS_GLOBS: readonly string[] = [
  "**/*.md",
  "**/*.txt",
  "LICENSE",
  "LICENSE.*",
  "**/CHANGELOG*",
  "**/CHANGELOG.*",
  "**/.gitignore",
  "**/.gitattributes",
  "**/*.rst",
  "**/*.adoc",
];

// Default migration path globs (override via DerivedRiskInput.migrationPaths).
const DEFAULT_MIGRATION_GLOBS: readonly string[] = [
  "migrations/**",
  "db/migrations/**",
];

function matchesSignal(signal: string, input: DerivedRiskInput): boolean {
  const { changedPaths } = input;

  switch (signal) {
    case "diff-intersects-sensitive-security": {
      const globs = input.sensitivePathsPolicy ?? [];
      if (globs.length === 0) return false;
      return changedPaths.some((p) => matchesAnyGlob(globs, p));
    }

    case "diff-modifies-dependency-manifests":
      return changedPaths.some((p) => matchesAnyGlob(MANIFEST_GLOBS, p));

    case "diff-modifies-migrations": {
      const globs = input.migrationPaths ?? DEFAULT_MIGRATION_GLOBS;
      return changedPaths.some((p) => matchesAnyGlob(globs, p));
    }

    case "diff-modifies-ci-workflows":
      return changedPaths.some((p) => matchesAnyGlob(CI_WORKFLOW_GLOBS, p));

    case "diff-modifies-policy-files":
      return changedPaths.some((p) => matchesAnyGlob(POLICY_GLOBS, p));

    case "diff-modifies-build-config":
      return changedPaths.some((p) => matchesAnyGlob(BUILD_CONFIG_GLOBS, p));

    case "diff-source-only":
      // Matches anything that is not docs-only. Used as the medium fallback.
      return !changedPaths.every((p) => matchesAnyGlob(DOCS_GLOBS, p));

    case "diff-docs-only":
      return changedPaths.length > 0 && changedPaths.every((p) => matchesAnyGlob(DOCS_GLOBS, p));

    default:
      return false;
  }
}

/**
 * Deterministically derives a RiskClass from diff signals.
 * Implements ROADMAP §"Risk Class Enumeration" Signal → DerivedClass table.
 * Rows are evaluated in order; first match wins.
 * No LLM, no randomness, no I/O.
 */
export function deriveRiskClassFromDiff(
  input: DerivedRiskInput,
  policy: RiskPolicy = DEFAULT_RISK_POLICY,
): DerivedRiskResult {
  for (const row of policy.rows) {
    if (matchesSignal(row.signal, input)) {
      return {
        class: row.derivedClass,
        matchedRow: { signal: row.signal, description: row.description },
      };
    }
  }
  // Fallback: no row matched at all (empty changeset or unrecognized policy).
  return { class: "medium", matchedRow: { signal: "diff-source-only" } };
}
