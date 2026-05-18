import type { ContractStorePort } from "../ports/contract-store.port.js";
import type { ContractVersionStorePort } from "../ports/contract-version-store.port.js";
import type { RunStateStorePort } from "../ports/run-state-store.port.js";
import type { TaskContinuationHistoryPort } from "../ports/task-continuation-history.port.js";
import type { TaskContinuationStorePort } from "../ports/task-continuation-store.port.js";
import type { TaskQueryPort } from "../ports/task-store.port.js";
import type { Spec, LegacySpecStorePort as SpecStorePort } from "@/shared/domain/legacy-spec";
import type { Verdict, VerdictStorePort } from "@/features/verdict";
import type {
  EvidenceRow,
  EvidenceStorePort,
  LintViolationPayload,
  SessionStartPayload,
} from "@/features/evidence";
import { inspectTask, type TaskInspectionView } from "./inspect-task.usecase.js";
import { checkCostBudget, type CostBudgetCheck } from "./check-cost-budget.js";
import { readCurrentContractWithBackfill } from "./read-current-contract-with-backfill.js";

export interface TaskIntrospectionDeps {
  readonly taskStore: TaskQueryPort;
  readonly continuationStore: TaskContinuationStorePort;
  readonly continuationHistory: TaskContinuationHistoryPort;
  readonly specStore: SpecStorePort;
  readonly verdictStore: VerdictStorePort;
  readonly evidenceStore: EvidenceStorePort;
  readonly runStateStore: RunStateStorePort;
  readonly contractStore: ContractStorePort;
  readonly contractVersionStore: ContractVersionStorePort;
  readonly listOpenHandoffIds?: (taskId: string) => Promise<readonly string[]>;
  readonly repoRoot: string;
  /**
   * Resolve recent commits in the working tree since a given anchor commit.
   * Defaults to a `git log <anchor>..HEAD` shell call. Overridable for tests.
   */
  readonly resolveCommitsSince?: (
    repoRoot: string,
    anchorSha: string,
  ) => Promise<readonly { sha: string; subject: string }[]>;
  readonly checkCommitReachable?: (repoRoot: string, sha: string) => Promise<boolean>;
}

export interface AnchorStatus {
  readonly sha: string;
  readonly stale: boolean;
}

export interface LoopWarning {
  readonly kind: string;
  readonly payloadHash: string;
  readonly count: number;
}

export interface TaskIntrospectionView extends TaskInspectionView {
  readonly spec?: Spec;
  /** Reserved for Phase 2 — no plan position store wired yet. */
  readonly planPosition?: { phase: number; total: number };
  readonly lastVerdict?: Verdict;
  readonly budgetCheck?: CostBudgetCheck;
  readonly openLintViolations: readonly EvidenceRow<"lint-violation">[];
  readonly recentEvidence: readonly EvidenceRow[];
  readonly recentCommits: readonly { sha: string; subject: string }[];
  readonly sessionAnchorSha?: string;
  readonly anchor?: AnchorStatus;
  readonly loopWarning?: LoopWarning;
}

export async function composeTaskIntrospection(
  deps: TaskIntrospectionDeps,
  taskId: string,
): Promise<TaskIntrospectionView> {
  const inspection = await inspectTask(
    {
      taskStore: deps.taskStore,
      continuationStore: deps.continuationStore,
      continuationHistory: deps.continuationHistory,
      ...(deps.listOpenHandoffIds ? { listOpenHandoffIds: deps.listOpenHandoffIds } : {}),
    },
    taskId,
  );

  const missionId = inspection.task.missionId;

  const [spec, lastVerdict, contract, runState, allEvidence] = await Promise.all([
    missionId ? deps.specStore.read(missionId) : Promise.resolve(undefined),
    deps.verdictStore.readLatest(taskId),
    readCurrentContractWithBackfill(
      deps.contractVersionStore,
      deps.contractStore,
      taskId,
    ),
    deps.runStateStore.read(taskId),
    deps.evidenceStore.list({ task_id: taskId }),
  ]);

  const budgetCheck = contract !== undefined ? checkCostBudget(contract, runState) : undefined;

  const openLintViolations = allEvidence.filter(
    (row): row is EvidenceRow<"lint-violation"> => row.kind === "lint-violation",
  );

  const recentEvidence = [...allEvidence]
    .sort((a, b) => b.created_at.localeCompare(a.created_at))
    .slice(0, 5);

  const sessionAnchorSha = findLatestSessionAnchorSha(allEvidence);
  const anchorReachable = sessionAnchorSha
    ? await (deps.checkCommitReachable ?? defaultCheckCommitReachable)(
        deps.repoRoot,
        sessionAnchorSha,
      )
    : true;
  const recentCommits = sessionAnchorSha && anchorReachable
    ? await (deps.resolveCommitsSince ?? defaultResolveCommitsSince)(
        deps.repoRoot,
        sessionAnchorSha,
      )
    : [];
  const anchor: AnchorStatus | undefined = sessionAnchorSha
    ? { sha: sessionAnchorSha, stale: !anchorReachable }
    : undefined;

  const loopWarning = detectLoopWarning(allEvidence);

  const view: TaskIntrospectionView = {
    ...inspection,
    openLintViolations,
    recentEvidence,
    recentCommits,
    ...(spec !== undefined ? { spec } : {}),
    ...(lastVerdict !== undefined ? { lastVerdict } : {}),
    ...(budgetCheck !== undefined ? { budgetCheck } : {}),
    ...(sessionAnchorSha !== undefined ? { sessionAnchorSha } : {}),
    ...(anchor !== undefined ? { anchor } : {}),
    ...(loopWarning !== undefined ? { loopWarning } : {}),
  };
  return view;
}

function detectLoopWarning(rows: readonly EvidenceRow[]): LoopWarning | undefined {
  const ordered = [...rows].sort((a, b) => a.created_at.localeCompare(b.created_at));
  let runKind: string | undefined;
  let runHash: string | undefined;
  let runCount = 0;
  let bestWarning: LoopWarning | undefined;
  for (const row of ordered) {
    if (row.kind === "session-start" || row.kind === "session-exit") {
      runKind = undefined;
      runHash = undefined;
      runCount = 0;
      continue;
    }
    const hash = stableHash(row.kind, row.payload);
    if (runKind === row.kind && runHash === hash) {
      runCount += 1;
    } else {
      runKind = row.kind;
      runHash = hash;
      runCount = 1;
    }
    if (runCount >= 3) {
      bestWarning = { kind: row.kind, payloadHash: hash, count: runCount };
    }
  }
  return bestWarning;
}

function stableHash(kind: string, payload: unknown): string {
  let str: string;
  try {
    str = JSON.stringify(payload);
  } catch {
    str = String(payload);
  }
  let h = 0;
  const combined = `${kind}|${str}`;
  for (let i = 0; i < combined.length; i++) {
    h = (h * 31 + combined.charCodeAt(i)) | 0;
  }
  return (h >>> 0).toString(16);
}

async function defaultCheckCommitReachable(repoRoot: string, sha: string): Promise<boolean> {
  const proc = Bun.spawnSync({
    cmd: ["git", "cat-file", "-e", `${sha}^{commit}`],
    cwd: repoRoot,
    stdout: "pipe",
    stderr: "pipe",
  });
  return proc.exitCode === 0;
}

function findLatestSessionAnchorSha(rows: readonly EvidenceRow[]): string | undefined {
  let latest: EvidenceRow<"session-start"> | undefined;
  for (const row of rows) {
    if (row.kind !== "session-start") continue;
    if (latest === undefined || row.created_at.localeCompare(latest.created_at) > 0) {
      latest = row as EvidenceRow<"session-start">;
    }
  }
  return latest ? (latest.payload as SessionStartPayload).headSha : undefined;
}

async function defaultResolveCommitsSince(
  repoRoot: string,
  anchorSha: string,
): Promise<readonly { sha: string; subject: string }[]> {
  const proc = Bun.spawnSync({
    cmd: ["git", "log", `${anchorSha}..HEAD`, "--format=%H %s", "-n", "5"],
    cwd: repoRoot,
    stdout: "pipe",
    stderr: "pipe",
  });
  if (proc.exitCode !== 0) return [];
  const text = new TextDecoder().decode(proc.stdout);
  return text
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line) => {
      const sp = line.indexOf(" ");
      if (sp === -1) return { sha: line, subject: "" };
      return { sha: line.slice(0, sp), subject: line.slice(sp + 1) };
    });
}

export function formatTaskIntrospectionMarkdown(
  view: TaskIntrospectionView,
): string {
  const lines: string[] = [];
  lines.push(`# Task: ${view.task.id}`);
  lines.push(view.task.title || "(untitled)");
  lines.push("");

  lines.push("## Spec — Acceptance Criteria");
  if (view.spec === undefined) {
    lines.push("No spec recorded for this task.");
  } else if (view.spec.acceptance_criteria.length === 0) {
    lines.push("(spec exists but has no acceptance criteria)");
  } else {
    for (const c of view.spec.acceptance_criteria) {
      lines.push(`- ${c.text}`);
    }
  }
  lines.push("");

  lines.push("## Spec — Non-goals");
  if (view.spec === undefined || view.spec.non_goals.length === 0) {
    lines.push("(none)");
  } else {
    for (const n of view.spec.non_goals) {
      lines.push(`- ${n.text}`);
    }
  }
  lines.push("");

  lines.push("## Plan position");
  lines.push("(deferred — no plan store wired yet)");
  lines.push("");

  lines.push("## Verdict");
  if (view.lastVerdict === undefined) {
    lines.push("No verdict requested for current tree.");
  } else {
    lines.push(`${view.lastVerdict.decision} at ${view.lastVerdict.computedAt}`);
    lines.push(`Risk class: ${view.lastVerdict.effectiveRiskClass}`);
  }
  lines.push("");

  lines.push("## Budget");
  if (view.budgetCheck === undefined) {
    lines.push("No budget configured.");
  } else if (view.budgetCheck.exhausted) {
    lines.push(`Exhausted (${view.budgetCheck.reason}).`);
  } else {
    lines.push("Within budget.");
  }
  lines.push("");

  lines.push(`## Open lints (${view.openLintViolations.length})`);
  if (view.openLintViolations.length === 0) {
    lines.push("(none)");
  } else {
    for (const row of view.openLintViolations) {
      const p = row.payload as LintViolationPayload;
      const loc = p.line !== undefined ? `${p.file}:${p.line}` : p.file;
      lines.push(`- ${p.ruleId}: ${loc} — ${p.message}`);
    }
  }
  lines.push("");

  lines.push(`## Open blockers (${view.activeBlockerIds.length})`);
  if (view.activeBlockerIds.length === 0) {
    lines.push("(none)");
  } else {
    for (const id of view.activeBlockerIds) {
      lines.push(`- ${id}`);
    }
  }
  lines.push("");

  lines.push("## Recent evidence (last 5)");
  if (view.recentEvidence.length === 0) {
    lines.push("(none)");
  } else {
    for (const row of view.recentEvidence) {
      lines.push(`- ${row.kind} ${row.created_at} witness=${row.witness_level}`);
    }
  }
  lines.push("");

  lines.push("## Recent commits (last 5 since last session-start)");
  if (view.sessionAnchorSha === undefined) {
    lines.push("No session-start evidence found; commit history unavailable.");
  } else if (view.anchor?.stale === true) {
    lines.push(`anchor: stale (commit ${view.anchor.sha.slice(0, 7)} not reachable)`);
    lines.push("Recovery hint: re-run `maestro session start` to anchor at HEAD.");
  } else if (view.recentCommits.length === 0) {
    lines.push("(no commits since session anchor)");
  } else {
    for (const c of view.recentCommits) {
      lines.push(`- ${c.sha.slice(0, 7)} ${c.subject}`);
    }
  }
  lines.push("");

  if (view.loopWarning !== undefined) {
    lines.push("## Loop warning");
    lines.push(`loopWarning: kind=${view.loopWarning.kind} count=${view.loopWarning.count} payloadHash=${view.loopWarning.payloadHash}`);
    lines.push("Recovery hint: review the last verdict reason, change approach, or run `maestro ralph review --stuck-threshold 1`.");
  }

  return lines.join("\n");
}
