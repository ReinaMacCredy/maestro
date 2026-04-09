import type { Feature } from "@/features/mission";
import type { MissionControlWorkerFitRecommendation } from "../tui/state/types.js";

const WORKER_KEYWORDS: Readonly<Record<string, readonly string[]>> = {
  codex: ["adapter", "api", "build", "cli", "command", "fix", "implement", "parser", "refactor", "test", "transport"],
  "claude-code": ["architecture", "complex", "config", "design", "orchestration", "recovery", "reliability", "review", "state", "supervision", "workflow"],
  gemini: ["copy", "doc", "docs", "guide", "notes", "overview", "readme", "research", "summary", "writeup"],
};

export function recommendWorkerFit(
  workerSlug: string,
  features: readonly Feature[],
): MissionControlWorkerFitRecommendation {
  const visibleFeatures = features.filter((feature) =>
    feature.status === "pending"
    || feature.status === "assigned"
    || feature.status === "in-progress"
    || feature.status === "review"
  );
  if (visibleFeatures.length === 0) {
    return {
      workerSlug,
      reason: "No visible mission work is available to score right now.",
      fallbackReason: "No clear match in this mission right now.",
    };
  }

  const best = visibleFeatures
    .map((feature) => ({
      feature,
      score: scoreFeatureForWorker(workerSlug, feature, features),
    }))
    .sort((left, right) => right.score - left.score)[0];

  if (!best || best.score <= 0) {
    return {
      workerSlug,
      reason: "Visible work exists, but none of it strongly matches this worker.",
      fallbackReason: "No clear match in this mission right now.",
    };
  }

  return {
    workerSlug,
    featureId: best.feature.id,
    featureTitle: best.feature.title,
    reason: isReadyFeature(best.feature, features)
      ? `${best.feature.id} is ready now and looks like a strong ${workerSlug} task.`
      : `${best.feature.id} fits ${workerSlug} well once its blockers clear.`,
  };
}

function scoreFeatureForWorker(
  workerSlug: string,
  feature: Feature,
  allFeatures: readonly Feature[],
): number {
  const text = [
    feature.title,
    feature.description,
    feature.preconditions,
    feature.expectedBehavior,
    feature.verificationSteps.join(" "),
    feature.fulfills.join(" "),
  ]
    .filter(Boolean)
    .join(" ")
    .toLowerCase();

  let score = isReadyFeature(feature, allFeatures) ? 40 : feature.status === "pending" ? 25 : 10;
  for (const keyword of WORKER_KEYWORDS[workerSlug] ?? []) {
    if (text.includes(keyword)) score += 12;
  }

  if (workerSlug === "claude-code") {
    if (feature.dependsOn.length > 0) score += 8;
    if (feature.verificationSteps.length >= 3) score += 8;
    if (feature.description.length >= 90) score += 5;
  }

  if (workerSlug === "codex" && feature.verificationSteps.some((step) => /build|test|bun|cli/i.test(step))) {
    score += 8;
  }

  if (workerSlug === "gemini" && /doc|docs|guide|copy|summary|research/i.test(text)) {
    score += 10;
  }

  return score;
}

function isReadyFeature(feature: Feature, allFeatures: readonly Feature[]): boolean {
  if (feature.status !== "pending") return false;
  return feature.dependsOn.every((dependencyId) =>
    allFeatures.find((candidate) => candidate.id === dependencyId)?.status === "done"
  );
}
