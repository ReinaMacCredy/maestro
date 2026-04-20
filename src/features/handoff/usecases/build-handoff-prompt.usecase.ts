import { join } from "node:path";
import type {
  Assertion,
  AssertionStorePort,
  Feature,
  FeatureStorePort,
  Milestone,
  Mission,
  MissionStorePort,
} from "@/features/mission";
import type { GitPort } from "@/infra/ports/git.port.js";
import type { GitState } from "@/infra/domain/git-types.js";
import type { HandoffPromptContext, HandoffRelevantFile } from "@/features/handoff";
import { MAESTRO_DIR } from "@/shared/domain/defaults.js";
import { fileExists } from "@/shared/lib/fs.js";

export interface BuildHandoffPromptDeps {
  readonly missionStore: MissionStorePort;
  readonly featureStore: FeatureStorePort;
  readonly assertionStore: AssertionStorePort;
  readonly git: GitPort;
}

export interface BuildHandoffPromptResult {
  readonly prompt: string;
  readonly context: HandoffPromptContext;
}

export async function buildHandoffPrompt(
  deps: BuildHandoffPromptDeps,
  input: {
    readonly cwd: string;
    readonly task: string;
    readonly extraConstraints?: readonly string[];
  },
): Promise<BuildHandoffPromptResult> {
  const gitState = await loadGitState(deps.git, input.cwd);
  const missionContext = await resolveMissionContext(
    deps.missionStore,
    deps.featureStore,
    deps.assertionStore,
  );

  const promptContext = missionContext
    ? await buildMissionPromptContext(input.cwd, input.task, gitState, missionContext)
    : buildRepositoryPromptContext(input.task, gitState);

  const constraints = input.extraConstraints && input.extraConstraints.length > 0
    ? [...promptContext.constraints, ...input.extraConstraints]
    : promptContext.constraints;

  const context: HandoffPromptContext = {
    ...promptContext,
    constraints,
  };

  return {
    context,
    prompt: renderHandoffPrompt(context),
  };
}

async function resolveMissionContext(
  missionStore: MissionStorePort,
  featureStore: FeatureStorePort,
  assertionStore: AssertionStorePort,
): Promise<{
  readonly mission: Mission;
  readonly milestone: Milestone;
  readonly feature: Feature;
  readonly assertions: readonly Assertion[];
} | undefined> {
  const missions = await missionStore.list();
  const mission = missions.find((item) => item.status === "executing" || item.status === "paused")
    ?? (missions.length === 1 ? missions[0] : undefined);

  if (!mission) return undefined;

  const features = await featureStore.list(mission.id);
  const actionable = features.filter((feature) => feature.status !== "done" && feature.status !== "blocked");
  if (actionable.length !== 1) return undefined;

  const feature = actionable[0]!;
  const milestone = mission.milestones.find((item) => item.id === feature.milestoneId);
  if (!milestone) return undefined;

  const assertions = (await assertionStore.list(mission.id)).filter((item) => item.featureId === feature.id);
  return { mission, milestone, feature, assertions };
}

async function buildMissionPromptContext(
  cwd: string,
  task: string,
  gitState: GitState | undefined,
  missionContext: {
    readonly mission: Mission;
    readonly milestone: Milestone;
    readonly feature: Feature;
    readonly assertions: readonly Assertion[];
  },
): Promise<HandoffPromptContext> {
  const { mission, milestone, feature, assertions } = missionContext;
  const relevantFiles = await collectMissionRelevantFiles(cwd, mission.id, feature.id, gitState);
  const whatWasTried = buildWhatWasTried(feature);
  const decisions = buildMissionDecisions(feature, milestone);
  const acceptanceCriteria = buildAcceptanceCriteria(feature, assertions);
  const constraints = buildMissionConstraints(feature, gitState);

  return {
    task: normalizeText(task),
    context: [
      `Mission ${mission.id}: ${mission.title}`,
      mission.description,
      `Milestone ${milestone.id}: ${milestone.title}${milestone.profile ? ` (${milestone.profile})` : ""}`,
      milestone.description,
      `Feature ${feature.id}: ${feature.title}`,
      feature.description,
    ].filter(Boolean),
    relevantFiles,
    currentState: buildCurrentState(gitState, [
      `Mission status: ${mission.status}`,
      `Feature status: ${feature.status}`,
    ]),
    whatWasTried,
    decisions,
    acceptanceCriteria,
    constraints,
    refs: {
      missionId: mission.id,
      featureId: feature.id,
      milestoneId: milestone.id,
    },
  };
}

function buildRepositoryPromptContext(
  task: string,
  gitState: GitState | undefined,
): HandoffPromptContext {
  return {
    task: normalizeText(task),
    context: [
      "This handoff was created from the current repository state without a single active mission feature to anchor it.",
      "Use the task description plus the current branch and changed files to recover the exact working context.",
    ],
    relevantFiles: buildRepositoryRelevantFiles(gitState),
    currentState: buildCurrentState(gitState),
    whatWasTried: [
      "No structured mission or worker report was available for this handoff.",
      "Start by inspecting the changed files and recent commits before editing.",
    ],
    decisions: [
      "No prior Maestro-specific decisions were attached to this handoff.",
      "Preserve the current workspace intent rather than broadening scope.",
    ],
    acceptanceCriteria: [
      "Complete the task described in the Task section.",
      "Verify the touched surface area before stopping.",
    ],
    constraints: buildRepositoryConstraints(gitState),
    refs: {},
  };
}

async function collectMissionRelevantFiles(
  cwd: string,
  missionId: string,
  featureId: string,
  gitState: GitState | undefined,
): Promise<readonly HandoffRelevantFile[]> {
  const files: HandoffRelevantFile[] = [];
  const workerPromptPath = join(MAESTRO_DIR, "missions", missionId, "workers", featureId, "prompt.md");
  const workerReportPath = join(MAESTRO_DIR, "missions", missionId, "workers", featureId, "report.json");
  const replyPath = join(MAESTRO_DIR, "replies", missionId, `${featureId}.yaml`);

  if (await fileExists(join(cwd, workerPromptPath))) {
    files.push({
      path: workerPromptPath,
      reason: "Current worker brief for the active feature.",
    });
  }

  if (await fileExists(join(cwd, workerReportPath))) {
    files.push({
      path: workerReportPath,
      reason: "Most recent structured worker report for the active feature.",
    });
  }

  if (await fileExists(join(cwd, replyPath))) {
    files.push({
      path: replyPath,
      reason: "Latest reply artifact for the active feature.",
    });
  }

  for (const changedFile of buildRepositoryRelevantFiles(gitState)) {
    if (!files.some((item) => item.path === changedFile.path)) {
      files.push(changedFile);
    }
  }

  return files;
}

function buildRepositoryRelevantFiles(gitState: GitState | undefined): readonly HandoffRelevantFile[] {
  if (!gitState || gitState.changedFiles.length === 0) {
    return [];
  }

  return gitState.changedFiles.slice(0, 12).map((path) => ({
    path,
    reason: "Changed locally in the current branch; inspect it before editing related code.",
  }));
}

function buildCurrentState(
  gitState: GitState | undefined,
  prefix: readonly string[] = [],
): readonly string[] {
  const lines = [...prefix];
  if (!gitState) {
    lines.push("Git state unavailable for the current working directory.");
    return lines;
  }

  lines.push(`Git branch: ${gitState.branch}`);
  lines.push(`Working tree: ${gitState.workingTreeClean ? "clean" : `dirty (${gitState.diffStat})`}`);
  if (gitState.recentCommits.length > 0) {
    lines.push(`Recent commits: ${gitState.recentCommits.slice(0, 3).join(" | ")}`);
  }
  if (gitState.changedFiles.length > 0) {
    lines.push(`Changed files: ${gitState.changedFiles.slice(0, 8).join(", ")}`);
  }
  return lines;
}

function buildWhatWasTried(feature: Feature): readonly string[] {
  if (!feature.report) {
    return ["No structured worker report is attached to this feature yet."];
  }

  const lines = [
    feature.report.salientSummary,
    `Implemented: ${feature.report.whatWasImplemented}`,
    `Left undone: ${feature.report.whatWasLeftUndone}`,
    ...feature.report.verification.commandsRun.map(
      (run) => `Verification: ${run.command} (exit ${run.exitCode}) — ${run.observation}`,
    ),
    ...feature.report.discoveredIssues.map(
      (issue) => `Issue (${issue.severity}): ${issue.description}${issue.suggestedFix ? ` — ${issue.suggestedFix}` : ""}`,
    ),
  ].filter((line) => line.trim().length > 0);

  return lines.length > 0 ? lines : ["A prior worker touched this feature, but no reusable notes were recorded."];
}

function buildMissionDecisions(feature: Feature, milestone: Milestone): readonly string[] {
  const decisions = [
    `Assigned worker type: ${feature.agentType}`,
    milestone.profile ? `Milestone profile: ${milestone.profile}` : undefined,
    feature.fulfills.length > 0 ? `Feature fulfills: ${feature.fulfills.join(", ")}` : undefined,
  ].filter((line): line is string => line !== undefined);

  return decisions.length > 0
    ? decisions
    : ["No explicit design decisions were recorded for this feature."];
}

function buildAcceptanceCriteria(
  feature: Feature,
  assertions: readonly Assertion[],
): readonly string[] {
  const criteria = [
    ...(feature.expectedBehavior ? [feature.expectedBehavior] : []),
    ...feature.verificationSteps,
    ...assertions.map((assertion) => assertion.description),
  ].map(normalizeText)
    .filter((line) => line.length > 0);

  return criteria.length > 0
    ? criteria
    : ["Complete the task and verify the touched surface area before stopping."];
}

function buildMissionConstraints(feature: Feature, gitState: GitState | undefined): readonly string[] {
  const constraints = [
    feature.preconditions,
    feature.dependsOn.length > 0 ? `Respect dependencies before closing this work: ${feature.dependsOn.join(", ")}` : undefined,
    !gitState?.workingTreeClean
      ? "The source workspace already has uncommitted changes. Preserve unrelated edits and do not revert work you did not make."
      : undefined,
    "Match the existing repo conventions and keep edits scoped to the task.",
  ].filter((line): line is string => typeof line === "string" && line.trim().length > 0);

  return constraints;
}

function buildRepositoryConstraints(gitState: GitState | undefined): readonly string[] {
  const constraints = [
    !gitState?.workingTreeClean
      ? "The source workspace already has uncommitted changes. Preserve unrelated edits and do not revert work you did not make."
      : undefined,
    "Do not broaden scope beyond the task described above.",
    "Match the existing repo conventions and keep edits scoped to the task.",
  ].filter((line): line is string => typeof line === "string" && line.trim().length > 0);

  return constraints;
}

async function loadGitState(git: GitPort, cwd: string): Promise<GitState | undefined> {
  const isRepo = await git.isRepo(cwd);
  if (!isRepo) return undefined;
  return git.getState(cwd);
}

function renderHandoffPrompt(context: HandoffPromptContext): string {
  return [
    "## Task",
    "",
    context.task,
    "",
    "## Context",
    "",
    ...renderBullets(context.context),
    "",
    "## Relevant Files",
    "",
    ...renderRelevantFiles(context.relevantFiles),
    "",
    "## Current State",
    "",
    ...renderBullets(context.currentState),
    "",
    "## What Was Tried",
    "",
    ...renderBullets(context.whatWasTried),
    "",
    "## Decisions",
    "",
    ...renderBullets(context.decisions),
    "",
    "## Acceptance Criteria",
    "",
    ...renderCheckboxes(context.acceptanceCriteria),
    "",
    "## Constraints",
    "",
    ...renderBullets(context.constraints),
  ].join("\n").trim();
}

function renderBullets(lines: readonly string[]): string[] {
  if (lines.length === 0) {
    return ["- None captured."];
  }
  return lines.map((line) => `- ${normalizeText(line)}`);
}

function renderRelevantFiles(files: readonly HandoffRelevantFile[]): string[] {
  if (files.length === 0) {
    return ["- No specific files were captured from the current workspace state."];
  }
  return files.map((file) => `- \`${file.path}\` — ${normalizeText(file.reason)}`);
}

function renderCheckboxes(lines: readonly string[]): string[] {
  if (lines.length === 0) {
    return ["- [ ] Complete the requested task and verify the result."];
  }
  return lines.map((line) => `- [ ] ${normalizeText(line)}`);
}

function normalizeText(value: string): string {
  return value.replace(/\s+/g, " ").trim();
}
