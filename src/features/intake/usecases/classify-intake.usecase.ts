import { existsSync } from "node:fs";
import { deriveRiskClassFromDiff, requiresThreatModel } from "@/features/risk/index.js";
import type { RiskPolicy } from "@/features/policy/index.js";
import { matchesAnyGlob } from "@/shared/lib/glob-match.js";
import { normalizeIntakePath } from "../domain/normalize-path.js";
import type {
  IntakeFlag,
  IntakeInput,
  IntakeLane,
  IntakeResult,
} from "../domain/types.js";
import {
  classifyWorkType,
  detectHarnessImpact,
  generateNextSteps,
} from "../domain/classify-work-type.js";

const HARD_GATES: ReadonlySet<IntakeFlag> = new Set([
  "auth",
  "authz",
  "data-model",
  "audit-security",
  "external-systems",
]);

// Heuristic globs for auto-detection. Declared flags are the primary input;
// auto-detection only covers cases where the path itself is a tell.
const AUTH_GLOBS = ["**/auth/**", "**/session/**", "**/jwt*", "**/login*", "**/logout*"];
const DATA_MODEL_GLOBS = ["**/migrations/**", "**/db/migrations/**", "**/schema/**"];
const EXTERNAL_GLOBS = [
  "package.json",
  "bun.lock*",
  "pnpm-lock.yaml",
  "yarn.lock",
  "**/Cargo.toml",
  "**/Cargo.lock",
  "**/pyproject.toml",
  "**/requirements*.txt",
  "**/Gemfile*",
  "**/go.mod",
  "**/go.sum",
];

/**
 * Pure-compute pre-flight risk classifier. Runs before code is written and
 * returns a lane recommendation, the derived risk class (via the existing
 * Risk Engine), and the next-step hint for the agent.
 *
 * Lane rules:
 *   any hard-gate flag       -> high-risk
 *   >= 4 flags total         -> high-risk
 *   >= 2 flags total         -> normal
 *   else                     -> tiny
 */
export function classifyIntake(
  input: IntakeInput,
  riskPolicy: RiskPolicy,
  sensitivePathsPolicy: readonly string[],
  pathExists: (path: string) => boolean = existsSync,
  cwd: string = process.cwd(),
): IntakeResult {
  const auto = new Set<IntakeFlag>();
  const paths = input.intendedPaths.map((p) => normalizeIntakePath(p, cwd));
  const normalizedInput: IntakeInput = { ...input, intendedPaths: paths };

  if (paths.some((p) => matchesAnyGlob(sensitivePathsPolicy, p))) auto.add("audit-security");
  if (paths.some((p) => matchesAnyGlob(AUTH_GLOBS, p))) auto.add("auth");
  if (paths.some((p) => matchesAnyGlob(DATA_MODEL_GLOBS, p))) auto.add("data-model");
  if (paths.some((p) => matchesAnyGlob(EXTERNAL_GLOBS, p))) auto.add("external-systems");

  const declared = input.declaredFlags ?? [];
  const all: IntakeFlag[] = Array.from(new Set<IntakeFlag>([...auto, ...declared]));
  const hardGates = all.filter((f) => HARD_GATES.has(f));

  const lane: IntakeLane =
    hardGates.length > 0 ? "high-risk"
      : all.length >= 4 ? "high-risk"
      : all.length >= 2 ? "normal"
      : "tiny";

  const derived = deriveRiskClassFromDiff(
    { changedPaths: paths, sensitivePathsPolicy },
    riskPolicy,
  );

  const threatModelRequired = requiresThreatModel(derived.class, derived.matchedRow.signal);

  const recommendedNextStep =
    lane === "tiny"
      ? "patch directly; run `maestro task verify` if a contract exists; otherwise run repo-level validation and close"
      : lane === "normal"
        ? "create a task via `maestro task plan` and run `maestro plan check`"
        : threatModelRequired
          ? "create a high-risk task with Spec acceptance criteria and a `threat-model` Evidence row"
          : "create a high-risk task with Spec acceptance criteria";

  const workType = classifyWorkType(normalizedInput, { allFlags: all, pathExists });
  const harnessImpact = detectHarnessImpact(paths);
  const recommendedNextSteps = generateNextSteps(workType, lane);

  return {
    lane,
    derivedRiskClass: derived.class,
    derivedRiskSignal: derived.matchedRow.signal,
    autoDetectedFlags: Array.from(auto).sort(),
    declaredFlags: declared,
    hardGatesTriggered: hardGates,
    threatModelRequired,
    recommendedNextStep,
    workType,
    harnessImpact,
    recommendedNextSteps,
  };
}
