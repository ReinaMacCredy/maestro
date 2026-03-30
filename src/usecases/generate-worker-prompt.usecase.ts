/**
 * Worker prompt generation usecase
 * Composes a self-contained markdown worker assignment using mission context,
 * milestone context, feature verification details, and skill documentation.
 */
import type { FeatureStorePort } from "../ports/feature-store.port.js";
import type { MissionStorePort } from "../ports/mission-store.port.js";
import type { AssertionStorePort } from "../ports/assertion-store.port.js";
import type { Feature, Mission, Milestone, Assertion } from "../domain/mission-types.js";
import { MaestroError } from "../domain/errors.js";
import { readText, writeText, ensureDir } from "../lib/fs.js";
import { dirname, join, resolve } from "node:path";
import { MAESTRO_DIR } from "../domain/defaults.js";

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

  // Generate the prompt
  const prompt = composePrompt(mission, milestone, feature, featureAssertions, skillContent);

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

/**
 * Read worker skill markdown from either:
 * 1. .maestro/skills/{workerType}/SKILL.md in the current workspace or any ancestor
 * 2. skills/built-in/{workerType}/SKILL.md in the current workspace or any ancestor
 */
async function readWorkerSkill(baseDir: string, workerType: string): Promise<string> {
  const searchedPaths: string[] = [];

  for (const dir of enumerateSearchRoots(baseDir)) {
    const workspaceSkillPath = join(dir, MAESTRO_DIR, "skills", workerType, "SKILL.md");
    searchedPaths.push(workspaceSkillPath);
    const workspaceContent = await readText(workspaceSkillPath);
    if (workspaceContent !== undefined) {
      return workspaceContent;
    }

    const builtInSkillPath = join(dir, "skills", "built-in", workerType, "SKILL.md");
    searchedPaths.push(builtInSkillPath);
    const builtInContent = await readText(builtInSkillPath);
    if (builtInContent !== undefined) {
      return builtInContent;
    }
  }

  const primaryPath = searchedPaths[0] ?? join(baseDir, MAESTRO_DIR, "skills", workerType, "SKILL.md");
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
    parts.push(delimitContent(mission.description));
    parts.push("");
  }

  // Milestone context
  parts.push("## Milestone Context");
  parts.push("");
  parts.push(`**${milestone.id}:** ${delimitContent(milestone.title)}`);
  if (milestone.description) {
    parts.push("");
    parts.push(delimitContent(milestone.description));
  }
  parts.push("");

  // Feature description
  parts.push("## Feature Assignment");
  parts.push("");
  parts.push("### Description");
  parts.push("");
  parts.push(delimitContent(feature.description));
  parts.push("");

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
      parts.push(`- **${assertion.id}** (${assertion.status}): ${delimitContent(assertion.description)}`);
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
