import type { VerdictStorePort } from "@/features/verdict";
import type { RunStateStorePort } from "@/features/task";
import type { TaskQueryPort } from "@/features/task";
import type { ContractVersionStorePort } from "@/features/task";
import type { VerdictDecision } from "@/features/verdict";

export interface AutopilotTaskRow {
  readonly taskId: string;
  readonly intent: string;
  readonly latestVerdict?: { readonly decision: VerdictDecision; readonly at: string };
  readonly retryCount: number;
  readonly maxRetries?: number;
  readonly wallClockElapsedSeconds: number;
  readonly maxWallClockSeconds?: number;
  readonly lastUpdatedAt?: string;
}

export interface AutopilotSnapshot {
  readonly tasks: readonly AutopilotTaskRow[];
}

export interface AutopilotSnapshotDeps {
  readonly taskStore: TaskQueryPort;
  readonly verdictStore: VerdictStorePort;
  readonly runStateStore: RunStateStorePort;
  readonly contractVersionStore: ContractVersionStorePort;
}

export async function buildAutopilotSnapshot(
  deps: AutopilotSnapshotDeps,
  missionId: string,
): Promise<AutopilotSnapshot> {
  const allTasks = await deps.taskStore.all();
  const missionTasks = allTasks.filter((task) => task.missionId === missionId);

  const rows = await Promise.all(
    missionTasks.map(async (task): Promise<AutopilotTaskRow> => {
      const [verdict, runState, contract] = await Promise.all([
        deps.verdictStore.readLatest(task.id),
        deps.runStateStore.read(task.id),
        deps.contractVersionStore.readCurrent(task.id),
      ]);

      return {
        taskId: task.id,
        intent: contract?.intent ?? task.title,
        latestVerdict: verdict
          ? { decision: verdict.decision, at: verdict.computedAt }
          : undefined,
        retryCount: runState?.retryCount ?? 0,
        maxRetries: contract?.costBudget?.maxRetries,
        wallClockElapsedSeconds: runState?.wallClockElapsedSeconds ?? 0,
        maxWallClockSeconds: contract?.costBudget?.maxWallClockSeconds,
        lastUpdatedAt: runState?.lastUpdatedAt,
      };
    }),
  );

  rows.sort((a, b) => a.taskId.localeCompare(b.taskId));

  return { tasks: rows };
}
