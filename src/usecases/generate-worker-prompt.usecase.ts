/**
 * Worker prompt generation usecase
 * Composes a self-contained markdown worker assignment using mission context,
 * milestone context, feature verification details, and skill documentation.
 */
import type { FeatureStorePort } from "../ports/feature-store.port.js";
import type { MissionStorePort } from "../ports/mission-store.port.js";
import type { AssertionStorePort } from "../ports/assertion-store.port.js";
import type { RuntimeStorePort } from "../ports/runtime-store.port.js";
import type { Feature, Mission, Milestone, Assertion, MilestoneProfile } from "../domain/mission-types.js";
import { MaestroError } from "../domain/errors.js";
import { WORKER_TYPE_PATTERN } from "../domain/mission-validators.js";
import { readText, writeText, ensureDir } from "../lib/fs.js";
import { sanitizePromptContent } from "../lib/sanitize.js";
import { dirname, join, resolve } from "node:path";
import { DEFAULT_RUNTIME_LEASE_MS, MAESTRO_DIR, UNKNOWN_AGENT } from "../domain/defaults.js";
import { assertSafeSegment, resolveWithin } from "../lib/path-safety.js";
import type { WorkerRuntime } from "../domain/runtime-types.js";
import { parseWorkerReport } from "./feature-lifecycle.usecase.js";

interface PreviousMilestoneReport {
  readonly featureId: string;
  readonly featureTitle: string;
  readonly summary: string;
}

const PREVIOUS_REPORT_SUMMARY_LIMIT = 600;

/** Result of generating a worker prompt */
export interface GenerateWorkerPromptResult {
  /** The generated markdown prompt */
  readonly prompt: string;
  /** The feature ID */
  readonly featureId: string;
  /** The worker type (skill name) */
  readonly workerType: string;
  /** Path where prompt was written (if --out was provided) */
  readonly writtenTo?: readonly string[];
}

/**
 * Generate a self-contained worker prompt for a feature.
 * The prompt includes mission context, milestone context, feature details,
 * verification expectations, and the worker skill instructions.
 */
export async function generateWorkerPrompt(
  missionStore: MissionStorePort,
  featureStore: FeatureStorePort,
  assertionStore: AssertionStorePort,
  runtimeStore: RuntimeStorePort,
  baseDir: string,
  missionId: string,
  featureId: string,
  outPath?: string,
): Promise<GenerateWorkerPromptResult> {
  // Verify mission exists
  const mission = await missionStore.get(missionId);
  if (!mission) {
    throw new MaestroError(`Mission ${missionId} not found`, [
      "List missions: maestro mission list",
      `Check that mission ID '${missionId}' is correct`,
    ]);
  }

  // Get feature
  const feature = await featureStore.get(missionId, featureId);
  if (!feature) {
    throw new MaestroError(`Feature ${featureId} not found in mission ${missionId}`, [
      `List features: maestro feature list --mission ${missionId}`,
      `Check that feature ID '${featureId}' is correct`,
    ]);
  }

  // Get milestone for the feature
  const milestone = mission.milestones.find((m) => m.id === feature.milestoneId);
  if (!milestone) {
    throw new MaestroError(
      `Milestone ${feature.milestoneId} not found for feature ${featureId}`,
    );
  }

  // Get assertions for this feature
  const assertions = await assertionStore.list(missionId);
  const featureAssertions = assertions.filter((a) => a.featureId === featureId);

  // Read skill file
  const skillContent = await readWorkerSkill(baseDir, feature.workerType);

  // Load all features for sibling context in prompt
  const allFeatures = await featureStore.list(missionId);

  // Read handoff protocol (worker-base skill) -- optional, don't error if missing
  let handoffProtocol: string | undefined;
  try {
    handoffProtocol = await readWorkerSkill(baseDir, "maestro:worker-base");
  } catch {
    // worker-base skill not found -- skip handoff protocol section
  }

  const previousMilestoneReports = await loadPreviousMilestoneReports(
    baseDir,
    mission,
    allFeatures,
    missionId,
    milestone,
  );

  // Generate the prompt
  const prompt = composePrompt(mission, milestone, feature, featureAssertions, skillContent, allFeatures, handoffProtocol, previousMilestoneReports);

  // Track written paths
  const writtenPaths: string[] = [];

  // Write to mission workers directory
  const workersDir = join(
    baseDir,
    MAESTRO_DIR,
    "missions",
    missionId,
    "workers",
    featureId,
  );
  await ensureDir(workersDir);
  const promptPath = join(workersDir, "prompt.md");
  await writeText(promptPath, prompt);
  await initializeWorkerRuntime(runtimeStore, missionId, featureId, promptPath);
  writtenPaths.push(promptPath);

  // If --out is provided, also write to that path
  if (outPath) {
    await writeText(outPath, prompt);
    writtenPaths.unshift(outPath); // User-specified path first
  }

  return {
    prompt,
    featureId,
    workerType: feature.workerType,
    writtenTo: writtenPaths.length > 0 ? writtenPaths : undefined,
  };
}

async function initializeWorkerRuntime(
  runtimeStore: RuntimeStorePort,
  missionId: string,
  featureId: string,
  promptPath: string,
): Promise<void> {
  const now = Date.now();
  const nowIso = new Date(now).toISOString();
  const priorRuntime = await runtimeStore.get(missionId, featureId);
  const runtime: WorkerRuntime = {
    featureId,
    attemptId: crypto.randomUUID(),
    attempt: (priorRuntime?.attempt ?? 0) + 1,
    agent: UNKNOWN_AGENT,
    runtimeState: "starting",
    startedAt: nowIso,
    lastSeenAt: nowIso,
    leaseExpiresAt: new Date(now + DEFAULT_RUNTIME_LEASE_MS).toISOString(),
    recoveryMetadata: {
      retryCount: priorRuntime?.recoveryMetadata.retryCount ?? 0,
      lastRecoveryAt: priorRuntime?.recoveryMetadata.lastRecoveryAt,
      lastRecoveryReason: priorRuntime?.recoveryMetadata.lastRecoveryReason,
      history: priorRuntime?.recoveryMetadata.history ?? [],
    },
  };
  void promptPath;
  await runtimeStore.save(missionId, featureId, runtime);
}

/**
 * Read worker skill markdown from either:
 * 1. .maestro/skills/{workerType}/SKILL.md in the current workspace or any ancestor
 * 2. skills/built-in/{workerType}/SKILL.md in the current workspace or any ancestor
 */
async function readWorkerSkill(baseDir: string, workerType: string): Promise<string> {
  assertSafeSegment(workerType, "worker type", WORKER_TYPE_PATTERN, "letters, numbers, colons, dashes, and underscores");
  const searchedPaths: string[] = [];

  for (const dir of enumerateSearchRoots(baseDir)) {
    const workspaceSkillPath = resolveWithin(
      join(dir, MAESTRO_DIR, "skills"),
      join(workerType, "SKILL.md"),
      "Workspace skill path",
    );
    searchedPaths.push(workspaceSkillPath);
    const workspaceContent = await readText(workspaceSkillPath);
    if (workspaceContent !== undefined) {
      return workspaceContent;
    }

    const builtInSkillPath = resolveWithin(
      join(dir, "skills", "built-in"),
      join(workerType, "SKILL.md"),
      "Built-in skill path",
    );
    searchedPaths.push(builtInSkillPath);
    const builtInContent = await readText(builtInSkillPath);
    if (builtInContent !== undefined) {
      return builtInContent;
    }
  }

  const primaryPath = searchedPaths[0] ?? resolveWithin(
    join(baseDir, MAESTRO_DIR, "skills"),
    join(workerType, "SKILL.md"),
    "Workspace skill path",
  );
  throw new MaestroError(
    `Worker skill '${workerType}' not found at ${primaryPath}`,
    [
      `Create workspace skill file: ${primaryPath}`,
      `Or add built-in skill file: skills/built-in/${workerType}/SKILL.md`,
      `Searched paths: ${searchedPaths.join(", ")}`,
    ],
  );
}

function enumerateSearchRoots(baseDir: string): string[] {
  const roots: string[] = [];
  let current = resolve(baseDir);

  while (true) {
    roots.push(current);
    const parent = dirname(current);
    if (parent === current) {
      break;
    }
    current = parent;
  }

  return roots;
}

async function loadPreviousMilestoneReports(
  baseDir: string,
  mission: Mission,
  allFeatures: readonly Feature[],
  missionId: string,
  milestone: Milestone,
): Promise<readonly PreviousMilestoneReport[] | undefined> {
  const reviewProfiles = new Set<MilestoneProfile>([
    "plan-review",
    "code-review",
    "bug-hunt",
    "simplify",
    "validation",
  ]);

  if (!milestone.profile || !reviewProfiles.has(milestone.profile)) {
    return undefined;
  }

  const prevMilestone = mission.milestones
    .filter((item) => item.order < milestone.order)
    .sort((a, b) => b.order - a.order)[0];

  if (!prevMilestone) {
    return undefined;
  }

  const prevFeatures = allFeatures.filter(
    (feature) => feature.milestoneId === prevMilestone.id && feature.status === "done",
  );

  const reports = (await Promise.all(prevFeatures.map(async (feature) => {
    const reportPath = join(
      baseDir,
      MAESTRO_DIR,
      "missions",
      missionId,
      "workers",
      feature.id,
      "report.json",
    );

    let reportContent: string | undefined;
    try {
      reportContent = await readText(reportPath);
    } catch {
      return undefined;
    }

    if (!reportContent) {
      return undefined;
    }

      try {
        const parsed = await parseWorkerReport(reportContent);
        const rawSummary = parsed.salientSummary || parsed.whatWasImplemented || "(no summary)";
        return {
          featureId: feature.id,
        featureTitle: feature.title,
        summary: sanitizePromptContent(
          truncatePreviousReportSummary(rawSummary),
          "previous-milestone-summary",
        ),
      } satisfies PreviousMilestoneReport;
    } catch {
      return undefined;
    }
  }))).filter((report): report is PreviousMilestoneReport => report !== undefined);

  return reports.length > 0 ? reports : undefined;
}

function truncatePreviousReportSummary(summary: string): string {
  if (summary.length <= PREVIOUS_REPORT_SUMMARY_LIMIT) {
    return summary;
  }

  return `${summary.slice(0, PREVIOUS_REPORT_SUMMARY_LIMIT)}... [truncated]`;
}

/** Profile-specific preambles injected into the milestone context section */
const PROFILE_PREAMBLE: Partial<Record<MilestoneProfile, string>> = {
  planning: "You are producing a design or specification. Focus on architecture decisions, interface contracts, and identifying risks before implementation begins.",
  "plan-review": "You are reviewing a plan. Evaluate completeness, feasibility, missed edge cases, and alignment with the mission objectives. Approve or request changes.",
  implementation: "You are implementing features according to the plan. Focus on correctness, test coverage, and clean code.",
  "code-review": "You are reviewing implementation quality. Check for correctness, security vulnerabilities, performance issues, and adherence to the plan. Approve or request changes.",
  "bug-hunt": "You are hunting for bugs. Try to break the implementation through edge cases, unexpected inputs, race conditions, and integration failures.",
  simplify: "You are simplifying. Reduce complexity, improve clarity, remove dead code, and consolidate abstractions without changing behavior.",
  validation: "You are validating. Run checks, verify contracts, confirm assertions pass, and ensure the implementation meets acceptance criteria.",
};

/**
 * Compose the complete worker prompt.
 * Sanitizes mission text to ensure prompt structure remains stable.
 */
function composePrompt(
  mission: Mission,
  milestone: Milestone,
  feature: Feature,
  assertions: readonly Assertion[],
  skillContent: string,
  allFeatures: readonly Feature[],
  handoffProtocol?: string,
  previousMilestoneReports?: readonly PreviousMilestoneReport[],
): string {
  const parts: string[] = [];

  // Header
  parts.push(`# Worker Assignment: ${feature.title}`);
  parts.push("");

  // Feature identification
  parts.push(`**Feature ID:** ${feature.id}`);
  parts.push(`**Worker Type:** ${feature.workerType}`);
  parts.push(`**Mission:** ${mission.id} - ${mission.title}`);
  parts.push(`**Milestone:** ${milestone.id} - ${milestone.title}`);
  parts.push("");

  // Mission context - delimited to prevent structure breaking
  parts.push("## Mission Context");
  parts.push("");
  parts.push(delimitContent(mission.title));
  parts.push("");
  if (mission.description) {
    parts.push("### Description");
    parts.push("");
    parts.push(sanitizePromptContent(mission.description, "mission-description"));
    parts.push("");
  }

  // Milestone context
  parts.push("## Milestone Context");
  parts.push("");
  parts.push(`**${milestone.id}:** ${delimitContent(milestone.title)}`);
  if (milestone.kind) {
    parts.push(`**Kind:** ${milestone.kind}`);
  }
  if (milestone.profile) {
    parts.push(`**Profile:** ${milestone.profile}`);
  }
  if (milestone.description) {
    parts.push("");
    parts.push(delimitContent(milestone.description));
  }
  parts.push("");

  // Profile-specific preamble
  const profile = milestone.profile ?? "custom";
  const preamble = PROFILE_PREAMBLE[profile];
  if (preamble) {
    parts.push("### Phase Intent");
    parts.push("");
    parts.push(preamble);
    parts.push("");
  }

  // Previous milestone output (for review/validation profiles)
  if (previousMilestoneReports && previousMilestoneReports.length > 0) {
    parts.push("### Previous Milestone Output");
    parts.push("");
    parts.push("The following features were completed in the preceding milestone:");
    parts.push("");
    for (const report of previousMilestoneReports) {
      parts.push(`#### ${report.featureId}: ${delimitContent(report.featureTitle)}`);
      parts.push("");
      parts.push(report.summary);
      parts.push("");
    }
  }

  // Sibling feature context (A3)
  const otherFeatures = allFeatures.filter((f) => f.id !== feature.id);
  const doneFeatures = otherFeatures.filter((f) => f.status === "done");
  const activeFeatures = otherFeatures.filter((f) =>
    f.status === "assigned" || f.status === "in-progress" || f.status === "review",
  );

  if (doneFeatures.length > 0) {
    parts.push("### Completed Features");
    parts.push("");
    for (const f of doneFeatures) {
      parts.push(`- ${f.id}: ${f.title}`);
    }
    parts.push("");
  }

  if (activeFeatures.length > 0) {
    parts.push("### In Progress Features");
    parts.push("");
    for (const f of activeFeatures) {
      parts.push(`- ${f.id}: ${f.title} (${f.status})`);
    }
    parts.push("");
  }

  // Feature description
  parts.push("## Feature Assignment");
  parts.push("");
  parts.push("### Description");
  parts.push("");
  parts.push(sanitizePromptContent(feature.description, "feature-description"));
  parts.push("");

  // Preconditions (A1)
  if (feature.preconditions) {
    parts.push("### Preconditions");
    parts.push("");
    parts.push(sanitizePromptContent(feature.preconditions, "preconditions"));
    parts.push("");
  }

  // Expected Behavior (A2)
  if (feature.expectedBehavior) {
    parts.push("### Expected Behavior");
    parts.push("");
    parts.push(sanitizePromptContent(feature.expectedBehavior, "expected-behavior"));
    parts.push("");
  }

  // Dependencies
  if (feature.dependsOn.length > 0) {
    parts.push("### Dependencies");
    parts.push("");
    parts.push("This feature depends on the following features:");
    for (const depId of feature.dependsOn) {
      parts.push(`- ${depId}`);
    }
    parts.push("");
  }

  // Verification steps
  parts.push("### Verification Steps");
  parts.push("");
  parts.push("Complete the following verification steps:");
  parts.push("");
  for (let i = 0; i < feature.verificationSteps.length; i++) {
    const step = feature.verificationSteps[i];
    parts.push(`${i + 1}. ${delimitContent(step ?? "")}`);
  }
  parts.push("");

  // Related assertions
  if (assertions.length > 0) {
    parts.push("### Related Assertions");
    parts.push("");
    parts.push("This feature fulfills the following assertions:");
    parts.push("");
    for (const assertion of assertions) {
      parts.push(`- **${assertion.id}** (${assertion.result}): ${delimitContent(assertion.description)}`);
    }
    parts.push("");
  }

  // Skill instructions - wrapped in a clearly delimited block
  parts.push("## Skill Instructions");
  parts.push("");
  parts.push("<!-- BEGIN SKILL -->");
  parts.push("");
  parts.push(skillContent);
  parts.push("");
  parts.push("<!-- END SKILL -->");
  parts.push("");

  // Handoff Protocol (A4) -- worker-base skill content
  if (handoffProtocol) {
    parts.push("## Handoff Protocol");
    parts.push("");
    parts.push("<!-- BEGIN HANDOFF PROTOCOL -->");
    parts.push("");
    parts.push(handoffProtocol);
    parts.push("");
    parts.push("<!-- END HANDOFF PROTOCOL -->");
    parts.push("");
  }

  // Footer with status reminder
  parts.push("---");
  parts.push("");
  parts.push(`**Current Status:** ${feature.status}`);
  parts.push(`**Generated:** ${new Date().toISOString()}`);
  parts.push("");
  parts.push("When complete, report results using:");
  parts.push(`\`maestro feature update ${feature.id} --mission ${mission.id} --status <status> --report @report.json\``);
  parts.push("");

  return parts.join("\n");
}

/**
 * Delimit content that might contain control-like text.
 * Wraps content in a way that preserves literal values without breaking
 * the overall markdown structure.
 */
function delimitContent(content: string): string {
  // Handle empty content
  if (!content || content.trim().length === 0) {
    return "_(no content)_";
  }

  // Escape markdown headers within content to prevent breaking structure
  const escaped = content
    .replace(/^(#{1,6})\s/gm, "\\$1 ") // Escape header syntax at line start
    .replace(/^(<!--)/gm, "\\$1") // Escape HTML comment open
    .replace(/^(-->)/gm, "\\$1"); // Escape HTML comment close

  return escaped;
}
