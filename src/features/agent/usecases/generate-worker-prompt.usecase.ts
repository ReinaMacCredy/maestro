/**
 * Worker prompt generation usecase
 * Composes a self-contained markdown worker assignment using mission context,
 * milestone context, feature verification details, and skill documentation.
 */
import type {
  FeatureStorePort,
  MissionStorePort,
  AssertionStorePort,
  PrincipleStorePort,
  Feature,
  Mission,
  Milestone,
  Assertion,
  Principle,
  MilestoneProfile,
} from "@/features/mission";
import { AGENT_TYPE_PATTERN, parseWorkerReport } from "@/features/mission";
import {
  recallMemory,
  type CorrectionStorePort,
  type LearningStorePort,
  type RecallResult,
} from "@/features/memory";
import {
  loadPriorHandoffs,
  type PriorSessionSummary,
  type HandoffStorePort,
} from "@/features/handoff";
import { MaestroError } from "@/shared/errors.js";
import { readText, writeText, ensureDir } from "@/shared/lib/fs.js";
import { sanitizeInlinePromptContent, sanitizePromptContent } from "@/shared/lib/sanitize.js";
import { dirname, join, resolve } from "node:path";
import { MAESTRO_DIR } from "@/shared/domain/defaults.js";
import { assertSafeSegment, resolveWithin } from "@/shared/lib/path-safety.js";

interface PreviousMilestoneReport {
  readonly featureId: string;
  readonly featureTitle: string;
  readonly summary: string;
}

const PREVIOUS_REPORT_SUMMARY_LIMIT = 600;
const REVIEW_PROFILES = new Set<MilestoneProfile>([
  "plan-review",
  "code-review",
  "bug-hunt",
  "simplify",
  "validation",
]);

/** Result of generating a worker prompt */
export interface GenerateWorkerPromptResult {
  /** The generated markdown prompt */
  readonly prompt: string;
  /** The feature ID */
  readonly featureId: string;
  /** The agent type (skill name) */
  readonly agentType: string;
  /** Path where prompt was written (if --out was provided) */
  readonly writtenTo?: readonly string[];
}

/**
 * Generate a self-contained worker prompt for a feature.
 * The prompt includes mission context, milestone context, feature details,
 * verification expectations, and the worker skill instructions.
 */
/** Optional enrichment stores for worker prompt generation. */
export interface WorkerPromptStores {
  readonly correctionStore?: CorrectionStorePort;
  readonly learningStore?: LearningStorePort;
  readonly principleStore?: PrincipleStorePort;
  readonly handoffStore?: HandoffStorePort;
}

export async function generateWorkerPrompt(
  missionStore: MissionStorePort,
  featureStore: FeatureStorePort,
  assertionStore: AssertionStorePort,
  baseDir: string,
  missionId: string,
  featureId: string,
  outPath?: string,
  stores?: WorkerPromptStores,
): Promise<GenerateWorkerPromptResult>;

export async function generateWorkerPrompt(
  missionStore: MissionStorePort,
  featureStore: FeatureStorePort,
  assertionStore: AssertionStorePort,
  baseDir: string,
  missionId: string,
  featureId: string,
  outPath?: string,
  correctionStore?: CorrectionStorePort,
  learningStore?: LearningStorePort,
): Promise<GenerateWorkerPromptResult>;

export async function generateWorkerPrompt(
  missionStore: MissionStorePort,
  featureStore: FeatureStorePort,
  assertionStore: AssertionStorePort,
  baseDir: string,
  missionId: string,
  featureId: string,
  outPath?: string,
  storesOrCorrectionStore?: WorkerPromptStores | CorrectionStorePort,
  learningStore?: LearningStorePort,
): Promise<GenerateWorkerPromptResult> {
  const stores = normalizeWorkerPromptStores(storesOrCorrectionStore, learningStore);
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
  const skillContent = await readWorkerSkill(baseDir, feature.agentType);

  // Load all features for sibling context in prompt
  const allFeatures = await featureStore.list(missionId);

  // Read handoff protocol (worker-base skill) -- optional, don't error if missing
  let handoffProtocol: string | undefined;
  try {
    handoffProtocol = await readWorkerSkill(baseDir, "maestro:worker-base");
  } catch {
    // worker-base skill not found -- skip handoff protocol section
  }

  // All four reads are independent filesystem operations -- run in parallel.
  // Memory recall, principle loading, and handoff replay are best-effort and
  // must never block prompt generation; missing data yields undefined.
  const [previousMilestoneReports, recalledMemory, principles, priorHandoffs] = await Promise.all([
    loadPreviousMilestoneReports(baseDir, mission, allFeatures, missionId, milestone),
    safeRecallMemory(stores?.correctionStore, stores?.learningStore, feature),
    safeLoadPrinciples(stores?.principleStore, milestone.profile),
    safeLoadPriorHandoffs(stores?.handoffStore, missionId, featureId),
  ]);

  // Generate the prompt
  const prompt = composePrompt(mission, milestone, feature, featureAssertions, skillContent, allFeatures, handoffProtocol, previousMilestoneReports, recalledMemory, principles, priorHandoffs);

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
    agentType: feature.agentType,
    writtenTo: writtenPaths.length > 0 ? writtenPaths : undefined,
  };
}

function normalizeWorkerPromptStores(
  storesOrCorrectionStore: WorkerPromptStores | CorrectionStorePort | undefined,
  learningStore: LearningStorePort | undefined,
): WorkerPromptStores | undefined {
  if (!storesOrCorrectionStore) {
    return learningStore ? { learningStore } : undefined;
  }

  if (isWorkerPromptStores(storesOrCorrectionStore)) {
    return storesOrCorrectionStore;
  }

  return {
    correctionStore: storesOrCorrectionStore,
    learningStore,
  };
}

function isWorkerPromptStores(value: WorkerPromptStores | CorrectionStorePort): value is WorkerPromptStores {
  return typeof value === "object" && value !== null && (
    "correctionStore" in value
    || "learningStore" in value
    || "principleStore" in value
    || "handoffStore" in value
  );
}

/**
 * Read worker skill markdown from either:
 * 1. .maestro/skills/{agentType}/SKILL.md in the current workspace or any ancestor
 * 2. skills/built-in/{agentType}/SKILL.md in the current workspace or any ancestor
 */
async function readWorkerSkill(baseDir: string, agentType: string): Promise<string> {
  assertSafeSegment(agentType, "agent type", AGENT_TYPE_PATTERN, "letters, numbers, colons, dashes, and underscores");
  const searchedPaths: string[] = [];

  for (const dir of enumerateSearchRoots(baseDir)) {
    const workspaceSkillPath = resolveWithin(
      join(dir, MAESTRO_DIR, "skills"),
      join(agentType, "SKILL.md"),
      "Workspace skill path",
    );
    searchedPaths.push(workspaceSkillPath);
    const workspaceContent = await readText(workspaceSkillPath);
    if (workspaceContent !== undefined) {
      return workspaceContent;
    }

    const builtInSkillPath = resolveWithin(
      join(dir, "skills", "built-in"),
      join(agentType, "SKILL.md"),
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
    join(agentType, "SKILL.md"),
    "Workspace skill path",
  );
  throw new MaestroError(
    `Worker skill '${agentType}' not found at ${primaryPath}`,
    [
      `Create workspace skill file: ${primaryPath}`,
      `Or add built-in skill file: skills/built-in/${agentType}/SKILL.md`,
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
  if (!milestone.profile || !REVIEW_PROFILES.has(milestone.profile)) {
    return undefined;
  }

  const prevMilestone = findPreviousMilestone(mission.milestones, milestone.order);

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

function findPreviousMilestone(
  milestones: readonly Milestone[],
  currentOrder: number,
): Milestone | undefined {
  let previous: Milestone | undefined;

  for (const candidate of milestones) {
    if (candidate.order >= currentOrder) {
      continue;
    }

    if (!previous || candidate.order > previous.order) {
      previous = candidate;
    }
  }

  return previous;
}

function truncatePreviousReportSummary(summary: string): string {
  if (summary.length <= PREVIOUS_REPORT_SUMMARY_LIMIT) {
    return summary;
  }

  return `${summary.slice(0, PREVIOUS_REPORT_SUMMARY_LIMIT)}... [truncated]`;
}

/**
 * Best-effort memory recall for worker prompt injection.
 *
 * Returns undefined (no section rendered) in any of these cases:
 *   - memory stores are not provided (backward compatibility)
 *   - the store throws (missing dir, corrupted file, etc.)
 *   - recall returns no corrections and no compiled learnings
 *
 * Memory enhances the prompt; it must never block it.
 */
async function safeRecallMemory(
  correctionStore: CorrectionStorePort | undefined,
  learningStore: LearningStorePort | undefined,
  feature: Feature,
): Promise<RecallResult | undefined> {
  if (!correctionStore || !learningStore) {
    return undefined;
  }

  try {
    const taskDescription = `${feature.title}\n${feature.description}`;
    const result = await recallMemory(correctionStore, learningStore, {
      taskDescription,
    });

    // Suppress the section when there's nothing to say. Avoids injecting
    // empty headings into every prompt on fresh or unseeded projects.
    if (result.corrections.length === 0 && !result.compiledLearnings) {
      return undefined;
    }

    return result;
  } catch {
    // Any failure in memory recall is swallowed. The prompt still generates.
    return undefined;
  }
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
  recalledMemory?: RecallResult,
  principles?: readonly Principle[],
  priorHandoffs?: readonly PriorSessionSummary[],
): string {
  const parts: string[] = [];

  // Header
  parts.push(`# Worker Assignment: ${feature.title}`);
  parts.push("");

  // Feature identification
  parts.push(`**Feature ID:** ${feature.id}`);
  parts.push(`**Agent Type:** ${feature.agentType}`);
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

  // Relevant Memory - auto-injected corrections and compiled learnings.
  // Placed before the skill block so the worker reads prior rules before
  // the generic skill instructions. safeRecallMemory() already filters out
  // empty results, so a non-undefined value always has something to render.
  if (recalledMemory) {
    appendMemorySection(parts, recalledMemory);
  }

  // Behavioral Principles -- injected between memory and skill so the
  // worker sees constraints before the generic skill instructions.
  if (principles && principles.length > 0) {
    appendPrincipleSection(parts, principles);
  }

  // Prior Session Replay -- insights from previous handoffs for this
  // feature. Placed after principles and before skill so the worker
  // sees accumulated context before the generic skill instructions.
  if (priorHandoffs && priorHandoffs.length > 0) {
    appendReplaySection(parts, priorHandoffs);
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

  // Reply Contract -- the agent's final action
  parts.push("## Reply Contract");
  parts.push("");
  parts.push("<!-- BEGIN REPLY CONTRACT -->");
  parts.push("");
    parts.push(buildReplyContractSection(mission.id, feature.id));
  parts.push("");
  parts.push("<!-- END REPLY CONTRACT -->");
  parts.push("");

  // Footer with status reminder
  parts.push("---");
  parts.push("");
  parts.push(`**Current Status:** ${feature.status}`);
  parts.push(`**Generated:** ${new Date().toISOString()}`);
  parts.push("");
  parts.push("When complete, use the Reply Contract above as the final handoff back to Maestro.");
  parts.push("");

  return parts.join("\n");
}

/**
 * Render the Reply Contract block. This tells the agent exactly what to
 * write when it finishes (or gives up), where to write it, and how maestro
 * interprets the outcome. The contract is the agent's half of the
 * closed-loop reply protocol -- without this section agents would not know
 * about .maestro/replies/ at all.
 */
function buildReplyContractSection(missionId: string, featureId: string): string {
  const lines = [
      `When you finish (or give up), write your reply as your final action:`,
      ``,
      `Path: \`.maestro/replies/${missionId}/${featureId}.yaml\``,
      ``,
      `Schema:`,
      `\`\`\`yaml`,
      `missionId: ${missionId}`,
      `featureId: ${featureId}`,
    `outcome: completed            # or: kicked-back | abandoned`,
    `writtenAt: <ISO-8601 UTC>     # e.g. 2026-04-13T12:34:56.000Z`,
    `writtenBy: agent              # always 'agent' for prompt-driven replies`,
    `source: agent:<your-name>     # optional free-form origin marker`,
    `notes: |                      # optional free-form notes`,
    `  What changed, what was tricky, what you could not verify.`,
    `report:                       # optional structured WorkerReport`,
    `  salientSummary: ...`,
    `  whatWasImplemented: ...`,
    `  whatWasLeftUndone: ...`,
    `  verification:`,
    `    commandsRun: [...]`,
    `    interactiveChecks: [...]`,
    `  tests:`,
    `    added: [...]`,
    `  discoveredIssues: [...]`,
    `\`\`\``,
    ``,
    `Outcome meanings:`,
    `  - completed:    assertions pass, feature is ready to mark done.`,
    `  - kicked-back:  cannot complete now; needs retry. Put the blocker in notes.`,
    `  - abandoned:    out of scope or impossible; put the reason in notes.`,
    ``,
    `Maestro cross-checks your outcome against feature status and assertion`,
    `results. If you claim 'completed' but assertions are not passing or the`,
    `feature is not in 'review', it will be downgraded to 'kicked-back' and`,
    `the discrepancy recorded. Be honest; cheating is visible.`,
  ];
  return lines.join("\n");
}

/**
 * Render the auto-injected "Relevant Memory" section from recalled corrections
 * and compiled learnings. Hard rules are flagged inline so the worker treats
 * them as non-negotiable. Content is sanitized to prevent prompt structure
 * breaking via embedded markdown headers or HTML comment tokens.
 */
function appendMemorySection(parts: string[], recalled: RecallResult): void {
  parts.push("## Relevant Memory");
  parts.push("");
  parts.push("<!-- Auto-injected from the maestro memory system based on this feature's context. -->");
  parts.push("<!-- These are rules and insights from prior sessions. Treat hard rules as non-negotiable. -->");
  parts.push("");

  if (recalled.corrections.length > 0) {
    parts.push(`### Corrections (${recalled.corrections.length})`);
    parts.push("");
    for (const c of recalled.corrections) {
      const badge = c.severity === "hard" ? "**[HARD]**" : "[soft]";
      // Rule text and keywords are rendered inline: use delimitContent to
      // escape header/comment-boundary syntax without wrapping in XML tags.
      // sanitizePromptContent is the wrong tool here because it assumes
      // block-level isolation and produces noisy output for single-line values.
      const keywords = c.trigger.keywords.length > 0
        ? ` _(${c.trigger.keywords.map((k) => delimitContent(k)).join(", ")})_`
        : "";
      parts.push(`- ${badge} ${delimitContent(c.rule)}${keywords}`);
    }
    parts.push("");
  }

  if (recalled.compiledLearnings) {
    parts.push("### Compiled Learnings");
    parts.push("");
    parts.push(`_Last compiled ${recalled.compiledLearnings.compiledAt} from ${recalled.compiledLearnings.rawCount} raw entries:_`);
    parts.push("");
    // Compiled summary is block-level (potentially multi-paragraph); full
    // sanitization with the memory-learnings tag is appropriate here.
    parts.push(sanitizePromptContent(recalled.compiledLearnings.summary, "memory-learnings"));
    parts.push("");
  }
}

/**
 * Best-effort principle loading for worker prompt injection.
 * Returns undefined when the store is absent or the store throws.
 * Principles enhance the prompt; they must never block it.
 */
async function safeLoadPrinciples(
  principleStore: PrincipleStorePort | undefined,
  profile: MilestoneProfile | undefined,
): Promise<readonly Principle[] | undefined> {
  if (!principleStore || !profile) return undefined;
  try {
    const principles = await principleStore.listByProfile(profile);
    return principles.length > 0 ? principles : undefined;
  } catch {
    return undefined;
  }
}

/**
 * Render the "Behavioral Principles" section from active principles.
 * Gate principles show a [GATE] badge and required handoff field notice.
 * Advisory principles show an [advisory] badge.
 */
function appendPrincipleSection(parts: string[], principles: readonly Principle[]): void {
  parts.push("## Behavioral Principles");
  parts.push("");
  parts.push("<!-- Auto-injected from the maestro principle system based on milestone profile. -->");
  parts.push("");

  for (const p of principles) {
    if (p.mode === "gate") {
      parts.push(`- **[GATE]** ${delimitContent(p.name)}: ${delimitContent(p.rule)}`);
      parts.push(`  _Required handoff field:_ \`${p.gateField}\` (${p.gateCheck})`);
    } else {
      parts.push(`- [advisory] ${delimitContent(p.name)}: ${delimitContent(p.rule)}`);
    }
  }
  parts.push("");
}

/**
 * Best-effort handoff replay loading for worker prompt injection.
 * Returns undefined when the store is absent or throws.
 */
async function safeLoadPriorHandoffs(
  handoffStore: HandoffStorePort | undefined,
  missionId: string,
  featureId: string,
): Promise<readonly PriorSessionSummary[] | undefined> {
  if (!handoffStore) return undefined;
  try {
    return await loadPriorHandoffs(handoffStore, missionId, featureId);
  } catch {
    return undefined;
  }
}

/**
 * Render the "Prior Session Replay" section from previous handoffs.
 * Each handoff becomes a subsection with its replay-worthy fields.
 * All content is sanitized because handoff data originates from
 * worker sessions that could inject malicious markdown/HTML.
 */
function appendReplaySection(parts: string[], handoffs: readonly PriorSessionSummary[]): void {
  parts.push("## Prior Session Replay");
  parts.push("");
  parts.push("<!-- Auto-injected from previous handoffs for this feature. -->");
  parts.push("<!-- Use these insights to avoid repeating mistakes and build on prior progress. -->");
  parts.push("");

  for (const h of handoffs) {
    parts.push(`### Session ${delimitContent(h.handoffId)} (${delimitContent(h.mode)}, ${delimitContent(h.timestamp)})`);
    parts.push("");
    parts.push(`**Summary:** ${delimitContent(h.summary)}`);
    parts.push("");

    renderListField(parts, "Risks", h.risks);
    renderListField(parts, "Divergences from plan", h.divergences);
    renderListField(parts, "Causal drivers", h.causalDrivers);

    if (h.blindSpot) {
      parts.push(`**Blind spot:** ${delimitContent(h.blindSpot)}`);
      parts.push("");
    }

    renderListField(parts, "Assumptions", h.assumptions);

    if (h.verificationResults && h.verificationResults.length > 0) {
      parts.push("**Verification results:**");
      for (const v of h.verificationResults) {
        const icon = v.passed ? "[PASS]" : "**[FAIL]**";
        parts.push(`- ${icon} ${delimitContent(v.step)}`);
      }
      parts.push("");
    }

    renderListField(parts, "Completed work", h.completedWork);
  }
}

function renderListField(parts: string[], label: string, items: readonly string[] | undefined): void {
  if (!items || items.length === 0) return;
  parts.push(`**${label}:**`);
  for (const item of items) {
    parts.push(`- ${delimitContent(item)}`);
  }
  parts.push("");
}

/**
 * Delimit content that might contain control-like text. Unlike
 * `sanitizePromptContent`, the value is emitted inline (no XML wrapper,
 * no HTML entity encoding) so memory rules read naturally in the prompt.
 */
function delimitContent(content: string): string {
  return sanitizeInlinePromptContent(content);
}
