import { join } from "node:path";
import type { LegacyTaskStorePort as TaskStorePort, LegacyTask as Task } from "@/shared/domain/legacy-task";
import type { VerdictStorePort, Verdict } from "@/features/verdict";
import type { LegacySpecStorePort as SpecStorePort } from "@/shared/domain/legacy-spec/index.js";
import type { EvidenceStorePort, EvidenceRow } from "@/features/evidence";
import { fileExists, readText } from "@/shared/lib/fs.js";

export interface PlanRegenDeps {
  readonly taskStore: TaskStorePort;
  readonly verdictStore: VerdictStorePort;
  readonly specStore: SpecStorePort;
  readonly evidenceStore: EvidenceStorePort;
}

export interface PlanRegenArgs {
  readonly projectRoot: string;
  readonly taskId: string;
}

export type PlanDriftKind =
  | "no-plan-file"
  | "no-spec"
  | "missing-acceptance-coverage"
  | "stale-since-last-pass"
  | "open-lints"
  | "blockers-active";

export interface PlanDrift {
  readonly kind: PlanDriftKind;
  readonly detail: string;
}

export interface PlanRegenResult {
  readonly taskId: string;
  readonly task?: Pick<Task, "id" | "title" | "status">;
  readonly hasPlanFile: boolean;
  readonly planPath?: string;
  readonly hasSpec: boolean;
  readonly latestVerdict?: Pick<Verdict, "id" | "decision" | "computedAt">;
  readonly drifts: readonly PlanDrift[];
}

const PLAN_FILE_CANDIDATES = (taskId: string): string[] => [
  `.maestro/plans/${taskId}.md`,
  `.maestro/plans/${taskId}/plan.md`,
  `.maestro/runs/${taskId}/plan.md`,
];

export async function regenPlan(
  deps: PlanRegenDeps,
  args: PlanRegenArgs,
): Promise<PlanRegenResult> {
  const task = await deps.taskStore.get(args.taskId);
  const missionId = task?.missionId;
  const [spec, verdict, evidence] = await Promise.all([
    missionId ? deps.specStore.read(missionId) : Promise.resolve(undefined),
    deps.verdictStore.readLatest(args.taskId),
    deps.evidenceStore.list({ task_id: args.taskId }),
  ]);

  const planPath = await findPlanFile(args.projectRoot, args.taskId);
  const drifts: PlanDrift[] = [];

  if (planPath === undefined) {
    drifts.push({
      kind: "no-plan-file",
      detail: `No plan file found at any of: ${PLAN_FILE_CANDIDATES(args.taskId).join(", ")}`,
    });
  }

  if (spec === undefined) {
    drifts.push({
      kind: "no-spec",
      detail: "No Spec recorded for this task; plan cannot be checked against acceptance criteria",
    });
  } else if (planPath !== undefined) {
    const planText = (await readText(join(args.projectRoot, planPath))) ?? "";
    const planLower = planText.toLowerCase();
    const missing = spec.acceptance_criteria.filter(
      (c) => !planLower.includes(c.text.toLowerCase()),
    );
    if (missing.length > 0) {
      const sample = missing.slice(0, 3).map((c) => c.text).join(" | ");
      drifts.push({
        kind: "missing-acceptance-coverage",
        detail: `${missing.length} acceptance criteria not referenced in plan: ${sample}${missing.length > 3 ? " …" : ""}`,
      });
    }
  }

  if (verdict !== undefined && verdict.decision === "PASS") {
    const sinceVerdict = evidence.filter(
      (e) => Date.parse(e.created_at) > Date.parse(verdict.computedAt),
    );
    if (sinceVerdict.length > 0) {
      drifts.push({
        kind: "stale-since-last-pass",
        detail: `${sinceVerdict.length} evidence row(s) recorded after the last PASS verdict (${verdict.computedAt})`,
      });
    }
  }

  const recordedLints = countAllLintViolations(evidence);
  if (recordedLints > 0) {
    drifts.push({
      kind: "open-lints",
      detail: `${recordedLints} lint-violation evidence row(s) recorded for this task (clean ones still in the ledger)`,
    });
  }

  if (task?.blockedBy && task.blockedBy.length > 0) {
    drifts.push({
      kind: "blockers-active",
      detail: `task is blocked by: ${task.blockedBy.join(", ")}`,
    });
  }

  return {
    taskId: args.taskId,
    task: task ? { id: task.id, title: task.title, status: task.status } : undefined,
    hasPlanFile: planPath !== undefined,
    planPath,
    hasSpec: spec !== undefined,
    latestVerdict: verdict
      ? { id: verdict.id, decision: verdict.decision, computedAt: verdict.computedAt }
      : undefined,
    drifts,
  };
}

async function findPlanFile(
  projectRoot: string,
  taskId: string,
): Promise<string | undefined> {
  for (const candidate of PLAN_FILE_CANDIDATES(taskId)) {
    if (await fileExists(join(projectRoot, candidate))) return candidate;
  }
  return undefined;
}

function countAllLintViolations(evidence: readonly EvidenceRow[]): number {
  return evidence.filter((e) => e.kind === "lint-violation").length;
}

export function formatPlanRegenLines(r: PlanRegenResult): string[] {
  const lines: string[] = [];
  lines.push(`Plan regen for ${r.taskId}`);
  if (r.task) lines.push(`  Task: ${r.task.title} (${r.task.status})`);
  lines.push(`  Plan file: ${r.planPath ?? "(missing)"}`);
  lines.push(`  Spec: ${r.hasSpec ? "yes" : "no"}`);
  if (r.latestVerdict) {
    lines.push(`  Latest verdict: ${r.latestVerdict.decision} @ ${r.latestVerdict.computedAt}`);
  } else {
    lines.push(`  Latest verdict: none`);
  }
  if (r.drifts.length === 0) {
    lines.push(`  Drifts: none`);
  } else {
    lines.push(`  Drifts:`);
    for (const d of r.drifts) lines.push(`    [${d.kind}] ${d.detail}`);
  }
  return lines;
}
