import { matchesAnyGlob } from "@/shared/lib/glob-match.js";
import type { IntakeFlag, IntakeInput, IntakeLane, WorkType } from "./types.js";

const HARNESS_GLOBS = [
  ".maestro/**",
  "policies/**",
  "skills/**",
  "hooks/**",
];

const MAINTENANCE_PATH_GLOBS = [
  "package.json",
  "package-lock.json",
  "bun.lock",
  "bun.lockb",
  "pnpm-lock.yaml",
  "yarn.lock",
  "**/Cargo.toml",
  "**/Cargo.lock",
  "**/pyproject.toml",
  "**/requirements*.txt",
  "**/Gemfile*",
  "**/Gemfile.lock",
  "**/go.mod",
  "**/go.sum",
  ".github/**",
  ".gitignore",
  ".editorconfig",
  ".npmrc",
  ".node-version",
  ".tool-versions",
  "tsconfig*.json",
];

/**
 * Heuristically classify a change into one of six work types.
 *
 * Decision order — first match wins:
 *   1. harness-improvement -- any path under .maestro/, policies/, skills/, hooks/
 *   2. initiative          -- multi-domain flag, OR paths span 3+ feature areas
 *   3. maintenance         -- all paths are manifests / .github / root config
 *   4. new-spec            -- no path exists on disk
 *   5. spec-slice          -- all paths share one `src/features/<one>/` root
 *   6. change-request      -- fallback
 *
 * A "feature area" is a `src/features/<name>/` root when the path falls
 * under one; otherwise it is the path's top-level directory. This matches
 * how a feature-first repo segments domains.
 *
 * Pure function; `pathExists` is injected so tests can stub the filesystem.
 */
export function classifyWorkType(
  input: IntakeInput,
  context: {
    readonly allFlags: readonly IntakeFlag[];
    readonly pathExists: (path: string) => boolean;
  },
): WorkType {
  if (input.declaredWorkType !== undefined) return input.declaredWorkType;

  const paths = input.intendedPaths;
  if (paths.length === 0) return "change-request";

  if (paths.some((p) => matchesAnyGlob(HARNESS_GLOBS, p))) {
    return "harness-improvement";
  }

  const hasMultiDomain = context.allFlags.includes("multi-domain");
  const domains = new Set(paths.map(domainOf));
  if (hasMultiDomain || domains.size >= 3) {
    return "initiative";
  }

  if (paths.every((p) => matchesAnyGlob(MAINTENANCE_PATH_GLOBS, p))) {
    return "maintenance";
  }

  if (paths.every((p) => !context.pathExists(p))) {
    return "new-spec";
  }

  const featureRoots = new Set(paths.map(featureRootOf).filter((r): r is string => r !== undefined));
  if (featureRoots.size === 1 && paths.every((p) => featureRootOf(p) !== undefined)) {
    return "spec-slice";
  }

  return "change-request";
}

/**
 * True when any intended path falls under `.maestro/`, `policies/`,
 * `skills/`, or `hooks/`. Independent of work-type.
 */
export function detectHarnessImpact(paths: readonly string[]): boolean {
  return paths.some((p) => matchesAnyGlob(HARNESS_GLOBS, p));
}

const NEXT_STEPS_TABLE: Record<WorkType, Record<IntakeLane, string>> = {
  "new-spec": {
    tiny: "Create task with `maestro task plan`",
    normal: "Create mission spec, then `maestro task plan`",
    "high-risk": "Create mission spec with threat model",
  },
  "spec-slice": {
    tiny: "Create task with `maestro task plan`",
    normal: "Create task, reference parent spec",
    "high-risk": "Create task with threat model, reference parent spec",
  },
  "change-request": {
    tiny: "Create task, implement, verify",
    normal: "Create task with regression test plan",
    "high-risk": "Create task with threat model and regression tests",
  },
  initiative: {
    tiny: "Create epic task, break into subtasks",
    normal: "Create mission spec, break into tasks",
    "high-risk": "Create mission spec with threat model",
  },
  maintenance: {
    tiny: "Create chore task, implement directly",
    normal: "Create chore task with verification plan",
    "high-risk": "Create chore task with impact analysis",
  },
  "harness-improvement": {
    tiny: "Create task, update harness, verify",
    normal: "Create task, record `harness-delta` evidence",
    "high-risk": "Create task with policy impact analysis",
  },
};

/**
 * Map (workType, lane) to a recommended next-step string. Used by the
 * `recommendedNextSteps` field on IntakeResult.
 */
export function generateNextSteps(workType: WorkType, lane: IntakeLane): string {
  return NEXT_STEPS_TABLE[workType][lane];
}

function topLevelDirOf(path: string): string {
  const slash = path.indexOf("/");
  return slash === -1 ? path : path.slice(0, slash);
}

function featureRootOf(path: string): string | undefined {
  const match = path.match(/^(src\/features\/[^/]+)\//);
  return match ? match[1] : undefined;
}

function domainOf(path: string): string {
  return featureRootOf(path) ?? topLevelDirOf(path);
}
